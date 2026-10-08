package org.kog.player

import android.net.Uri
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import kotlin.math.roundToInt

@Composable
internal fun ChannelInspector(state: KogState, dismiss: () -> Unit) {
    var snapshot by remember { mutableStateOf(InspectionSnapshot()) }
    var mode by remember { mutableIntStateOf(2) }
    var follow by remember { mutableStateOf(true) }
    LaunchedEffect(state) {
        var identity = ""
        val windows = mutableListOf<InspectionWindow>()
        var pending = false
        var lastRequest = 0L
        while (isActive) {
            val track = state.current.takeIf { state.inspectionActive }
            val source = state.inspectionStream().orEmpty()
            val key = (track?.key.orEmpty() + source)
            if (key != identity) { identity = key; windows.clear(); snapshot = InspectionSnapshot(); lastRequest = 0; pending = false }
            if (track != null) {
                val position = state.inspectionPosition()
                if (track.isDevice) {
                    val current = runCatching { withContext(Dispatchers.IO) { InspectionSnapshot.parse(JSONObject(NativeAudio.snapshot(track,position,state.playing))) } }
                    if (state.current?.key == track.key) snapshot = current.getOrElse { InspectionSnapshot(detail = it.message ?: "Could not read channel data") }
                } else {
                    val start = Uri.parse(source).getQueryParameter("start_ms")?.toDoubleOrNull()?.div(1000.0) ?: 0.0
                    val seconds = position / 1000.0 + start
                    val window = windows.firstOrNull { seconds >= it.start && seconds < it.end }
                    snapshot = window?.snapshot(seconds) ?: InspectionSnapshot(detail = "Waiting for channel data from the streaming decoder…")
                    val request = if (window == null) seconds else if (seconds > window.end - 0.3 && windows.none { it.start >= window.end }) window.end + 0.001 else null
                    if (request != null && !pending && android.os.SystemClock.elapsedRealtime() - lastRequest > 400) {
                        pending = true; lastRequest = android.os.SystemClock.elapsedRealtime()
                        launch {
                            try {
                                val reply = state.api.channelWindow(source,request)
                                if (identity == key) reply.optJSONObject("window")?.let {
                                    val next = InspectionWindow(it)
                                    windows.removeAll { old -> old.start == next.start }; windows.add(next)
                                    while (windows.size > 3) windows.removeAt(0)
                                }
                            } catch (failure: Exception) {
                                if (identity == key && windows.isEmpty()) snapshot = InspectionSnapshot(detail = failure.message ?: "Could not read channel data")
                            } finally { if (identity == key) pending = false }
                        }
                    }
                }
            } else { snapshot = InspectionSnapshot() }
            delay(33)
        }
    }
    Dialog(onDismissRequest = dismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize().padding(8.dp), color = Color(0xff202427)) {
            Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Channel Inspector", Modifier.weight(1f), fontSize = 20.sp)
                    TextButton(onClick = { state.toggle() }) { Text(if (state.playing) "Pause" else "Play") }
                    TextButton(onClick = dismiss) { Text("Close") }
                }
                Row { listOf("Keyboards", "Tracker", "Both", "MML").forEachIndexed { index,label ->
                    TextButton(onClick = { mode = index }, enabled = mode != index) { Text(label) }
                } }
                Text(snapshot.backend, color = Color(0xff72d0fc), fontSize = 12.sp)
                Text(snapshot.detail, fontSize = 11.sp, color = Color(0xffadb7c0))
                if (snapshot.global.isNotEmpty()) Text(snapshot.global.joinToString(" · ") { it.label() },fontSize = 11.sp)
                if (mode == 3) MmlScore(state, follow, Modifier.weight(1f))
                if (mode == 0 || mode == 2) {
                    LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        items(snapshot.channels, key = { it.id }) { channel ->
                            var details by remember(channel.id) { mutableStateOf(false) }
                            Column {
                                Text(channel.name, fontSize = 14.sp)
                                Text(channel.instrument, fontSize = 11.sp, color = Color(0xffadb7c0))
                                Text(if (channel.notes.isNotEmpty()) channel.notes.joinToString(" ") { noteName(it.key) + if (it.held) "" else "~" }
                                    else if (channel.active) channel.kind.uppercase() else "—", fontFamily = FontFamily.Monospace, fontSize = 11.sp)
                                ChannelLevelMeter(channel)
                                ChannelPiano(channel)
                                TextButton(onClick = { details = !details }) { Text(if (details) "Hide controls" else "Controls and effects", fontSize = 11.sp) }
                                if (details) Text(channel.fields.joinToString(" · ") { it.label() }, fontSize = 11.sp)
                            }
                        }
                    }
                }
                if (mode == 1 || mode == 2) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = follow, onCheckedChange = { follow = it }); Text("Follow playback", fontSize = 12.sp)
                    }
                    InspectorTracker(snapshot,follow,Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun ChannelLevelMeter(channel: InspectionChannel) {
    val level = if (channel.level.isFinite()) channel.level.coerceIn(0f,1f) else 0f
    Row(
        Modifier.fillMaxWidth().padding(vertical = 4.dp).semantics(mergeDescendants = true) {
            contentDescription = "${channel.name} level"
            progressBarRangeInfo = ProgressBarRangeInfo(level, 0f..1f)
        },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text("Level", fontSize = 11.sp, color = Color(0xffadb7c0))
        Box(Modifier.weight(1f).height(8.dp).clip(RoundedCornerShape(3.dp)).background(Color(0xff33434d))) {
            Box(Modifier.fillMaxWidth(level).fillMaxHeight().background(Color(0xff72d0fc)))
        }
        Text("${(level * 100).roundToInt()}%", fontSize = 11.sp, fontFamily = FontFamily.Monospace,
            color = Color(0xffadb7c0), modifier = Modifier.width(36.dp), textAlign = androidx.compose.ui.text.style.TextAlign.End)
    }
}

@Composable
private fun ChannelPiano(channel: InspectionChannel) {
    Row(Modifier.horizontalScroll(rememberScrollState())) {
        Canvas(Modifier.widthIn(min = 600.dp).height(58.dp)) {
            val whiteWidth = size.width / 75
            for (pass in listOf(false,true)) {
                var white = 0
                for (key in 0..127) {
                    val black = key % 12 in listOf(1,3,6,8,10)
                    if (black == pass) {
                        val note = channel.notes.firstOrNull { it.key.roundToInt() == key }
                        val x = white * whiteWidth - if (black) whiteWidth * 0.32f else 0f
                        val width = if (black) whiteWidth * 0.64f else whiteWidth - 1f
                        val height = if (black) size.height * 0.62f else size.height
                        val color = if (note != null) { if (note.held) Color(0xff4fc3f7) else Color(0xff70d9aa) }
                            else if (black) Color(0xff171b22) else Color(0xffe7eaf0)
                        drawRect(color,Offset(x,0f),Size(width,height),alpha = note?.let { 0.5f + 0.5f * it.velocity.coerceIn(0f,1f) } ?: 1f)
                        note?.let {
                            val bend = it.key - it.key.roundToInt()
                            if (!channel.fields.any { f -> f.name == "Pitch basis" && f.value.startsWith("Relative") } && kotlin.math.abs(bend) > 0.02f) drawLine(Color(0xffea6c24),Offset(x+width/2+bend*width,2f),Offset(x+width/2+bend*width,height-2f),2f)
                        }
                    }
                    if (!black) white++
                }
            }
        }
    }
}

@Composable
private fun InspectorTracker(snapshot: InspectionSnapshot, follow: Boolean, modifier: Modifier) {
    val scroll = rememberLazyListState()
    LaunchedEffect(snapshot.current,snapshot.rows.firstOrNull()?.time,follow) {
        if (follow && snapshot.current != null) scroll.scrollToItem((snapshot.current - 4).coerceAtLeast(0))
    }
    val context = androidx.compose.ui.platform.LocalContext.current
    // The Spleen 6x12 pixel font, as in the desktop and web trackers.
    val pixel = remember { FontFamily(androidx.compose.ui.text.font.Font("spleen-6x12.otf", context.assets)) }
    val char = 7.dp
    val widths = remember(snapshot.backend, snapshot.channels.size) { mutableStateMapOf<Int, IntArray>() }
    snapshot.rows.forEach { row ->
        snapshot.channels.forEach { channel ->
            val parts = trackerParts(row.cells.filter { it.channel == channel.id })
            val current = widths[channel.id] ?: TRACKER_LIMITS.map { it.first }.toIntArray()
            val next = IntArray(4) { maxOf(current[it], minOf(TRACKER_LIMITS[it].second, parts[it].length)) }
            if (!next.contentEquals(widths[channel.id])) widths[channel.id] = next
        }
    }
    fun chars(channel: Int) = widths[channel] ?: TRACKER_LIMITS.map { it.first }.toIntArray()
    fun columnWidth(channel: Int) = char * (chars(channel).sum() + 3) + 10.dp
    val text = androidx.compose.ui.text.TextStyle(fontFamily = pixel, fontSize = 12.sp, lineHeight = 15.sp)
    Box(modifier.background(Color(0xff070b0e)).horizontalScroll(rememberScrollState())) {
        Column(Modifier.width(char * 13 + snapshot.channels.fold(0.dp) { sum, c -> sum + columnWidth(c.id) } + 200.dp)) {
            Row(Modifier.background(Color(0xff101a21)).padding(vertical = 3.dp)) {
                Text("ROW", Modifier.width(char * 13).padding(start = 5.dp), style = text, color = Color(0xff6c8494))
                snapshot.channels.forEachIndexed { index, channel ->
                    Text("%02d %s".format(index + 1, channel.name), Modifier.width(columnWidth(channel.id)).padding(start = 5.dp),
                        style = text, color = Color(0xffa9c4d2), maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Clip)
                }
                Text("SONG", style = text, color = Color(0xff6c8494))
            }
            LazyColumn(state = scroll) {
                itemsIndexed(snapshot.rows) { index, row ->
                    val current = snapshot.current == index
                    Row(Modifier.background(if (current) Color(0xff1d4a5e) else if (index % 4 == 0) Color(0xff0f171d) else Color.Transparent)) {
                        Text(row.label, Modifier.width(char * 13).padding(start = 5.dp), style = text,
                            color = if (current) Color.White else Color(0xffd8b85a), maxLines = 1)
                        snapshot.channels.forEach { channel ->
                            val parts = trackerParts(row.cells.filter { it.channel == channel.id })
                            val chars = chars(channel.id)
                            Row(Modifier.width(columnWidth(channel.id)).height(IntrinsicSize.Min)) {
                                Box(Modifier.width(1.dp).fillMaxHeight().background(Color(0xff2a3b46)))
                                Spacer(Modifier.width(5.dp))
                                parts.forEachIndexed { column, part ->
                                    val blank = part.isEmpty()
                                    Text(if (blank) (if (column == 0) "---" else "..") else part,
                                        Modifier.width(char * chars[column]), style = text, maxLines = 1,
                                        overflow = androidx.compose.ui.text.style.TextOverflow.Clip,
                                        color = if (blank) Color(0xff33454f) else TRACKER_COLOURS[column])
                                    Spacer(Modifier.width(char))
                                }
                            }
                        }
                        Text(row.global.joinToString(" ") { it.label() }, style = text, color = Color(0xff83d4bb), maxLines = 1)
                    }
                }
            }
        }
    }
}

/** Fewest and most characters of each tracker column: note, instrument, volume, effects. */
private val TRACKER_LIMITS = listOf(3 to 7, 2 to 8, 2 to 4, 3 to 18)
private val TRACKER_COLOURS = listOf(Color(0xffeef4f7), Color(0xff58c8f0), Color(0xff8fdc7c), Color(0xffe39cf2))

/** A channel's tracker cells as classic columns; effects named FX… are the tracker's own and show their value alone. */
private fun trackerParts(cells: List<InspectionCell>): List<String> = listOf(
    cells.map { it.notes }.filter { it.isNotEmpty() }.joinToString(" "),
    cells.map { it.instrument }.firstOrNull { it.isNotEmpty() }.orEmpty(),
    cells.map { it.volume }.firstOrNull { it.isNotEmpty() }.orEmpty(),
    cells.flatMap { it.effects }.joinToString(" ") { if (Regex("FX\\d*").matches(it.name)) it.value else "${it.name} ${it.value}" },
)
