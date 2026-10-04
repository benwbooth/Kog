package org.kog.player

import org.json.JSONArray
import org.json.JSONObject

/** JNI serialization only; Rust owns every playback decision and comparison. */
internal class SharedPlaybackPolicy {
    private var state: String? = null
    var snapshot = JSONObject()
        private set

    fun send(command: JSONObject): JSONObject {
        check(NativeAudio.available) { "Kog's shared backend could not load: ${NativeAudio.loadError.orEmpty()}" }
        val request = JSONObject().put("state", state ?: JSONObject.NULL).put("command", command)
        val reply = JSONObject(NativeAudio.nativePolicy(request.toString()))
        state = reply.getString("state")
        snapshot = reply
        return reply
    }

    fun sync(tracks: List<Track>, current: Int, oldToNew: List<Int?>? = null) {
        val command = JSONObject().put("op", "sync").put("current", index(current))
            .put("tracks", JSONArray(tracks.map { track ->
                JSONObject().put("id", track.key).put("album", track.album)
                    .put("disc_number", track.discNumber ?: JSONObject.NULL)
                    .put("track_number", track.trackNumber ?: JSONObject.NULL)
            }))
        if (oldToNew != null) command.put("old_to_new", JSONArray(oldToNew.map { it ?: JSONObject.NULL }))
        send(command)
    }

    val radio: JSONObject get() = snapshot.optJSONObject("radio") ?: JSONObject()
    val generation: Long get() = radio.optLong("generation")
    val waiting: Boolean get() = radio.optBoolean("waiting")
    val needsRefill: Boolean get() = radio.optBoolean("needs_refill")

    companion object {
        fun query(command: JSONObject): JSONObject = JSONObject(NativeAudio.nativePolicy(JSONObject().put("command", command).toString()))
        fun command(op: String) = JSONObject().put("op", op)
        fun index(index: Int): Any = if (index >= 0) index else JSONObject.NULL
    }
}
