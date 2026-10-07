package org.kog.player

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.io.File
import java.util.Base64

data class Track(
    val kind: String,
    val path: String,
    val entry: String = "",
    val fragment: String = "",
    val name: String = "",
    val title: String = "",
    val artist: String = "",
    val album: String = "",
    val duration: Long = 0,
    val discNumber: Int? = null,
    val trackNumber: Int? = null,
    val metadata: Map<String, String> = emptyMap(),
) {
    val key: String get() = "$kind|$path|$entry|$fragment"
    val label: String get() = title.ifBlank {
        name.ifBlank { entry.ifBlank { path }.substringAfterLast('/') }
    }
    val detail: String get() = listOf(artist, album).filter(String::isNotBlank).joinToString(" · ")
    val isDevice: Boolean get() = kind == "device"

    fun locator(): JSONObject = JSONObject()
        .put("kind", if (isDevice) (if (entry.isEmpty()) "local" else "archive") else kind).put("path", path).put("entry", entry).put("fragment", fragment)

    fun saved(): JSONObject = locator().put("kind", kind).put("name", name).put("title", title)
        .put("artist", artist).put("album", album).put("duration", duration)
        .put("discNumber", discNumber ?: JSONObject.NULL).put("trackNumber", trackNumber ?: JSONObject.NULL).put("metadata", JSONObject(metadata))

    companion object {
        fun parse(row: JSONObject): Track = Track(
            kind = row.optString("kind", "local"),
            path = row.optString("path"),
            entry = row.optString("entry", ""),
            fragment = if (row.isNull("fragment")) "" else row.optString("fragment", ""),
            name = row.optString("name", ""),
            title = row.optString("title", ""),
            artist = row.optString("artist", ""),
            album = row.optString("album", ""),
            duration = row.optLong("duration", 0),
            discNumber = if (row.isNull("discNumber")) null else row.optInt("discNumber"),
            trackNumber = if (row.isNull("trackNumber")) null else row.optInt("trackNumber"),
            metadata = row.optJSONObject("metadata")?.let { data -> data.keys().asSequence().filterNot { data.isNull(it) }.associateWith { data.get(it).toString() } } ?: emptyMap(),
        )
    }
}

data class Folder(val name: String, val path: String)
data class Listing(val path: String, val parent: String, val directories: List<Folder>, val files: List<Track>)
data class SavedPlaylist(val id: Long, val name: String, val count: Int)
data class SearchPage(val tracks: List<Track>, val folders: List<Folder>, val generation: Long,
                      val count: Int, val scanned: Int, val done: Boolean)

/** The mobile client uses the same HTTP endpoints and locator shape as Kog Web. */
class KogApi(private val context: Context, val onDevice: Boolean = false) {
    var sessionID: String = SharedBackendSession.defaultID(context)
    var radioRequest: JSONObject? = null
    private var captured: Map<String, String>? = null
    /** Freeze connection credentials for a complete asynchronous operation. */
    fun snapshot(): KogApi = KogApi(context, onDevice).also {
        it.sessionID = sessionID
        it.captured = mapOf("server" to server, "token" to token, "username" to username,
            "password" to password, "codec" to codec, "midi_engine" to midiEngine)
    }
    val deviceRoot get() = File(context.filesDir, "Kog Imports").absolutePath
    /** Small, synchronous on-device state operations use the shared SQLite API. */
    fun localState(namespace: String, id: String, value: JSONObject? = null, revision: Long = 0): JSONObject {
        val request = JSONObject().put("root", deviceRoot)
            .put("storage", File(context.filesDir, "kog-library").absolutePath)
            .put("method", if (value == null) "GET" else "PUT")
            .put("uri", "/api/state/${Uri.encode(namespace)}/${Uri.encode(id)}")
        if (value != null) request.put("body", JSONObject().put("expected_revision", revision).put("value", value))
        val response = JSONObject(NativeAudio.nativeLibrary(request.toString()))
        val result = response.getJSONObject("body")
        check(response.getInt("status") in 200..299) { result.optString("error", "Cannot save the session") }
        return result
    }
    private fun parseTrack(row: JSONObject): Track = Track.parse(row).let { if (onDevice && it.kind != "remote") it.copy(kind = "device") else it }

    private val prefs = KogPreferences(context)
    var server: String
        get() = captured?.get("server") ?: (prefs.getString("server", "") ?: "")
        set(value) {
            val address = value.trim().trimEnd('/')
            prefs.edit().putString("server", if (address.isNotBlank() && "://" !in address)
                "http://$address" else address).apply()
        }
    var token: String
        get() = captured?.get("token") ?: (prefs.getString("token", "") ?: "")
        set(value) { prefs.edit().putString("token", value.trim()).apply() }
    var username: String
        get() = captured?.get("username") ?: (prefs.getString("username", "") ?: "")
        set(value) { prefs.edit().putString("username", value).apply() }
    var password: String
        get() = captured?.get("password") ?: (prefs.getString("password", "") ?: "")
        set(value) { prefs.edit().putString("password", value).apply() }
    var codec: String
        get() = captured?.get("codec") ?: (prefs.getString("codec", "aac") ?: "aac")
        set(value) { prefs.edit().putString("codec", value).apply() }
    var midiEngine: String
        get() = captured?.get("midi_engine") ?: (prefs.getString("midi_engine", "opl3windows") ?: "opl3windows")
        set(value) { prefs.edit().putString("midi_engine", value).apply() }

    fun uri(endpoint: String, vararg params: Pair<String, String>): String {
        val origin = server.ifBlank { "http://127.0.0.1:8420" }
        val builder = Uri.parse("$origin$endpoint").buildUpon()
        for ((key, value) in params) builder.appendQueryParameter(key, value)
        return builder.build().toString()
    }

    fun stream(track: Track): String {
        if (track.isDevice) return track.path
        val path = if (track.kind == "archive") track.entry else track.path
        val midi = path.substringAfterLast('.', "").lowercase() in setOf(
            "kar", "mid", "midi", "rmi", "mids", "mds", "lds", "xmf", "mxmf")
        val options = mutableListOf("kind" to track.kind, "path" to track.path,
            "entry" to track.entry, "fragment" to track.fragment, "codec" to codec,
            "token" to token)
        if (midi) options.add("midi_engine" to midiEngine)
        return uri("/api/stream", *options.toTypedArray())
    }

    internal suspend fun channelWindow(stream: String, position: Double): JSONObject {
        val original = Uri.parse(stream)
        require(original.path?.endsWith("/api/stream") == true) { "Channel data is unavailable for this source" }
        val endpoint = original.buildUpon().path(original.path!!.removeSuffix("stream") + "inspection")
            .appendQueryParameter("position", position.toString()).build().toString()
        return JSONObject(request(endpoint))
    }

    suspend fun setMidiEngine(engine: String) {
        request(uri("/api/settings/midi"), "POST", JSONObject().put("engine", engine))
    }

    suspend fun serverMidiEngine(): String =
        JSONObject(request(uri("/api/settings/midi"))).getString("engine")

    fun art(track: Track): String? = if (track.isDevice) null else uri(
        "/api/art", "kind" to track.kind, "path" to track.path, "token" to token,
    )

    private suspend fun request(path: String, method: String = "GET", body: Any? = null): String =
        withContext(Dispatchers.IO) {
            if (onDevice) {
                val uri = Uri.parse(path)
                val apiPath = uri.encodedPath.orEmpty() + (uri.encodedQuery?.let { "?$it" } ?: "")
                val request = JSONObject().put("root", deviceRoot)
                    .put("storage", File(context.filesDir, "kog-library").absolutePath)
                    .put("method", method).put("uri", apiPath).put("body", body ?: JSONObject.NULL)
                val response = JSONObject(NativeAudio.nativeLibrary(request.toString()))
                val result = response.get("body")
                check(response.getInt("status") in 200..299) { (result as? JSONObject)?.optString("error") ?: "Device library request failed" }
                return@withContext result.toString()
            }
            val connection = URL(path).openConnection() as HttpURLConnection
            try {
                connection.requestMethod = method
                connection.connectTimeout = 10_000
                connection.readTimeout = 45_000
                val authorization = when {
                    token.isNotBlank() -> "Bearer $token"
                    username.isNotBlank() -> "Basic " + Base64.getEncoder().encodeToString(
                        "$username:$password".toByteArray(Charsets.UTF_8))
                    else -> ""
                }
                if (authorization.isNotEmpty()) connection.setRequestProperty("Authorization", authorization)
                if (body != null) {
                    connection.doOutput = true
                    connection.setRequestProperty("Content-Type", "application/json")
                    connection.outputStream.use { it.write(body.toString().toByteArray(Charsets.UTF_8)) }
                }
                val response = if (connection.responseCode in 200..299) connection.inputStream else connection.errorStream
                val text = response?.bufferedReader()?.use { it.readText() }.orEmpty()
                if (connection.responseCode !in 200..299) {
                    val message = runCatching { JSONObject(text).optString("error") }.getOrNull()
                    error(message?.ifBlank { null } ?: "HTTP ${connection.responseCode}")
                }
                text
            } finally {
                connection.disconnect()
            }
        }

    suspend fun health(): Boolean = runCatching { request(uri("/api/health")); true }.getOrDefault(false)

    suspend fun browse(path: String = ""): Listing {
        val response = JSONObject(request(uri("/api/library", "path" to path)))
        return Listing(
            response.optString("path"), response.optString("parent", ""),
            response.optJSONArray("directories").objects().map {
                Folder(it.optString("name"), it.optString("path"))
            },
            response.optJSONArray("files").objects().map(::parseTrack),
        )
    }

    suspend fun collect(path: String, query: String = "", root: String = ""): List<Track> {
        val tracks = JSONObject(request(uri("/api/library/collect", "path" to path, "q" to query, "root" to root)))
            .optJSONArray("tracks").objects().map(::parseTrack)
        return withMetadata(tracks)
    }

    suspend fun expand(track: Track): List<Track> = expand(listOf(track))

    suspend fun expand(tracks: List<Track>): List<Track> {
        val response = JSONObject(request(uri("/api/expand"), "POST", JSONArray(tracks.map { it.locator().put("name", it.name) })))
        val rows = response.optJSONArray("tracks") ?: JSONArray()
        return withMetadata((0 until rows.length()).flatMap { rows.optJSONArray(it).objects().map(::parseTrack) })
    }

    suspend fun withMetadata(tracks: List<Track>): List<Track> {
        if (tracks.isEmpty()) return tracks
        return tracks.chunked(100).flatMap { chunk ->
            val body = JSONArray()
            chunk.forEach { body.put(it.locator()) }
            val rows = JSONArray(request(uri("/api/metadata"), "POST", body))
            chunk.mapIndexed { index, track ->
                val row = rows.optJSONObject(index)
                track.copy(
                    title = row?.optString("title", "")?.takeUnless { it == "null" }.orEmpty(),
                    artist = row?.optString("artist", "")?.takeUnless { it == "null" }.orEmpty(),
                    album = row?.optString("album", "")?.takeUnless { it == "null" }.orEmpty(),
                    duration = ((row?.optDouble("duration", 0.0) ?: 0.0) * 1000).toLong(),
                    discNumber = if (row == null || row.isNull("discNumber")) null else row.optInt("discNumber"),
                    trackNumber = if (row == null || row.isNull("trackNumber")) null else row.optInt("trackNumber"),
                    metadata = row?.let { data -> data.keys().asSequence().filterNot { data.isNull(it) }.associateWith { data.get(it).toString() } } ?: track.metadata,
                )
            }
        }
    }

    suspend fun search(query: String): SearchPage = searchPage("/api/library/search", "q" to query, "session" to sessionID)
    suspend fun more(generation: Long, offset: Int): SearchPage = searchPage(
        "/api/library/search/more", "g" to generation.toString(), "offset" to offset.toString(), "session" to sessionID)

    private suspend fun searchPage(path: String, vararg params: Pair<String, String>): SearchPage {
        val response = JSONObject(request(uri(path, *params)))
        val rows = response.optJSONArray("results").objects()
        return SearchPage(
            rows.filterNot { it.optBoolean("is_dir") }.map(::parseTrack),
            rows.filter { it.optBoolean("is_dir") }.map {
                Folder(it.optString("name"), it.optString("path"))
            }, response.optLong("generation"), response.optInt("total"),
            response.optInt("scanned"), response.optBoolean("done"),
        )
    }

    suspend fun playlists(): List<SavedPlaylist> = JSONObject(request(uri("/api/playlists")))
        .optJSONArray("playlists").objects().map {
            SavedPlaylist(it.optLong("id"), it.optString("name"), it.optInt("entryCount"))
        }

    suspend fun playlist(id: Long): List<Track> {
        val tracks = JSONObject(request(uri("/api/playlists/$id")))
            .optJSONArray("entries").objects().map(::parseTrack)
        return withMetadata(tracks)
    }

    suspend fun createPlaylist(name: String) { request(uri("/api/playlists"), "POST", JSONObject().put("name", name)) }
    suspend fun renamePlaylist(id: Long, name: String) {
        request(uri("/api/playlists/$id/rename"), "POST", JSONObject().put("name", name))
    }
    suspend fun deletePlaylist(id: Long) { request(uri("/api/playlists/$id"), "DELETE") }
    suspend fun appendPlaylist(id: Long, tracks: List<Track>) {
        val entries = JSONArray()
        require(tracks.all { it.kind == "remote" || it.isDevice == onDevice }) { "Choose a playlist in the same library as these tracks." }
        tracks.forEach { entries.put(it.locator()) }
        if (entries.length() > 0) request(uri("/api/playlists/$id/entries"), "POST",
            JSONObject().put("entries", entries))
    }

    suspend fun replacePlaylist(id: Long, tracks: List<Track>, expected: List<Track>) {
        require(tracks.all { it.kind == "remote" || it.isDevice == onDevice }) { "Choose a playlist in the same library as these tracks." }
        request(uri("/api/playlists/$id/entries"), "PUT", JSONObject()
            .put("entries", JSONArray(tracks.map { it.locator() }))
            .put("expected_entries", JSONArray(expected.map { it.locator() })))
    }

    suspend fun stars(): Set<String> = JSONObject(request(uri("/api/stars")))
        .optJSONArray("entries").objects().map { parseTrack(it).key }.toSet()

    suspend fun star(track: Track, starred: Boolean) {
        request(uri("/api/stars"), "POST", track.locator().put("starred", starred))
    }

    private fun radioUri(endpoint: String, root: String): String {
        val values = mutableListOf("incremental" to "true", "session" to sessionID)
        if (root.isNotEmpty()) values += "root" to root
        radioRequest?.let { values += "incarnation" to it.getLong("incarnation").toString(); values += "serial" to it.getLong("serial").toString() }
        return uri(endpoint, *values.toTypedArray())
    }
    suspend fun radio(enabled: Boolean, root: String): RadioBatch = radioBatch(
        JSONObject(request(radioUri("/api/radio/enabled", root), "POST",
            JSONObject().put("enabled", enabled))))

    suspend fun radioAdvance(root: String): RadioBatch = radioBatch(
        JSONObject(request(radioUri("/api/radio/advance", root), "POST")))

    suspend fun reshuffleRadio(root: String): RadioBatch = radioBatch(
        JSONObject(request(radioUri("/api/radio/reshuffle", root), "POST")))

    private suspend fun radioBatch(reply: JSONObject): RadioBatch {
        val tracks = withMetadata(reply.optJSONArray("entries").objects().map(::parseTrack))
        return RadioBatch(tracks, reply.optBoolean("exhausted", tracks.isEmpty()))
    }
}

private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else
    (0 until length()).mapNotNull(::optJSONObject)

data class RadioBatch(val tracks: List<Track>, val exhausted: Boolean)
