package org.kog.player

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import android.os.Bundle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.documentfile.provider.DocumentFile
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.media3.session.SessionCommand
import androidx.core.content.ContextCompat
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Queue and selection belong to this Android client; library rules belong to Rust. */
class KogState(private val context: Context) {
    val api = KogApi(context)
    val deviceApi = KogApi(context, onDevice = true)
    private val deviceLibrary = DeviceLibrary(context)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val prefs = context.getSharedPreferences("kog", Context.MODE_PRIVATE)
    private var controllerFuture: ListenableFuture<MediaController>? = null
    private var controller: MediaController? = null
    private var searchJob: Job? = null

    val queue = mutableStateListOf<Track>()
    val playlists = mutableStateListOf<SavedPlaylist>()
    val playlistTracks = mutableStateListOf<Track>()
    val searchTracks = mutableStateListOf<Track>()
    val searchFolders = mutableStateListOf<Folder>()
    val deviceFolders = mutableStateListOf<DocumentFile>()
    val deviceFiles = mutableStateListOf<DocumentFile>()
    var deviceCurrent by mutableStateOf<DocumentFile?>(null)
        private set
    var stars by mutableStateOf(setOf<String>())
        private set
    var listing by mutableStateOf<Listing?>(null)
        private set
    var libraryRoot by mutableStateOf("")
        private set
    var queueSelection by mutableStateOf(setOf<Int>())
        private set
    var workspace by mutableStateOf(PlaylistWorkspaceSnapshot())
        private set
    var selectedPlaylist by mutableStateOf<SavedPlaylist?>(null)
        private set
    var searchText by mutableStateOf("")
    var searching by mutableStateOf(false)
        private set
    var searchScanned by mutableStateOf(0)
        private set
    var libraryOnDevice by mutableStateOf(api.server.isBlank())
    var currentIndex by mutableStateOf(-1)
        private set
    var playing by mutableStateOf(false)
        private set
    var position by mutableStateOf(0L)
        private set
    var duration by mutableStateOf(0L)
        private set
    var connected by mutableStateOf(false)
        private set
    var error by mutableStateOf("")
        private set
    var radioOn by mutableStateOf(false)
        private set
    var shuffleMode by mutableStateOf("off")
        private set
    var repeatMode by mutableStateOf("off")
        private set
    val shuffleOn get() = shuffleMode != "off"
    val repeatOn get() = repeatMode != "off"
    var radioWaiting by mutableStateOf(false)
        private set
    var queuedIndices by mutableStateOf(emptyList<Int>())
        private set
    var stopAfterIndices by mutableStateOf(emptyList<Int>())
        private set
    var localMidiEngine by mutableStateOf(prefs.getString("local_midi_engine", "opl3windows") ?: "opl3windows")
        private set
    var soundfontReady by mutableStateOf(prefs.getString("midi_soundfont", "")
        ?.takeIf(String::isNotBlank)?.let { File(it).isFile } == true)
        private set
    var sc55RomsReady by mutableStateOf(prefs.getString("midi_sc55_roms", "")
        ?.takeIf(String::isNotBlank)?.let { File(it).isDirectory } == true)
        private set
    var mt32RomsReady by mutableStateOf(prefs.getString("midi_mt32_roms", "")
        ?.takeIf(String::isNotBlank)?.let { File(it).isDirectory } == true)
        private set
    var localRoot by mutableStateOf("")
        private set
    var importing by mutableStateOf(false)
        private set

    val current: Track? get() = queue.getOrNull(currentIndex)

    private val listener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) = syncPlayer()
        override fun onPlayerError(failure: PlaybackException) {
            error = "Cannot play ${current?.label ?: "this file"}: ${failure.message ?: "unsupported format"}"
        }
    }

    init {
        runCatching {
            val rows = JSONArray(prefs.getString("queue", "[]"))
            for (i in 0 until rows.length()) rows.optJSONObject(i)?.let { queue.add(Track.parse(it)) }
        }
        shuffleMode = prefs.getString("shuffle_mode", if (prefs.getBoolean("shuffle", false)) "all" else "off") ?: "off"
        repeatMode = prefs.getString("repeat_mode", if (prefs.getBoolean("repeat", false)) "all" else "off") ?: "off"
        localRoot = prefs.getString("local_root", "").orEmpty()
        task { stars = deviceApi.stars() }
        if (localRoot.isNotBlank()) {
            DocumentFile.fromTreeUri(context, Uri.parse(localRoot))?.let(::browseDevice)
        }
    }

    fun connectPlayer() {
        if (controllerFuture != null) return
        val future = MediaController.Builder(context, SessionToken(context,
            ComponentName(context, PlaybackService::class.java))).setListener(object : MediaController.Listener {
                override fun onExtrasChanged(controller: MediaController, extras: Bundle) { applyPolicy(extras) }
            }).buildAsync()
        controllerFuture = future
        future.addListener({
            runCatching {
                controller = future.get().also { it.addListener(listener) }
                if (controller?.mediaItemCount == 0 && queue.isNotEmpty()) {
                    controller?.setMediaItems(queue.map(::mediaItem),
                        prefs.getInt("index", 0).coerceIn(0, queue.lastIndex),
                        prefs.getLong("position", 0))
                    controller?.prepare()
                }
                policyCommand("sync")
                syncPlayer()
            }.onFailure { error = it.message ?: "Cannot start playback" }
        }, ContextCompat.getMainExecutor(context))
    }

    fun release() {
        saveQueue()
        controller?.removeListener(listener)
        controllerFuture?.let(MediaController::releaseFuture)
        controllerFuture = null
        controller = null
    }

    fun connectionChanged() {
        val player = controller ?: return
        val index = player.currentMediaItemIndex.coerceAtLeast(0)
        val at = player.currentPosition.coerceAtLeast(0)
        val wasPlaying = player.isPlaying
        if (queue.isNotEmpty()) {
            player.setMediaItems(queue.map(::mediaItem), index.coerceAtMost(queue.lastIndex), at)
            player.prepare()
            if (wasPlaying) player.play()
        }
    }

    private fun restartCurrentMidi() {
        val track = current ?: return
        val name = when {
            track.isDevice -> track.name
            track.kind == "archive" -> track.entry
            else -> track.path
        }
        if (name.substringAfterLast('.', "").lowercase() !in setOf(
                "kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf")) return
        val player = controller ?: return
        val index = player.currentMediaItemIndex.coerceAtLeast(0)
        val at = player.currentPosition.coerceAtLeast(0)
        val wasPlaying = player.isPlaying
        player.setMediaItems(queue.map(::mediaItem), index.coerceAtMost(queue.lastIndex), at)
        player.prepare()
        if (wasPlaying) player.play()
    }

    fun selectMidiEngine(engine: String) {
        api.midiEngine = engine
        if (connected) task {
            api.setMidiEngine(engine)
            if (current?.isDevice == false) restartCurrentMidi()
        }
    }

    fun selectLocalMidiEngine(engine: String) {
        if (engine == localMidiEngine) return
        localMidiEngine = engine
        prefs.edit().putString("local_midi_engine", engine).apply()
        if (current?.isDevice == true) restartCurrentMidi()
    }

    fun importMidiSoundfont(uri: Uri) = task {
        val path = withContext(Dispatchers.IO) {
            require(DocumentFile.fromSingleUri(context, uri)?.name?.endsWith(".sf2", true) == true) {
                "Choose an .sf2 SoundFont file"
            }
            val destination = File(context.filesDir, "kog-midi/soundfont.sf2")
            destination.parentFile?.mkdirs()
            val temporary = File.createTempFile("soundfont-", ".sf2", destination.parentFile)
            try {
                context.contentResolver.openInputStream(uri)?.use { source ->
                    temporary.outputStream().use(source::copyTo)
                } ?: throw IllegalStateException("Cannot read the selected SoundFont")
                if (destination.exists()) destination.delete()
                check(temporary.renameTo(destination)) { "Cannot save the SoundFont" }
            } finally { temporary.delete() }
            destination.absolutePath
        }
        prefs.edit().putString("midi_soundfont", path).apply()
        soundfontReady = true
        if (current?.isDevice == true) restartCurrentMidi()
    }

    fun importMidiRoms(uri: Uri, kind: String) = task {
        require(kind == "sc55" || kind == "mt32")
        val path = withContext(Dispatchers.IO) {
            val source = DocumentFile.fromTreeUri(context, uri)
                ?: throw IllegalStateException("Cannot open the selected ROM folder")
            val destination = File(context.filesDir, "kog-midi/$kind")
            val temporary = File(context.filesDir, "kog-midi/$kind.tmp")
            temporary.deleteRecursively()
            fun copyFolder(folder: DocumentFile, target: File) {
                target.mkdirs()
                for (child in folder.listFiles()) {
                    val name = child.name ?: continue
                    require(name != "." && name != ".." && '/' !in name && '\\' !in name)
                    val output = File(target, name)
                    if (child.isDirectory) copyFolder(child, output)
                    else if (child.isFile) {
                        context.contentResolver.openInputStream(child.uri)?.use { input ->
                            output.outputStream().use(input::copyTo)
                        } ?: throw IllegalStateException("Cannot read $name")
                    }
                }
            }
            try {
                copyFolder(source, temporary)
                if (destination.exists()) destination.deleteRecursively()
                check(temporary.renameTo(destination)) { "Cannot save the ROM folder" }
            } finally { temporary.deleteRecursively() }
            destination.absolutePath
        }
        prefs.edit().putString(if (kind == "sc55") "midi_sc55_roms" else "midi_mt32_roms", path).apply()
        if (kind == "sc55") sc55RomsReady = true else mt32RomsReady = true
        if (current?.isDevice == true) restartCurrentMidi()
    }

    private fun syncPlayer() {
        val player = controller ?: return
        val tracks = (0 until player.mediaItemCount).map { player.getMediaItemAt(it).kogTrack() }
        if (tracks != queue.toList()) queue.replaceWith(tracks)
        currentIndex = player.sessionExtras.getString("kog_policy")?.let { JSONObject(it).optInt("current", -1) } ?: -1
        playing = player.isPlaying
        position = player.currentPosition.coerceAtLeast(0)
        duration = player.duration.coerceAtLeast(0)
        saveQueue()
    }

    fun tick() {
        val player = controller ?: return
        position = player.currentPosition.coerceAtLeast(0)
        duration = player.duration.coerceAtLeast(0)
    }

    private fun mediaItem(track: Track): MediaItem = track.mediaItem(api)

    private fun applyPolicy(extras: Bundle) {
        extras.getString("error")?.let { error = it }
        val snapshot = extras.getString("kog_policy")?.let(::JSONObject) ?: return
        currentIndex = snapshot.optInt("current", -1)
        shuffleMode = snapshot.optString("shuffle", "off")
        repeatMode = snapshot.optString("repeat", "off")
        radioOn = snapshot.optJSONObject("radio")?.optBoolean("enabled") ?: false
        radioWaiting = snapshot.optJSONObject("radio")?.optBoolean("waiting") ?: false
        fun indices(key: String): List<Int> = snapshot.optJSONArray(key)?.let { rows ->
            (0 until rows.length()).map(rows::getInt)
        }.orEmpty()
        queuedIndices = indices("queued")
        stopAfterIndices = indices("stop_after")
        snapshot.optJSONObject("queue_selection")?.optJSONArray("indices")?.let { indices -> queueSelection = (0 until indices.length()).map(indices::getInt).toSet() }
        snapshot.optJSONObject("workspace")?.let { workspace = PlaylistWorkspaceSnapshot.parse(it) }
        if (!snapshot.isNull("error")) snapshot.optString("error").takeIf(String::isNotEmpty)?.let { error = it }
    }
    private fun policyCommand(op: String, fields: JSONObject = JSONObject()) {
        val player = controller ?: return
        fields.put("op", op)
        val future = player.sendCustomCommand(SessionCommand(PlaybackService.POLICY_COMMAND, Bundle.EMPTY),
            Bundle().apply { putString("command", fields.toString()) })
        future.addListener({ runCatching { applyPolicy(future.get().extras) }
            .onFailure { error = it.message ?: "Playback command failed" } }, ContextCompat.getMainExecutor(context))
    }

    private fun saveQueue() {
        val rows = JSONArray()
        queue.forEach { rows.put(it.saved()) }
        prefs.edit().putString("queue", rows.toString())
            .putInt("index", currentIndex.coerceAtLeast(0))
            .putLong("position", position)
            .putString("shuffle_mode", shuffleMode).putString("repeat_mode", repeatMode).apply()
    }

    private fun task(work: suspend () -> Unit) {
        scope.launch { runCatching { work() }.onFailure { error = it.message ?: "Request failed" } }
    }

    fun clearError() { error = "" }

    fun refresh() = task {
        if (api.server.isBlank()) return@task
        listing = api.browse()
        libraryRoot = listing?.path.orEmpty()
        playlists.replaceWith(api.playlists())
        stars = api.stars() + deviceApi.stars()
        runCatching { api.serverMidiEngine() }.getOrNull()?.let { serverEngine ->
            if (serverEngine != api.midiEngine) {
                api.midiEngine = serverEngine
                restartCurrentMidi()
            }
        }
        connected = true
        error = ""
    }

    fun browse(path: String) = task {
        listing = api.browse(path)
        searchText = ""
        searchTracks.clear()
        searchFolders.clear()
    }

    fun search(query: String) {
        searchText = query
        searchJob?.cancel()
        if (query.isBlank()) {
            searching = false
            searchTracks.clear()
            searchFolders.clear()
            return
        }
        searchJob = scope.launch {
            delay(250)
            searching = true
            runCatching {
                var page = api.search(query)
                searchTracks.replaceWith(page.tracks)
                searchFolders.replaceWith(page.folders)
                searchScanned = page.scanned
                while (!page.done && searchText == query) {
                    delay(180)
                    page = api.more(page.generation, searchTracks.size + searchFolders.size)
                    searchTracks.addAll(page.tracks)
                    searchFolders.addAll(page.folders)
                    searchScanned = page.scanned
                }
            }.onFailure { error = it.message ?: "Search failed" }
            searching = false
        }
    }

    fun addFile(track: Track, play: Boolean = false, onAdded: () -> Unit = {}) = task {
        val tracks = (if (track.isDevice) deviceApi else api).expand(track)
        add(tracks, play)
        if (tracks.isNotEmpty()) onAdded()
    }

    fun addFolder(folder: Folder, play: Boolean = false, onAdded: () -> Unit = {}) = task {
        val tracks = api.collect(folder.path, searchText, libraryRoot)
        add(tracks, play)
        if (tracks.isNotEmpty()) onAdded()
    }

    fun add(tracks: List<Track>, play: Boolean = false) {
        if (tracks.isEmpty()) return
        val start = queue.size
        queue.addAll(tracks)
        controller?.addMediaItems(tracks.map(::mediaItem))
        controller?.prepare()
        if (play) {
            controller?.seekTo(start, 0)
            controller?.play()
        }
        saveQueue()
    }

    fun play(index: Int) {
        val player = controller ?: return
        if (index !in queue.indices) return
        player.seekTo(index, 0)
        player.prepare()
        player.play()
        syncPlayer()
    }

    fun toggle() {
        controller?.let { if (radioWaiting) it.stop() else if (it.isPlaying) it.pause() else it.play() }
        syncPlayer()
    }
    fun previous() { controller?.seekToPreviousMediaItem(); syncPlayer() }
    fun next() { controller?.seekToNextMediaItem(); syncPlayer() }
    fun stop() { controller?.stop(); syncPlayer() }
    fun seek(milliseconds: Long) { controller?.seekTo(milliseconds); syncPlayer() }

    fun remove(index: Int) {
        if (index !in queue.indices) return
        queue.removeAt(index)
        controller?.removeMediaItem(index)
        syncPlayer()
        saveQueue()
    }
    fun clear() { queue.clear(); controller?.clearMediaItems(); syncPlayer(); saveQueue() }
    fun move(from: Int, to: Int) {
        if (from !in queue.indices || to !in queue.indices) return
        val row = queue.removeAt(from)
        queue.add(to, row)
        controller?.moveMediaItem(from, to)
        syncPlayer()
        saveQueue()
    }

    var queueFilter by mutableStateOf("")
    private var sortField = ""
    private var sortDescending = false
    fun sortQueue(field: String) {
        if (field == sortField) sortDescending = !sortDescending else { sortField = field; sortDescending = false }
        policyCommand("sort_rows", JSONObject().put("rows", policyRows()).put("column", field).put("descending", sortDescending))
    }
    private fun policyRows(): JSONArray = JSONArray(queue.map { track ->
        val row = JSONObject().put("title", track.label).put("artist", track.artist).put("album", track.album)
            .put("track_number", track.trackNumber ?: JSONObject.NULL).put("disc_number", track.discNumber ?: JSONObject.NULL)
            .put("duration", if (track.duration > 0) track.duration / 1000.0 else JSONObject.NULL)
            .put("path", if (track.entry.isEmpty()) track.path else "${track.path}/${track.entry}")
            .put("filename", track.entry.ifBlank { track.path }.substringAfterLast('/')).put("star", track.key in stars)
        for (key in listOf("albumArtist", "composer", "genre", "codec")) row.put(key, track.metadata[key].orEmpty())
        for (key in listOf("year", "fileSizeBytes", "sampleRate", "bitsPerSample", "bitrate", "channels"))
            row.put(key, track.metadata[key]?.toDoubleOrNull() ?: JSONObject.NULL)
        row
    })
    fun filteredQueueIndices(): List<Int> {
        if (queueFilter.isBlank()) return queue.indices.toList()
        val rows = SharedPlaybackPolicy.query(JSONObject().put("op", "filter_rows").put("rows", policyRows()).put("query", queueFilter)).getJSONArray("indices")
        return (0 until rows.length()).map(rows::getInt)
    }
    fun selectQueueIndices(indices: List<Int>) = policyCommand("select_queue", JSONObject().put("command",
        JSONObject().put("op", "set").put("indices", JSONArray(indices)).put("anchor", indices.firstOrNull() ?: JSONObject.NULL)))

    fun shuffle() = policyCommand("cycle_shuffle")
    fun repeat() = policyCommand("cycle_repeat")
    private fun radioRequest(op: String) = task {
        if (libraryOnDevice && localRoot.isNotBlank()) {
            withContext(Dispatchers.IO) {
                DocumentFile.fromTreeUri(context, Uri.parse(localRoot))?.let { deviceLibrary.stageFolder(it) }
            }
        }
        policyCommand(op, JSONObject().put("root", if (libraryOnDevice) deviceApi.deviceRoot else libraryRoot)
            .put("on_device", libraryOnDevice))
    }
    fun radio() = radioRequest("radio_toggle")
    fun reshuffleRadio() = radioRequest("radio_reshuffle")
    fun toggleQueued(index: Int) = policyCommand("toggle_queue", JSONObject().put("indices", JSONArray().put(index)))
    fun toggleStopAfter(index: Int) = policyCommand("toggle_stop_after", JSONObject().put("indices", JSONArray().put(index)))

    fun workspaceCommand(op: String, fields: JSONObject = JSONObject()) {
        policyCommand("workspace", JSONObject().put("command", fields.put("op", op)))
    }
    fun openPlaylistTab(item: SavedPlaylist) {
        val scope = if (libraryOnDevice) "device" else "server:${api.server}"
        workspaceCommand("open", JSONObject().put("key", "$scope:${item.id}").put("scope", scope)
            .put("playlist_id", item.id).put("name", item.name).put("readonly", item.id == 0L))
    }
    fun workspaceSelect(index: Int) {
        workspaceCommand("selection", JSONObject().put("command", JSONObject().put("op", "choose").put("index", index).put("gesture", "toggle")))
    }
    fun activate(index: Int) = policyCommand("activate", JSONObject().put("index", index))
    fun selectQueue(op: String, index: Int? = null) {
        val command = JSONObject().put("op", op)
        if (index != null) command.put("index", index).put("gesture", "toggle")
        policyCommand("select_queue", JSONObject().put("command", command))
    }
    fun workspaceAppendQueue(selectionOnly: Boolean = false) {
        val rows = queue.filterIndexed { index, _ -> !selectionOnly || index in queueSelection }
        val onDevice = workspace.tabs.firstOrNull { it.key == workspace.active }?.scope == "device"
        if (rows.any { it.kind != "remote" && it.isDevice != onDevice }) { error = "Choose a playlist in the same library as these tracks."; return }
        workspaceCommand("append", JSONObject().put("entries", JSONArray(rows.map { it.saved() })))
    }

    private val playlistApi get() = if (libraryOnDevice) deviceApi else api
    fun loadPlaylists() = task { playlists.replaceWith(playlistApi.playlists()) }
    fun openPlaylist(item: SavedPlaylist) = task {
        selectedPlaylist = item
        playlistTracks.replaceWith(playlistApi.playlist(item.id))
    }
    fun closePlaylist() { selectedPlaylist = null; playlistTracks.clear() }
    fun createPlaylist(name: String) = task { playlistApi.createPlaylist(name); playlists.replaceWith(playlistApi.playlists()) }
    fun renamePlaylist(item: SavedPlaylist, name: String) = task {
        playlistApi.renamePlaylist(item.id, name); playlists.replaceWith(playlistApi.playlists())
        workspaceCommand("renamed", JSONObject().put("key", "${if (libraryOnDevice) "device" else "server:${api.server}"}:${item.id}").put("name", name))
        selectedPlaylist = selectedPlaylist?.copy(name = name)
    }
    fun deletePlaylist(item: SavedPlaylist) = task {
        playlistApi.deletePlaylist(item.id); workspaceCommand("deleted", JSONObject().put("key", "${if (libraryOnDevice) "device" else "server:${api.server}"}:${item.id}")); playlists.replaceWith(playlistApi.playlists()); closePlaylist()
    }
    fun saveToPlaylist(item: SavedPlaylist, tracks: List<Track>) = task {
        playlistApi.appendPlaylist(item.id, tracks)
        playlists.replaceWith(playlistApi.playlists())
        if (selectedPlaylist?.id == item.id) playlistTracks.replaceWith(playlistApi.playlist(item.id))
    }
    fun replacePlaylist(item: SavedPlaylist, tracks: List<Track>) = task {
        playlistApi.appendPlaylist(item.id, tracks)
        playlists.replaceWith(playlistApi.playlists())
    }
    fun toggleStar(track: Track) = task {
        val newValue = track.key !in stars
        (if (track.isDevice) deviceApi else api).star(track, newValue)
        stars = if (newValue) stars + track.key else stars - track.key
    }

    fun importFiles(uris: List<Uri>, play: Boolean = false, onAdded: () -> Unit = {}) = task {
        val files = withContext(Dispatchers.IO) {
            val tree = localRoot.takeIf(String::isNotBlank)?.let { DocumentFile.fromTreeUri(context, Uri.parse(it)) }
            uris.map { deviceLibrary.stageFile(it, tree) }
        }
        val tracks = files.flatMap { deviceApi.expand(Track(kind = "device", path = it.absolutePath, name = it.name)) }
        add(tracks, play)
        if (tracks.isNotEmpty()) onAdded()
    }

    fun importFolder(uri: Uri) = task {
        localRoot = uri.toString()
        prefs.edit().putString("local_root", localRoot).apply()
        DocumentFile.fromTreeUri(context, uri)?.let(::browseDevice)
    }

    fun browseDevice(directory: DocumentFile) = task {
        val children = withContext(Dispatchers.IO) { directory.listFiles().toList() }
        deviceCurrent = directory
        deviceFolders.replaceWith(children.filter { it.isDirectory && it.name?.startsWith('.') != true }
            .sortedBy { it.name?.lowercase() })
        deviceFiles.replaceWith(children.filter { it.isFile && it.name?.startsWith('.') != true }
            .sortedBy { it.name?.lowercase() })
    }

    fun refreshDevice() {
        deviceCurrent?.let(::browseDevice)
    }

    fun deviceUp() {
        val current = deviceCurrent ?: return
        current.parentFile?.let(::browseDevice)
    }

    fun addDeviceFolder(folder: DocumentFile, onAdded: () -> Unit = {}) = task {
        importing = true
        try {
            val staged = withContext(Dispatchers.IO) { deviceLibrary.stageFolder(folder) }
            val tracks = deviceApi.collect(staged.absolutePath, root = deviceApi.deviceRoot)
            add(tracks)
            if (tracks.isNotEmpty()) onAdded()
        } finally {
            importing = false
        }
    }


}

private fun <T> androidx.compose.runtime.snapshots.SnapshotStateList<T>.replaceWith(items: List<T>) {
    clear()
    addAll(items)
}
