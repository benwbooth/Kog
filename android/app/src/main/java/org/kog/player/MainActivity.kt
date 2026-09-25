package org.kog.player

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Casino
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.PlaylistPlay
import androidx.compose.material.icons.filled.QueueMusic
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Shuffle
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material.icons.filled.Sort
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil.compose.AsyncImage
import coil.request.ImageRequest
import kotlinx.coroutines.delay

private val Window = Color(0xff1b1e20)
private val Base = Color(0xff23272a)
private val Surface = Color(0xff202427)
private val Alternate = Color(0xff2b3034)
private val TextColor = Color(0xffeff0f1)
private val Muted = Color(0xff9aa0a6)
private val Accent = Color(0xff3daee9)
private val Border = Color(0xff34393d)

class MainActivity : ComponentActivity() {
    private val state by lazy { KogState(applicationContext) }
    private val openFiles = registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        uris.forEach { uri ->
            runCatching { contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
        }
        state.importFiles(uris)
    }
    private val openFolder = registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri != null) {
            runCatching { contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
            state.importFolder(uri)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.statusBarColor = android.graphics.Color.rgb(32, 36, 39)
        window.navigationBarColor = android.graphics.Color.rgb(32, 36, 39)
        state.connectPlayer()
        state.refresh()
        setContent {
            MaterialTheme(colorScheme = darkColorScheme(
                primary = Accent, onPrimary = Color.White, background = Window,
                onBackground = TextColor, surface = Surface, onSurface = TextColor,
                surfaceVariant = Alternate, onSurfaceVariant = Muted, outline = Border)) {
                Surface(color = Window, contentColor = TextColor) {
                    KogApp(state,
                        pickFiles = { openFiles.launch(arrayOf("*/*")) },
                        pickFolder = { openFolder.launch(null) })
                }
            }
        }
    }

    override fun onDestroy() {
        state.release()
        super.onDestroy()
    }
}

private enum class Tab { Library, Queue, Playlists }

@Composable
private fun KogApp(state: KogState, pickFiles: () -> Unit, pickFolder: () -> Unit) {
    var tab by remember { mutableStateOf(Tab.Queue) }
    var settings by remember { mutableStateOf(state.api.server.isBlank()) }
    var playerExpanded by remember { mutableStateOf(false) }
    var createPlaylist by remember { mutableStateOf(false) }
    var sortMenu by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        while (true) { delay(500); state.tick() }
    }

    Column(Modifier.fillMaxSize().background(Window).statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().height(56.dp).background(Surface).padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically) {
            when (tab) {
                Tab.Library -> Text("Library", Modifier.weight(1f), fontSize = 21.sp, fontWeight = FontWeight.Bold)
                Tab.Queue -> Text("Queue", Modifier.weight(1f), fontSize = 21.sp, fontWeight = FontWeight.Bold)
                Tab.Playlists -> {
                    if (state.selectedPlaylist != null) {
                        IconButton(onClick = state::closePlaylist) { Icon(Icons.Default.ArrowBack, "Back") }
                    }
                    Text(state.selectedPlaylist?.name ?: "Playlists", Modifier.weight(1f),
                        fontSize = 21.sp, fontWeight = FontWeight.Bold, maxLines = 1,
                        overflow = TextOverflow.Ellipsis)
                }
            }
            if (tab == Tab.Queue) {
                Box {
                    IconButton(onClick = { sortMenu = true }) { Icon(Icons.Default.Sort, "Sort queue") }
                    DropdownMenu(expanded = sortMenu, onDismissRequest = { sortMenu = false }) {
                        listOf("Title", "Artist", "Album", "Duration").forEach { name ->
                            DropdownMenuItem(text = { Text("Sort by $name") }, onClick = {
                                state.sortQueue(name); sortMenu = false
                            })
                        }
                        DropdownMenuItem(text = { Text("Clear queue") }, onClick = {
                            state.clear(); sortMenu = false
                        }, leadingIcon = { Icon(Icons.Default.Delete, null) })
                    }
                }
            }
            if (tab == Tab.Playlists && state.selectedPlaylist == null) {
                IconButton(onClick = { createPlaylist = true }) { Icon(Icons.Default.Add, "Create playlist", tint = Accent) }
            }
            IconButton(onClick = { settings = true }) {
                Icon(Icons.Default.Settings, "Connection and local files", tint = if (state.connected) Accent else Muted)
            }
        }
        HorizontalDivider(color = Border)

        Box(Modifier.weight(1f)) {
            when (tab) {
                Tab.Library -> LibraryView(state, pickFiles, pickFolder,
                    openQueue = { tab = Tab.Queue })
                Tab.Queue -> QueueView(state, openLibrary = { tab = Tab.Library })
                Tab.Playlists -> PlaylistsView(state, openQueue = { tab = Tab.Queue })
            }
        }
        MiniPlayer(state) { playerExpanded = true }
        BottomTabs(tab) { selected ->
            tab = selected
            if (selected == Tab.Playlists) state.loadPlaylists()
        }
    }

    if (settings) SettingsSheet(state, pickFiles, pickFolder) { settings = false }
    if (playerExpanded) FullPlayer(state) { playerExpanded = false }
    if (createPlaylist) NameDialog("New playlist", "Create", onDismiss = { createPlaylist = false }) {
        state.createPlaylist(it); createPlaylist = false
    }
    if (state.error.isNotBlank()) AlertDialog(
        onDismissRequest = state::clearError,
        title = { Text("Kog") }, text = { Text(state.error) },
        confirmButton = { TextButton(onClick = state::clearError) { Text("OK") } })
}

@Composable
private fun BottomTabs(selected: Tab, choose: (Tab) -> Unit) {
    Row(Modifier.fillMaxWidth().height(62.dp).background(Surface),
        horizontalArrangement = Arrangement.SpaceEvenly) {
        listOf(Triple(Tab.Library, "Library", Icons.Default.FolderOpen),
            Triple(Tab.Queue, "Queue", Icons.Default.QueueMusic),
            Triple(Tab.Playlists, "Playlists", Icons.Default.PlaylistPlay)).forEach { (tab, label, icon) ->
            Column(Modifier.weight(1f).fillMaxHeight()
                .clip(RoundedCornerShape(10.dp)).clickable { choose(tab) },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center) {
                Icon(icon, null, Modifier.size(22.dp), tint = if (selected == tab) Accent else Muted)
                Text(label, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                    color = if (selected == tab) Accent else Muted)
            }
        }
    }
}

@Composable
private fun LibraryView(state: KogState, pickFiles: () -> Unit, pickFolder: () -> Unit,
                        openQueue: () -> Unit) {
    var deviceMode by remember { mutableStateOf(state.api.server.isBlank() && state.localRoot.isNotBlank()) }
    val listing = state.listing
    Column(Modifier.fillMaxSize().background(Base)) {
        Row(Modifier.fillMaxWidth().height(42.dp).background(Surface).padding(horizontal = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { deviceMode = false }) {
                Text("Server", color = if (!deviceMode) Accent else Muted)
            }
            TextButton(onClick = { deviceMode = true }) {
                Text("On this device", color = if (deviceMode) Accent else Muted)
            }
        }
        if (deviceMode) {
            DeviceLibraryView(state, pickFiles, pickFolder, openQueue)
            return@Column
        }
        SearchBox(state.searchText, state::search)
        Row(Modifier.fillMaxWidth().height(44.dp).background(Surface).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically) {
            if (state.searchText.isBlank() && listing?.parent?.isNotBlank() == true &&
                listing.path != state.libraryRoot && listing.parent.startsWith(state.libraryRoot)) {
                IconButton(onClick = { state.browse(listing.parent) }) {
                    Icon(Icons.Default.ArrowBack, "Parent folder", tint = Accent)
                }
            }
            Text(if (state.searchText.isNotBlank()) "Search results" else
                listing?.path?.substringAfterLast('/')?.ifBlank { "Library" } ?: "Library",
                Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis,
                fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = Muted)
            if (state.searching) Text("${state.searchScanned} scanned", color = Muted, fontSize = 11.sp)
            IconButton(onClick = pickFiles) { Icon(Icons.Default.Add, "Add device files", tint = Accent) }
        }
        val folders = if (state.searchText.isNotBlank()) state.searchFolders.toList() else listing?.directories.orEmpty()
        val files = if (state.searchText.isNotBlank()) state.searchTracks.toList() else listing?.files.orEmpty()
        if (listing == null && state.api.server.isBlank()) {
            EmptyPanel("Connect to a Kog server", "You can also play files stored on this device.") {
                OutlinedButton(onClick = pickFolder) { Text("Open device folder") }
            }
        } else {
            LazyColumn(Modifier.fillMaxSize()) {
                itemsIndexed(folders, key = { _, folder -> "folder:${folder.path}" }) { _, folder ->
                    Row(Modifier.fillMaxWidth().height(46.dp).clickable { state.browse(folder.path) }
                        .padding(start = 14.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        FolderIcon()
                        Spacer(Modifier.width(12.dp))
                        Text(folder.name, Modifier.weight(1f), maxLines = 1,
                            overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
                        IconButton(onClick = { state.addFolder(folder, onAdded = openQueue) }) {
                            Icon(Icons.Default.Add, "Add folder to queue", tint = Accent)
                        }
                    }
                    HorizontalDivider(color = Border)
                }
                itemsIndexed(files, key = { index, track -> "file:${track.key}:$index" }) { _, track ->
                    Row(Modifier.fillMaxWidth().height(46.dp).clickable { state.addFile(track, true, openQueue) }
                        .padding(start = 14.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        FormatIcon(track)
                        Spacer(Modifier.width(12.dp))
                        Text(track.name.ifBlank { track.label }, Modifier.weight(1f), maxLines = 1,
                            overflow = TextOverflow.Ellipsis)
                        IconButton(onClick = { state.addFile(track, onAdded = openQueue) }) {
                            Icon(Icons.Default.Add, "Add to queue", tint = Accent)
                        }
                    }
                    HorizontalDivider(color = Border)
                }
            }
        }
    }
}

@Composable
private fun DeviceLibraryView(state: KogState, pickFiles: () -> Unit, pickFolder: () -> Unit,
                              openQueue: () -> Unit) {
    val current = state.deviceCurrent
    Column(Modifier.fillMaxSize().background(Base)) {
        Row(Modifier.fillMaxWidth().height(46.dp).background(Surface).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically) {
            if (current?.parentFile != null) IconButton(onClick = state::deviceUp) {
                Icon(Icons.Default.ArrowBack, "Parent folder", tint = Accent)
            }
            Text(current?.name ?: "Files on this device", Modifier.weight(1f),
                maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
            if (state.importing) Text("Adding…", color = Muted, fontSize = 11.sp)
            IconButton(onClick = pickFolder) { Icon(Icons.Default.FolderOpen, "Choose device folder", tint = Accent) }
            IconButton(onClick = pickFiles) { Icon(Icons.Default.Add, "Choose files", tint = Accent) }
        }
        if (current == null) {
            EmptyPanel("Files on this device", "Choose a folder to browse local music.") {
                Button(onClick = pickFolder) { Text("Choose folder") }
            }
        } else {
            LazyColumn(Modifier.fillMaxSize()) {
                itemsIndexed(state.deviceFolders.toList(), key = { _, file -> file.uri.toString() }) { _, folder ->
                    Row(Modifier.fillMaxWidth().height(46.dp).clickable { state.browseDevice(folder) }
                        .padding(start = 14.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        FolderIcon()
                        Spacer(Modifier.width(12.dp))
                        Text(folder.name.orEmpty(), Modifier.weight(1f), maxLines = 1,
                            overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
                        IconButton(onClick = { state.addDeviceFolder(folder, openQueue) }) {
                            Icon(Icons.Default.Add, "Add folder to queue", tint = Accent)
                        }
                    }
                    HorizontalDivider(color = Border)
                }
                itemsIndexed(state.deviceFiles.toList(), key = { _, file -> file.uri.toString() }) { _, file ->
                    Row(Modifier.fillMaxWidth().height(46.dp)
                        .clickable { state.importFiles(listOf(file.uri), true, openQueue) }
                        .padding(start = 14.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        FormatIcon(Track("device", file.uri.toString(), name = file.name.orEmpty()))
                        Spacer(Modifier.width(12.dp))
                        Text(file.name.orEmpty(), Modifier.weight(1f), maxLines = 1,
                            overflow = TextOverflow.Ellipsis)
                        IconButton(onClick = { state.importFiles(listOf(file.uri), onAdded = openQueue) }) {
                            Icon(Icons.Default.Add, "Add to queue", tint = Accent)
                        }
                    }
                    HorizontalDivider(color = Border)
                }
            }
        }
    }
}

@Composable
private fun SearchBox(text: String, onChange: (String) -> Unit) {
    OutlinedTextField(value = text, onValueChange = onChange,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp).height(50.dp),
        singleLine = true, placeholder = { Text("Search files and folders", fontSize = 14.sp) },
        leadingIcon = { Icon(Icons.Default.Search, null, Modifier.size(20.dp)) },
        trailingIcon = { if (text.isNotEmpty()) IconButton(onClick = { onChange("") }) {
            Icon(Icons.Default.Close, "Clear search") } },
        shape = RoundedCornerShape(12.dp), textStyle = MaterialTheme.typography.bodyMedium)
}

@Composable
private fun QueueView(state: KogState, openLibrary: () -> Unit) {
    val rows = state.queue.toList()
    if (rows.isEmpty()) {
        EmptyPanel("Your queue is empty", "Browse your library or add files from this device.") {
            Button(onClick = openLibrary) { Text("Browse library") }
        }
        return
    }
    LazyColumn(Modifier.fillMaxSize().background(Base)) {
        itemsIndexed(rows, key = { index, track -> "${track.key}:$index" }) { index, track ->
            QueueTrack(state, track, index)
            HorizontalDivider(color = Border)
        }
    }
}

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
private fun QueueTrack(state: KogState, track: Track, index: Int) {
    var menu by remember { mutableStateOf(false) }
    val active = index == state.currentIndex
    Row(Modifier.fillMaxWidth().height(60.dp)
        .background(if (active) Accent.copy(alpha = 0.12f) else Base)
        .combinedClickable(onClick = { state.play(index) }, onLongClick = { menu = true })
        .padding(start = 12.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        FormatIcon(track, Modifier.width(28.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (active) Icon(if (state.playing) Icons.Default.PlayArrow else Icons.Default.Pause,
                    null, Modifier.size(15.dp), tint = Accent)
                Text(track.label, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    fontSize = 15.sp, fontWeight = FontWeight.Medium)
            }
            Text(track.detail.ifBlank { track.path.substringBeforeLast('/').substringAfterLast('/') }, maxLines = 1,
                overflow = TextOverflow.Ellipsis, color = Muted, fontSize = 12.sp)
        }
        Text(formatTime(track.duration), color = Muted, fontSize = 12.sp)
        IconButton(onClick = { state.toggleStar(track) }, enabled = !track.isDevice) {
            Icon(if (track.key in state.stars) Icons.Default.Favorite else Icons.Default.FavoriteBorder,
                "Favorite", tint = if (track.key in state.stars) Accent else Muted,
                modifier = Modifier.size(19.dp))
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Default.MoreVert, "Track actions", tint = Muted) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text("Remove from queue") }, onClick = {
                    state.remove(index); menu = false
                })
                if (index > 0) DropdownMenuItem(text = { Text("Move up") }, onClick = {
                    state.move(index, index - 1); menu = false
                })
                if (index < state.queue.lastIndex) DropdownMenuItem(text = { Text("Move down") }, onClick = {
                    state.move(index, index + 1); menu = false
                })
                state.playlists.filter { it.id != 0L }.forEach { playlist ->
                    DropdownMenuItem(text = { Text("Add to ${playlist.name}") }, onClick = {
                        state.saveToPlaylist(playlist, listOf(track)); menu = false
                    }, enabled = !track.isDevice)
                }
            }
        }
    }
}

@Composable
private fun PlaylistsView(state: KogState, openQueue: () -> Unit) {
    val selected = state.selectedPlaylist
    if (selected != null) {
        LazyColumn(Modifier.fillMaxSize().background(Base)) {
            item {
                Row(Modifier.fillMaxWidth().padding(12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = { state.add(state.playlistTracks.toList(), true); openQueue() }) {
                        Icon(Icons.Default.PlayArrow, null); Text("Play")
                    }
                    OutlinedButton(onClick = { state.add(state.playlistTracks.toList()); openQueue() }) {
                        Icon(Icons.Default.Add, null); Text("Add to queue")
                    }
                }
            }
            itemsIndexed(state.playlistTracks.toList()) { _, track ->
                Row(Modifier.fillMaxWidth().height(52.dp)
                    .clickable { state.add(listOf(track), true); openQueue() }
                    .padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                    FormatIcon(track)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(track.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(track.detail, color = Muted, fontSize = 11.sp, maxLines = 1)
                    }
                    IconButton(onClick = { state.add(listOf(track)); openQueue() }) {
                        Icon(Icons.Default.Add, "Add to queue", tint = Accent)
                    }
                }
                HorizontalDivider(color = Border)
            }
        }
        return
    }
    val playlists = listOf(SavedPlaylist(0, "Favorites", state.stars.size)) + state.playlists.filter { it.id != 0L }
    LazyColumn(Modifier.fillMaxSize().background(Base).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp)) {
        itemsIndexed(playlists) { _, playlist ->
            var menu by remember { mutableStateOf(false) }
            var rename by remember { mutableStateOf(false) }
            Row(Modifier.fillMaxWidth().height(60.dp).clip(RoundedCornerShape(12.dp))
                .background(Alternate).clickable { state.openPlaylist(playlist) }
                .padding(start = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(if (playlist.id == 0L) Icons.Default.Favorite else Icons.Default.PlaylistPlay,
                    null, tint = Accent)
                Spacer(Modifier.width(12.dp))
                Text(playlist.name, Modifier.weight(1f), fontWeight = FontWeight.SemiBold)
                Text(playlist.count.toString(), color = Muted, fontSize = 12.sp)
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Default.MoreVert, "Playlist actions") }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text("Open") }, onClick = {
                            state.openPlaylist(playlist); menu = false
                        })
                        if (playlist.id != 0L) {
                            DropdownMenuItem(text = { Text("Add queue") }, onClick = {
                                state.saveToPlaylist(playlist, state.queue.toList()); menu = false
                            })
                            DropdownMenuItem(text = { Text("Rename") }, onClick = {
                                rename = true; menu = false
                            })
                            DropdownMenuItem(text = { Text("Delete") }, onClick = {
                                state.deletePlaylist(playlist); menu = false
                            })
                        }
                    }
                }
            }
            if (rename) NameDialog("Rename playlist", "Save", playlist.name,
                onDismiss = { rename = false }) { state.renamePlaylist(playlist, it); rename = false }
        }
    }
}

@Composable
private fun EmptyPanel(title: String, subtitle: String, action: @Composable () -> Unit) {
    Column(Modifier.fillMaxSize().background(Base).padding(28.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center) {
        Icon(Icons.Default.QueueMusic, null, Modifier.size(65.dp), tint = Accent)
        Spacer(Modifier.height(16.dp))
        Text(title, fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(6.dp))
        Text(subtitle, color = Muted, fontSize = 13.sp)
        Spacer(Modifier.height(20.dp))
        action()
    }
}

@Composable
private fun MiniPlayer(state: KogState, expand: () -> Unit) {
    val track = state.current
    Row(Modifier.fillMaxWidth().height(66.dp).background(Window).padding(horizontal = 8.dp, vertical = 4.dp)
        .clip(RoundedCornerShape(13.dp)).background(Alternate).clickable(onClick = expand)
        .padding(horizontal = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        CoverArt(state, track, Modifier.size(44.dp).clip(RoundedCornerShape(8.dp)))
        Spacer(Modifier.width(9.dp))
        Column(Modifier.weight(1f)) {
            Text(track?.label ?: "Ready to play", maxLines = 1, overflow = TextOverflow.Ellipsis,
                fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            Text(track?.detail?.ifBlank { track.path.substringAfterLast('/') } ?: "Kog",
                maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 11.sp, color = Muted)
        }
        IconButton(onClick = state::toggle, enabled = track != null) {
            Icon(if (state.playing) Icons.Default.Pause else Icons.Default.PlayArrow,
                if (state.playing) "Pause" else "Play", tint = TextColor)
        }
        IconButton(onClick = state::next, enabled = track != null) {
            Icon(Icons.Default.SkipNext, "Next", tint = TextColor)
        }
    }
}

@Composable
private fun CoverArt(state: KogState, track: Track?, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val url = track?.let(state.api::art)
    if (url == null) {
        Box(modifier.background(Surface), contentAlignment = Alignment.Center) {
            Icon(Icons.Default.MusicNote, null, tint = Accent)
        }
    } else {
        val request = ImageRequest.Builder(context).data(url).apply {
            if (state.api.username.isNotBlank() && state.api.token.isBlank()) {
                val encoded = java.util.Base64.getEncoder().encodeToString(
                    "${state.api.username}:${state.api.password}".toByteArray())
                addHeader("Authorization", "Basic $encoded")
            }
        }.build()
        AsyncImage(model = request, contentDescription = "Album artwork", modifier = modifier.background(Surface),
            contentScale = ContentScale.Crop)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FullPlayer(state: KogState, dismiss: () -> Unit) {
    val track = state.current
    var seek by remember(track?.key) { mutableStateOf<Float?>(null) }
    ModalBottomSheet(onDismissRequest = dismiss, containerColor = Surface,
        sheetState = androidx.compose.material3.rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 22.dp).padding(bottom = 22.dp),
            horizontalAlignment = Alignment.CenterHorizontally) {
            Spacer(Modifier.height(12.dp))
            CoverArt(state, track, Modifier.size(200.dp).clip(RoundedCornerShape(18.dp)))
            Spacer(Modifier.height(22.dp))
            Text(track?.label ?: "Ready to play", fontSize = 20.sp, fontWeight = FontWeight.Bold,
                maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(track?.detail ?: "", color = Muted, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(20.dp))
            val duration = state.duration.coerceAtLeast(track?.duration ?: 0)
            Slider(value = seek ?: state.position.toFloat().coerceAtMost(duration.toFloat()),
                onValueChange = { seek = it }, onValueChangeFinished = {
                    state.seek((seek ?: 0f).toLong()); seek = null
                }, valueRange = 0f..duration.coerceAtLeast(1).toFloat())
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(formatTime(state.position), color = Muted, fontSize = 11.sp)
                Text(formatTime(duration), color = Muted, fontSize = 11.sp)
            }
            Spacer(Modifier.height(16.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = state::shuffle) {
                    Icon(Icons.Default.Shuffle, "Shuffle", tint = if (state.shuffleOn) Accent else Muted)
                }
                IconButton(onClick = state::previous) { Icon(Icons.Default.SkipPrevious, "Previous", Modifier.size(34.dp)) }
                IconButton(onClick = state::toggle) {
                    Icon(if (state.playing) Icons.Default.Pause else Icons.Default.PlayArrow,
                        if (state.playing) "Pause" else "Play", Modifier.size(45.dp), tint = Accent)
                }
                IconButton(onClick = state::next) { Icon(Icons.Default.SkipNext, "Next", Modifier.size(34.dp)) }
                IconButton(onClick = state::repeat) {
                    Icon(Icons.Default.Repeat, "Repeat", tint = if (state.repeatOn) Accent else Muted)
                }
            }
            TextButton(onClick = state::radio) {
                Icon(Icons.Default.Casino, null, tint = if (state.radioOn) Accent else Muted)
                Spacer(Modifier.width(6.dp))
                Text("Random radio", color = if (state.radioOn) Accent else Muted)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsSheet(state: KogState, pickFiles: () -> Unit, pickFolder: () -> Unit,
                          dismiss: () -> Unit) {
    var server by remember { mutableStateOf(state.api.server) }
    var token by remember { mutableStateOf(state.api.token) }
    var username by remember { mutableStateOf(state.api.username) }
    var password by remember { mutableStateOf(state.api.password) }
    var codec by remember { mutableStateOf(state.api.codec) }
    var codecMenu by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = dismiss, containerColor = Surface) {
        val maxHeight = LocalConfiguration.current.screenHeightDp.dp * 0.78f
        Column(Modifier.fillMaxWidth().heightIn(max = maxHeight).verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp).padding(bottom = 30.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("Connection", fontSize = 20.sp, fontWeight = FontWeight.Bold)
            Text("Use the address of the computer running Kog, not localhost on your phone.",
                fontSize = 12.sp, color = Muted)
            OutlinedTextField(server, { server = it }, Modifier.fillMaxWidth(),
                label = { Text("Server URL") }, placeholder = { Text("http://192.168.1.10:8420") },
                singleLine = true)
            OutlinedTextField(token, { token = it }, Modifier.fillMaxWidth(),
                label = { Text("Access token") }, singleLine = true)
            OutlinedTextField(username, { username = it }, Modifier.fillMaxWidth(),
                label = { Text("Username, if using Basic auth") }, singleLine = true)
            OutlinedTextField(password, { password = it }, Modifier.fillMaxWidth(),
                label = { Text("Password") }, singleLine = true,
                visualTransformation = PasswordVisualTransformation())
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Stream format", Modifier.weight(1f))
                Box {
                    TextButton(onClick = { codecMenu = true }) { Text(codec.uppercase()) }
                    DropdownMenu(expanded = codecMenu, onDismissRequest = { codecMenu = false }) {
                        listOf("aac", "opus", "flac").forEach { choice ->
                            DropdownMenuItem(text = { Text(choice.uppercase()) }, onClick = {
                                codec = choice; codecMenu = false
                            })
                        }
                    }
                }
            }
            Button(onClick = {
                state.api.server = server
                state.api.token = token
                state.api.username = username
                state.api.password = password
                state.api.codec = codec
                state.connectionChanged()
                state.refresh()
                dismiss()
            }, modifier = Modifier.fillMaxWidth()) { Text("Connect") }
            HorizontalDivider(color = Border)
            Text("Files on this device", fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { pickFiles(); dismiss() }) { Text("Add files") }
                OutlinedButton(onClick = { pickFolder(); dismiss() }) { Text("Add folder") }
            }
            Text("Browse device files in Library. Kog server playlists contain server files.",
                color = Muted, fontSize = 11.sp)
        }
    }
}

@Composable
private fun NameDialog(title: String, action: String, initial: String = "",
                       onDismiss: () -> Unit, onSubmit: (String) -> Unit) {
    var name by remember { mutableStateOf(initial) }
    AlertDialog(onDismissRequest = onDismiss, title = { Text(title) },
        text = { OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true) },
        confirmButton = { TextButton(onClick = { if (name.isNotBlank()) onSubmit(name.trim()) }) {
            Text(action)
        } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } })
}

private fun formatTime(milliseconds: Long): String {
    if (milliseconds <= 0) return ""
    val seconds = milliseconds / 1000
    return if (seconds >= 3600) "%d:%02d:%02d".format(seconds / 3600, seconds / 60 % 60, seconds % 60)
    else "%d:%02d".format(seconds / 60, seconds % 60)
}
