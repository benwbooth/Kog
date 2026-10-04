package org.kog.player

import android.app.Activity
import android.app.Instrumentation
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
            status.putString("stream", "Kotlin UI contract: ${steps.length()} steps passed through production JNI and workspace decoder\n")
            sendStatus(0, status)
            finish(Activity.RESULT_OK, Bundle().apply { putString("stream", status.getString("stream")) })
        } catch (error: Throwable) {
            status.putString("stack", error.stackTraceToString()); status.putString("stream", error.stackTraceToString())
            sendStatus(-2, status); finish(Activity.RESULT_CANCELED, status)
        }
    }
    private fun equalJson(a: Any?, b: Any?): Boolean = when {
        a is JSONArray && b is JSONArray -> a.length() == b.length() && (0 until a.length()).all { equalJson(a.get(it), b.get(it)) }
        a is JSONObject && b is JSONObject -> a.length() == b.length() && b.keys().asSequence().all { a.has(it) && equalJson(a.get(it), b.get(it)) }
        a is Number && b is Number -> a.toDouble() == b.toDouble()
        else -> a == b
    }
}
