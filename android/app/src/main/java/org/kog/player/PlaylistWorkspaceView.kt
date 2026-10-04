package org.kog.player

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
internal fun PlaylistWorkspaceTabs(state: KogState) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        state.workspace.tabs.forEach { tab ->
            Row(verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.background(if (state.workspace.active == tab.key) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surface)) {
                TextButton(onClick = { state.workspaceCommand("focus", JSONObject().put("key", tab.key)) }) {
                    Text(tab.name + if (tab.dirty) " •" else "", maxLines = 1)
                }
                if (tab.key != "queue") IconButton(onClick = { state.workspaceCommand("close", JSONObject().put("key", tab.key)) }) {
                    Icon(Icons.Default.Close, "Close ${tab.name}", Modifier.size(16.dp))
                }
            }
        }
    }
    if (state.workspace.pendingClose != null) AlertDialog(
        onDismissRequest = { state.workspaceCommand("resolve_close", JSONObject().put("choice", "cancel")) },
        title = { Text("Save playlist changes?") }, text = { Text("The playlist has unsaved changes.") },
        confirmButton = { TextButton(onClick = { state.workspaceCommand("resolve_close", JSONObject().put("choice", "save")) }) { Text("Save") } },
        dismissButton = {
            Row {
                TextButton(onClick = { state.workspaceCommand("resolve_close", JSONObject().put("choice", "discard")) }) { Text("Discard") }
                TextButton(onClick = { state.workspaceCommand("resolve_close", JSONObject().put("choice", "cancel")) }) { Text("Cancel") }
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
                IconButton(onClick = { menu = true }) { Icon(Icons.Default.MoreVert, "Playlist editor actions") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text("Select all") }, enabled = workspace.actions["select_all"] == true, onClick = {
                        state.workspaceCommand("selection", JSONObject().put("command", JSONObject().put("op", "all"))); menu = false
                    })
                    DropdownMenuItem(text = { Text("Clear selection") }, enabled = workspace.actions["clear_selection"] == true, onClick = {
                        state.workspaceCommand("selection", JSONObject().put("command", JSONObject().put("op", "clear"))); menu = false
                    })
                    if (tab?.readonly != true) {
                        DropdownMenuItem(text = { Text("Add Play Queue") }, enabled = workspace.actions["add_play_queue"] == true, onClick = { state.workspaceAppendQueue(); menu = false })
                        DropdownMenuItem(text = { Text("Add Queue Selection") }, enabled = workspace.actions["add_queue_selection"] == true, onClick = { state.workspaceAppendQueue(true); menu = false })
                        DropdownMenuItem(text = { Text("Remove selected") }, enabled = workspace.actions["remove"] == true, onClick = { state.workspaceCommand("remove"); menu = false })
                        listOf("Move Up" to -1, "Move Down" to 1).forEach { (label, delta) ->
                            DropdownMenuItem(text = { Text(label) }, enabled = workspace.actions[if (delta < 0) "move_up" else "move_down"] == true, onClick = {
                                state.workspaceCommand("nudge", JSONObject().put("delta", delta)); menu = false
                            })
                        }
                        DropdownMenuItem(text = { Text("Undo") }, enabled = workspace.actions["undo"] == true, onClick = { state.workspaceCommand("undo"); menu = false })
                        DropdownMenuItem(text = { Text("Redo") }, enabled = workspace.actions["redo"] == true, onClick = { state.workspaceCommand("redo"); menu = false })
                    }
                    DropdownMenuItem(text = { Text("Reload") }, enabled = workspace.actions["reload"] == true, onClick = { state.workspaceCommand("reload"); menu = false })
                }
            }
        }
        workspace.error?.let { Text(it, Modifier.padding(12.dp), color = MaterialTheme.colorScheme.error) }
        if (tab?.loading == true) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (tab?.readonly == true) Text("Favorites · use star controls to change this list", Modifier.padding(12.dp), style = MaterialTheme.typography.labelMedium)
        LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
            itemsIndexed(workspace.entries) { index, track ->
                Row(Modifier.fillMaxWidth().clickable { state.workspaceSelect(index) }.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
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
