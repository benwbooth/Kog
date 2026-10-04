package org.kog.player

import android.content.Context
import android.os.Handler
import androidx.media3.common.C
import androidx.media3.common.ForwardingSimpleBasePlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import kotlin.random.Random

/** Media3 supplies audio output and system integration. Rust supplies policy.
 * The service owns this adapter so background EOS and headset commands use
 * exactly the same commands as the foreground UI. */
@UnstableApi
internal class PolicyPlayer(
    context: Context,
    private val output: ExoPlayer,
    private val publish: (JSONObject) -> Unit,
) : ForwardingSimpleBasePlayer(output) {
    private val policy = SharedPlaybackPolicy()
    private val api = KogApi(context)
    private val deviceApi = KogApi(context, onDevice = true)
    private var radioOnDevice = false
    private val prefs = context.getSharedPreferences("kog", Context.MODE_PRIVATE)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val handler = Handler(output.applicationLooper)
    private var radioJob: Job? = null
    private var radioRoot = ""
    private var editing = false
    private var awaitingStart = false
    private var previous = -1
    private var cursor = -1
    private var transportGeneration = 0L
    private var endScheduled = false
    private val workspaceQueueLock = Mutex()
    private var workspaceQueueGeneration = 0
    private var lastWorkspaceState = ""

    init {
        policy.send(command("init").put("seed", Random.nextLong(1, Long.MAX_VALUE))
            .put("shuffle", prefs.getString("shuffle_mode", if (prefs.getBoolean("shuffle", false)) "all" else "off"))
            .put("repeat", prefs.getString("repeat_mode", if (prefs.getBoolean("repeat", false)) "all" else "off")))
        prefs.getString("playlist_workspace", null)?.let { saved ->
            runCatching { policy.send(command("workspace_restore").put("value", JSONObject(saved))) }
        }
        // Prevent Media3 from choosing the next item at the end of a track.
        output.pauseAtEndOfMediaItems = true
        output.repeatMode = Player.REPEAT_MODE_OFF
        output.shuffleModeEnabled = false
        output.addListener(object : Player.Listener {
            override fun onTimelineChanged(timeline: Timeline, reason: Int) {
                if (!editing) sync()
            }
            override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
                if (reason == Player.PLAY_WHEN_READY_CHANGE_REASON_END_OF_MEDIA_ITEM) ended()
            }
            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState == Player.STATE_ENDED && output.playWhenReady) ended()
                if (playbackState == Player.STATE_READY && awaitingStart) started()
            }
            override fun onPlayerError(error: PlaybackException) {
                val generation = transportGeneration
                handler.post { if (generation == transportGeneration) navigate("failed") }
            }
        })
    }

    private fun tracks() = (0 until output.mediaItemCount).map { output.getMediaItemAt(it).kogTrack() }
    private fun current() = cursor.takeIf { it in 0 until output.mediaItemCount } ?: -1
    private fun sync(oldToNew: List<Int?>? = null) {
        if (oldToNew != null) cursor = oldToNew.getOrNull(cursor) ?: -1
        policy.sync(tracks(), current(), oldToNew)
        changed()
    }
    private fun publicSnapshot() = JSONObject(policy.snapshot.toString()).apply {
        remove("state"); remove("workspace_state"); remove("workspace_effect"); put("current", current())
    }
    private fun changed() {
        val workspaceState = policy.snapshot.optJSONObject("workspace_state")?.toString().orEmpty()
        if (workspaceState.isNotEmpty() && workspaceState != lastWorkspaceState) {
            prefs.edit().putString("playlist_workspace", workspaceState).apply(); lastWorkspaceState = workspaceState
        }
        val snapshot = publicSnapshot()
        prefs.edit().putString("shuffle_mode", snapshot.optString("shuffle"))
            .putString("repeat_mode", snapshot.optString("repeat")).apply()
        publish(snapshot)
        invalidateState()
    }
    private fun send(command: JSONObject): JSONObject {
        val reply = policy.send(command)
        changed()
        return reply
    }

    fun dispatch(request: JSONObject): JSONObject {
        sync()
        when (request.getString("op")) {
            "sync" -> Unit
            "workspace" -> workspace(request.getJSONObject("command"))
            "radio_toggle", "radio_reshuffle" -> {
                radioRoot = request.optString("root", radioRoot)
                radioOnDevice = request.optBoolean("on_device", radioOnDevice)
                val reshuffle = request.getString("op") == "radio_reshuffle"
                radioJob?.cancel()
                send(command("radio_reset").put("enabled", reshuffle || !policy.radio.optBoolean("enabled"))
                    .put("current", index(current())))
                refill(initial = true, reshuffle = reshuffle)
            }
            "radio_root" -> {
                val root = request.optString("root")
                if (root != radioRoot && policy.radio.optBoolean("enabled")) {
                    radioRoot = root
                    radioJob?.cancel()
                    send(command("radio_reset").put("enabled", true).put("current", index(current())))
                    refill(initial = true)
                }
            }
            "sort", "sort_rows" -> {
                val reply = send(request)
                val order = reply.getJSONArray("indices")
                val identities = (0 until output.mediaItemCount).toMutableList()
                editing = true
                try {
                    for (destination in 0 until order.length()) {
                        val source = identities.indexOf(order.getInt(destination))
                        output.moveMediaItem(source, destination)
                        identities.add(destination, identities.removeAt(source))
                    }
                } finally { editing = false }
                sync(identities.indices.map { identities.indexOf(it) })
            }
            else -> {
                request.put("current", index(current()))
                send(request)
                if (!policy.radio.optBoolean("enabled")) radioJob?.cancel()
            }
        }
        return publicSnapshot()
    }

    private fun workspaceApi(scope: String): KogApi {
        if (scope == "device") return deviceApi
        check(scope == "server:${api.server}") { "Reconnect to this playlist's server to load or save it." }
        return api
    }
    private fun workspaceTracks(rows: JSONArray?): List<Track> = if (rows == null) emptyList() else
        (0 until rows.length()).map { Track.parse(rows.getJSONObject(it)) }
    private fun workspace(request: JSONObject) {
        val effect = send(command("workspace").put("command", request)).optJSONObject("workspace_effect") ?: return
        when (effect.optString("action")) {
            "load" -> scope.launch {
                val key = effect.getString("key"); val generation = effect.getLong("generation")
                try {
                    val entries = workspaceApi(effect.getString("scope")).playlist(effect.getLong("playlist_id"))
                    workspace(command("loaded").put("key", key).put("generation", generation).put("entries", JSONArray(entries.map { it.saved() })))
                } catch (error: Exception) {
                    workspace(command("load_failed").put("key", key).put("generation", generation).put("error", error.message))
                }
            }
            "save" -> scope.launch {
                val key = effect.getString("key"); val revision = effect.getLong("revision")
                try {
                    workspaceApi(effect.getString("scope")).replacePlaylist(effect.getLong("playlist_id"),
                        workspaceTracks(effect.optJSONArray("entries")), workspaceTracks(effect.optJSONArray("expected_entries")))
                    workspace(command("saved").put("key", key).put("revision", revision))
                } catch (error: Exception) {
                    workspace(command("save_failed").put("key", key).put("revision", revision).put("error", error.message))
                }
            }
            "queue" -> {
                val generation = workspaceQueueGeneration
                scope.launch { workspaceQueueLock.withLock {
                    try {
                        if (generation != workspaceQueueGeneration) return@withLock
                        val source = effect.getString("scope")
                        val client = workspaceApi(source)
                        val expanded = workspaceTracks(effect.optJSONArray("entries")).flatMap { client.expand(it) }
                        workspaceApi(source) // A connection change invalidates the old request.
                        if (generation != workspaceQueueGeneration) return@withLock
                        val start = output.mediaItemCount
                        output.addMediaItems(expanded.map { it.mediaItem(api) }); sync()
                        val decision = send(command("apply_queue_action").put("action", effect.getString("mode"))
                            .put("start", start).put("count", expanded.size)).optJSONObject("decision")
                        if (decision?.optString("action") == "play") {
                            send(command("cancel_waiting")); activate(decision.getInt("index"))
                        }
                    } catch (error: Exception) {
                        publish(publicSnapshot().put("error", error.message))
                    }
                } }
            }
        }
    }

    private fun ended() {
        if (endScheduled) return
        endScheduled = true
        val generation = transportGeneration
        handler.post {
            endScheduled = false
            if (generation == transportGeneration) navigate("ended")
        }
    }
    private fun started() {
        awaitingStart = false
        if (current() >= 0) send(command("started").put("previous", index(previous)).put("index", current()))
    }
    private fun activate(index: Int) {
        transportGeneration++
        previous = current()
        cursor = index
        awaitingStart = true
        output.seekTo(index, 0)
        output.prepare()
        output.play()
        if (output.playbackState == Player.STATE_READY) started()
    }
    private fun navigate(event: String) {
        sync()
        val decision = send(command("navigate").put("event", event).put("current", index(current())))
            .getJSONObject("decision")
        when (decision.getString("action")) {
            "play" -> activate(decision.getInt("index"))
            "radio" -> { consumeRadio("radio_next"); refill() }
            else -> stopOutput()
        }
    }
    private fun cancelNavigation() {
        transportGeneration++
        awaitingStart = false
        send(command("cancel_navigation"))
        send(command("cancel_waiting"))
    }
    private fun stopOutput() {
        cancelNavigation()
        output.pause()
        output.stop()
        if (current() >= 0) output.seekTo(current(), 0)
    }

    private fun consumeRadio(op: String) {
        val entry = send(command(op)).optJSONObject("entry")
        if (entry != null) {
            output.addMediaItem(Track.parse(entry).mediaItem(api))
            sync()
            send(command("radio_candidate").put("index", output.mediaItemCount - 1))
            activate(output.mediaItemCount - 1)
        } else if (op == "radio_next" && !policy.waiting) stopOutput()
    }
    private fun refill(initial: Boolean = false, reshuffle: Boolean = false) {
        if (!initial && !policy.needsRefill) return
        send(command("radio_begin"))
        val generation = policy.generation
        val enabled = policy.radio.optBoolean("enabled")
        val root = radioRoot
        val client = if (radioOnDevice) deviceApi else api
        radioJob = scope.launch {
            try {
                val batch = if (reshuffle) client.reshuffleRadio(root) else if (initial) client.radio(enabled, root) else client.radioAdvance(root)
                if (generation != policy.generation) return@launch
                send(command("radio_accept").put("generation", generation)
                    .put("entries", JSONArray(batch.tracks.map { it.saved() })).put("exhausted", batch.exhausted))
                consumeRadio("radio_pending")
                refill()
            } catch (error: Exception) {
                if (generation != policy.generation) return@launch
                send(command("radio_fail").put("generation", generation)).put("error", error.message)
                changed()
            }
        }
    }

    override fun getState(): State {
        val base = super.getState()
        val commands = base.availableCommands.buildUpon().addAll(
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM).build()
        return base.buildUpon().setAvailableCommands(commands)
            .setShuffleModeEnabled(policy.snapshot.optString("shuffle", "off") != "off")
            .setRepeatMode(when (policy.snapshot.optString("repeat")) {
                "one" -> Player.REPEAT_MODE_ONE
                "all", "album" -> Player.REPEAT_MODE_ALL
                else -> Player.REPEAT_MODE_OFF
            }).build()
    }
    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> navigate("next")
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> navigate("previous")
            else -> {
                if (seekCommand == Player.COMMAND_SEEK_TO_MEDIA_ITEM) {
                    cancelNavigation(); previous = current(); cursor = mediaItemIndex; awaitingStart = true
                }
                val result = super.handleSeek(mediaItemIndex, positionMs, seekCommand)
                if (awaitingStart && output.playbackState == Player.STATE_READY) started()
                changed()
                return result
            }
        }
        return Futures.immediateVoidFuture()
    }
    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (!playWhenReady) send(command("cancel_waiting"))
        if (playWhenReady && current() < 0 && output.mediaItemCount > 0) activate(0)
        else if (playWhenReady && current() < 0 && policy.radio.optBoolean("enabled")) navigate("next")
        else {
            if (playWhenReady && output.playbackState == Player.STATE_IDLE) output.prepare()
            output.playWhenReady = playWhenReady
        }
        return Futures.immediateVoidFuture()
    }
    override fun handleStop(): ListenableFuture<*> {
        stopOutput()
        return Futures.immediateVoidFuture()
    }
    override fun handleSetShuffleModeEnabled(enabled: Boolean): ListenableFuture<*> {
        dispatch(command("set_shuffle").put("mode", if (enabled) "all" else "off"))
        return Futures.immediateVoidFuture()
    }
    override fun handleSetRepeatMode(mode: Int): ListenableFuture<*> {
        dispatch(command("set_repeat").put("mode", when (mode) {
            Player.REPEAT_MODE_ONE -> "one"; Player.REPEAT_MODE_ALL -> "all"; else -> "off"
        }))
        return Futures.immediateVoidFuture()
    }
    override fun handleSetMediaItems(items: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> {
        workspaceQueueGeneration++
        cancelNavigation()
        cursor = startIndex.takeIf { it in items.indices } ?: -1
        editing = true
        val result = try { super.handleSetMediaItems(items, startIndex, startPositionMs) } finally { editing = false }
        sync()
        return result
    }
    override fun handleMoveMediaItems(fromIndex: Int, toIndex: Int, newIndex: Int): ListenableFuture<*> {
        val order = (0 until output.mediaItemCount).toMutableList()
        val moving = order.subList(fromIndex, toIndex).toList()
        order.subList(fromIndex, toIndex).clear()
        order.addAll(newIndex.coerceAtMost(order.size), moving)
        editing = true
        val result = try { super.handleMoveMediaItems(fromIndex, toIndex, newIndex) } finally { editing = false }
        sync(order.indices.map { order.indexOf(it) })
        return result
    }
    override fun handleRemoveMediaItems(fromIndex: Int, toIndex: Int): ListenableFuture<*> {
        val oldSize = output.mediaItemCount
        val removedCurrent = current() in fromIndex until toIndex
        if (removedCurrent) stopOutput()
        editing = true
        val result = try { super.handleRemoveMediaItems(fromIndex, toIndex) } finally { editing = false }
        sync((0 until oldSize).map { if (it < fromIndex) it else if (it < toIndex) null else it - (toIndex - fromIndex) })
        if (output.mediaItemCount == 0) { workspaceQueueGeneration++; stopOutput() }
        return result
    }
    override fun handleRelease(): ListenableFuture<*> {
        scope.cancel()
        return super.handleRelease()
    }

    private fun command(op: String) = SharedPlaybackPolicy.command(op)
    private fun index(index: Int) = SharedPlaybackPolicy.index(index)
}
