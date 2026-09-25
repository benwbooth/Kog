package org.kog.player

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
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
) {
    val key: String get() = "$kind|$path|$entry|$fragment"
    val label: String get() = title.ifBlank {
        name.ifBlank { entry.ifBlank { path }.substringAfterLast('/') }
    }
    val detail: String get() = listOf(artist, album).filter(String::isNotBlank).joinToString(" · ")
    val isDevice: Boolean get() = kind == "device"

    fun locator(): JSONObject = JSONObject()
        .put("kind", kind).put("path", path).put("entry", entry).put("fragment", fragment)

    fun saved(): JSONObject = locator().put("name", name).put("title", title)
        .put("artist", artist).put("album", album).put("duration", duration)

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
        )
    }
}

data class Folder(val name: String, val path: String)
data class Listing(val path: String, val parent: String, val directories: List<Folder>, val files: List<Track>)
data class SavedPlaylist(val id: Long, val name: String, val count: Int)
data class SearchPage(val tracks: List<Track>, val folders: List<Folder>, val generation: Long,
                      val count: Int, val scanned: Int, val done: Boolean)

/** The mobile client uses the same HTTP endpoints and locator shape as Kog Web. */
class KogApi(private val context: Context) {
    private val prefs = context.getSharedPreferences("kog", Context.MODE_PRIVATE)
    var server: String
        get() = prefs.getString("server", "") ?: ""
        set(value) {
            val address = value.trim().trimEnd('/')
            prefs.edit().putString("server", if (address.isNotBlank() && "://" !in address)
                "http://$address" else address).apply()
        }
    var token: String
        get() = prefs.getString("token", "") ?: ""
        set(value) { prefs.edit().putString("token", value.trim()).apply() }
    var username: String
        get() = prefs.getString("username", "") ?: ""
        set(value) { prefs.edit().putString("username", value).apply() }
    var password: String
        get() = prefs.getString("password", "") ?: ""
        set(value) { prefs.edit().putString("password", value).apply() }
    var codec: String
        get() = prefs.getString("codec", "aac") ?: "aac"
        set(value) { prefs.edit().putString("codec", value).apply() }

    fun uri(endpoint: String, vararg params: Pair<String, String>): String {
        val origin = server.ifBlank { "http://127.0.0.1:8420" }
        val builder = Uri.parse("$origin$endpoint").buildUpon()
        for ((key, value) in params) builder.appendQueryParameter(key, value)
        return builder.build().toString()
    }

    fun stream(track: Track): String = if (track.isDevice) track.path else uri(
        "/api/stream", "kind" to track.kind, "path" to track.path,
        "entry" to track.entry, "fragment" to track.fragment, "codec" to codec,
        "token" to token,
    )

    fun art(track: Track): String? = if (track.isDevice) null else uri(
        "/api/art", "kind" to track.kind, "path" to track.path, "token" to token,
    )

    private suspend fun request(path: String, method: String = "GET", body: Any? = null): String =
        withContext(Dispatchers.IO) {
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
            response.optJSONArray("files").objects().map(Track::parse),
        )
    }

    suspend fun collect(path: String): List<Track> {
        val tracks = JSONObject(request(uri("/api/library/collect", "path" to path)))
            .optJSONArray("tracks").objects().map(Track::parse)
        return withMetadata(tracks)
    }

    suspend fun expand(track: Track): List<Track> {
        val response = JSONObject(request(uri("/api/expand"), "POST", JSONArray().put(
            track.locator().put("name", track.name))))
        val rows = response.optJSONArray("tracks")?.optJSONArray(0)
        return withMetadata(rows.objects().map(Track::parse))
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
                )
            }
        }
    }

    suspend fun search(query: String): SearchPage = searchPage("/api/library/search", "q" to query)
    suspend fun more(generation: Long, offset: Int): SearchPage = searchPage(
        "/api/library/search/more", "g" to generation.toString(), "offset" to offset.toString())

    private suspend fun searchPage(path: String, vararg params: Pair<String, String>): SearchPage {
        val response = JSONObject(request(uri(path, *params)))
        val rows = response.optJSONArray("results").objects()
        return SearchPage(
            rows.filterNot { it.optBoolean("is_dir") }.map(Track::parse),
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
            .optJSONArray("entries").objects().map(Track::parse)
        return withMetadata(tracks)
    }

    suspend fun createPlaylist(name: String) { request(uri("/api/playlists"), "POST", JSONObject().put("name", name)) }
    suspend fun renamePlaylist(id: Long, name: String) {
        request(uri("/api/playlists/$id/rename"), "POST", JSONObject().put("name", name))
    }
    suspend fun deletePlaylist(id: Long) { request(uri("/api/playlists/$id"), "DELETE") }
    suspend fun appendPlaylist(id: Long, tracks: List<Track>) {
        val entries = JSONArray()
        tracks.filterNot(Track::isDevice).forEach { entries.put(it.locator()) }
        if (entries.length() > 0) request(uri("/api/playlists/$id/entries"), "POST",
            JSONObject().put("entries", entries))
    }

    suspend fun stars(): Set<String> = JSONObject(request(uri("/api/stars")))
        .optJSONArray("entries").objects().map { Track.parse(it).key }.toSet()

    suspend fun star(track: Track, starred: Boolean) {
        request(uri("/api/stars"), "POST", track.locator().put("starred", starred))
    }

    suspend fun radio(enabled: Boolean, root: String): List<Track> {
        val response = JSONObject(request(uri("/api/radio/enabled", "root" to root), "POST",
            JSONObject().put("enabled", enabled)))
        return if (enabled) withMetadata(response.optJSONArray("entries").objects().map(Track::parse)) else emptyList()
    }

    suspend fun radioAdvance(root: String): List<Track> = withMetadata(
        JSONObject(request(uri("/api/radio/advance", "root" to root), "POST"))
            .optJSONArray("entries").objects().map(Track::parse))
}

private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else
    (0 until length()).mapNotNull(::optJSONObject)
