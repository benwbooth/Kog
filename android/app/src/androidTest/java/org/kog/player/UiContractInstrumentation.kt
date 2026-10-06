package org.kog.player

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.graphics.Rect
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import androidx.activity.compose.setContent
import androidx.compose.runtime.mutableStateOf
import androidx.compose.material3.MaterialTheme
import android.os.Bundle
import org.json.JSONArray
import org.json.JSONObject

/** Runs the same expected UI-state transcript through the packaged JNI backend and UI decoder. */
class UiContractInstrumentation : Instrumentation() {
    override fun onCreate(arguments: Bundle?) { super.onCreate(arguments); start() }
    override fun onStart() {
        val status = Bundle().apply {
            putString("id", "KogUiContract"); putString("class", UiContractInstrumentation::class.java.name)
            putString("test", "equivalentPlaylistActions"); putInt("numtests", 1); putInt("current", 1)
        }
        sendStatus(1, status)
        try {
            tabGestureContract()
            persistenceContract()
            librarySessionContract()
            val sessionSteps = sessionContract()
            mediaPortContract()
            val fixture = JSONObject(context.assets.open("playlist.json").bufferedReader().use { it.readText() })
            val policy = SharedPlaybackPolicy()
            var load = JSONObject(); var save = JSONObject()
            val steps = fixture.getJSONArray("steps")
            for (index in 0 until steps.length()) {
                val step = steps.getJSONObject(index)
                val command = when {
                    step.has("load") -> JSONObject().put("op", "workspace").put("command", JSONObject()
                        .put("op", "loaded").put("key", load.getString("key"))
                        .put("generation", load.getLong("generation")).put("entries", step.getJSONArray("load")))
                    step.has("save_ok") -> JSONObject().put("op", "workspace").put("command", JSONObject()
                        .put("op", "saved").put("key", save.getString("key")).put("revision", save.getLong("revision")))
                    else -> step.getJSONObject("command")
                }
                val reply = policy.send(command)
                PlaylistWorkspaceSnapshot.parse(reply.getJSONObject("workspace"))
                reply.optJSONObject("workspace_effect")?.let { effect -> when (effect.optString("action")) {
                    "load" -> load = effect; "save" -> save = effect
                } }
                val expected = step.getJSONObject("expect")
                for (path in expected.keys()) {
                    val actual = path.split('.').fold(reply as Any?) { value, key -> when (value) {
                        is JSONObject -> value.opt(key); is JSONArray -> value.opt(key.toInt()); else -> null
                    } }
                    check(equalJson(actual, expected.get(path))) { "Kotlin UI contract step $index $path: expected ${expected.get(path)}, got $actual" }
                }
            }
            status.putString("stream", "Kotlin contracts: real tab long-press drags and cancellation, $sessionSteps application-session steps, two Media3 sessions with restore and native audio EOS, and ${steps.length()} UI steps passed through production JNI and workspace decoder\n")
            sendStatus(0, status)
            finish(Activity.RESULT_OK, Bundle().apply { putString("stream", status.getString("stream")) })
        } catch (error: Throwable) {
            status.putString("stack", error.stackTraceToString()); status.putString("stream", error.stackTraceToString())
            sendStatus(-2, status); finish(Activity.RESULT_CANCELED, status)
        }
    }
    private fun tabGestureContract() {
        val activity = startActivitySync(Intent(targetContext, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) as MainActivity
        val initial = PlaylistWorkspaceSnapshot(active = "b", tabs = listOf(
            PlaylistWorkspaceTab("queue", "Play Queue"), PlaylistWorkspaceTab("a", "Alpha", dirty = true), PlaylistWorkspaceTab("b", "Beta")))
        val state = mutableStateOf(initial)
        val commands = mutableListOf<String>()
        runOnMainSync {
            activity.setContent { MaterialTheme {
                PlaylistWorkspaceTabs(state.value) { op, fields ->
                    commands.add(op)
                    if (op == "move_tab") {
                        val next = state.value.tabs.toMutableList()
                        val tab = next.first { it.key == fields.getString("key") }
                        next.remove(tab)
                        val before = if (fields.isNull("before")) null else fields.getString("before")
                        next.add(if (before == null) next.size else next.indexOfFirst { it.key == before }, tab)
                        state.value = state.value.copy(tabs = next)
                    } else if (op == "focus") state.value = state.value.copy(active = fields.getString("key"))
                }
            } }
        }
        waitForIdleSync()
        fun bounds(label: String): Rect {
            repeat(50) {
                val nodes = uiAutomation.rootInActiveWindow?.findAccessibilityNodeInfosByText(label).orEmpty()
                if (nodes.isNotEmpty()) return Rect().also { nodes.first().getBoundsInScreen(it) }
                SystemClock.sleep(100)
            }
            error("Missing tab $label")
        }
        fun drag(from: Rect, toX: Float, toY: Float) {
            val down = SystemClock.uptimeMillis()
            fun pointer(action: Int, x: Float, y: Float) {
                val event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0)
                event.source = InputDevice.SOURCE_TOUCHSCREEN
                sendPointerSync(event); event.recycle()
            }
            val x = from.centerX().toFloat(); val y = from.centerY().toFloat()
            pointer(MotionEvent.ACTION_DOWN, x, y); SystemClock.sleep(650)
            for (step in 1..12) { pointer(MotionEvent.ACTION_MOVE, x+(toX-x)*step/12, y+(toY-y)*step/12); SystemClock.sleep(20) }
            check(commands.isEmpty()) { "Drag switched tabs or committed before release: $commands" }
            pointer(MotionEvent.ACTION_UP, toX, toY); waitForIdleSync(); SystemClock.sleep(150)
        }
        try {
            val queue = bounds("Play Queue")
            drag(bounds("Alpha"), queue.left.toFloat()+3, queue.centerY().toFloat())
            check(state.value.tabs.map { it.key } == listOf("a", "queue", "b")) { "Long press did not reorder: ${state.value.tabs}" }
            check(state.value.active == "b" && state.value.tabs[0].dirty)
            check(commands == listOf("move_tab")) { "Drag fired extra actions: $commands" }
            commands.clear()
            val beta = bounds("Beta")
            drag(bounds("Play Queue"), beta.right.toFloat()+25, beta.centerY().toFloat())
            check(state.value.tabs.map { it.key } == listOf("a", "b", "queue"))
            check(state.value.active == "b")
            commands.clear()
            val alpha = bounds("Alpha")
            drag(alpha, alpha.centerX().toFloat()+50, alpha.bottom.toFloat()+150)
            check(commands.isEmpty() && state.value.tabs.map { it.key } == listOf("a", "b", "queue"))
        } finally { runOnMainSync { activity.finish() } }
    }
    private fun persistenceContract() {
        val key = "contract:${java.util.UUID.randomUUID()}"
        val legacy = targetContext.getSharedPreferences("kog", android.content.Context.MODE_PRIVATE)
        legacy.edit().putString(key, "legacy").commit()
        val first = KogPreferences(targetContext)
        val second = KogPreferences(targetContext)
        check(first.getString(key, null) == "legacy")
        second.edit().putString(key, "sqlite").apply()
        check(first.getString(key, null) == "sqlite")
        check(legacy.getString(key, null) == "legacy")
        first.edit().remove(key).apply()
        check(second.getString(key, null) == null)
        legacy.edit().remove(key).commit()

        val api = KogApi(targetContext, onDevice = true)
        check(api.localState("sessions", key).getLong("revision") == 0L)
        val checkpoint = JSONObject().put("version", 1).put("session_id", key).put("queue", JSONArray())
        check(api.localState("sessions", key, checkpoint).getLong("revision") == 1L)
        check(api.localState("sessions", key, checkpoint).getLong("revision") == 1L)
        checkpoint.put("queue", JSONArray().put("stale"))
        check(runCatching { api.localState("sessions", key, checkpoint) }.isFailure)
        check(KogApi(targetContext, onDevice = true).localState("sessions", key)
            .getJSONObject("value").getJSONArray("queue").length() == 0)
    }
    private fun sessionContract(): Int {
        val fixture = JSONObject(context.assets.open("session.json").bufferedReader().use { it.readText() })
        val sessions = mutableMapOf<String, SharedBackendSession>()
        val captures = mutableMapOf<String, Any>()
        fun lookup(value: Any?, path: String): Any? = path.split('.').fold(value) { current, key -> when (current) {
            is JSONObject -> current.opt(key); is JSONArray -> current.opt(key.toInt()); else -> null
        } }
        fun resolve(value: Any?): Any? = when (value) {
            is String -> if (value.startsWith("@")) captures.getValue(value.substring(1)) else value
            is JSONArray -> JSONArray((0 until value.length()).map { resolve(value.get(it)) })
            is JSONObject -> JSONObject().apply { value.keys().forEach { key -> put(key, resolve(value.get(key))) } }
            else -> value
        }
        val steps = fixture.getJSONArray("steps")
        for (index in 0 until steps.length()) {
            val step = steps.getJSONObject(index)
            val session = sessions.getOrPut(step.getString("session")) { SharedBackendSession(step.getString("session")) }
            val reply = session.send(resolve(step.opt("command")) as? JSONObject, resolve(step.opt("restore")) as? JSONObject)
            PlaylistWorkspaceSnapshot.parse(session.snapshot.getJSONObject("workspace"))
            val expected = step.getJSONObject("expect")
            expected.keys().forEach { path -> check(equalJson(lookup(reply, path), expected.get(path))) {
                "Kotlin session step $index $path: expected ${expected.get(path)}, got ${lookup(reply,path)}"
            } }
            step.optJSONObject("capture")?.let { values -> values.keys().forEach { name -> captures[name] = lookup(reply, values.getString(name))!! } }
        }
        return steps.length()
    }
    private fun librarySessionContract() {
        val api = KogApi(targetContext, onDevice = true)
        check(api.sessionID.isNotBlank() && api.sessionID == KogApi(targetContext).sessionID)
        java.io.File(api.deviceRoot).mkdirs()
        kotlinx.coroutines.runBlocking {
            val result = api.search("kog-contract-no-such-track")
            check(result.generation > 0)
            check(api.more(result.generation, 0).tracks.isEmpty())
        }
    }
    private fun mediaPortContract() {
        lateinit var player: PolicyPlayer
        lateinit var second: PolicyPlayer
        var snapshot = JSONObject()
        val started = mutableSetOf<Int>()
        var secondSnapshot = JSONObject()
        val id = "contract:${java.util.UUID.randomUUID()}"
        fun command(op: String) = JSONObject().put("op", op)
        fun rows() = JSONArray((0..2).map { Track("device", "/not-playing-$it.wav", title = "Track $it").saved() })
        fun onMain(work: () -> Unit) {
            var failure: Throwable? = null
            runOnMainSync { try { work() } catch (error: Throwable) { failure = error } }
            failure?.let { throw it }
        }
        val wav = java.io.File(targetContext.cacheDir, "session-contract.wav")
        val samples = 8000
        val bytes = java.nio.ByteBuffer.allocate(44 + samples * 2).order(java.nio.ByteOrder.LITTLE_ENDIAN)
        bytes.put("RIFF".toByteArray()).putInt(36 + samples * 2).put("WAVEfmt ".toByteArray()).putInt(16)
            .putShort(1).putShort(1).putInt(16000).putInt(32000).putShort(2).putShort(16).put("data".toByteArray()).putInt(samples * 2)
        repeat(samples) { bytes.putShort((kotlin.math.sin(it * 2 * Math.PI * 440 / 16000) * 1000).toInt().toShort()) }
        wav.writeBytes(bytes.array())
        try {
            onMain {
                player = PolicyPlayer(targetContext, PlaybackService.createOutput(targetContext), id) { snapshot = it; if (it.optString("transport") == "playing") started.add(it.optInt("current")) }
                second = PolicyPlayer(targetContext, PlaybackService.createOutput(targetContext), "$id:second") { secondSnapshot = it }
                player.dispatch(command("append").put("tracks", rows()))
                second.dispatch(command("append").put("tracks", JSONArray().put(Track("device", "/separate.wav").saved())))
                player.moveMediaItem(2, 0)
                check(snapshot.getJSONArray("queue").getJSONObject(0).getString("title") == "Track 2")
                check(player.getMediaItemAt(0).kogTrack().title == "Track 2")
                player.removeMediaItem(1)
                check(snapshot.getJSONArray("queue").length() == 2 && player.mediaItemCount == 2)
                check(secondSnapshot.getJSONArray("queue").length() == 1 && second.mediaItemCount == 1)
                player.dispatch(command("shuffle").put("mode", "albums"))
                check(secondSnapshot.getString("shuffle") == "off")
                player.release()
                player = PolicyPlayer(targetContext, PlaybackService.createOutput(targetContext), id) { snapshot = it; if (it.optString("transport") == "playing") started.add(it.optInt("current")) }
                check(snapshot.getJSONArray("queue").length() == 2 && player.mediaItemCount == 2)
                check(snapshot.getString("transport") == "stopped" && !player.playWhenReady)
                player.clearMediaItems()
                check(snapshot.getJSONArray("queue").length() == 0 && player.mediaItemCount == 0)
                player.dispatch(command("shuffle").put("mode", "off"))
                val audio = Track("device", wav.absolutePath, title = "Audio", duration = 500).saved()
                player.dispatch(command("append").put("tracks", JSONArray().put(audio).put(audio)))
                player.dispatch(command("play").put("index", 0))
            }
            val deadline = android.os.SystemClock.elapsedRealtime() + 12000
            var completed = false
            while (!completed && android.os.SystemClock.elapsedRealtime() < deadline) {
                android.os.SystemClock.sleep(50)
                onMain { completed = started.containsAll(listOf(0, 1)) && snapshot.optString("transport") == "stopped" && snapshot.optInt("current") == 1 }
            }
            check(completed) { "Media3 EOS did not traverse the shared session: $snapshot; started=$started" }
        } finally {
            wav.delete()
            onMain { runCatching { player.release() }; runCatching { second.release() } }
            targetContext.getSharedPreferences("kog", android.content.Context.MODE_PRIVATE).edit()
                .remove("backend_session.$id").remove("backend_session.$id:second").apply()
        }
    }
    private fun equalJson(a: Any?, b: Any?): Boolean = when {
        a is JSONArray && b is JSONArray -> a.length() == b.length() && (0 until a.length()).all { equalJson(a.get(it), b.get(it)) }
        a is JSONObject && b is JSONObject -> a.length() == b.length() && b.keys().asSequence().all { a.has(it) && equalJson(a.get(it), b.get(it)) }
        a is Number && b is Number -> a.toDouble() == b.toDouble()
        else -> a == b
    }
}
