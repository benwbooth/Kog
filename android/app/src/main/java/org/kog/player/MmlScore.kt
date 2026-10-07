package org.kog.player

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONObject

/** One rendered piece of the score and the ticks it covers (see docs/KOG_MML.md). */
internal class MmlSpan(val track: Int, val start: Long, val end: Long, val from: Int, val to: Int,
    val kind: String, val sound: Long?)

internal class MmlBar(val index: Int, val start: Long, val end: Long, val from: Int, val to: Int)

internal class MmlDocument(json: JSONObject) {
    val text: String = json.getString("text")
    val tickSeconds: Double = json.getDouble("tick_seconds")
    val spans: List<MmlSpan> = json.getJSONArray("spans").let { array ->
        List(array.length()) { i -> array.getJSONObject(i).run {
            MmlSpan(getInt("track"), getLong("start"), getLong("end"), getInt("from"), getInt("to"),
                getString("kind"), if (has("sound") && !isNull("sound")) getLong("sound") else null)
        } }
    }
    val bars: List<MmlBar> = json.getJSONArray("bars").let { array ->
        List(array.length()) { i -> array.getJSONObject(i).run {
            MmlBar(getInt("index"), getLong("start"), getLong("end"), getInt("from"), getInt("to"))
        } }
    }
    val header: String = text.substring(0, bars.firstOrNull()?.from ?: text.length).trimEnd()

    /** Every piece of each sounding note, plus the playing bar. */
    fun active(seconds: Double): Pair<Set<Int>, Int> {
        val tick = (seconds.coerceAtLeast(0.0) / tickSeconds).toLong()
        val sounding = spans.filter { it.sound != null && it.start <= tick && tick < it.end }
            .map { it.track to it.sound }.toSet()
        val indices = spans.indices.filter { spans[it].sound != null && (spans[it].track to spans[it].sound) in sounding }.toSet()
        return indices to (bars.firstOrNull { it.start <= tick && tick < it.end }?.index ?: -1)
    }
}

@Composable
internal fun MmlScore(state: KogState, follow: Boolean, modifier: Modifier) {
    var document by remember { mutableStateOf<MmlDocument?>(null) }
    var message by remember { mutableStateOf("Recording every channel of this song…") }
    var active by remember { mutableStateOf(emptySet<Int>() to -1) }
    LaunchedEffect(state) {
        var identity = ""
        var revision = -1L
        var done = false
        var lastRequest = 0L
        while (isActive) {
            val track = state.current
            val source = state.inspectionStream().orEmpty()
            val key = track?.key.orEmpty() + source
            if (key != identity) {
                identity = key; revision = -1; done = false; document = null; lastRequest = 0
                message = if (track == null) "Play a song to see its MML score." else "Recording every channel of this song…"
            }
            if (track != null && !done && android.os.SystemClock.elapsedRealtime() - lastRequest > 1000) {
                lastRequest = android.os.SystemClock.elapsedRealtime()
                val reply = runCatching {
                    withContext(Dispatchers.IO) {
                        if (track.isDevice) NativeAudio.mml(track, revision)?.let(::JSONObject)
                        else state.api.mmlScore(source, revision)
                    }
                }
                reply.onSuccess { json ->
                    if (json == null || identity != key) return@onSuccess
                    json.optJSONObject("document")?.let { document = MmlDocument(it); revision = json.getLong("revision") }
                    val time = { ms: Long -> "%d:%02d".format(ms / 60_000, ms / 1000 % 60) }
                    when (json.getString("status")) {
                        "ready" -> { done = true; message = "" }
                        "error" -> { done = true; message = json.optString("detail", "The score could not be recorded.") }
                        else -> message = "Still recording… ${time(json.getLong("recorded_ms"))} of ${time(json.getLong("total_ms"))}"
                    }
                }.onFailure { message = it.message ?: "Could not read the MML score" }
            }
            document?.let { score ->
                val start = android.net.Uri.parse(source).getQueryParameter("start_ms")?.toDoubleOrNull()?.div(1000.0) ?: 0.0
                active = withContext(Dispatchers.Default) { score.active(state.inspectionPosition() / 1000.0 + start) }
            }
            delay(50)
        }
    }
    val list = rememberLazyListState()
    LaunchedEffect(active.second, follow) {
        if (follow && active.second >= 0) list.animateScrollToItem(active.second + 1)
    }
    Column(modifier) {
        if (message.isNotEmpty()) Text(message, fontSize = 11.sp, color = Color(0xffadb7c0))
        val score = document ?: return@Column
        LazyColumn(state = list, verticalArrangement = Arrangement.spacedBy(6.dp)) {
            item { Text(score.header, fontSize = 10.sp, fontFamily = FontFamily.Monospace, color = Color(0xff6f8794)) }
            itemsIndexed(score.bars, key = { _, bar -> bar.index }) { _, bar ->
                val playing = bar.index == active.second
                val text = buildAnnotatedString {
                    var cursor = bar.from
                    for ((index, span) in score.spans.withIndex()) {
                        if (span.from < bar.from || span.to > bar.to) continue
                        val style = when {
                            playing && index in active.first -> SpanStyle(background = Color(0xff50c8ef), color = Color(0xff0b1016), fontWeight = FontWeight.Bold)
                            span.kind == "rest" -> SpanStyle(color = Color(0xff6f8794))
                            span.kind == "command" -> SpanStyle(color = Color(0xff83d4bb))
                            else -> null
                        } ?: continue
                        append(score.text.substring(cursor, span.from))
                        withStyle(style) { append(score.text.substring(span.from, span.to)) }
                        cursor = span.to
                    }
                    append(score.text.substring(cursor, bar.to).trimEnd())
                }
                Text(text, Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp))
                    .background(if (playing) Color(0xff173946) else Color(0xff121f27)).padding(8.dp),
                    fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = Color(0xffd7e6ed))
            }
        }
    }
}
