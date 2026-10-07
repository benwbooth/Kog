package org.kog.player

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
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
    /** Colour runs as from, to, class triples in text order. */
    val styles: IntArray = json.getJSONArray("styles").let { array ->
        IntArray(array.length() * 3) { i -> array.getJSONArray(i / 3).getInt(i % 3) }
    }
    val palette: List<Color> = json.getJSONArray("palette").let { array ->
        List(array.length()) { i -> Color(android.graphics.Color.parseColor(array.getString(i))) }
    }
    val headerEnd: Int = bars.firstOrNull()?.from ?: text.length

    /** Index of the first style run ending after [from], by binary search. */
    private fun firstStyle(from: Int): Int {
        var low = 0; var high = styles.size / 3
        while (low < high) { val mid = (low + high) / 2; if (styles[mid * 3 + 1] <= from) low = mid + 1 else high = mid }
        return low
    }

    /** The text in from..to with token colours; [lit] ranges get the playing highlight. */
    fun styled(from: Int, to: Int, lit: List<IntRange> = emptyList()) = buildAnnotatedString {
        var cursor = from
        var run = firstStyle(from)
        while (run < styles.size / 3 && styles[run * 3] < to) {
            val start = maxOf(styles[run * 3], from); val end = minOf(styles[run * 3 + 1], to)
            val classIndex = styles[run * 3 + 2]
            append(text.substring(cursor, start))
            val playing = lit.any { start >= it.first && end <= it.last }
            val style = if (playing) SpanStyle(background = Color(0xff50c8ef), color = Color(0xff0b1016), fontWeight = FontWeight.Bold)
                else SpanStyle(color = palette.getOrElse(classIndex) { Color(0xffdce3e8) },
                    fontWeight = if (classIndex == 2 || classIndex == 4) FontWeight.Bold else null)
            withStyle(style) { append(text.substring(start, end)) }
            cursor = end
            run++
        }
        append(text.substring(cursor, to).trimEnd())
    }

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
    val currentBar = active.second
    LaunchedEffect(currentBar, follow) {
        if (follow && currentBar >= 0) list.animateScrollToItem(currentBar + 1)
    }
    Column(modifier) {
        if (message.isNotEmpty()) Text(message, fontSize = 11.sp, color = Color(0xffadb7c0))
        val score = document ?: return@Column
        LazyColumn(state = list, modifier = Modifier.background(Color(0xff0f171c))) {
            item { Text(remember(score) { score.styled(0, score.headerEnd) }, Modifier.padding(8.dp),
                fontSize = 10.sp, fontFamily = FontFamily.Monospace) }
            itemsIndexed(score.bars, key = { _, bar -> bar.index }) { _, bar ->
                val playing = bar.index == currentBar
                // Only the playing bar reads the sounding spans, so the other
                // bars keep their cached text while playback advances.
                val text = if (playing) {
                    val lit = active.first.mapNotNull { score.spans.getOrNull(it) }
                        .filter { it.from >= bar.from && it.to <= bar.to }.map { it.from..it.to }
                    score.styled(bar.from, bar.to, lit)
                } else remember(score, bar.index) { score.styled(bar.from, bar.to) }
                Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min).background(if (playing) Color(0xff13303b) else if (bar.index % 2 == 0) Color(0xff0f171c) else Color(0xff111b21))) {
                    Box(Modifier.width(3.dp).fillMaxHeight().background(if (playing) Color(0xff50c8ef) else Color.Transparent))
                    Text(text, Modifier.weight(1f).padding(horizontal = 8.dp, vertical = 6.dp),
                        fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = Color(0xffdce3e8))
                }
            }
        }
    }
}
