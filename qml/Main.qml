pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Layouts
import QtQml.Models
import Qt.labs.platform as Platform

import org.kog.player 1.0

ApplicationWindow {
    id: root

    width: 1120
    height: 540
    minimumWidth: 800
    minimumHeight: 380
    objectName: "kogMainWindow"
    visible: false // Native window-state restoration shows it after restoring geometry.
    property bool restoreMaximized: false
    flags: Qt.Window | Qt.FramelessWindowHint
    title: playbackTitle.windowTitle

    PlaybackTitle {
        id: playbackTitle
        playbackState: appController.playback_state
        trackTitle: appController.now_title
    }
    color: palette.window

    property alias sidebarVisible: mainWindowSettings.sidebarVisible
    property alias sidebarWidth: mainWindowSettings.sidebarWidth
    property alias treeSectionExpanded: mainWindowSettings.treeSectionExpanded
    property alias playlistsSectionExpanded: mainWindowSettings.playlistsSectionExpanded
    MainWindowSettings { id: mainWindowSettings }
    property string playlistHighlightQuery: ""
    // A track can outlive the pane: clearing the playlist detaches what is
    // playing, so transport that acts on the loaded track must not be gated
    // on the list being populated.
    readonly property bool hasLoadedTrack: appController.playback_state !== "stopped"
    readonly property bool transportReady:
        appController.playlist_count > 0 || appController.radio_active || hasLoadedTrack
    property int selectedRow: -1
    property int selectionAnchor: -1
    property var selectedRows: []
    property int playlistDropTarget: -1
    property var selectedPlaylistIds: []
    property int playlistSelectionAnchor: -1
    property int renamingPlaylistId: -2
    property int playlistInsertIndex: -1
    property int playlistMenuPid: -1
    property int playlistRenamePid: -1
    property real volumeBeforeMute: 0.75
    property int mprisRaiseSerialSeen: 0
    property int notificationSerialSeen: 0
    property bool applicationQuitRequested: false
    property var treeSelectedPaths: []
    property int treeSelectionAnchorRow: -1
    property var pendingTreeExpanded: []
    property int treeExpandRestoreAttempts: 0
    property string pendingShowPath: ""
    property int showSelectAttempts: 0
    property string treeContextPath: ""
    readonly property string selectedQueueState: {
        appController.playlist_revision
        return selectedRows.length > 0
            ? appController.queue_selection_state(selectedRows.join(","))
            : "none"
    }
    readonly property string selectedStopAfterState: {
        appController.playlist_revision
        return selectedRows.length > 0
            ? appController.stop_after_selection_state(selectedRows.join(","))
            : "none"
    }
    // Full path of the row under the pointer in the file tree, and where that
    // row sits in the pane. Shown as a popup parented to the view: a ToolTip
    // attached to the row is positioned in the row's own (scrolled) content
    // coordinates, which lands it outside the pane.
    property string treeHoverPath: ""
    property real treeHoverY: 0
    // The delegate the pointer is on: its live position keeps the popup glued
    // to the right row while the tree scrolls.
    property Item treeHoverItem: null
    readonly property bool compactToolbar: width < 980
    // Which build is running: the stamped revision plus, when it is known,
    // when the binary was linked. Shown small in the toolbar and in About.
    readonly property string buildRevision: appController.build_revision()
    readonly property string buildStamp: {
        const when = appController.build_timestamp()
        const time = when > 0
            ? Qt.formatDateTime(new Date(when), "yyyy-MM-dd HH:mm")
            : ""
        return [buildRevision, time].filter(part => part.length > 0).join(" · ")
    }
    readonly property bool useMacWindowControls: Qt.platform.os === "osx"
    readonly property bool playerShowing: (root.visible
        && root.visibility !== Window.Hidden
        && root.visibility !== Window.Minimized)
        || (miniPlayer.visible
            && miniPlayer.visibility !== Window.Hidden
            && miniPlayer.visibility !== Window.Minimized)
        || (classicPlayer.visible && classicPlayer.visibility !== Window.Minimized)
        || (!!modernLoader.item && modernLoader.item.visible && modernLoader.item.visibility !== Window.Minimized)
    readonly property real baseLuminance: 0.2126 * palette.base.r
        + 0.7152 * palette.base.g
        + 0.0722 * palette.base.b
    readonly property color toolbarSurface: baseLuminance < 0.5
        ? Qt.lighter(palette.base, 1.35)
        : palette.window

    onClosing: close => {
        if (!applicationQuitRequested && appController.show_tray_icon
                && appController.close_to_tray && trayIcon.available) {
            close.accepted = false
            root.hide()
            return
        }
        applicationQuitRequested = true
        try {
            appController.flush_session(root.collectTreeExpanded())
        } catch (error) {
        }
        appController.shutdown_synth_helpers()
        Qt.callLater(Qt.quit)
    }
    onVisibilityChanged: function(visibility) {
        if (visibility === Window.Minimized && appController.show_tray_icon
                && appController.minimize_to_tray && trayIcon.available) {
            Qt.callLater(function() {
                if (root.visibility === Window.Minimized)
                    root.hide()
            })
        }
    }
    component WindowButton: AbstractButton {
        id: windowControl
        required property color buttonColor
        property string symbol: ""

        implicitWidth: 14
        implicitHeight: 14
        Accessible.name: ToolTip.text
        ToolTip.visible: hovered
        ToolTip.delay: 700

        contentItem: Label {
            text: windowControl.symbol
            visible: windowControl.hovered
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: "#401b14"
            font.pixelSize: 9
            font.bold: true
        }
        background: Rectangle {
            radius: width / 2
            color: windowControl.enabled ? windowControl.buttonColor : root.palette.mid
            border.width: 1
            border.color: Qt.darker(color, 1.18)
            opacity: windowControl.pressed ? 0.72 : 1
        }
    }

    component TitleDragArea: Item {
        DragHandler {
            target: null
            acceptedButtons: Qt.LeftButton
            onActiveChanged: {
                if (!active)
                    return
                // Active requires a real drag: presses and double-clicks
                // never activate the handler, so restoring here cannot race
                // the double-tap toggle below.
                if (root.visibility === Window.Maximized)
                    root.showNormal()
                root.startSystemMove()
            }
        }
        TapHandler {
            acceptedButtons: Qt.LeftButton
            onDoubleTapped: root.visibility === Window.Maximized
                ? root.showNormal()
                : root.showMaximized()
        }
    }

    component DesktopWindowButton: ToolButton {
        id: desktopWindowControl

        required property string themedIconName
        required property string description

        Layout.preferredWidth: 34
        Layout.preferredHeight: 34
        icon.name: themedIconName
        icon.color: root.palette.text
        icon.width: 16
        icon.height: 16
        display: AbstractButton.IconOnly
        hoverEnabled: true
        palette.window: root.toolbarSurface
        palette.button: root.toolbarSurface
        palette.windowText: root.palette.text
        palette.buttonText: root.palette.text
        Accessible.name: description
        ToolTip.visible: hovered
        ToolTip.delay: 700
        ToolTip.text: description
    }

    component ToolbarButton: CogButton {
        iconBackground: root.toolbarSurface
        forceLightIcon: root.baseLuminance < 0.5
        palette.window: root.toolbarSurface
        palette.button: root.toolbarSurface
        palette.windowText: root.palette.text
        palette.buttonText: root.palette.text
    }

    component ResizeHandle: MouseArea {
        required property int edges
        acceptedButtons: Qt.LeftButton
        onPressed: root.startSystemResize(edges)
    }

    // VSCode-style sidebar accordion header: expander glyph plus title,
    // with an optional trailing action for the playlists header (instant
    // playlist creation). The action uses a plain MouseArea like the
    // toggle itself, so it cannot depend on button styling to receive
    // clicks.
    component SidebarSectionHeader: Rectangle {
        id: sectionHeader
        required property string sectionTitle
        required property bool sectionExpanded
        property bool showAddButton: false
        property string addToolTip: ""
        signal toggled()
        signal addClicked()

        Layout.fillWidth: true
        Layout.preferredHeight: 30
        color: "transparent"

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            hoverEnabled: true
            onClicked: sectionHeader.toggled()
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 6
            spacing: 4

            Label {
                text: sectionHeader.sectionExpanded ? "▾" : "▸"
                font.pixelSize: 11
                color: root.palette.placeholderText
            }
            Label {
                Layout.fillWidth: true
                text: sectionHeader.sectionTitle
                font.bold: true
                font.pixelSize: 11
                color: root.palette.text
                elide: Text.ElideRight
            }
            Rectangle {
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
                visible: sectionHeader.showAddButton
                radius: 5
                color: addMouse.containsMouse
                    ? root.palette.button : "transparent"
                border.color: addMouse.containsMouse
                    ? root.palette.mid : "transparent"

                Label {
                    anchors.centerIn: parent
                    text: "+"
                    font.pixelSize: 16
                    font.bold: true
                    color: root.palette.text
                }
                ToolTip.visible: addMouse.containsMouse
                    && sectionHeader.addToolTip.length > 0
                ToolTip.delay: 450
                ToolTip.text: sectionHeader.addToolTip

                MouseArea {
                    id: addMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: mouse => {
                        mouse.accepted = true
                        sectionHeader.addClicked()
                    }
                }
            }
        }
    }

    function timeLabel(seconds) {
        const value = Math.max(0, Math.floor(seconds))
        const hours = Math.floor(value / 3600)
        const minutes = Math.floor((value % 3600) / 60)
        const remaining = value % 60
        return hours > 0
            ? hours + ":" + String(minutes).padStart(2, "0") + ":" + String(remaining).padStart(2, "0")
            : minutes + ":" + String(remaining).padStart(2, "0")
    }

    function chooseMusicFolder() {
        appController.choose_music_folder()
        fileTreeModel.set_root_path_text(appController.directory_path)
        clearTreeSelection()
        clearTreeExpandRestore()
    }

    function useTreeRoot(path) {
        if (!fileTreeModel.is_path_directory(path))
            return
        appController.choose_directory(fileTreeModel.path_url(path))
        fileTreeModel.set_root_path_text(appController.directory_path)
        clearTreeSelection()
        clearTreeExpandRestore()
    }

    function showFromTray() {
        if (modernLoader.item) modernLoader.item.hide()
        miniPlayer.hide()
        classicPlayer.hide()
        root.visible = true
        if (root.restoreMaximized)
            root.showMaximized()
        else
            root.showNormal()
        Qt.callLater(function() {
            root.raise()
            root.requestActivate()
        })
    }

    function toggleFromTray() {
        if (root.playerShowing) {
            root.hide()
            miniPlayer.hide()
            classicPlayer.hide()
            if (modernLoader.item) modernLoader.item.hide()
            return
        }
        root.showFromTray()
    }

    function showMiniPlayer() {
        if (modernLoader.item) modernLoader.item.hide()
        classicPlayer.hide()
        miniPlayer.show()
        miniPlayer.raise()
        miniPlayer.requestActivate()
        root.hide()
    }

    function showClassicPlayer() {
        const skin = JSON.parse(skinLibrary.active_json)
        if (skin.kind === "modern") {
            modernLoader.setSource("ModernPlayer.qml", { app: appController, mainWindow: root, skin: skin, libraryModel: modernLibraryModel })
            if (!modernLoader.item) return
            modernLoader.item.skin = skin
            modernLoader.item.show()
            modernLoader.item.raise()
            modernLoader.item.requestActivate()
            classicPlayer.hide()
            miniPlayer.hide()
            root.hide()
            return
        }
        classicPlayer.skin = skin
        if (!classicPlayer.skin.assets) return
        if (modernLoader.item) modernLoader.item.hide()
        miniPlayer.hide()
        classicPlayer.show()
        classicPlayer.raise()
        classicPlayer.requestActivate()
        root.hide()
    }

    function quitKog() {
        applicationQuitRequested = true
        Qt.quit()
    }

    function treePathAtRow(row) {
        if (row < 0 || row >= directoryTree.rows)
            return ""
        return fileTreeModel.path_for_index(directoryTree.index(row, 0))
    }

    // Track the popup to the hovered row through the tree's scroll offset.
    // A destroyed delegate is only ever the sign of a stale hover, so the
    // tooltip goes with it.
    function refreshTreeHoverY() {
        if (treeHoverPath.length === 0 || !treeHoverItem)
            return
        try {
            treeHoverY = treeHoverItem.mapToItem(treeSection, 0, 0).y
        } catch (e) {
            treeHoverPath = ""
            treeHoverItem = null
        }
    }

    // JSON array of the visible tree rows that are currently expanded.
    // Collection must never break the UI: any failure reports an empty list.
    function collectTreeExpanded() {
        try {
            const expanded = []
            for (let row = 0; row < directoryTree.rows; ++row) {
                if (!directoryTree.isExpanded(row))
                    continue
                const path = treePathAtRow(row)
                if (path.length > 0 && expanded.indexOf(path) === -1)
                    expanded.push(path)
            }
            return JSON.stringify(expanded)
        } catch (error) {
            return "[]"
        }
    }

    // Re-expand the folders remembered by a previous run. The tree loads
    // children lazily, so this retries on a timer: each pass expands whatever
    // is visible and keeps the rest for the next one.
    function requestTreeExpandRestore() {
        try {
            const list = JSON.parse(appController.session_expanded_json)
            if (!Array.isArray(list)) {
                root.pendingTreeExpanded = []
                return
            }
            const paths = list.filter(path => typeof path === "string" && path.length > 0)
            paths.sort((left, right) => {
                const leftDepth = left.split("/").length
                const rightDepth = right.split("/").length
                return leftDepth - rightDepth || left.length - right.length
            })
            root.pendingTreeExpanded = paths
            root.treeExpandRestoreAttempts = 0
            if (paths.length > 0)
                treeExpandRestoreTimer.restart()
        } catch (error) {
            root.pendingTreeExpanded = []
        }
    }

    function clearTreeExpandRestore() {
        root.pendingTreeExpanded = []
        root.treeExpandRestoreAttempts = 0
        treeExpandRestoreTimer.stop()
    }

    // Reveal a playlist song in the file tree: show the pane, expand the
    // folders on the path (the restore timer retries while children load),
    // then select the song's row and scroll it into view.
    function showInFileTree(path) {
        if (path.length === 0)
            return
        if (!root.sidebarVisible)
            root.sidebarVisible = true
        const music = appController.directory_path.replace(/\/+$/, "")
        const cut = path.lastIndexOf("/")
        const folder = cut > 0 ? path.slice(0, cut) : "/"
        if (!path.startsWith(music)) {
            // Outside the music folder the tree can only root at the folder.
            root.useTreeRoot(folder)
            root.pendingShowPath = path
            root.showSelectAttempts = 0
            showSelectTimer.restart()
            return
        }
        const ancestors = []
        let cursor = folder
        while (cursor.length > music.length && cursor !== "/") {
            ancestors.push(cursor)
            const cut = cursor.lastIndexOf("/")
            cursor = cut > 0 ? cursor.slice(0, cut) : "/"
        }
        ancestors.reverse()
        root.pendingTreeExpanded = ancestors
        root.treeExpandRestoreAttempts = 0
        if (ancestors.length > 0)
            treeExpandRestoreTimer.restart()
        root.pendingShowPath = path
        root.showSelectAttempts = 0
        showSelectTimer.restart()
    }

    function trySelectPendingShow() {
        const path = root.pendingShowPath
        if (path.length === 0) {
            showSelectTimer.stop()
            return
        }
            try {
                for (let candidate = 0; candidate < directoryTree.rows; ++candidate) {
                    if (treePathAtRow(candidate) !== path)
                        continue
                    directoryTree.selectionModel.clear()
                    const index = directoryTree.index(candidate, 0)
                    directoryTree.selectionModel.select(index,
                        ItemSelectionModel.Select | ItemSelectionModel.Rows)
                    // Selecting does not scroll: a row far outside the
                    // viewport has no delegate yet, so the delegate's own
                    // centering cannot fire either. Pull the row in first;
                    // its delegate then finishes the centering once it exists.
                    directoryTree.positionViewAtRow(candidate, TableView.Contain)
                    // The delegate centers the pane; selection order matters, so
                    // the flag is only cleared after it has fired.
                    showSelectTimer.stop()
                    return
                }
            } catch (error) { /* the model may still be settling */ }
        root.showSelectAttempts += 1
        if (root.showSelectAttempts >= 40) {
            showSelectTimer.stop()
            root.pendingShowPath = ""
        }
    }

    function expandPendingTreeFolders() {
        if (root.pendingTreeExpanded.length === 0) {
            treeExpandRestoreTimer.stop()
            return
        }
        const remaining = []
        for (const path of root.pendingTreeExpanded) {
            let row = -1
            try {
                for (let candidate = 0; candidate < directoryTree.rows; ++candidate) {
                    if (treePathAtRow(candidate) === path) {
                        row = candidate
                        break
                    }
                }
            } catch (error) {
                row = -1
            }
            if (row === -1) {
                remaining.push(path)
                continue
            }
            if (!directoryTree.isExpanded(row))
                directoryTree.toggleExpanded(row)
        }
        root.pendingTreeExpanded = remaining
        root.treeExpandRestoreAttempts += 1
        if (remaining.length === 0 || root.treeExpandRestoreAttempts >= 40)
            treeExpandRestoreTimer.stop()
    }

    function setTreeSelection(paths, currentRow, anchorRow) {
        const requested = []
        for (const path of paths) {
            if (path.length > 0 && requested.indexOf(path) === -1)
                requested.push(path)
        }

        directoryTree.selectionModel.clear()
        const ordered = []
        for (let row = 0; row < directoryTree.rows; ++row) {
            const path = treePathAtRow(row)
            if (requested.indexOf(path) === -1)
                continue
            const index = directoryTree.index(row, 0)
            directoryTree.selectionModel.select(index,
                ItemSelectionModel.Select | ItemSelectionModel.Rows)
            ordered.push(path)
        }
        for (const path of requested) {
            if (ordered.indexOf(path) === -1)
                ordered.push(path)
        }
        treeSelectedPaths = ordered
        treeSelectionAnchorRow = ordered.length > 0 ? anchorRow : -1
        if (currentRow >= 0 && currentRow < directoryTree.rows)
            directoryTree.selectionModel.setCurrentIndex(
                directoryTree.index(currentRow, 0),
                ItemSelectionModel.NoUpdate)
    }

    function clearTreeSelection() {
        treeSelectedPaths = []
        treeSelectionAnchorRow = -1
        if (directoryTree.selectionModel)
            directoryTree.selectionModel.clear()
    }

    function selectTreeRow(row, path, modifiers) {
        const extend = (modifiers & Qt.ShiftModifier) !== 0
        const toggle = (modifiers
            & (Qt.ControlModifier | Qt.MetaModifier)) !== 0
        if (extend && treeSelectionAnchorRow >= 0) {
            const paths = toggle ? treeSelectedPaths.slice() : []
            const first = Math.min(treeSelectionAnchorRow, row)
            const last = Math.max(treeSelectionAnchorRow, row)
            for (let candidate = first; candidate <= last; ++candidate) {
                const candidatePath = treePathAtRow(candidate)
                if (candidatePath.length > 0
                        && paths.indexOf(candidatePath) === -1)
                    paths.push(candidatePath)
            }
            setTreeSelection(paths, row, treeSelectionAnchorRow)
            return false
        }
        if (toggle) {
            const paths = treeSelectedPaths.slice()
            const selectedIndex = paths.indexOf(path)
            if (selectedIndex === -1)
                paths.push(path)
            else
                paths.splice(selectedIndex, 1)
            setTreeSelection(paths, row, row)
            return false
        }
        if (treeSelectedPaths.length > 1
                && treeSelectedPaths.indexOf(path) !== -1)
            return true
        setTreeSelection([path], row, row)
        return false
    }

    function selectedTreePathsFor(path) {
        if (treeSelectedPaths.indexOf(path) === -1)
            return [path]
        return treeSelectedPaths.slice()
    }

    function addTreeSelection(path, activate) {
        const paths = selectedTreePathsFor(path)
        if (activate)
            appController.activate_local_paths_json(JSON.stringify(paths))
        else
            appController.add_local_paths_json(JSON.stringify(paths))
    }

    function isPlaylistRowSelected(row) {
        return selectedRows.indexOf(row) !== -1
    }

    function setPlaylistSelection(rows, current, anchor) {
        const unique = []
        for (const row of rows) {
            if (row >= 0 && row < appController.playlist_count
                    && unique.indexOf(row) === -1)
                unique.push(row)
        }
        unique.sort((left, right) => left - right)
        selectedRows = unique
        selectedRow = unique.length > 0
            ? (unique.indexOf(current) !== -1 ? current : unique[0])
            : -1
        selectionAnchor = unique.length > 0 ? anchor : -1
    }

    function selectPlaylistRow(row, modifiers) {
        const extend = (modifiers & Qt.ShiftModifier) !== 0
        const toggle = (modifiers & (Qt.ControlModifier | Qt.MetaModifier)) !== 0
        if (extend && selectionAnchor >= 0) {
            const first = Math.min(selectionAnchor, row)
            const last = Math.max(selectionAnchor, row)
            const rows = toggle ? selectedRows.slice() : []
            for (let index = first; index <= last; ++index) {
                if (rows.indexOf(index) === -1)
                    rows.push(index)
            }
            setPlaylistSelection(rows, row, selectionAnchor)
        } else if (toggle) {
            const rows = selectedRows.slice()
            const selectedIndex = rows.indexOf(row)
            if (selectedIndex === -1)
                rows.push(row)
            else
                rows.splice(selectedIndex, 1)
            setPlaylistSelection(rows, row, row)
        } else {
            setPlaylistSelection([row], row, row)
        }
        playlistView.forceActiveFocus()
    }

    function clearPlaylistSelection() {
        selectedRows = []
        selectedRow = -1
        selectionAnchor = -1
    }

    function isPlaylistSelected(pid) {
        return selectedPlaylistIds.indexOf(pid) !== -1
    }

    function setPlaylistSelectionById(ids) {
        const unique = []
        for (const pid of ids) {
            if (unique.indexOf(pid) === -1)
                unique.push(pid)
        }
        selectedPlaylistIds = unique
        if (renamingPlaylistId !== -2 && unique.indexOf(renamingPlaylistId) === -1)
            renamingPlaylistId = -2
    }

    function selectPlaylistById(pid, modifiers) {
        const items = playlistsModelItems()
        const row = items.findIndex(item => item.pid === pid)
        const extend = (modifiers & Qt.ShiftModifier) !== 0
        const toggle = (modifiers & (Qt.ControlModifier | Qt.MetaModifier)) !== 0
        if (extend && playlistSelectionAnchor >= 0) {
            const anchorRow = items.findIndex(item => item.pid === playlistSelectionAnchor)
            const first = Math.min(anchorRow < 0 ? row : anchorRow, row)
            const last = Math.max(anchorRow < 0 ? row : anchorRow, row)
            const ids = toggle ? selectedPlaylistIds.slice() : []
            for (let index = first; index <= last; ++index) {
                if (index >= 0 && index < items.length
                        && ids.indexOf(items[index].pid) === -1)
                    ids.push(items[index].pid)
            }
            setPlaylistSelectionById(ids)
            playlistSelectionAnchor = pid
        } else if (toggle) {
            const ids = selectedPlaylistIds.slice()
            const selectedIndex = ids.indexOf(pid)
            if (selectedIndex === -1)
                ids.push(pid)
            else
                ids.splice(selectedIndex, 1)
            setPlaylistSelectionById(ids)
            playlistSelectionAnchor = pid
        } else {
            setPlaylistSelectionById([pid])
            playlistSelectionAnchor = pid
        }
    }

    function clearPlaylistPickerSelection() {
        selectedPlaylistIds = []
        playlistSelectionAnchor = -1
        renamingPlaylistId = -2
    }

    function playlistsModelItems() {
        const items = []
        for (let index = 0; index < playlistsModel.count; ++index)
            items.push(playlistsModel.get(index))
        return items
    }

    function orderedSelectedPlaylistIds() {
        const order = {}
        playlistsModelItems().forEach((item, index) => {
            order[item.pid] = index
        })
        return selectedPlaylistIds.slice().sort((a, b) => order[a] - order[b])
    }

    function renamePlaylistById(pid, name) {
        try {
            const result = JSON.parse(appController.rename_playlist(pid, name))
            return result && result.ok !== false
        } catch (e) {
            return false
        }
    }

    // Header + button: prompt for a name, then save the selection when
    // rows are selected, otherwise the whole pane.
    function quickCreatePlaylist() {
        savePlaylistDialog.openFor(root.selectedRows.length > 0)
    }

    function enqueueSelectedPlaylists(startPlayback) {
        const ids = orderedSelectedPlaylistIds()
        if (ids.length === 0)
            return
        for (const pid of ids)
            appController.enqueue_playlist(pid, startPlayback && pid === ids[ids.length - 1])
    }

    function commitPlaylistDrag(draggedPid, insertIndex) {
        // UI index 0 is the pinned Favorites row: backend positions skip
        // it, and drops before/after the dragged row itself are no-ops.
        const items = playlistsModelItems()
        const from = items.findIndex(item => item.pid === draggedPid)
        const last = items.length
        if (from < 1 || insertIndex < 1 || insertIndex > last)
            return
        if (insertIndex === from || insertIndex === from + 1)
            return
        let toPosition
        if (insertIndex === last)
            toPosition = last // backend clamps to the end
        else if (from < insertIndex)
            toPosition = insertIndex - 2
        else
            toPosition = insertIndex - 1
        appController.move_playlist(draggedPid, toPosition)
    }

    function parsePlaylists() {
        try {
            const parsed = JSON.parse(appController.playlists_json())
            if (parsed && parsed.ok && Array.isArray(parsed.playlists))
                return parsed.playlists
        } catch (e) {}
        return []
    }

    function reloadPlaylists() {
        const items = parsePlaylists()
        const known = {}
        for (const item of items)
            known[item.id] = true
        playlistsModel.clear()
        for (const item of items) {
            playlistsModel.append({
                pid: item.id,
                name: item.name,
                entryCount: item.entryCount
            })
        }
        setPlaylistSelectionById(selectedPlaylistIds.filter(pid => known[pid]))
        if (renamingPlaylistId !== -2 && !known[renamingPlaylistId])
            renamingPlaylistId = -2
        if (playlistInsertIndex >= playlistsModel.count)
            playlistInsertIndex = -1
    }

    function removeSelectedTracks() {
        if (selectedRows.length === 0)
            return
        const next = appController.remove_tracks(selectedRows.join(","))
        if (next >= 0)
            setPlaylistSelection([next], next, next)
        else
            clearPlaylistSelection()
    }

    function keyboardSelectPlaylistRow(delta, modifiers) {
        if (appController.playlist_count === 0)
            return
        const start = selectedRow >= 0
            ? selectedRow
            : (delta > 0 ? -1 : appController.playlist_count)
        const target = Math.max(0, Math.min(start + delta,
            appController.playlist_count - 1))
        selectPlaylistRow(target, modifiers)
        playlistView.positionViewAtIndex(target, ListView.Contain)
    }

    function playlistDropIndex(y) {        const contentPosition = y + playlistView.contentY
        const row = playlistView.indexAt(1, contentPosition)
        if (row < 0)
            return contentPosition <= 0 ? 0 : appController.playlist_count
        const item = playlistView.itemAtIndex(row)
        return item && contentPosition >= item.y + item.height / 2 ? row + 1 : row
    }

    function applyMovedSelection(encodedRows) {
        if (encodedRows.length === 0) {
            clearPlaylistSelection()
            return
        }
        const rows = encodedRows.split(",").map(value => Number(value))
        setPlaylistSelection(rows, rows[0], rows[0])
    }

    AppController { id: appController }
    FileTreeModel { id: fileTreeModel }
    FileTreeModel { id: modernLibraryModel }

    Platform.SystemTrayIcon {
        id: trayIcon

        visible: appController.show_tray_icon && available
        tooltip: appController.now_title === "Not Playing"
            ? qsTr("Kog — Not Playing")
            : appController.now_title
                + (appController.now_artist.length > 0
                    ? "\n" + appController.now_artist : "")
                + "\n" + (appController.playback_state === "playing"
                    ? qsTr("Playing")
                    : (appController.playback_state === "paused"
                        ? qsTr("Paused") : qsTr("Stopped")))
                + qsTr(" — right-click for playback controls")
        icon.source: {
            if (appController.playback_state === "playing")
                return Qt.resolvedUrl("icons/kog-symbolic-play.svg")
            if (appController.playback_state === "paused")
                return Qt.resolvedUrl("icons/kog-symbolic-pause.svg")
            return Qt.resolvedUrl("icons/kog-symbolic.svg")
        }
        icon.mask: false
        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger
                    || reason === Platform.SystemTrayIcon.Unknown)
                Qt.callLater(function() { root.toggleFromTray() })
            else if (reason === Platform.SystemTrayIcon.DoubleClick)
                Qt.callLater(function() { root.showFromTray() })
        }
        menu: Platform.Menu {
            Platform.MenuItem {
                text: root.playerShowing ? qsTr("Hide Kog") : qsTr("Show Kog")
                icon.source: Qt.resolvedUrl("icons/kog.svg")
                onTriggered: Qt.callLater(function() {
                    root.toggleFromTray()
                })
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("Show Now Playing Notification")
                icon.name: "dialog-information"
                enabled: root.hasLoadedTrack
                onTriggered: appController.show_now_playing_notification()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: appController.playback_state === "playing"
                    ? qsTr("Pause") : qsTr("Play")
                icon.name: appController.playback_state === "playing"
                    ? "media-playback-pause" : "media-playback-start"
                enabled: root.transportReady
                onTriggered: appController.play_pause()
            }
            Platform.MenuItem {
                text: qsTr("Stop")
                icon.name: "media-playback-stop"
                enabled: root.hasLoadedTrack
                onTriggered: appController.stop()
            }
            Platform.MenuItem {
                text: qsTr("Previous")
                icon.name: "media-skip-backward"
                enabled: appController.playlist_count > 0
                onTriggered: appController.previous()
            }
            Platform.MenuItem {
                text: qsTr("Next")
                icon.name: "media-skip-forward"
                enabled: appController.playlist_count > 0 || appController.radio_active
                onTriggered: appController.next()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("About Kog…")
                icon.name: "help-about"
                onTriggered: aboutKog.open()
            }
            Platform.MenuItem {
                text: qsTr("Quit Kog")
                icon.name: "application-exit"
                onTriggered: root.quitKog()
            }
        }
    }

    Component.onCompleted: {
        fileTreeModel.set_root_path_text(appController.directory_path)
        root.requestTreeExpandRestore()
        // Warm the selected synth backend in the background so the first
        // MIDI track starts immediately instead of booting an emulator.
        appController.prewarm_synths()
        // The server switch is persistent: if it is switched on, serve with the
        // app rather than waiting for someone to visit Preferences.
        try {
            const settings = JSON.parse(appController.server_settings_json())
            const running = !!(settings && settings.status && settings.status.running)
            if (settings && settings.enabled && !running)
                appController.start_api_server()
        } catch (error) {
        }
    }

    Timer {
        interval: 200
        running: true
        repeat: true
        onTriggered: {
            appController.poll_playback()
            appController.poll_cover_art()
            appController.poll_radio()
            if (root.notificationSerialSeen !== appController.notification_serial) {
                root.notificationSerialSeen = appController.notification_serial
                nowPlayingPopup.present()
            }
            if (root.mprisRaiseSerialSeen !== appController.mpris_raise_serial) {
                root.mprisRaiseSerialSeen = appController.mpris_raise_serial
                root.showFromTray()
            }
        }
    }

    Timer {
        interval: 50
        running: true
        repeat: true
        onTriggered: appController.poll_audio_levels()
    }

    Timer {
        id: playlistSearchTimer

        interval: 90
        repeat: false
        onTriggered: {
            appController.filter_playlist(searchField.text)
            root.playlistHighlightQuery = searchField.text
            root.clearPlaylistSelection()
        }
    }

    Timer {
        id: playlistRenameTimer

        interval: 450
        repeat: false
        onTriggered: {
            if (root.playlistRenamePid >= 0)
                root.renamingPlaylistId = root.playlistRenamePid
        }
    }

    Timer {
        id: treeDeleteTimer

        interval: 100
        running: appController.tree_delete_active
        repeat: true
        onTriggered: {
            appController.poll_tree_delete()
            if (!appController.tree_delete_active && treeDeleteDialog.opened) {
                treeDeleteDialog.close()
                root.clearTreeSelection()
                root.clearPlaylistSelection()
            }
        }
    }

    Timer {
        id: directoryScanTimer

        interval: 35
        running: appController.directory_scan_active
        repeat: true
        onTriggered: {
            appController.poll_directory_scan()
            if (!appController.directory_scan_active
                    && directoryScanDialog.opened)
                directoryScanDialog.close()
        }
    }

    Timer {
        interval: 2000
        running: appController.directory_scan_active
        repeat: false
        onTriggered: if (appController.directory_scan_active)
            directoryScanDialog.open()
    }

    Timer {
        id: sessionFlushTimer

        interval: 1500
        running: true
        repeat: true
        onTriggered: {
            try {
                appController.flush_session(root.collectTreeExpanded())
            } catch (error) {
            }
        }
    }

    Timer {
        id: treeExpandRestoreTimer

        interval: 250
        running: false
        repeat: true
        onTriggered: root.expandPendingTreeFolders()
    }

    Timer {
        id: showSelectTimer

        interval: 300
        running: false
        repeat: true
        onTriggered: root.trySelectPendingShow()
    }

    InfoInspector { id: infoInspector; app: appController }
    TagEditor { id: tagEditor; app: appController }
    Equalizer { id: equalizerWindow; app: appController }
    Lyrics { id: lyricsWindow; app: appController }
    MiniPlayer {
        id: miniPlayer
        app: appController
        mainWindow: root
    }
    Preferences { id: preferences; app: appController }
    AboutKog { id: aboutKog; buildStamp: root.buildStamp }
    RemoteBrowser {
        id: remoteBrowser
        app: appController
        onOpenPlayer: root.showFromTray()
    }
    Shortcut {
        sequence: "F1"
        context: Qt.ApplicationShortcut
        onActivated: aboutKog.open()
    }
    Shortcut {
        sequence: "F5"
        enabled: root.sidebarVisible
        onActivated: fileTreeModel.refresh_tree()
    }
    SkinLibrary { id: skinLibrary }
    Timer { interval: 100; running: skinLibrary.busy; repeat: true; onTriggered: skinLibrary.poll() }
    SkinBrowser { id: skinBrowser; library: skinLibrary; onOpenClassic: root.showClassicPlayer() }
    Loader { id: modernLoader }
    Connections {
        target: modernLoader.item
        function onOpenGallery() { skinBrowser.show() }
        function onOpenEqualizer() { equalizerWindow.show() }
        function onOpenVisualizer() { visualizerWindow.show() }
    }
    ClassicPlayer {
        id: classicPlayer
        app: appController
        mainWindow: root
        onOpenGallery: skinBrowser.show()
        onOpenEqualizer: equalizerWindow.show()
        onOpenVisualizer: visualizerWindow.show()
    }
    Visualizer { id: visualizerWindow; app: appController }
    NowPlayingNotification {
        id: nowPlayingPopup
        app: appController
        screen: root.screen
        onOpenPlayer: root.showFromTray()
    }

    Dialog {
        id: directoryScanDialog

        anchors.centerIn: parent
        width: Math.min(520, root.width - 48)
        modal: true
        title: qsTr("Adding Music")
        closePolicy: Popup.NoAutoClose

        contentItem: ColumnLayout {
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                BusyIndicator {
                    Layout.preferredWidth: 32
                    Layout.preferredHeight: 32
                    running: appController.directory_scan_active
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Finding files and loading music in the background…")
                        font.bold: true
                        wrapMode: Text.Wrap
                    }
                    Label {
                        Layout.fillWidth: true
                        text: qsTr("%1 files scanned   •   %2 tracks added to queue")
                            .arg(appController.directory_scan_files_scanned)
                            .arg(appController.directory_scan_tracks_added)
                        color: root.palette.placeholderText
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                text: appController.directory_scan_current_path
                color: root.palette.placeholderText
                elide: Text.ElideMiddle
                font.pixelSize: 11
            }

            Button {
                Layout.alignment: Qt.AlignRight
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                enabled: appController.directory_scan_active
                onClicked: appController.cancel_directory_scan()
            }
        }
    }

    Dialog {
        id: treeDeleteConfirmDialog

        property var deletePaths: []

        function openFor(paths) {
            deletePaths = paths
            permanentCheckbox.checked = false
            open()
        }

        anchors.centerIn: parent
        width: Math.min(520, root.width - 48)
        modal: true
        title: qsTr("Delete Files")
        closePolicy: Popup.CloseOnEscape

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: permanentCheckbox.checked
                    ? qsTr("Permanently delete %n item(s)? This cannot be undone.", "", treeDeleteConfirmDialog.deletePaths.length)
                    : qsTr("Move %n item(s) to the system trash?", "", treeDeleteConfirmDialog.deletePaths.length)
                font.bold: true
                wrapMode: Text.WordWrap
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Repeater {
                    model: Math.min(6, treeDeleteConfirmDialog.deletePaths.length)

                    Label {
                        required property int index

                        Layout.fillWidth: true
                        text: treeDeleteConfirmDialog.deletePaths[index]
                        color: root.palette.placeholderText
                        font.pixelSize: 11
                        elide: Text.ElideMiddle
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: treeDeleteConfirmDialog.deletePaths.length > 6
                    text: qsTr("…and %1 more").arg(treeDeleteConfirmDialog.deletePaths.length - 6)
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }
            }
            CheckBox {
                id: permanentCheckbox

                text: qsTr("Delete permanently (cannot be undone)")
            }
            Label {
                Layout.fillWidth: true
                visible: !permanentCheckbox.checked
                text: qsTr("Trashed items can be restored from your file manager. Playlist entries for deleted files are removed as well.")
                color: root.palette.placeholderText
                font.pixelSize: 11
                wrapMode: Text.WordWrap
            }
        }

        footer: RowLayout {
            spacing: 8

            Item { Layout.fillWidth: true }
            Button {
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                onClicked: treeDeleteConfirmDialog.reject()
            }
            Button {
                text: permanentCheckbox.checked ? qsTr("Delete Permanently") : qsTr("Move to Trash")
                icon.name: "edit-delete"
                onClicked: treeDeleteConfirmDialog.accept()
            }
        }

        onAccepted: {
            if (appController.start_tree_delete(
                    JSON.stringify(treeDeleteConfirmDialog.deletePaths),
                    permanentCheckbox.checked))
                treeDeleteDialog.open()
        }
    }

    Dialog {
        id: treeDeleteDialog

        anchors.centerIn: parent
        width: Math.min(520, root.width - 48)
        modal: true
        title: qsTr("Deleting Files")
        closePolicy: Popup.NoAutoClose

        contentItem: ColumnLayout {
            spacing: 10

            ProgressBar {
                Layout.fillWidth: true
                from: 0
                to: Math.max(1, appController.tree_delete_total)
                value: appController.tree_delete_done
                Accessible.name: qsTr("Delete progress")
            }
            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: qsTr("%1 of %2").arg(appController.tree_delete_done).arg(appController.tree_delete_total)
                color: root.palette.placeholderText
            }
            Label {
                Layout.fillWidth: true
                text: appController.tree_delete_current_path
                color: root.palette.placeholderText
                elide: Text.ElideMiddle
                font.pixelSize: 11
            }
            Label {
                Layout.fillWidth: true
                visible: appController.tree_delete_error.length > 0
                text: appController.tree_delete_error
                wrapMode: Text.WordWrap
                font.pixelSize: 11
            }
            Button {
                Layout.alignment: Qt.AlignRight
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                enabled: appController.tree_delete_active
                onClicked: appController.cancel_tree_delete()
            }
        }
    }

    Dialog {
        id: coverArtDialog

        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        height: Math.min(560, root.height - 48)
        modal: true
        title: playbackTitle.text.length > 0 ? playbackTitle.text : qsTr("Album cover")
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        contentItem: ColumnLayout {
            spacing: 10

            Image {
                Layout.fillWidth: true
                Layout.fillHeight: true
                source: appController.current_artwork_path.length > 0
                    ? "file://" + appController.current_artwork_path
                    : Qt.resolvedUrl("icons/kog.svg")
                fillMode: Image.PreserveAspectFit
                mipmap: true
                asynchronous: true
                cache: false
                Accessible.name: qsTr("Album cover enlarged")

                MouseArea {
                    anchors.fill: parent
                    onClicked: coverArtDialog.close()
                }
            }
            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                visible: root.footer.trackSubtitle.length > 0
                text: root.footer.trackSubtitle
                color: root.palette.placeholderText
                font.pixelSize: 11
                elide: Text.ElideRight
            }
        }
    }

    Dialog {
        id: openUrlDialog

        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        modal: true
        title: qsTr("Add URL")
        standardButtons: Dialog.Ok | Dialog.Cancel
        closePolicy: Popup.CloseOnEscape
        function updateAcceptButton() {
            const button = standardButton(Dialog.Ok)
            if (button)
                button.enabled = urlField.acceptableInput
        }
        onOpened: {
            urlField.forceActiveFocus()
            urlField.selectAll()
            updateAcceptButton()
        }
        onAccepted: {
            appController.add_url(urlField.text.trim())
            urlField.text = ""
        }

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: qsTr("Enter an HTTP or HTTPS audio stream or HLS playlist URL.")
                wrapMode: Text.WordWrap
            }
            TextField {
                id: urlField

                Layout.fillWidth: true
                placeholderText: qsTr("https://example.com/music.m3u8")
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
                validator: RegularExpressionValidator {
                    regularExpression: /^https?:\/\/\S+$/i
                }
                onTextChanged: openUrlDialog.updateAcceptButton()
                onAccepted: if (acceptableInput) openUrlDialog.accept()
            }
        }
    }

    Dialog {
        id: savePlaylistDialog

        // False saves the whole pane, true saves the current selection.
        property bool forSelection: false
        property string pendingText: ""

        function openFor(useSelection) {
            forSelection = useSelection
            pendingText = ""
            open()
        }
        function updateAcceptButton() {
            const button = standardButton(Dialog.Ok)
            if (button)
                button.enabled = nameField.text.trim().length > 0
        }

        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        modal: true
        title: forSelection ? qsTr("Save Selection as Playlist") : qsTr("Save Playlist")
        standardButtons: Dialog.Ok | Dialog.Cancel
        closePolicy: Popup.CloseOnEscape
        onOpened: {
            nameField.text = pendingText
            nameField.forceActiveFocus()
            nameField.selectAll()
            updateAcceptButton()
        }
        onAccepted: {
            if (savePlaylistDialog.forSelection)
                appController.save_selection_as_playlist(
                    root.selectedRows.join(","), nameField.text.trim())
            else
                appController.save_pane_as_playlist(nameField.text.trim())
            nameField.text = ""
        }

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: savePlaylistDialog.forSelection
                    ? qsTr("Name the new playlist. The selected rows are saved into it.")
                    : qsTr("Name the new playlist. It appears at the bottom of the playlists list.")
                wrapMode: Text.WordWrap
            }
            TextField {
                id: nameField

                Layout.fillWidth: true
                placeholderText: qsTr("Playlist name")
                maximumLength: 120
                selectByMouse: true
                onTextChanged: savePlaylistDialog.updateAcceptButton()
                onAccepted: if (text.trim().length > 0) savePlaylistDialog.accept()
            }
        }
    }

    Dialog {
        id: duplicatePlaylistDialog
        property int sourcePid: -1
        property string pendingText: ""

        function openFor(suggested) {
            sourcePid = root.playlistMenuPid
            pendingText = suggested
            open()
        }
        function updateAcceptButton() {
            const button = standardButton(Dialog.Ok)
            if (button)
                button.enabled = duplicateNameField.text.trim().length > 0
        }

        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        modal: true
        title: qsTr("Duplicate Playlist")
        standardButtons: Dialog.Ok | Dialog.Cancel
        closePolicy: Popup.CloseOnEscape
        onOpened: {
            duplicateNameField.text = pendingText
            duplicateNameField.forceActiveFocus()
            duplicateNameField.selectAll()
            updateAcceptButton()
        }
        onAccepted: {
            appController.duplicate_playlist(
                sourcePid, duplicateNameField.text.trim())
            duplicateNameField.text = ""
        }

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: qsTr("Name the duplicate. It appears at the bottom of the playlists list.")
                wrapMode: Text.WordWrap
            }
            TextField {
                id: duplicateNameField

                Layout.fillWidth: true
                placeholderText: qsTr("Playlist name")
                maximumLength: 120
                selectByMouse: true
                onTextChanged: duplicatePlaylistDialog.updateAcceptButton()
                onAccepted: if (text.trim().length > 0) duplicatePlaylistDialog.accept()
            }
        }
    }

    Dialog {
        id: deletePlaylistConfirmDialog

        property var deleteIds: []

        function openFor(ids) {
            deleteIds = ids
            open()
        }

        anchors.centerIn: parent
        width: Math.min(480, root.width - 48)
        modal: true
        title: qsTr("Delete Playlists")
        closePolicy: Popup.CloseOnEscape
        onAccepted: {
            for (const pid of deleteIds)
                appController.delete_playlist(pid)
            deleteIds = []
        }

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: deletePlaylistConfirmDialog.deleteIds.length === 1
                    ? qsTr("Delete this playlist? Its songs stay in your library.")
                    : qsTr("Delete %n playlists? Their songs stay in your library.", "", deletePlaylistConfirmDialog.deleteIds.length)
                wrapMode: Text.WordWrap
            }
        }

        footer: RowLayout {
            spacing: 8

            Item { Layout.fillWidth: true }
            Button {
                text: qsTr("Cancel")
                icon.name: "dialog-cancel"
                onClicked: deletePlaylistConfirmDialog.reject()
            }
            Button {
                text: qsTr("Delete")
                icon.name: "edit-delete"
                onClicked: deletePlaylistConfirmDialog.accept()
            }
        }
    }


    Action {
        id: removeSelectedAction
        text: qsTr("Remove Selected")
        shortcut: StandardKey.Delete
        enabled: root.selectedRows.length > 0
        onTriggered: root.removeSelectedTracks()
    }

    Action {
        id: selectAllAction
        text: qsTr("Select All")
        shortcut: StandardKey.SelectAll
        enabled: playlistView.activeFocus && appController.playlist_count > 0
        onTriggered: {
            const rows = []
            for (let index = 0; index < appController.playlist_count; ++index)
                rows.push(index)
            root.setPlaylistSelection(rows, rows[0], rows[0])
        }
    }

    Action {
        id: savePlaylistAction
        text: qsTr("Save As…")
        icon.name: "document-save-as"
        shortcut: StandardKey.Save
        enabled: appController.playlist_count > 0
        onTriggered: appController.save_playlist()
    }

    Action {
        id: saveSelectionAction
        text: qsTr("Save Selection As…")
        icon.name: "document-save-as"
        enabled: root.selectedRows.length > 0
        onTriggered: appController.save_playlist_selection(
            root.selectedRows.join(","))
    }

    Action {
        id: editTagsAction
        text: qsTr("Edit Tags…")
        icon.name: "document-edit"
        shortcut: "Ctrl+Shift+E"
        enabled: root.selectedRows.length > 0
        onTriggered: tagEditor.openForRows(root.selectedRows)
    }

    Action {
        id: toggleQueueAction
        text: root.selectedQueueState === "all"
            ? qsTr("Remove from Queue")
            : (root.selectedQueueState === "mixed"
                ? qsTr("Toggle Queue")
                : qsTr("Add to Queue"))
        icon.name: root.selectedQueueState === "all"
            ? "list-remove"
            : "list-add"
        enabled: root.selectedRows.length > 0
        onTriggered: appController.toggle_queue(root.selectedRows.join(","))
    }

    Action {
        id: stopAfterSelectionAction
        text: root.selectedStopAfterState === "all"
            ? qsTr("Clear Stop After")
            : (root.selectedStopAfterState === "mixed"
                ? qsTr("Toggle Stop After")
                : qsTr("Stop After Selection"))
        icon.name: "media-playback-stop"
        enabled: root.selectedRows.length > 0
        onTriggered: appController.toggle_stop_after(root.selectedRows.join(","))
    }

    Action {
        id: clearQueueAction
        text: qsTr("Clear Queue")
        icon.name: "edit-clear-list"
        enabled: appController.queue_count > 0
        onTriggered: appController.clear_queue()
    }

    Action {
        id: clearPlaylistAction
        text: qsTr("Clear Playlist")
        icon.name: "edit-clear-list"
        enabled: appController.playlist_count > 0
        onTriggered: {
            appController.clear_playlist()
            root.clearPlaylistSelection()
        }
    }

    Menu {
        id: playlistContextMenu
        MenuItem {
            text: qsTr("Play")
            icon.name: "media-playback-start"
            enabled: root.selectedRow >= 0
            onTriggered: appController.play_index(root.selectedRow)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Show in File Tree")
            icon.name: "folder-open"
            enabled: root.selectedRow >= 0
            onTriggered: root.showInFileTree(
                String(appController.track_value_at(root.selectedRow, "path")))
        }
        MenuItem { action: toggleQueueAction }
        MenuItem { action: stopAfterSelectionAction }
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuSeparator {}
        MenuItem { action: saveSelectionAction }
        MenuItem { action: editTagsAction }
        MenuItem {
            text: qsTr("Blacklist Song")
            icon.name: "list-remove"
            enabled: root.selectedRows.length > 0
            onTriggered: appController.blacklist_pane_selection(
                root.selectedRows.join(","), false)
        }
        MenuItem {
            text: qsTr("Blacklist Folder")
            icon.name: "edit-delete"
            enabled: root.selectedRows.length > 0
            onTriggered: appController.blacklist_pane_selection(
                root.selectedRows.join(","), true)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Select All")
            icon.name: "edit-select-all"
            enabled: appController.playlist_count > 0
            onTriggered: selectAllAction.trigger()
        }
        MenuItem { action: clearPlaylistAction }
    }

    Menu {
        id: treeContextMenu

        MenuItem {
            text: qsTr("Use as Tree Root")
            icon.name: "folder-open"
            enabled: root.treeSelectedPaths.length <= 1
                && fileTreeModel.is_path_directory(root.treeContextPath)
            onTriggered: root.useTreeRoot(root.treeContextPath)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Add to Playlist")
            icon.name: "list-add"
            onTriggered: root.addTreeSelection(root.treeContextPath, false)
        }
        MenuItem {
            text: qsTr("Blacklist Song")
            icon.name: "list-remove"
            enabled: !fileTreeModel.is_path_directory(root.treeContextPath)
            onTriggered: appController.blacklist_tree_paths(
                JSON.stringify(root.selectedTreePathsFor(root.treeContextPath)), false)
        }
        MenuItem {
            text: qsTr("Blacklist Folder")
            icon.name: "edit-delete"
            enabled: fileTreeModel.is_path_directory(root.treeContextPath)
            onTriggered: appController.blacklist_tree_paths(
                JSON.stringify(root.selectedTreePathsFor(root.treeContextPath)), true)
        }
        MenuItem {
            text: qsTr("Delete…")
            icon.name: "edit-delete"
            enabled: root.treeSelectedPaths.length > 0
            onTriggered: {
                const paths = root.selectedTreePathsFor(root.treeContextPath)
                    .filter(path => path.length > 0)
                if (paths.length > 0)
                    treeDeleteConfirmDialog.openFor(paths)
            }
        }
    }

    Menu {
        id: playlistMenu

        MenuItem {
            text: qsTr("Add to Pane")
            icon.name: "list-add"
            onTriggered: {
                const ids = root.orderedSelectedPlaylistIds()
                for (const pid of ids)
                    appController.enqueue_playlist(pid, false)
            }
        }
        MenuItem {
            text: qsTr("Play")
            icon.name: "media-playback-start"
            onTriggered: {
                const ids = root.orderedSelectedPlaylistIds()
                for (let index = 0; index < ids.length; ++index)
                    appController.enqueue_playlist(ids[index], index === ids.length - 1)
            }
        }
        MenuItem {
            text: qsTr("Replace Pane")
            icon.name: "document-open"
            onTriggered: appController.load_playlist_into_pane(root.playlistMenuPid)
        }
        MenuItem {
            text: qsTr("Remove Missing Files")
            icon.name: "edit-clear"
            enabled: root.playlistMenuPid > 0
            onTriggered: appController.prune_missing_playlist_entries(root.playlistMenuPid)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Rename")
            icon.name: "edit-rename"
            enabled: root.playlistMenuPid > 0
            onTriggered: root.renamingPlaylistId = root.playlistMenuPid
        }
        MenuItem {
            text: qsTr("Duplicate")
            icon.name: "edit-copy"
            enabled: root.playlistMenuPid > 0
            onTriggered: {
                const row = root.playlistsModelItems()
                    .find(item => item.pid === root.playlistMenuPid)
                duplicatePlaylistDialog.openFor(
                    row ? row.name + qsTr(" copy") : "")
            }
        }
        MenuItem {
            text: qsTr("Export as m3u…")
            icon.name: "document-save-as"
            onTriggered: appController.export_playlist(root.playlistMenuPid)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Delete…")
            icon.name: "edit-delete"
            enabled: root.playlistMenuPid > 0
            onTriggered: {
                const ids = root.orderedSelectedPlaylistIds()
                    .filter(pid => pid !== 0)
                if (ids.length > 0)
                    deletePlaylistConfirmDialog.openFor(ids)
                else if (root.playlistMenuPid > 0)
                    deletePlaylistConfirmDialog.openFor([root.playlistMenuPid])
            }
        }
    }

    Menu {
        id: hamburgerMenu

        Action {
            text: qsTr("Add Files…")
            icon.name: "document-open"
            shortcut: StandardKey.Open
            onTriggered: appController.open_audio_files()
        }
        Action {
            text: qsTr("Add URL…")
            icon.name: "network-connect"
            shortcut: "Ctrl+Shift+O"
            onTriggered: openUrlDialog.open()
        }
        MenuItem {
            text: qsTr("Connect to Server…")
            icon.name: "network-server"
            onTriggered: {
                remoteBrowser.show()
                remoteBrowser.raise()
                remoteBrowser.requestActivate()
            }
        }
        MenuItem {
            text: qsTr("Choose Music Folder…")
            icon.name: "folder-open"
            onTriggered: root.chooseMusicFolder()
        }
        MenuSeparator {}
        MenuItem { action: savePlaylistAction }
        MenuItem { action: saveSelectionAction }
        MenuItem { action: editTagsAction }
        MenuSeparator {}
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuItem { action: clearPlaylistAction }
        MenuSeparator {}

        Menu {
            title: qsTr("View")
            icon.name: "view-visible"
            Action {
                text: qsTr("Show File Tree")
                icon.name: "view-list-tree"
                shortcut: "Ctrl+D"
                checkable: true
                checked: root.sidebarVisible
                onTriggered: root.sidebarVisible = checked
            }
            Action { text: qsTr("Show Info Inspector"); icon.name: "dialog-information"; shortcut: "Ctrl+I"; onTriggered: infoInspector.show() }
            Action { text: qsTr("Show Equalizer"); icon.name: "audio-equalizer"; shortcut: "Ctrl+E"; onTriggered: equalizerWindow.visible ? equalizerWindow.hide() : equalizerWindow.show() }
            Action { text: qsTr("Show Lyrics"); icon.name: "view-media-lyrics"; shortcut: "Ctrl+Shift+L"; onTriggered: lyricsWindow.show() }
            Action { text: qsTr("Show Mini Player"); icon.name: "view-restore"; shortcut: "Ctrl+Shift+M"; onTriggered: root.showMiniPlayer() }
            Action { text: qsTr("Winamp Skins…"); icon.name: "preferences-desktop-theme"; onTriggered: skinBrowser.show() }
            Action { text: qsTr("Visualizer"); icon.name: "view-media-visualization"; shortcut: "Ctrl+Shift+V"; onTriggered: visualizerWindow.visible ? visualizerWindow.hide() : visualizerWindow.show() }
        }

        Menu {
            title: qsTr("Playback")
            icon.name: "media-playback-start"
            Action { text: qsTr("Play/Pause"); icon.name: "media-playback-start"; shortcut: "Space"; onTriggered: appController.play_pause() }
            Action { text: qsTr("Stop"); icon.name: "media-playback-stop"; shortcut: "Ctrl+."; onTriggered: appController.stop() }
            MenuSeparator {}
            Action { text: qsTr("Previous"); icon.name: "media-skip-backward"; shortcut: "Ctrl+Left"; onTriggered: appController.previous() }
            Action { text: qsTr("Next"); icon.name: "media-skip-forward"; shortcut: "Ctrl+Right"; onTriggered: appController.next() }
            MenuSeparator {}
            Menu {
                title: qsTr("Shuffle")
                icon.name: "media-playlist-shuffle"
                Action {
                    text: qsTr("Off")
                    icon.name: "media-playlist-shuffle"
                    checkable: true
                    checked: appController.shuffle_mode === "off"
                    onTriggered: appController.select_shuffle_mode("off")
                }
                Action {
                    text: qsTr("Albums")
                    icon.name: "media-playlist-shuffle"
                    checkable: true
                    checked: appController.shuffle_mode === "albums"
                    onTriggered: appController.select_shuffle_mode("albums")
                }
                Action {
                    text: qsTr("All Tracks")
                    icon.name: "media-playlist-shuffle"
                    checkable: true
                    checked: appController.shuffle_mode === "all"
                    onTriggered: appController.select_shuffle_mode("all")
                }
            }
            Menu {
                title: qsTr("Repeat")
                icon.name: "media-playlist-repeat"
                Action {
                    text: qsTr("Off")
                    icon.name: "media-playlist-repeat"
                    checkable: true
                    checked: appController.repeat_mode === "off"
                    onTriggered: appController.select_repeat_mode("off")
                }
                Action {
                    text: qsTr("One Track")
                    icon.name: "media-playlist-repeat"
                    checkable: true
                    checked: appController.repeat_mode === "one"
                    onTriggered: appController.select_repeat_mode("one")
                }
                Action {
                    text: qsTr("Album")
                    icon.name: "media-playlist-repeat"
                    checkable: true
                    checked: appController.repeat_mode === "album"
                    onTriggered: appController.select_repeat_mode("album")
                }
                Action {
                    text: qsTr("All Tracks")
                    icon.name: "media-playlist-repeat"
                    checkable: true
                    checked: appController.repeat_mode === "all"
                    onTriggered: appController.select_repeat_mode("all")
                }
            }
            Action { text: qsTr("Reshuffle Radio"); icon.name: "view-refresh"; enabled: appController.radio_active; onTriggered: appController.reshuffle_radio() }
            MenuSeparator {}
            MenuItem { action: toggleQueueAction }
            MenuItem { action: stopAfterSelectionAction }
            MenuItem { action: clearQueueAction }
        }

        MenuSeparator {}
        Action { text: qsTr("Preferences…"); icon.name: "configure"; shortcut: "Ctrl+,"; onTriggered: preferences.show() }
        Action { text: qsTr("About Kog…"); icon.name: "help-about"; onTriggered: aboutKog.open() }
        Action { text: qsTr("Quit Kog"); icon.name: "application-exit"; shortcut: StandardKey.Quit; onTriggered: root.quitKog() }
    }

    header: ToolBar {
        id: mainToolbar

        implicitHeight: 48
        padding: 5
        palette.window: root.toolbarSurface
        palette.button: root.toolbarSurface
        palette.windowText: root.palette.text
        palette.buttonText: root.palette.text

        background: Rectangle {
            color: root.toolbarSurface
            border.width: 0

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 1
                color: root.palette.mid
            }
        }

        RowLayout {
            anchors.fill: parent
            spacing: 2

            RowLayout {
                visible: root.useMacWindowControls
                Layout.leftMargin: 5
                Layout.rightMargin: 6
                spacing: 7

                WindowButton {
                    buttonColor: "#ff5f57"
                    symbol: "×"
                    ToolTip.text: qsTr("Close")
                    onClicked: root.close()
                }
                WindowButton {
                    buttonColor: "#febc2e"
                    symbol: "−"
                    ToolTip.text: qsTr("Minimize")
                    onClicked: root.showMinimized()
                }
                WindowButton {
                    buttonColor: "#28c840"
                    symbol: root.visibility === Window.Maximized ? "−" : "+"
                    ToolTip.text: root.visibility === Window.Maximized
                        ? qsTr("Restore")
                        : qsTr("Maximize")
                    onClicked: root.visibility === Window.Maximized
                        ? root.showNormal()
                        : root.showMaximized()
                }
            }

            Item {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                Layout.alignment: Qt.AlignVCenter
                Accessible.name: qsTr("Kog")
                Accessible.role: Accessible.Button

                Image {
                    anchors.centerIn: parent
                    width: 28
                    height: 28
                    source: Qt.resolvedUrl("icons/kog.svg")
                    sourceSize.width: 56
                    sourceSize.height: 56
                    fillMode: Image.PreserveAspectFit
                    mipmap: true
                }

                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    Accessible.name: qsTr("About Kog")
                    Accessible.role: Accessible.Button
                    ToolTip.visible: containsMouse
                    ToolTip.text: qsTr("About Kog")
                    onClicked: aboutKog.open()
                }
            }
            ToolbarButton {
                id: hamburgerButton
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "☰"
                iconName: "application-menu"
                toolTip: hamburgerMenu.visible ? "" : qsTr("Kog menu")
                onClicked: hamburgerMenu.popup(hamburgerButton, 0, hamburgerButton.height)
            }
            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: root.sidebarVisible ? "«" : "»"
                iconName: "view-list-tree"
                checkable: true
                checked: root.sidebarVisible
                toolTip: root.sidebarVisible ? qsTr("Hide File Tree") : qsTr("Show File Tree")
                onToggled: root.sidebarVisible = checked
            }
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                TitleDragArea { anchors.fill: parent }

                Rectangle {
                    anchors.centerIn: parent
                    width: Math.max(140, Math.min(parent.width - 24, root.compactToolbar ? 340 : 460))
                    height: 34
                    radius: 17
                    color: root.palette.base
                    border.width: 1
                    border.color: searchField.activeFocus ? root.palette.highlight : root.palette.mid

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 5
                        spacing: 6

                        Image {
                            Layout.preferredWidth: 16
                            Layout.preferredHeight: 16
                            Layout.alignment: Qt.AlignVCenter
                            source: Qt.resolvedUrl("icons/edit-find" + (root.baseLuminance < 0.5 ? "-light" : "") + ".svg")
                            sourceSize.width: 32
                            sourceSize.height: 32
                            fillMode: Image.PreserveAspectFit
                            mipmap: true
                        }
                        TextField {
                            id: searchField
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            verticalAlignment: TextInput.AlignVCenter
                            placeholderText: qsTr("Search playlist")
                            selectByMouse: true
                            background: Item {}
                            onTextChanged: {
                                if (text.length === 0) {
                                    playlistSearchTimer.stop()
                                    appController.filter_playlist("")
                                    root.playlistHighlightQuery = ""
                                    root.clearPlaylistSelection()
                                } else {
                                    playlistSearchTimer.restart()
                                }
                            }
                            Keys.onEscapePressed: text = ""
                        }
                        ToolbarButton {
                            Layout.preferredWidth: 26
                            Layout.preferredHeight: 26
                            visible: searchField.text.length > 0
                            glyph: "×"
                            toolTip: qsTr("Clear playlist search")
                            onClicked: searchField.clear()
                        }
                    }
                }
            }

            RowLayout {
                visible: !root.useMacWindowControls
                Layout.leftMargin: 3
                Layout.rightMargin: 3
                spacing: 0

                DesktopWindowButton {
                    themedIconName: "window-minimize"
                    description: qsTr("Minimize")
                    onClicked: root.showMinimized()
                }
                DesktopWindowButton {
                    themedIconName: root.visibility === Window.Maximized
                        ? "window-restore"
                        : "window-maximize"
                    description: root.visibility === Window.Maximized
                        ? qsTr("Restore")
                        : qsTr("Maximize")
                    onClicked: root.visibility === Window.Maximized
                        ? root.showNormal()
                        : root.showMaximized()
                }
                DesktopWindowButton {
                    themedIconName: "window-close"
                    description: qsTr("Close")
                    onClicked: root.close()
                }
            }
        }
    }

    footer: Rectangle {
        implicitHeight: 92
        color: root.toolbarSurface
        border.width: 0

        // Which build is running, tucked into the lower-right corner beneath
        // the transport controls.
        Label {
            objectName: "buildStampFooter"
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.rightMargin: 10
            anchors.bottomMargin: 4
            visible: !root.compactToolbar && root.buildStamp.length > 0
            text: qsTr("v%1 · %2").arg(Qt.application.version).arg(root.buildStamp)
            font.pixelSize: 10
            color: Qt.darker(root.palette.placeholderText, 1.2)
            Accessible.name: text
            ToolTip.visible: buildStampCornerHover.hovered
            ToolTip.text: qsTr("Version %1, build %2")
                .arg(Qt.application.version).arg(root.buildStamp)

            HoverHandler { id: buildStampCornerHover }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 1
            color: root.palette.mid
        }

        readonly property string trackSubtitle: {
            const artist = appController.now_artist
            const album = appController.current_album
            if (artist.length > 0 && album.length > 0)
                return artist + "  •  " + album
            if (artist.length > 0)
                return artist
            if (album.length > 0)
                return album
            return qsTr("Ready to play")
        }
        readonly property bool transientStatus:
            appController.status.length > 0
            && appController.status !== "Drop audio files here or use the Kog menu to add files"

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            spacing: 12

            RowLayout {
                Layout.preferredWidth: root.compactToolbar ? 160 : 240
                Layout.maximumWidth: 260
                Layout.fillHeight: true
                spacing: 10

                Rectangle {
                    Layout.preferredWidth: 72
                    Layout.preferredHeight: 72
                    Layout.alignment: Qt.AlignVCenter
                    radius: 10
                    color: root.palette.alternateBase
                    border.width: 1
                    border.color: root.palette.mid

                    Image {
                        anchors.centerIn: parent
                        width: 60
                        height: 60
                        source: appController.current_artwork_path.length > 0
                            ? "file://" + appController.current_artwork_path
                            : Qt.resolvedUrl("icons/kog.svg")
                        sourceSize.width: 120
                        sourceSize.height: 120
                        fillMode: Image.PreserveAspectFit
                        mipmap: true
                        asynchronous: true
                        Accessible.name: qsTr("Album cover")
                    }

                    MouseArea {
                        anchors.fill: parent
                        enabled: appController.current_artwork_path.length > 0
                        cursorShape: Qt.PointingHandCursor
                        Accessible.name: qsTr("Show album cover enlarged")
                        Accessible.role: Accessible.Button
                        onClicked: coverArtDialog.open()
                    }

                    ToolTip.visible: coverArtHover.hovered
                        && appController.current_artwork_path.length > 0
                    ToolTip.delay: 500
                    ToolTip.text: qsTr("Show enlarged")

                    HoverHandler { id: coverArtHover }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: playbackTitle.text
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                        color: root.palette.text
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: !root.compactToolbar
                        text: root.footer.trackSubtitle
                        font.pixelSize: 11
                        color: root.palette.placeholderText
                        elide: Text.ElideRight
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumWidth: 220
                spacing: 4

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 2

                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "⇄"
                        iconName: "media-playlist-shuffle"
                        modeActive: appController.shuffle_mode !== "off"
                        badgeText: appController.shuffle_mode === "albums" ? "A"
                            : (appController.shuffle_mode === "all" ? "•" : "")
                        toolTip: appController.shuffle_mode === "off"
                            ? qsTr("Shuffle Off — click for Albums")
                            : (appController.shuffle_mode === "albums"
                                ? qsTr("Shuffle Albums — click for All Tracks")
                                : qsTr("Shuffle All Tracks — click to turn off"))
                        enabled: appController.playlist_count > 1
                        opacity: enabled ? (modeActive ? 1 : 0.62) : 0.38
                        onClicked: appController.cycle_shuffle_mode()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "◀"
                        iconName: "media-skip-backward"
                        toolTip: qsTr("Previous")
                        enabled: appController.playlist_count > 0
                        onClicked: appController.previous()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 40
                        Layout.preferredHeight: 40
                        glyph: appController.playback_state === "playing" ? "Ⅱ" : "▶"
                        iconName: appController.playback_state === "playing"
                            ? "media-playback-pause"
                            : "media-playback-start"
                        toolTip: qsTr("Play/Pause")
                        enabled: root.transportReady
                        onClicked: appController.play_pause()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "■"
                        iconName: "media-playback-stop"
                        toolTip: qsTr("Stop")
                        enabled: root.hasLoadedTrack
                        onClicked: appController.stop()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "▶"
                        iconName: "media-skip-forward"
                        toolTip: qsTr("Next")
                        enabled: appController.playlist_count > 0 || appController.radio_active
                        onClicked: appController.next()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "↻"
                        iconName: "media-playlist-repeat"
                        modeActive: appController.repeat_mode !== "off"
                        badgeText: appController.repeat_mode === "one" ? "1"
                            : (appController.repeat_mode === "album" ? "A"
                                : (appController.repeat_mode === "all" ? "∞" : ""))
                        toolTip: appController.repeat_mode === "off"
                            ? qsTr("Repeat Off — click for One Track")
                            : (appController.repeat_mode === "one"
                                ? qsTr("Repeat One Track — click for Album")
                                : (appController.repeat_mode === "album"
                                    ? qsTr("Repeat Album — click for All Tracks")
                                    : qsTr("Repeat All Tracks — click to turn off")))
                        enabled: appController.playlist_count > 0
                        opacity: enabled ? (modeActive ? 1 : 0.62) : 0.38
                        onClicked: appController.cycle_repeat_mode()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "⚄"
                        toolTip: qsTr("Random Radio")
                        checkable: true
                        checked: appController.radio_active
                        modeActive: appController.radio_active
                        opacity: appController.radio_active ? 1 : 0.62
                        onToggled: appController.set_radio_enabled(checked)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        Layout.preferredWidth: 48
                        horizontalAlignment: Text.AlignRight
                        text: root.timeLabel(appController.position_seconds)
                        color: root.palette.text
                        font.pixelSize: 11
                    }
                    Slider {
                        Layout.fillWidth: true
                        from: 0
                        to: Math.max(1, appController.duration_seconds)
                        value: appController.position_seconds
                        enabled: root.hasLoadedTrack
                        Accessible.name: qsTr("Playback position")
                        onMoved: appController.seek(value)
                    }
                    Label {
                        Layout.preferredWidth: 48
                        text: root.timeLabel(appController.duration_seconds)
                        color: root.palette.placeholderText
                        font.pixelSize: 11
                    }
                }
            }

            ColumnLayout {
                Layout.preferredWidth: root.compactToolbar ? 160 : 240
                Layout.maximumWidth: 260
                Layout.fillHeight: true
                Layout.alignment: Qt.AlignVCenter
                spacing: 4

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Item { Layout.fillWidth: true }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "×"
                        iconName: "edit-clear-list"
                        toolTip: qsTr("Clear Playlist")
                        enabled: clearPlaylistAction.enabled
                        onClicked: clearPlaylistAction.trigger()
                    }
                    ToolbarButton {
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 34
                        glyph: "♪"
                        iconName: "audio-volume-high"
                        toolTip: appController.volume <= 0 ? qsTr("Unmute") : qsTr("Mute")
                        onClicked: {
                            if (appController.volume <= 0)
                                appController.set_volume_level(
                                    root.volumeBeforeMute > 0 ? root.volumeBeforeMute : 0.75)
                            else {
                                root.volumeBeforeMute = appController.volume
                                appController.set_volume_level(0)
                            }
                        }
                    }
                    Slider {
                        id: volumeSlider
                        Layout.preferredWidth: root.compactToolbar ? 80 : 140
                        Layout.alignment: Qt.AlignVCenter
                        from: 0
                        to: 1
                        value: appController.volume
                        Accessible.name: qsTr("Volume")
                        onMoved: appController.set_volume_level(value)

                        // Percent readout while hovering or dragging.
                        HoverHandler { id: volumeHover }
                        ToolTip.visible: volumeHover.hovered || pressed
                        ToolTip.delay: 350
                        ToolTip.text: Math.round(value * 100) + qsTr("%")
                    }
                }

                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignRight
                    text: {
                        if (root.footer.transientStatus)
                            return appController.status
                        const count = appController.playlist_count
                        const tracks = count === 1
                            ? qsTr("1 track")
                            : qsTr("%1 tracks").arg(count)
                        return count > 0
                            ? tracks + " · " + appController.total_duration
                            : appController.total_duration
                    }
                    font.pixelSize: 10
                    color: root.palette.placeholderText
                    elide: Text.ElideRight
                }
            }
        }
    }

    PaneSplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal

        Rectangle {
            id: sidebarPane
            SplitView.preferredWidth: root.sidebarVisible ? root.sidebarWidth : 0
            SplitView.minimumWidth: root.sidebarVisible ? 170 : 0
            visible: root.sidebarVisible
            color: root.palette.window
            border.color: root.palette.mid
            onWidthChanged: {
                // Debounced: SplitView drags fire per pixel, but only
                // the settled width is worth persisting.
                if (root.sidebarVisible && width >= 170)
                    sidebarWidthSaver.restart()
            }

            Timer {
                id: sidebarWidthSaver
                interval: 800
                repeat: false
                onTriggered: {
                    if (root.sidebarVisible && sidebarPane.width >= 170)
                        root.sidebarWidth = sidebarPane.width
                }
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0


                SidebarSectionHeader {
                    sectionTitle: qsTr("Files")
                    sectionExpanded: root.treeSectionExpanded
                    onToggled: root.treeSectionExpanded = !root.treeSectionExpanded
                }

                ColumnLayout {
                    id: treeSection
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.treeSectionExpanded
                    spacing: 0

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 40
                        color: root.palette.button
                        border.color: root.palette.mid

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 7
                            anchors.rightMargin: 7
                            spacing: 5

                            CogButton {
                                Layout.preferredWidth: 30
                                Layout.preferredHeight: 30
                                glyph: "▣"
                                iconName: "folder-open"
                                toolTip: qsTr("Choose music folder")
                                onClicked: root.chooseMusicFolder()
                            }
                            CogButton {
                                Layout.preferredWidth: 30
                                Layout.preferredHeight: 30
                                glyph: "↻"
                                toolTip: qsTr("Refresh file tree")
                                onClicked: fileTreeModel.refresh_tree()
                            }
                            // The root path is elided when the pane is narrow;
                            // the tooltip carries all of it.
                            Label {
                                id: treeRootLabel
                                Layout.fillWidth: true
                                text: appController.directory_path
                                font.bold: true
                                elide: Text.ElideMiddle
                                Accessible.name: qsTr("Music folder: %1").arg(text)

                                MouseArea {
                                    id: treeRootHover
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.NoButton
                                }
                                ToolTip.visible: treeRootHover.containsMouse
                                    && text.length > 0
                                ToolTip.delay: 600
                                ToolTip.text: appController.directory_path
                            }
                        }
                    }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.leftMargin: 6
                    Layout.rightMargin: 6
                    Layout.topMargin: 6
                    Layout.bottomMargin: 6
                    Layout.preferredHeight: 34
                    radius: 17
                    color: root.palette.base
                    border.width: 1
                    border.color: treeSearchField.activeFocus ? root.palette.highlight : root.palette.mid

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 5
                        spacing: 6

                        Image {
                            Layout.preferredWidth: 16
                            Layout.preferredHeight: 16
                            Layout.alignment: Qt.AlignVCenter
                            source: Qt.resolvedUrl("icons/edit-find" + (root.baseLuminance < 0.5 ? "-light" : "") + ".svg")
                            sourceSize.width: 32
                            sourceSize.height: 32
                            fillMode: Image.PreserveAspectFit
                            mipmap: true
                        }
                        TextField {
                            id: treeSearchField
                            objectName: "treeSearchField"
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            verticalAlignment: TextInput.AlignVCenter
                            placeholderText: qsTr("Search files and folders…")
                            Accessible.name: qsTr("Search music folder, subfolders, and archive contents")
                            selectByMouse: true
                            maximumLength: 200
                            background: Item {}
                            onTextChanged: {
                                root.clearTreeSelection()
                                if (text.trim().length === 0) {
                                    treeSearchDebounce.stop()
                                    fileTreeModel.searchText = ""
                                } else {
                                    treeSearchDebounce.restart()
                                }
                            }
                            Keys.onEscapePressed: clear()
                        }
                        ToolbarButton {
                            Layout.preferredWidth: 26
                            Layout.preferredHeight: 26
                            // Stays while the spinner runs: a slow walk must
                            // never take away the way to abandon the query.
                            visible: treeSearchField.text.length > 0
                            glyph: "×"
                            toolTip: qsTr("Clear folder search")
                            onClicked: treeSearchField.clear()
                        }
                        // A plain Item, not a BusyIndicator: BusyIndicator
                        // always draws its own built-in spinner while
                        // visible, which turned under the gear no matter
                        // what the gear's own animation did — the
                        // ghost-spin under the paused gear.
                        Item {
                            id: treeSearchSpinner
                            objectName: "treeSearchSpinner"
                            Layout.preferredWidth: 18
                            Layout.preferredHeight: 18
                            visible: fileTreeModel.searching || treeSearchLayout.busy
                            opacity: fileTreeModel.searchPaused ? 0.55 : 1.0
                            Accessible.name: fileTreeModel.searchPaused
                                ? qsTr("Search paused. Click to resume.")
                                : qsTr("Searching files and archives. Click to pause.")

                            // The gear icon itself turns; an Image rotates
                            // about its own center, so there is no wobble.
                            // Paused stands it still and shows the badge.
                            Image {
                                id: treeSearchGear
                                anchors.fill: parent
                                source: Qt.resolvedUrl("icons/gear"
                                    + (root.baseLuminance < 0.5 ? "-light" : "") + ".svg")
                                sourceSize.width: 36
                                sourceSize.height: 36
                                fillMode: Image.PreserveAspectFit
                                mipmap: true
                                asynchronous: true

                                RotationAnimation on rotation {
                                    from: 0
                                    to: 360
                                    duration: 1600
                                    loops: Animation.Infinite
                                    running: treeSearchSpinner.visible
                                    // Freeze mid-turn instead of stopping:
                                    // a stopped property-source animation
                                    // gets re-driven whenever the running
                                    // binding's dependencies re-emit (the
                                    // search signals fire constantly), which
                                    // showed as a second gear turning under
                                    // the paused one.
                                    paused: fileTreeModel.searchPaused
                                }
                            }

                            Rectangle {
                                visible: fileTreeModel.searchPaused
                                width: 10
                                height: 10
                                radius: 2
                                color: root.palette.window
                                border.color: root.palette.mid
                                border.width: 1
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom

                                Row {
                                    spacing: 1
                                    anchors.centerIn: parent

                                    Rectangle {
                                        width: 1.5
                                        height: 4
                                        radius: 0.5
                                        color: root.palette.highlight
                                    }
                                    Rectangle {
                                        width: 1.5
                                        height: 4
                                        radius: 0.5
                                        color: root.palette.highlight
                                    }
                                }
                            }

                            // Clicking the spinner pauses the walk where it
                            // is and a second click resumes it.
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked:
                                    fileTreeModel.searchPaused =
                                        !fileTreeModel.searchPaused
                            }
                        }
                    }
                }
                Timer {
                    id: treeSearchDebounce
                    interval: 250
                    onTriggered: fileTreeModel.searchText = treeSearchField.text
                }
                Timer {
                    id: treeHoverClearTimer
                    interval: 150
                    onTriggered: {
                        root.treeHoverPath = ""
                        root.treeHoverItem = null
                    }
                }
                Connections {
                    target: fileTreeModel
                    function onSearchResultsChanged() {
                        root.clearTreeSelection()
                    }
                }
                TreeSearchLayout {
                    id: treeSearchLayout
                    view: directoryTree
                    model: fileTreeModel
                }
                Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: 8
                    Layout.rightMargin: 8
                    Layout.bottomMargin: visible ? 5 : 0
                    visible: fileTreeModel.searchStatus.length > 0
                    text: fileTreeModel.searchStatus
                    font.pointSize: root.font.pointSize * 0.9
                    wrapMode: Text.Wrap
                    opacity: 0.75
                }

                // The full path of a row cannot be read when it is elided, and a
                // ToolTip attached to the row is positioned in the row's own
                // scrolled content coordinates, which lands it outside the pane.
                // Show it in a popup parented to the pane instead: parenting it
                // to the tree itself put the popup in the tree's scrolled
                // coordinate space, where it wandered under the cursor and
                // fought the row hover (flicker: visible, empty, gone).
                Popup {
                    id: treePathTip
                    visible: root.treeHoverPath.length > 0
                    width: Math.min(tipLabel.implicitWidth + 18,
                        Math.max(120, treeSection.width - 16))
                    height: tipLabel.implicitHeight + 12
                    x: 4
                    y: Math.max(4, Math.min(treeSection.height - height - 4,
                        root.treeHoverY + 28))
                    modal: false
                    focus: false
                    closePolicy: Popup.NoAutoClose
                    padding: 0
                    opacity: 0.96

                    background: Rectangle {
                        radius: 5
                        color: root.palette.window
                        border.width: 1
                        border.color: root.palette.mid
                    }
                    contentItem: Label {
                        id: tipLabel
                        leftPadding: 9
                        rightPadding: 9
                        topPadding: 6
                        bottomPadding: 6
                        text: root.treeHoverPath
                        color: root.palette.text
                        font.pixelSize: 12
                        elide: Text.ElideMiddle
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                ItemDelegate {
                    id: parentDirectoryRow

                    Layout.fillWidth: true
                    Layout.preferredHeight: visible ? 28 : 0
                    visible: fileTreeModel.can_go_up
                    text: ".."
                    icon.name: "go-up"
                    icon.width: 18
                    icon.height: 18
                    leftPadding: 9
                    Accessible.name: qsTr("Go to parent folder")
                    ToolTip.visible: hovered && fileTreeModel.parent_path.length > 0
                    ToolTip.delay: 700
                    ToolTip.text: fileTreeModel.parent_path
                    onClicked: {
                        appController.parent_directory()
                        fileTreeModel.set_root_path_text(
                            appController.directory_path)
                        root.clearTreeSelection()
                        root.clearTreeExpandRestore()
                    }
                }

                TreeView {
                    id: directoryTree
                    opacity: treeSearchLayout.ready ? 1 : 0
                    enabled: opacity === 1
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: fileTreeModel
                    rootIndex: fileTreeModel.viewRootIndex
                    alternatingRows: true
                    selectionBehavior: TableView.SelectRows
                    selectionMode: TableView.ExtendedSelection
                    // Delegate handlers own pointer selection, activation,
                    // and drag arbitration. Disabling TreeView's hidden tap
                    // handler avoids competing exclusive grabs on Wayland.
                    pointerNavigationEnabled: false
                    selectionModel: ItemSelectionModel {
                        model: fileTreeModel
                    }
                    // Qt 6.10's TreeViewDelegate can retain the previous
                    // QFileSystemModel row when expansion shifts the flattened
                    // rows. Only visible delegates exist here, so disabling
                    // reuse is cheap and keeps labels, paths, and activation in
                    // lockstep.
                    reuseItems: false
                    boundsBehavior: Flickable.StopAtBounds
                    maximumFlickVelocity: 12000
                    flickDeceleration: 2200
                    onDraggingChanged: if (dragging)
                        directoryKineticWheel.stop()
                    onContentYChanged: root.refreshTreeHoverY()
                    readonly property real scrollGutter:
                        directoryScrollBar.visible
                            ? directoryScrollBar.implicitWidth + 4 : 0
                    columnWidthProvider: function(column) {
                        return Math.max(0, width - scrollGutter)
                    }

                    delegate: TreeViewDelegate {
                        id: treeDelegate
                        // Bind the QFileSystemModel role as a required delegate
                        // property. Accessing it through the transient `model`
                        // object leaves recycled TreeView delegates displaying
                        // and activating the preceding row after expansion.
                        required property string fileName
                        required property string filePath
                        required property string fileIcon
                        readonly property string dragPath: filePath
                        // A destroyed delegate must not leave the shared
                        // tooltip showing its (now stale) path.
                        Component.onDestruction: {
                            if (root.treeHoverItem === treeDelegate) {
                                root.treeHoverPath = ""
                                root.treeHoverItem = null
                            }
                        }
                        // Show in File Tree: when this row is the reveal
                        // target, center it. Creation and selection both land
                        // here, so the row scrolls no matter which fires.
                        readonly property bool revealTarget:
                            root.pendingShowPath === filePath && filePath.length > 0
                        onFilePathChanged: maybeRevealScroll()
                        onSelectedChanged: maybeRevealScroll()
                        Component.onCompleted: maybeRevealScroll()
                        function maybeRevealScroll() {
                            if (!revealTarget)
                                return
                            const center = mapToItem(directoryTree, 0, height / 2).y
                            directoryTree.contentY +=
                                center - directoryTree.height / 2
                            root.pendingShowPath = ""
                        }
                        width: Math.max(0,
                            directoryTree.width - directoryTree.scrollGutter)
                        implicitHeight: 26
                        icon.name: fileIcon
                        icon.width: 18
                        icon.height: 18
                        contentItem: RowLayout {
                            spacing: 5

                            ControlsImpl.IconImage {
                                Layout.preferredWidth: 18
                                Layout.preferredHeight: 18
                                name: treeDelegate.icon.name
                                sourceSize.width: 18
                                sourceSize.height: 18
                                fillMode: Image.PreserveAspectFit
                            }
                            SearchHighlightLabel {
                                Layout.fillWidth: true
                                sourceText: treeDelegate.fileName
                                query: fileTreeModel.searchText
                                searchModel: fileTreeModel
                                color: treeDelegate.selected
                                    ? treeDelegate.palette.highlightedText
                                    : treeDelegate.palette.text
                            }
                        }
                        // An explicit ToolTip item rather than the attached
                        // property: the attached form did not show inside this
                        // tree delegate at all.
                        MouseArea {
                            id: treePointer
                            property real pressX: 0
                            property real pressY: 0
                            property bool manualDragging: false
                            property bool collapseSelectionOnClick: false

                            anchors.fill: parent
                            z: 2
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            hoverEnabled: true
                            onEntered: {
                                treeHoverClearTimer.stop()
                                root.treeHoverItem = treeDelegate
                                root.treeHoverPath = treeDelegate.filePath.length > 0
                                    ? treeDelegate.filePath
                                    : root.treePathAtRow(treeDelegate.row)
                                root.treeHoverY = treeDelegate.mapToItem(
                                    treeSection, 0, 0).y
                            }
                            // Delay the clear: moving to a neighbouring row
                            // delivers this exit after that row's enter, and
                            // an immediate clear ate the fresh tooltip.
                            onExited: treeHoverClearTimer.restart()
                            preventStealing: true
                            scrollGestureEnabled: false

                            onPressed: mouse => {
                                pressX = mouse.x
                                pressY = mouse.y
                                manualDragging = false
                                if (mouse.button === Qt.RightButton) {
                                    root.treeContextPath = treeDelegate.dragPath
                                    if (root.treeSelectedPaths.indexOf(
                                            treeDelegate.dragPath) === -1)
                                        root.setTreeSelection(
                                            [treeDelegate.dragPath],
                                            treeDelegate.row, treeDelegate.row)
                                    treeContextMenu.popup()
                                    return
                                }
                                collapseSelectionOnClick = root.selectTreeRow(
                                    treeDelegate.row, treeDelegate.dragPath,
                                    mouse.modifiers)
                            }
                            onPositionChanged: mouse => {
                                // Keep the popup pinned to the row while the
                                // pointer moves within it.
                                root.treeHoverY = treeDelegate.mapToItem(
                                    treeSection, 0, 0).y
                                if ((mouse.buttons & Qt.LeftButton) === 0)
                                    return
                                if (!manualDragging
                                        && (Math.abs(mouse.x - pressX)
                                            >= Application.styleHints.startDragDistance
                                            || Math.abs(mouse.y - pressY)
                                            >= Application.styleHints.startDragDistance))
                                    manualDragging = true
                                if (!manualDragging)
                                    return
                                const point = mapToItem(playlistView,
                                    mouse.x, mouse.y)
                                root.playlistDropTarget = point.x >= 0
                                        && point.x <= playlistView.width
                                        && point.y >= 0
                                        && point.y <= playlistView.height
                                    ? root.playlistDropIndex(point.y)
                                    : -1
                            }
                            onClicked: mouse => {
                                if (mouse.button !== Qt.LeftButton)
                                    return
                                if (collapseSelectionOnClick)
                                    root.setTreeSelection(
                                        [treeDelegate.dragPath],
                                        treeDelegate.row, treeDelegate.row)
                                collapseSelectionOnClick = false
                                if ((mouse.modifiers
                                        & (Qt.ControlModifier
                                            | Qt.MetaModifier
                                            | Qt.ShiftModifier)) === 0
                                        && fileTreeModel.is_directory(
                                            directoryTree.index(treeDelegate.row, 0)))
                                    directoryTree.toggleExpanded(treeDelegate.row)
                            }
                            onDoubleClicked: mouse => {
                                // A folder double click only queues it —
                                // activating (clearing the pane and playing)
                                // is for a double clicked file.
                                const folder = fileTreeModel.is_directory(
                                    directoryTree.index(treeDelegate.row, 0))
                                if (mouse.button === Qt.LeftButton)
                                    root.addTreeSelection(
                                        treeDelegate.dragPath, !folder)
                            }
                            onReleased: mouse => {
                                if (manualDragging) {
                                    const point = mapToItem(playlistView,
                                        mouse.x, mouse.y)
                                    if (point.x >= 0
                                            && point.x <= playlistView.width
                                            && point.y >= 0
                                            && point.y <= playlistView.height)
                                        root.addTreeSelection(
                                            treeDelegate.dragPath, false)
                                    mouse.accepted = true
                                }
                                manualDragging = false
                                collapseSelectionOnClick = false
                                root.playlistDropTarget = -1
                            }
                            onCanceled: {
                                manualDragging = false
                                collapseSelectionOnClick = false
                                root.playlistDropTarget = -1
                            }
                        }
                    }

                    ScrollBar.vertical: ScrollBar {
                        id: directoryScrollBar
                        policy: ScrollBar.AsNeeded
                        onPressedChanged: if (pressed)
                            directoryKineticWheel.stop()
                    }

                    KineticWheelHandler {
                        id: directoryKineticWheel
                        view: directoryTree
                    }
                }
                }

                SidebarSectionHeader {
                    sectionTitle: qsTr("Playlists")
                    sectionExpanded: root.playlistsSectionExpanded
                    showAddButton: true
                    addToolTip: root.selectedRows.length > 0
                        ? qsTr("Create playlist from selection")
                        : qsTr("Create playlist from this pane")
                    onToggled: root.playlistsSectionExpanded = !root.playlistsSectionExpanded
                    onAddClicked: root.quickCreatePlaylist()
                }

                Item {
                    id: playlistsSection
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(playlistsList.contentHeight + 8, 10000)
                    Layout.maximumHeight: root.treeSectionExpanded ? 260 : 10000
                    Layout.fillHeight: !root.treeSectionExpanded
                    visible: root.playlistsSectionExpanded
                    clip: true

                    readonly property var playlistsData: {
                        appController.playlists_revision
                        return root.parsePlaylists()
                    }
                    onPlaylistsDataChanged: root.reloadPlaylists()

                    ListModel {
                        id: playlistsModel
                    }

                    ListView {
                        id: playlistsList
                        anchors.fill: parent
                        anchors.leftMargin: 4
                        anchors.rightMargin: 4
                        anchors.topMargin: 4
                        anchors.bottomMargin: 4
                        clip: true
                        model: playlistsModel
                        spacing: 1

                        delegate: Item {
                            id: playlistRow
                            required property int index
                            required property int pid
                            required property string name
                            required property int entryCount
                            width: playlistsList.width
                            height: 30

                            readonly property bool isFavorite: pid === 0
                            readonly property bool isSelected:
                                root.isPlaylistSelected(pid)
                            readonly property bool renaming:
                                root.renamingPlaylistId === pid

                            Rectangle {
                                anchors.fill: parent
                                radius: 4
                                visible: playlistRow.isSelected || rowHover.hovered
                                color: playlistRow.isSelected
                                    ? root.palette.highlight
                                    : root.palette.button
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 6

                                Image {
                                    visible: playlistRow.isFavorite
                                    Layout.preferredWidth: 14
                                    Layout.preferredHeight: 14
                                    Layout.alignment: Qt.AlignVCenter
                                    source: Qt.resolvedUrl("icons/star-filled.svg")
                                    sourceSize.width: 28
                                    sourceSize.height: 28
                                    fillMode: Image.PreserveAspectFit
                                    mipmap: true
                                    Accessible.name: qsTr("Favorites")
                                }
                                Label {
                                    Layout.fillWidth: true
                                    visible: !playlistRow.renaming
                                    text: playlistRow.isFavorite
                                        ? qsTr("Favorites")
                                        : playlistRow.name
                                    font.pixelSize: 12
                                    color: playlistRow.isSelected
                                        ? root.palette.highlightedText
                                        : root.palette.text
                                    elide: Text.ElideRight
                                }
                                TextField {
                                    Layout.fillWidth: true
                                    visible: playlistRow.renaming
                                    text: playlistRow.name
                                    font.pixelSize: 12
                                    selectByMouse: true
                                    maximumLength: 120
                                    background: Item {}
                                    onVisibleChanged: if (visible) {
                                        forceActiveFocus()
                                        selectAll()
                                    }
                                    onAccepted: {
                                        const result = root.renamePlaylistById(
                                            playlistRow.pid, text)
                                        if (result)
                                            root.renamingPlaylistId = -2
                                    }
                                    Keys.onEscapePressed:
                                        root.renamingPlaylistId = -2
                                    onActiveFocusChanged: if (!activeFocus
                                            && playlistRow.renaming) {
                                        const result = root.renamePlaylistById(
                                            playlistRow.pid, text)
                                        if (result)
                                            root.renamingPlaylistId = -2
                                    }
                                }
                                Label {
                                    visible: !playlistRow.renaming
                                    text: String(playlistRow.entryCount)
                                    font.pixelSize: 11
                                    color: root.palette.placeholderText
                                }
                            }

                            HoverHandler {
                                id: rowHover
                            }

                            MouseArea {
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                hoverEnabled: true
                                onPressed: mouse => {
                                    pressX = mouse.x
                                    pressY = mouse.y
                                    dragging = false
                                    if (mouse.button === Qt.RightButton) {
                                        if (!root.isPlaylistSelected(playlistRow.pid)) {
                                            root.setPlaylistSelectionById([playlistRow.pid])
                                            root.playlistSelectionAnchor = playlistRow.pid
                                        }
                                        root.playlistMenuPid = playlistRow.pid
                                        playlistMenu.popup()
                                    }
                                }
                                onPositionChanged: mouse => {
                                    if ((mouse.buttons & Qt.LeftButton) === 0
                                            || playlistRow.isFavorite)
                                        return
                                    if (!dragging
                                            && (Math.abs(mouse.x - pressX)
                                                >= Application.styleHints.startDragDistance
                                                || Math.abs(mouse.y - pressY)
                                                >= Application.styleHints.startDragDistance)) {
                                        dragging = true
                                        playlistRenameTimer.stop()
                                        playlistsList.interactive = false
                                        if (!root.isPlaylistSelected(playlistRow.pid)) {
                                            root.setPlaylistSelectionById([playlistRow.pid])
                                            root.playlistSelectionAnchor = playlistRow.pid
                                        }
                                    }
                                    if (!dragging)
                                        return
                                    const panePoint = mapToItem(playlistView,
                                        mouse.x, mouse.y)
                                    if (panePoint.x >= 0
                                            && panePoint.x <= playlistView.width
                                            && panePoint.y >= 0
                                            && panePoint.y <= playlistView.height) {
                                        root.playlistDropTarget =
                                            root.playlistDropIndex(panePoint.y)
                                        root.playlistInsertIndex = -1
                                    } else {
                                        root.playlistDropTarget = -1
                                        const listPoint = mapToItem(playlistsList,
                                            mouse.x, mouse.y)
                                        const row = Math.floor(
                                            (listPoint.y + playlistsList.contentY) / 31)
                                        // Favorites at row 0 never moves.
                                        root.playlistInsertIndex = Math.max(1,
                                            Math.min(row, playlistsModel.count))
                                    }
                                }
                                onClicked: mouse => {
                                    if (mouse.button !== Qt.LeftButton)
                                        return
                                    if (root.isPlaylistSelected(playlistRow.pid)
                                            && mouse.modifiers === Qt.NoModifier) {
                                        if (playlistRow.pid === 0)
                                            return
                                        root.playlistRenamePid = playlistRow.pid
                                        playlistRenameTimer.restart()
                                    } else {
                                        root.selectPlaylistById(
                                            playlistRow.pid, mouse.modifiers)
                                    }
                                }
                                onDoubleClicked: mouse => {
                                    if (mouse.button !== Qt.LeftButton)
                                        return
                                    // Add to the pane without touching
                                    // playback: double-clicking a playlist
                                    // must never interrupt the current song.
                                    // Playback stays on the context menu.
                                    playlistRenameTimer.stop()
                                    root.renamingPlaylistId = -2
                                    root.enqueueSelectedPlaylists(false)
                                }
                                onReleased: mouse => {
                                    if (!dragging)
                                        return
                                    dragging = false
                                    playlistsList.interactive = true
                                    if (root.playlistDropTarget >= 0) {
                                        root.enqueueSelectedPlaylists(false)
                                    } else if (root.playlistInsertIndex >= 0) {
                                        root.commitPlaylistDrag(
                                            playlistRow.pid, root.playlistInsertIndex)
                                    }
                                    root.playlistDropTarget = -1
                                    root.playlistInsertIndex = -1
                                }
                                onCanceled: {
                                    dragging = false
                                    playlistsList.interactive = true
                                    root.playlistDropTarget = -1
                                    root.playlistInsertIndex = -1
                                }

                                property real pressX: 0
                                property real pressY: 0
                                property bool dragging: false
                            }
                        }

                        Rectangle {
                            id: playlistInsertIndicator
                            width: parent.width - 8
                            x: 4
                            height: 2
                            radius: 1
                            color: root.palette.highlight
                            visible: root.playlistInsertIndex >= 0
                            y: root.playlistInsertIndex * 31 - playlistsList.contentY
                        }
                    }
                }
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: !root.treeSectionExpanded
                        && !root.playlistsSectionExpanded
                }

            }
        }

        Rectangle {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 320
            color: root.palette.base

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Item {
                    id: playlistHeaderViewport
                    Layout.fillWidth: true
                    Layout.rightMargin: playlistView.verticalScrollGutter
                    Layout.preferredHeight: playlistHeader.implicitHeight
                    clip: true

                    PlaylistHeader {
                        id: playlistHeader
                        x: -playlistView.contentX
                        width: Math.max(playlistHeaderViewport.width, totalWidth)
                        height: implicitHeight
                        availableWidth: playlistHeaderViewport.width
                        theme: root.palette
                        app: appController
                        savedLayout: appController.playlist_column_layout
                        sortColumn: appController.playlist_sort_column
                        sortAscending: appController.playlist_sort_ascending
                        onSortRequested: column => {
                            const selected = appController.sort_playlist(
                                column, root.selectedRows.join(","))
                            root.applyMovedSelection(selected)
                        }
                        onColumnLayoutChanged: layout =>
                            appController.save_playlist_column_layout(layout)
                    }
                }

                ListView {
                    id: playlistView
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: appController.playlist_count
                    reuseItems: true
                    cacheBuffer: Math.min(height * 1.5, 1200)
                    boundsBehavior: Flickable.StopAtBounds
                    readonly property real verticalScrollGutter:
                        playlistVerticalScrollBar.visible
                            ? playlistVerticalScrollBar.implicitWidth + 4 : 0
                    readonly property real horizontalScrollGutter:
                        playlistHorizontalScrollBar.visible
                            ? playlistHorizontalScrollBar.implicitHeight + 4 : 0
                    contentWidth: Math.max(width,
                        playlistHeader.totalWidth + verticalScrollGutter)
                    flickableDirection: Flickable.AutoFlickDirection
                    currentIndex: root.selectedRow
                    keyNavigationEnabled: true
                    focus: true
                    highlightMoveDuration: 0
                    maximumFlickVelocity: 12000
                    flickDeceleration: 2200
                    onDraggingChanged: if (dragging) {
                        playlistKineticWheel.stop()
                        playlistHorizontalWheel.stop()
                    }

                    Keys.onReturnPressed: if (root.selectedRow >= 0)
                        appController.play_index(root.selectedRow)
                    Keys.onEnterPressed: if (root.selectedRow >= 0)
                        appController.play_index(root.selectedRow)
                    Keys.onDeletePressed: removeSelectedAction.trigger()
                    Keys.onUpPressed: event => {
                        root.keyboardSelectPlaylistRow(-1, event.modifiers)
                        event.accepted = true
                    }
                    Keys.onDownPressed: event => {
                        root.keyboardSelectPlaylistRow(1, event.modifiers)
                        event.accepted = true
                    }

                    delegate: PlaylistRow {
                        required property int index
                        width: Math.max(0, playlistView.contentWidth
                            - playlistView.verticalScrollGutter)
                        app: appController
                        columns: playlistHeader
                        searchModel: fileTreeModel
                        searchQuery: root.playlistHighlightQuery
                        theme: root.palette
                        rowIndex: index
                        selected: root.isPlaylistRowSelected(index)
                        onPressed: (row, modifiers, button) => {
                            if (button !== Qt.RightButton
                                    || !root.isPlaylistRowSelected(row))
                                root.selectPlaylistRow(row, modifiers)
                            if (button === Qt.RightButton) {
                                playlistView.forceActiveFocus()
                                playlistContextMenu.popup()
                            }
                        }
                        onActivated: row => {
                            root.setPlaylistSelection([row], row, row)
                            appController.activate_playlist_index(row)
                        }
                        onDragStarted: row => {
                            if (!root.isPlaylistRowSelected(row))
                                root.setPlaylistSelection([row], row, row)
                        }
                        onDragMoved: (viewX, viewY) => {
                            root.playlistDropTarget = viewX >= 0
                                    && viewX <= playlistView.width
                                    && viewY >= 0
                                    && viewY <= playlistView.height
                                ? root.playlistDropIndex(viewY)
                                : -1
                        }
                        onDragFinished: (viewX, viewY) => {
                            if (viewX >= 0 && viewX <= playlistView.width
                                    && viewY >= 0
                                    && viewY <= playlistView.height) {
                                const target = root.playlistDropTarget >= 0
                                    ? root.playlistDropTarget
                                    : root.playlistDropIndex(viewY)
                                const moved = appController.move_tracks(
                                    root.selectedRows.join(","), target)
                                root.applyMovedSelection(moved)
                            }
                            root.playlistDropTarget = -1
                        }
                        onDragCanceled: root.playlistDropTarget = -1
                    }

                    ScrollBar.vertical: ScrollBar {
                        id: playlistVerticalScrollBar
                        policy: ScrollBar.AsNeeded
                        onPressedChanged: if (pressed) {
                            playlistKineticWheel.stop()
                            playlistHorizontalWheel.stop()
                        }
                    }
                    ScrollBar.horizontal: ScrollBar {
                        id: playlistHorizontalScrollBar
                        policy: ScrollBar.AsNeeded
                        onPressedChanged: if (pressed)
                            playlistHorizontalWheel.stop()
                    }

                    footer: Item {
                        width: 1
                        height: playlistView.horizontalScrollGutter
                    }

                    // Physical mouse wheels use Kog's per-frame kinetic
                    // motion on both axes. Touchpad gestures keep their
                    // native pixel precision per axis the same way.
                    KineticWheelHandler {
                        id: playlistKineticWheel
                        view: playlistView
                    }
                    KineticWheelHandler {
                        id: playlistHorizontalWheel
                        view: playlistView
                        orientation: Qt.Horizontal
                    }

                    Item {
                        anchors.fill: parent
                        visible: appController.playlist_count === 0
                        z: -1

                        Repeater {
                            model: Math.ceil(parent.height / 24)

                            Rectangle {
                                required property int index
                                x: 6
                                y: index * 24 + 3
                                width: Math.max(0, parent.width - 12
                                    - playlistView.verticalScrollGutter)
                                height: 18
                                radius: 4
                                visible: index % 2 === 1
                                color: root.palette.alternateBase
                            }
                        }
                    }
                }
            }

            Rectangle {
                z: 20
                x: 6
                width: parent.width - 12
                height: 2
                radius: 1
                color: root.palette.highlight
                visible: root.playlistDropTarget >= 0
                y: Math.max(playlistHeaderViewport.height,
                    Math.min(parent.height - height,
                        playlistHeaderViewport.height + root.playlistDropTarget * 24
                            - playlistView.contentY))
            }

            DropArea {
                x: 0
                y: playlistHeaderViewport.height
                width: parent.width
                height: parent.height - y
                onEntered: drag => {
                    if (drag.hasUrls
                            || drag.formats.indexOf("text/uri-list") !== -1)
                        root.playlistDropTarget = root.playlistDropIndex(drag.y)
                }
                onPositionChanged: drag => {
                    if (drag.hasUrls
                            || drag.formats.indexOf("text/uri-list") !== -1)
                        root.playlistDropTarget = root.playlistDropIndex(drag.y)
                }
                onExited: root.playlistDropTarget = -1
                onDropped: drop => {
                    const urls = []
                    if (!drop.hasUrls) {
                        if (drop.formats.indexOf("text/uri-list") === -1)
                            return
                        const uriList = drop.getDataAsString("text/uri-list")
                            .split(/\r?\n/).filter(value => value.length > 0)
                        for (const url of uriList)
                            urls.push(url)
                    } else {
                        for (const url of drop.urls)
                            urls.push(url.toString())
                    }
                    appController.enqueue_urls_json(JSON.stringify(urls))
                    drop.acceptProposedAction()
                    root.playlistDropTarget = -1
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        z: 9000
        color: "transparent"
        border.width: 1
        border.color: root.palette.mid
    }

    ResizeHandle {
        z: 10000
        edges: Qt.LeftEdge
        width: 5
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
        cursorShape: Qt.SizeHorCursor
    }
    ResizeHandle {
        z: 10000
        edges: Qt.RightEdge
        width: 5
        anchors { right: parent.right; top: parent.top; bottom: parent.bottom }
        cursorShape: Qt.SizeHorCursor
    }
    ResizeHandle {
        z: 10000
        edges: Qt.TopEdge
        height: 5
        anchors { left: parent.left; top: parent.top; right: parent.right }
        cursorShape: Qt.SizeVerCursor
    }
    ResizeHandle {
        z: 10000
        edges: Qt.BottomEdge
        height: 5
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        cursorShape: Qt.SizeVerCursor
    }
    ResizeHandle {
        z: 10001
        edges: Qt.TopEdge | Qt.LeftEdge
        width: 9
        height: 9
        anchors { left: parent.left; top: parent.top }
        cursorShape: Qt.SizeFDiagCursor
    }
    ResizeHandle {
        z: 10001
        edges: Qt.TopEdge | Qt.RightEdge
        width: 9
        height: 9
        anchors { right: parent.right; top: parent.top }
        cursorShape: Qt.SizeBDiagCursor
    }
    ResizeHandle {
        z: 10001
        edges: Qt.BottomEdge | Qt.LeftEdge
        width: 9
        height: 9
        anchors { left: parent.left; bottom: parent.bottom }
        cursorShape: Qt.SizeBDiagCursor
    }
    ResizeHandle {
        z: 10001
        edges: Qt.BottomEdge | Qt.RightEdge
        width: 9
        height: 9
        anchors { right: parent.right; bottom: parent.bottom }
        cursorShape: Qt.SizeFDiagCursor
    }

}
