package org.kog.player

import android.content.ComponentName
import android.content.Context
import android.media.MediaMetadataRetriever
import android.net.Uri
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
import java.io.File

/** Queue and selection belong to this Android client; library rules belong to Rust. */
class KogState(private val context: Context) {
    val api = KogApi(context)
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
    var selectedPlaylist by mutableStateOf<SavedPlaylist?>(null)
        private set
    var searchText by mutableStateOf("")
    var searching by mutableStateOf(false)
        private set
    var searchScanned by mutableStateOf(0)
        private set
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
    var shuffleOn by mutableStateOf(false)
    var repeatOn by mutableStateOf(false)
    var localMidiEngine by mutableStateOf(prefs.getString("local_midi_engine", "opl3windows") ?: "opl3windows")
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
        shuffleOn = prefs.getBoolean("shuffle", false)
        repeatOn = prefs.getBoolean("repeat", false)
        localRoot = prefs.getString("local_root", "").orEmpty()
        if (localRoot.isNotBlank()) {
            DocumentFile.fromTreeUri(context, Uri.parse(localRoot))?.let(::browseDevice)
        }
    }

    fun connectPlayer() {
        if (controllerFuture != null) return
        val future = MediaController.Builder(context, SessionToken(context,
            ComponentName(context, PlaybackService::class.java))).buildAsync()
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
                controller?.shuffleModeEnabled = shuffleOn
                controller?.repeatMode = if (repeatOn) Player.REPEAT_MODE_ALL else Player.REPEAT_MODE_OFF
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
        if (current?.isDevice == true) restartCurrentMidi()
    }

    private fun syncPlayer() {
        val player = controller ?: return
        currentIndex = player.currentMediaItemIndex
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

    private fun mediaItem(track: Track): MediaItem {
        val meta = MediaMetadata.Builder()
            .setTitle(track.label).setArtist(track.artist).setAlbumTitle(track.album)
        api.art(track)?.let { meta.setArtworkUri(Uri.parse(it)) }
        val builder = MediaItem.Builder().setMediaId(track.key).setMediaMetadata(meta.build())
        if (NativeAudio.useFor(track)) {
            builder.setUri(NativeAudio.uri(track)).setMimeType(MimeTypes.AUDIO_WAV)
        } else {
            builder.setUri(api.stream(track))
        }
        return builder.build()
    }

    private fun saveQueue() {
        val rows = JSONArray()
        queue.forEach { rows.put(it.saved()) }
        prefs.edit().putString("queue", rows.toString())
            .putInt("index", currentIndex.coerceAtLeast(0))
            .putLong("position", position)
            .putBoolean("shuffle", shuffleOn).putBoolean("repeat", repeatOn).apply()
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
        stars = api.stars()
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
        val tracks = if (track.isDevice) listOf(track) else api.expand(track)
        add(tracks, play)
        if (tracks.isNotEmpty()) onAdded()
    }

    fun addFolder(folder: Folder, play: Boolean = false, onAdded: () -> Unit = {}) = task {
        val tracks = api.collect(folder.path)
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
        if (currentIndex == index) {
            if (player.isPlaying) player.pause() else player.play()
        } else {
            player.seekTo(index, 0)
            player.play()
        }
        syncPlayer()
    }

    fun toggle() {
        controller?.let { if (it.isPlaying) it.pause() else it.play() }
        syncPlayer()
    }
    fun previous() { controller?.seekToPreviousMediaItem(); controller?.play(); syncPlayer() }
    fun next() {
        val player = controller ?: return
        if (radioOn && currentIndex >= queue.lastIndex) {
            task {
                add(api.radioAdvance(libraryRoot))
                player.seekToNextMediaItem()
                player.play()
                syncPlayer()
            }
        } else {
            if (radioOn && currentIndex >= queue.lastIndex - 2) refillRadio()
            player.seekToNextMediaItem()
            player.play()
            syncPlayer()
        }
    }
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

    fun sortQueue(field: String) {
        val playingKey = current?.key
        val at = position
        val wasPlaying = playing
        val sorted = when (field) {
            "Artist" -> queue.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.artist })
            "Album" -> queue.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.album })
            "Duration" -> queue.sortedBy { it.duration }
            else -> queue.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.label })
        }
        queue.replaceWith(sorted)
        val index = queue.indexOfFirst { it.key == playingKey }.coerceAtLeast(0)
        controller?.setMediaItems(queue.map(::mediaItem), index, at)
        controller?.prepare()
        if (wasPlaying) controller?.play()
        saveQueue()
    }

    fun shuffle() {
        shuffleOn = !shuffleOn
        controller?.shuffleModeEnabled = shuffleOn
        saveQueue()
    }
    fun repeat() {
        repeatOn = !repeatOn
        controller?.repeatMode = if (repeatOn) Player.REPEAT_MODE_ALL else Player.REPEAT_MODE_OFF
        saveQueue()
    }

    fun radio() = task {
        val enabled = !radioOn
        val tracks = api.radio(enabled, libraryRoot)
        radioOn = enabled
        if (enabled) add(tracks, queue.isEmpty())
    }
    private fun refillRadio() = task { add(api.radioAdvance(libraryRoot)) }

    fun loadPlaylists() = task { playlists.replaceWith(api.playlists()) }
    fun openPlaylist(item: SavedPlaylist) = task {
        selectedPlaylist = item
        playlistTracks.replaceWith(api.playlist(item.id))
    }
    fun closePlaylist() { selectedPlaylist = null; playlistTracks.clear() }
    fun createPlaylist(name: String) = task { api.createPlaylist(name); playlists.replaceWith(api.playlists()) }
    fun renamePlaylist(item: SavedPlaylist, name: String) = task {
        api.renamePlaylist(item.id, name); playlists.replaceWith(api.playlists())
        selectedPlaylist = selectedPlaylist?.copy(name = name)
    }
    fun deletePlaylist(item: SavedPlaylist) = task {
        api.deletePlaylist(item.id); playlists.replaceWith(api.playlists()); closePlaylist()
    }
    fun saveToPlaylist(item: SavedPlaylist, tracks: List<Track>) = task {
        api.appendPlaylist(item.id, tracks)
        playlists.replaceWith(api.playlists())
        if (selectedPlaylist?.id == item.id) playlistTracks.replaceWith(api.playlist(item.id))
    }
    fun replacePlaylist(item: SavedPlaylist, tracks: List<Track>) = task {
        api.appendPlaylist(item.id, tracks)
        playlists.replaceWith(api.playlists())
    }
    fun toggleStar(track: Track) = task {
        if (track.isDevice) return@task
        val newValue = track.key !in stars
        api.star(track, newValue)
        stars = if (newValue) stars + track.key else stars - track.key
    }

    fun importFiles(uris: List<Uri>, play: Boolean = false, onAdded: () -> Unit = {}) = task {
        val tracks = withContext(Dispatchers.IO) { uris.mapNotNull(::localTrack) }
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
            val tracks = withContext(Dispatchers.IO) {
            val found = mutableListOf<Uri>()
            fun visit(directory: DocumentFile) {
                for (child in directory.listFiles()) {
                    if (child.isDirectory) visit(child) else if (child.isFile) found.add(child.uri)
                }
            }
            visit(folder)
            found.mapNotNull(::localTrack)
            }
            add(tracks)
            if (tracks.isNotEmpty()) onAdded()
        } finally {
            importing = false
        }
    }

    private fun localTrack(uri: Uri): Track? {
        val row = DocumentFile.fromSingleUri(context, uri)
        val name = row?.name ?: uri.lastPathSegment.orEmpty()
        val retriever = MediaMetadataRetriever()
        val tags = runCatching {
            retriever.setDataSource(context, uri)
            listOf(
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_TITLE).orEmpty(),
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ARTIST).orEmpty(),
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUM).orEmpty(),
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION).orEmpty(),
            )
        }.getOrDefault(listOf("", "", "", ""))
        runCatching { retriever.release() }
        return Track("device", uri.toString(), name = name, title = tags[0],
            artist = tags[1], album = tags[2], duration = tags[3].toLongOrNull() ?: 0)
    }
}

private fun <T> androidx.compose.runtime.snapshots.SnapshotStateList<T>.replaceWith(items: List<T>) {
    clear()
    addAll(items)
}
