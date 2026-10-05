package org.kog.player

import android.content.Context
import android.os.Handler
import androidx.media3.common.C
import androidx.media3.common.ForwardingSimpleBasePlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

/** The Rust session owns the application. Media3 is its audio/system-control port.
 * This adapter mirrors the session's row IDs into Media3's timeline, executes
 * effects, and returns callbacks bearing the originating session token. */
@UnstableApi
internal class PolicyPlayer(
    context: Context,
    private val output: ExoPlayer,
    sessionID: String? = null,
    private val publish: (JSONObject) -> Unit,
) : ForwardingSimpleBasePlayer(output) {
    private val prefs = KogPreferences(context)
    private val legacy = context.getSharedPreferences("kog", Context.MODE_PRIVATE)
    private val session = SharedBackendSession(sessionID ?: SharedBackendSession.defaultID(context))
    private val storageKey = "backend_session.${session.id}"
    private val api = KogApi(context).apply { this.sessionID = session.id }
    private val deviceApi = KogApi(context, onDevice = true).apply { this.sessionID = session.id }
    private var storageRevision: Long? = null
    private var storageError: String? = null
    private var lastSavedCheckpoint: String? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val handler = Handler(output.applicationLooper)
    private var applyingOutput = false
    private var outputToken: JSONObject? = null
    private var outputRow: Long? = null
    private var awaitingStart = false
    private var endScheduled = false
    private var knownScopes: String? = null
    private var radioOnDevice = false
    private var radioRoot = ""
    private val progress = object : Runnable {
        override fun run() {
            if (outputToken != null && !applyingOutput) outputEvent(command("progress")
                .put("seconds", output.currentPosition.coerceAtLeast(0) / 1000.0)
                .put("duration", output.duration.coerceAtLeast(0) / 1000.0))
            handler.postDelayed(this, 500)
        }
    }

    init {
        output.pauseAtEndOfMediaItems = true
        output.repeatMode = Player.REPEAT_MODE_OFF
        output.shuffleModeEnabled = false
        val stored = runCatching { deviceApi.localState("sessions", session.id) }
            .onSuccess { storageRevision = it.getLong("revision") }
            .onFailure { storageError = it.message ?: "Cannot load the saved session" }.getOrNull()
        val saved = stored?.optJSONObject("value")?.toString() ?: if (storageRevision == 0L) legacy.getString(storageKey, null) else null
        if (stored != null && storageRevision != 0L && stored.optJSONObject("value") == null) {
            storageError = "Invalid saved session; the original has been preserved"
            storageRevision = null
        }
        if (saved != null) {
            runCatching { apply(session.send(restore = JSONObject(saved))) }.onFailure {
                storageError = it.message ?: "Invalid saved session; the original has been preserved"
                storageRevision = null
            }
        } else if (storageRevision == 0L) {
            send(command("replace").put("tracks", if (sessionID == null) JSONArray(legacy.getString("queue", "[]")) else JSONArray())
                .put("current", if (sessionID == null) prefs.getInt("index", 0) else JSONObject.NULL))
            if (sessionID == null) {
                send(command("shuffle").put("mode", prefs.getString("shuffle_mode", if (prefs.getBoolean("shuffle", false)) "all" else "off")))
                send(command("repeat").put("mode", prefs.getString("repeat_mode", if (prefs.getBoolean("repeat", false)) "all" else "off")))
                legacy.getString("playlist_workspace", null)?.let { send(command("workspace_restore").put("value", JSONObject(it))) }
            }
        }
        updateScopes()
        output.volume = session.snapshot.optDouble("volume", 1.0).toFloat()
        output.addListener(object : Player.Listener {
            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                if (!applyingOutput && reason == Player.PLAY_WHEN_READY_CHANGE_REASON_END_OF_MEDIA_ITEM) ended()
            }
            override fun onPlaybackStateChanged(playbackState: Int) {
                if (applyingOutput) return
                if (playbackState == Player.STATE_ENDED && output.playWhenReady) ended()
                if (playbackState == Player.STATE_READY && awaitingStart) started()
            }
            override fun onPlayerError(error: PlaybackException) {
                if (applyingOutput) return
                val token = outputToken ?: return
                handler.post { outputEvent(command("failed").put("error", error.message), token) }
            }
        })
        val checkpoint = session.send().getJSONObject("checkpoint")
        persistCheckpoint(checkpoint)
        radioRoot = checkpoint.optString("radio_root")
        radioOnDevice = checkpoint.optString("radio_scope") == "device"
        if (session.snapshot.optBoolean("radio_enabled")) send(command("radio").put("enabled", true)
            .put("scope", checkpoint.optString("radio_scope")).put("root", radioRoot))
        handler.post(progress)
    }

    private fun tracks(rows: JSONArray? = session.snapshot.optJSONArray("queue")): List<Track> =
        if (rows == null) emptyList() else (0 until rows.length()).map { Track.parse(rows.getJSONObject(it)) }
    private fun current() = session.snapshot.optInt("current", -1)
    private fun send(request: JSONObject): JSONObject {
        val reply = session.send(request)
        apply(reply)
        return session.snapshot
    }
    private fun apply(reply: JSONObject) {
        val effects = reply.optJSONArray("effects") ?: JSONArray()
        // Persist before output callbacks can re-enter the session.
        for (i in 0 until effects.length()) if (effects.getJSONObject(i).optString("action") == "persist") {
            persistCheckpoint(effects.getJSONObject(i).getJSONObject("value"))
        }
        mirrorTimeline()
        publishSnapshot()
        invalidateState()
        for (i in 0 until effects.length()) execute(effects.getJSONObject(i))
    }
    private fun persistCheckpoint(value: JSONObject) {
        val text = value.toString()
        if (text == lastSavedCheckpoint) return
        storageRevision?.let { revision ->
            runCatching { deviceApi.localState("sessions", session.id, value, revision) }
                .onSuccess { storageRevision = it.getLong("revision"); lastSavedCheckpoint = text; storageError = null }
                .onFailure { storageError = it.message ?: "Cannot save the session" }
        }
    }
    private fun publishSnapshot() {
        val view = JSONObject(session.snapshot.toString())
        storageError?.let { view.put("error", it) }
        publish(view)
    }
    private inline fun outputMutation(block: () -> Unit) {
        val previous = applyingOutput; applyingOutput = true
        try { block() } finally { applyingOutput = previous }
    }
    private fun mirrorTimeline() = outputMutation {
        val rows = session.snapshot.optJSONArray("row_ids") ?: JSONArray()
        val entries = tracks()
        val wanted = (0 until rows.length()).map { "session-row:${rows.getLong(it)}" }
        for (index in output.mediaItemCount - 1 downTo 0)
            if (output.getMediaItemAt(index).mediaId !in wanted) output.removeMediaItem(index)
        wanted.forEachIndexed { index, id ->
            val existing = (index until output.mediaItemCount).firstOrNull { output.getMediaItemAt(it).mediaId == id }
            if (existing == null) output.addMediaItem(index, entries[index].mediaItem(api).buildUpon().setMediaId(id).build())
            else if (existing != index) output.moveMediaItem(existing, index)
        }
    }
    private fun updateScopes() {
        val key = api.server
        if (knownScopes != key || !session.snapshot.has("session_id")) {
            knownScopes = key
            send(command("scopes").put("scopes", JSONArray().put("device").put("server:$key")))
        }
    }
    fun dispatch(request: JSONObject): JSONObject {
        updateScopes()
        when (request.getString("op")) {
            "sync" -> { publishSnapshot(); invalidateState() }
            "radio_toggle", "radio_reshuffle", "radio_root" -> {
                val op = request.getString("op")
                val root = request.optString("root", radioRoot)
                val device = request.optBoolean("on_device", radioOnDevice)
                if (op != "radio_root" || root != radioRoot || device != radioOnDevice) {
                    radioRoot = root; radioOnDevice = device
                    send(command("radio").put("root", root).put("scope", if (device) "device" else "server:${api.server}")
                        .put("enabled", if (op == "radio_toggle") !session.snapshot.optBoolean("radio_enabled") else true)
                        .put("reshuffle", op == "radio_reshuffle"))
                }
            }
            "reload_output" -> {
                outputToken?.let { outputEvent(command("progress").put("seconds", output.currentPosition.coerceAtLeast(0) / 1000.0)
                    .put("duration", output.duration.coerceAtLeast(0) / 1000.0), it) }
                send(request)
            }
            else -> send(request)
        }
        return session.snapshot
    }
    private fun client(source: String): KogApi {
        if (source == "device") return deviceApi.snapshot()
        check(source == "server:${api.server}") { "Reconnect to this playlist's server to load or save it." }
        return api.snapshot()
    }
    private fun execute(effect: JSONObject) {
        when (effect.getString("action")) {
            "play" -> {
                val token = effect.getJSONObject("token")
                outputToken = token
                val index = effect.getInt("index")
                outputRow = session.snapshot.getJSONArray("row_ids").getLong(index)
                awaitingStart = true; endScheduled = false
                try {
                    outputMutation {
                        // Refresh the URI/decoder configuration for this play request.
                        output.replaceMediaItem(index, tracks()[index].mediaItem(api).buildUpon().setMediaId("session-row:$outputRow").build())
                        output.seekTo(index, (effect.getDouble("seconds") * 1000).toLong())
                        output.prepare(); output.playWhenReady = effect.getBoolean("playing")
                    }
                    if (output.playbackState == Player.STATE_READY) started()
                } catch (error: Exception) { outputEvent(command("failed").put("error", error.message), token) }
            }
            "pause" -> output.pause()
            "resume" -> output.play()
            "stop" -> {
                outputToken = null; outputRow = null; awaitingStart = false; endScheduled = false
                outputMutation { output.pause(); output.stop(); if (current() >= 0) output.seekTo(current(), 0) }
            }
            "seek" -> output.seekTo((effect.getDouble("seconds") * 1000).toLong())
            "volume" -> output.volume = effect.getDouble("value").toFloat()
            "load", "save", "expand", "collect", "radio" -> scope.launch {
                val token = effect.getJSONObject("token")
                try {
                    val source = effect.getString("scope")
                    val api = client(source).apply { radioRequest = token }
                    val result = when (effect.getString("action")) {
                        "load" -> JSONObject().put("kind", "loaded").put("entries", JSONArray(api.playlist(effect.getLong("playlist_id")).map { it.saved() }))
                        "save" -> {
                            api.replacePlaylist(effect.getLong("playlist_id"), tracks(effect.getJSONArray("entries")), tracks(effect.getJSONArray("expected_entries")))
                            JSONObject().put("kind", "saved")
                        }
                        "expand" -> JSONObject().put("kind", "expanded").put("tracks", JSONArray(api.expand(tracks(effect.getJSONArray("entries"))).map { it.saved() }))
                        "collect" -> JSONObject().put("kind", "expanded").put("tracks", JSONArray(api.collect(effect.getString("path"), effect.getString("query"), effect.getString("root")).map { it.saved() }))
                        else -> {
                            val root = effect.getString("root")
                            val batch = if (effect.getBoolean("reshuffle")) api.reshuffleRadio(root)
                                else if (effect.getBoolean("reset")) api.radio(effect.getBoolean("enabled"), root) else api.radioAdvance(root)
                            JSONObject().put("kind", "radio").put("tracks", JSONArray(batch.tracks.map { it.saved() })).put("exhausted", batch.exhausted)
                        }
                    }
                    updateScopes()
                    send(command("complete").put("token", token).put("result", result))
                } catch (error: Exception) {
                    send(command("complete").put("token", token).put("result", JSONObject().put("kind", "failed").put("error", error.message ?: "Request failed")))
                }
            }
        }
    }
    private fun outputEvent(event: JSONObject, token: JSONObject? = outputToken) {
        if (token == null) return
        val payload = JSONObject(event.toString()).put("event", event.getString("op")); payload.remove("op")
        send(command("output").put("token", token).put("event", payload))
    }
    private fun started() {
        if (output.currentMediaItem?.mediaId != "session-row:$outputRow") return
        awaitingStart = false
        outputEvent(command("started"))
    }
    private fun ended() {
        if (endScheduled || output.currentMediaItem?.mediaId != "session-row:$outputRow") return
        val token = outputToken ?: return
        endScheduled = true
        handler.post { endScheduled = false; outputEvent(command("ended"), token) }
    }
    override fun getState(): State {
        val base = super.getState()
        return base.buildUpon().setAvailableCommands(base.availableCommands.buildUpon().addAll(
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM).build())
            .setShuffleModeEnabled(session.snapshot.optString("shuffle", "off") != "off")
            .setRepeatMode(when (session.snapshot.optString("repeat")) {
                "one" -> Player.REPEAT_MODE_ONE; "all", "album" -> Player.REPEAT_MODE_ALL; else -> Player.REPEAT_MODE_OFF
            }).build()
    }
    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> send(command("navigate").put("event", "next"))
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> send(command("navigate").put("event", "previous"))
            else -> {
                if (mediaItemIndex != current()) send(command("play").put("index", mediaItemIndex))
                if (positionMs != C.TIME_UNSET) send(command("seek").put("seconds", positionMs / 1000.0))
            }
        }
        return Futures.immediateVoidFuture()
    }
    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> = done(command(if (playWhenReady) "resume" else "pause"))
    override fun handleStop(): ListenableFuture<*> = done(command("stop"))
    override fun handleSetShuffleModeEnabled(enabled: Boolean): ListenableFuture<*> = done(command("shuffle").put("mode", if (enabled) "all" else "off"))
    override fun handleSetRepeatMode(mode: Int): ListenableFuture<*> = done(command("repeat").put("mode", when (mode) {
        Player.REPEAT_MODE_ONE -> "one"; Player.REPEAT_MODE_ALL -> "all"; else -> "off"
    }))
    override fun handleSetVolume(volume: Float, flags: Int): ListenableFuture<*> = done(command("volume").put("value", volume.toDouble()))
    override fun handleSetMediaItems(items: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> =
        done(command("replace").put("tracks", JSONArray(items.map { it.kogTrack().saved() })).put("current", SharedPlaybackPolicy.index(startIndex)))
    override fun handleAddMediaItems(index: Int, items: MutableList<MediaItem>): ListenableFuture<*> =
        done(command("insert").put("index", index).put("tracks", JSONArray(items.map { it.kogTrack().saved() })))
    override fun handleMoveMediaItems(fromIndex: Int, toIndex: Int, newIndex: Int): ListenableFuture<*> =
        done(command("move").put("indices", JSONArray((fromIndex until toIndex).toList()))
            .put("target", if (newIndex > fromIndex) newIndex + toIndex - fromIndex else newIndex))
    override fun handleReplaceMediaItems(fromIndex: Int, toIndex: Int, items: MutableList<MediaItem>): ListenableFuture<*> =
        done(command("replace_range").put("start", fromIndex).put("end", toIndex).put("tracks", JSONArray(items.map { it.kogTrack().saved() })))
    override fun handleRemoveMediaItems(fromIndex: Int, toIndex: Int): ListenableFuture<*> =
        done(command("remove").put("indices", JSONArray((fromIndex until toIndex).toList())))
    override fun handleRelease(): ListenableFuture<*> {
        handler.removeCallbacks(progress); scope.cancel()
        return super.handleRelease()
    }
    private fun done(request: JSONObject): ListenableFuture<*> { send(request); return Futures.immediateVoidFuture() }
    private fun command(op: String) = JSONObject().put("op", op)
}
