package org.kog.player

import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.unit.toSize
import kotlinx.coroutines.delay
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

data class PlaylistWorkspaceTab(
    val key: String, val name: String, val dirty: Boolean = false, val readonly: Boolean = false,
    val loading: Boolean = false, val saving: Boolean = false, val scope: String = "",
)
data class PlaylistWorkspaceSnapshot(
    val active: String = "queue",
    val tabs: List<PlaylistWorkspaceTab> = listOf(PlaylistWorkspaceTab("queue", "Play Queue")),
    val entries: List<Track> = emptyList(), val selected: List<Int> = emptyList(),
    val canUndo: Boolean = false, val canRedo: Boolean = false,
    val pendingClose: String? = null, val error: String? = null,
    val actions: Map<String, Boolean> = emptyMap(),
) {
    val activeTab get() = tabs.firstOrNull { it.key == active }
    companion object {
        fun parse(row: JSONObject): PlaylistWorkspaceSnapshot {
            val tabs = row.getJSONArray("tabs")
            val entries = row.getJSONArray("entries")
            val selected = row.getJSONArray("selected")
            return PlaylistWorkspaceSnapshot(row.getString("active"),
                (0 until tabs.length()).map { tabs.getJSONObject(it).let { tab -> PlaylistWorkspaceTab(
                    tab.getString("key"), tab.getString("name"), tab.optBoolean("dirty"), tab.optBoolean("readonly"),
                    tab.optBoolean("loading"), tab.optBoolean("saving"), tab.optString("scope")) } },
                (0 until entries.length()).map { Track.parse(entries.getJSONObject(it)) },
                (0 until selected.length()).map(selected::getInt), row.optBoolean("can_undo"), row.optBoolean("can_redo"),
                if (row.isNull("pending_close")) null else row.optString("pending_close"),
                if (row.isNull("error")) null else row.optString("error"),
                row.optJSONObject("actions")?.let { actions -> actions.keys().asSequence().associateWith { actions.optBoolean(it) } } ?: emptyMap())
        }
    }
}

@Composable
internal fun EditMenuItems(state: KogState, dismiss: () -> Unit) {
    val actions = state.workspace.actions
    val queue = state.workspace.active == "queue"
    fun send(op: String, fields: JSONObject = JSONObject()) { state.workspaceCommand(op, fields); dismiss() }
    DropdownMenuItem(text = { Text("Undo") }, enabled = actions["undo"] == true, onClick = { send("undo") })
    DropdownMenuItem(text = { Text("Redo") }, enabled = actions["redo"] == true, onClick = { send("redo") })
    HorizontalDivider()
    DropdownMenuItem(text = { Text("Select All") }, enabled = actions["select_all"] == true, onClick = {
        send("selection", JSONObject().put("command", JSONObject().put("op", "all")))
    })
    DropdownMenuItem(text = { Text("Clear Selection") }, enabled = actions["clear_selection"] == true, onClick = {
        send("selection", JSONObject().put("command", JSONObject().put("op", "clear")))
    })
    DropdownMenuItem(text = { Text("Remove Selected") }, enabled = actions["remove"] == true, onClick = { send("remove") })
    DropdownMenuItem(text = { Text(if (queue) "Clear Play Queue" else "Clear Playlist") }, enabled = actions["clear"] == true, onClick = { send("clear") })
    listOf("Move Up" to -1, "Move Down" to 1).forEach { (label, delta) ->
        DropdownMenuItem(text = { Text(label) }, enabled = actions[if (delta < 0) "move_up" else "move_down"] == true,
            onClick = { send("nudge", JSONObject().put("delta", delta)) })
    }
    HorizontalDivider()
    DropdownMenuItem(text = { Text("Save Changes") }, enabled = actions["save"] == true, onClick = { send("save") })
    DropdownMenuItem(text = { Text("Reload Saved Playlist") }, enabled = actions["reload"] == true, onClick = { send("reload") })
    DropdownMenuItem(text = { Text("Add Play Queue") }, enabled = actions["add_play_queue"] == true, onClick = { state.workspaceAppendQueue(); dismiss() })
    DropdownMenuItem(text = { Text("Add Queue Selection") }, enabled = actions["add_queue_selection"] == true, onClick = { state.workspaceAppendQueue(true); dismiss() })
}

@Composable
internal fun PlaylistWorkspaceTabs(state: KogState) = PlaylistWorkspaceTabs(state.workspace, state::workspaceCommand)

@Composable
internal fun PlaylistWorkspaceTabs(workspace: PlaylistWorkspaceSnapshot, command: (String, JSONObject) -> Unit) {
    val currentWorkspace by rememberUpdatedState(workspace)
    val send by rememberUpdatedState(command)
    val densityForTabs = androidx.compose.ui.platform.LocalDensity.current.density
    val scroll = rememberScrollState()
    val bounds = remember { mutableStateMapOf<String, Rect>() }
    var strip by remember { mutableStateOf(Rect.Zero) }
    var dragging by remember { mutableStateOf<String?>(null) }
    var point by remember { mutableStateOf(Offset.Zero) }
    val tabs by rememberUpdatedState(currentWorkspace.tabs)
    val accent = MaterialTheme.colorScheme.primary
    fun destination(): String? = tabs.firstOrNull {
        it.key != dragging && bounds[it.key]?.let { rect -> point.x < rect.center.x } == true
    }?.key
    fun validDrop() = strip.contains(point) && currentWorkspace.pendingClose == null
    LaunchedEffect(dragging) {
        while (dragging != null) {
            if (validDrop()) {
                val edge = 28 * densityForTabs
                val delta = when { point.x < strip.left + edge -> -8f; point.x > strip.right - edge -> 8f; else -> 0f }
                scroll.scrollBy(delta * densityForTabs)
            }
            delay(16)
        }
    }
    if (tabs.size > 1) PrimaryScrollableTabRow(
        selectedTabIndex = tabs.indexOfFirst { it.key == currentWorkspace.active }.coerceAtLeast(0),
        scrollState = scroll,
        edgePadding = 8.dp, containerColor = MaterialTheme.colorScheme.surface,
        modifier = Modifier.onGloballyPositioned { strip = Rect(it.positionInRoot(), it.size.toSize()) }
            .drawWithContent {
                drawContent()
                if (dragging != null && validDrop()) {
                    val next = destination()
                    val edge = (if (next != null) bounds[next]?.left else tabs.lastOrNull { it.key != dragging }?.let { bounds[it.key]?.right }) ?: strip.left
                    val x = (edge - strip.left).coerceIn(1f, size.width - 2f)
                    drawLine(accent, Offset(x, 2f), Offset(x, size.height - 2f), strokeWidth = 3.dp.toPx())
                }
            },
    ) {
        tabs.forEach { tab -> key(tab.key) {
            Tab(selected = currentWorkspace.active == tab.key,
                modifier = Modifier.onGloballyPositioned { bounds[tab.key] = Rect(it.positionInRoot(), it.size.toSize()) }
                    .graphicsLayer { alpha = if (dragging == tab.key) 0.5f else 1f }
                    .pointerInput(tab.key) {
                        detectDragGesturesAfterLongPress(
                            onDragStart = { offset ->
                                if (currentWorkspace.pendingClose == null) {
                                    dragging = tab.key
                                    point = (bounds[tab.key]?.topLeft ?: Offset.Zero) + offset
                                }
                            },
                            onDrag = { change, amount -> if (dragging == tab.key) { change.consume(); point += amount } },
                            onDragCancel = { dragging = null },
                            onDragEnd = {
                                if (dragging == tab.key && validDrop()) send("move_tab",
                                    JSONObject().put("key", tab.key).put("before", destination() ?: JSONObject.NULL))
                                dragging = null
                            },
                        )
                    },
                onClick = { send("focus", JSONObject().put("key", tab.key)) },
                text = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(tab.name + if (tab.dirty) " •" else "", maxLines = 1,
                            overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 220.dp))
                        if (tab.key != "queue") IconButton(
                            modifier = Modifier.size(32.dp),
                            onClick = { send("close", JSONObject().put("key", tab.key)) }) {
                            Icon(Icons.Default.Close, "Close ${tab.name}", Modifier.size(16.dp))
                        }
                    }
                })
        } }
    }
    if (currentWorkspace.pendingClose != null) AlertDialog(
        onDismissRequest = { send("resolve_close", JSONObject().put("choice", "cancel")) },
        title = { Text("Save playlist changes?") }, text = { Text("The playlist has unsaved changes.") },
        confirmButton = { TextButton(onClick = { send("resolve_close", JSONObject().put("choice", "save")) }) { Text("Save") } },
        dismissButton = {
            Row {
                TextButton(onClick = { send("resolve_close", JSONObject().put("choice", "discard")) }) { Text("Discard") }
                TextButton(onClick = { send("resolve_close", JSONObject().put("choice", "cancel")) }) { Text("Cancel") }
            }
        })
}

@Composable
internal fun PlaylistWorkspaceEditor(state: KogState) {
    val workspace = state.workspace
    val tab = workspace.activeTab
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
            listOf("Play Now" to "play_now", "Play Next" to "play_next", "Add to Queue" to "add_to_queue").forEach { (label, mode) ->
                TextButton(enabled = workspace.actions["queue"] == true,
                    onClick = { state.workspaceCommand("queue", JSONObject().put("action", mode)) }) { Text(label) }
            }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(if (workspace.selected.isEmpty()) "${workspace.entries.size} tracks · whole playlist" else "${workspace.selected.size} selected",
                Modifier.weight(1f), style = MaterialTheme.typography.labelMedium)
            TextButton(enabled = workspace.actions["save"] == true,
                onClick = { state.workspaceCommand("save") }) { Text(if (tab?.saving == true) "Saving…" else "Save") }
            Box {
                TextButton(onClick = { menu = true }) { Text("Edit") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    Text("Edit", Modifier.padding(horizontal = 12.dp, vertical = 8.dp), style = MaterialTheme.typography.labelLarge)
                    EditMenuItems(state) { menu = false }
                }
            }
        }
        workspace.error?.let { Text(it, Modifier.padding(12.dp), color = MaterialTheme.colorScheme.error) }
        if (tab?.loading == true) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (tab?.readonly == true) Text("Favorites · use star controls to change this list", Modifier.padding(12.dp), style = MaterialTheme.typography.labelMedium)
        LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
            itemsIndexed(workspace.entries) { index, track ->
                Row(Modifier.fillMaxWidth().combinedClickable(
                    onClick = { state.workspaceSelect(index) },
                    onDoubleClick = { state.workspaceCommand("activate", JSONObject().put("index", index)) },
                ).padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(index in workspace.selected, onCheckedChange = { state.workspaceSelect(index) })
                    Column(Modifier.weight(1f)) {
                        Text(track.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(track.detail, maxLines = 1, style = MaterialTheme.typography.bodySmall)
                    }
                }
                HorizontalDivider()
            }
        }
    }
}
