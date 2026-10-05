package org.kog.player

import android.content.Context
import java.io.File
import org.json.JSONObject

/** Each preference is an independent SQLite row. Editors write only changed keys,
 * in one transaction, so another client cannot lose unrelated settings. */
internal class KogPreferences(context: Context) {
    private val database = File(context.filesDir, "kog-library/library.sqlite").absolutePath
    private val legacy = context.getSharedPreferences("kog", Context.MODE_PRIVATE)

    private fun request(input: JSONObject): JSONObject {
        check(NativeAudio.available) { "Kog preferences are unavailable: ${NativeAudio.loadError}" }
        val reply = JSONObject(NativeAudio.nativePreferences(input.put("database", database).toString()))
        if (reply.has("error")) error(reply.getString("error"))
        return reply
    }

    private fun value(key: String): Any? {
        var reply = request(JSONObject().put("op", "get").put("key", key))
        if (!reply.getBoolean("found")) {
            val old = legacy.all[key]
            reply = request(JSONObject().put("op", "import").put("key", key)
                .put("value", old ?: JSONObject.NULL))
        }
        return reply.opt("value").takeUnless { it == JSONObject.NULL }
    }

    fun getString(key: String, default: String?): String? = value(key) as? String ?: default
    fun getBoolean(key: String, default: Boolean): Boolean = value(key) as? Boolean ?: default
    fun getInt(key: String, default: Int): Int = (value(key) as? Number)?.toInt() ?: default
    fun getOrCreateString(key: String, default: String): String {
        getString(key, null)?.let { return it }
        return request(JSONObject().put("op", "default").put("key", key).put("value", default)).getString("value")
    }
    fun edit(): Editor = Editor()

    inner class Editor {
        private val values = JSONObject()
        fun putString(key: String, value: String?): Editor = apply { values.put(key, value ?: JSONObject.NULL) }
        fun putBoolean(key: String, value: Boolean): Editor = apply { values.put(key, value) }
        fun putInt(key: String, value: Int): Editor = apply { values.put(key, value) }
        fun remove(key: String): Editor = apply { values.put(key, JSONObject.NULL) }
        fun apply() { request(JSONObject().put("op", "write").put("values", values)) }
    }
}
