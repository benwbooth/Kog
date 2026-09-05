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
    title: appController.now_title === "Not Playing" ? qsTr("Kog") : appController.now_title + " — Kog"
    color: palette.window

    property bool sidebarVisible: false
    property bool searchVisible: false
    property int selectedRow: -1
    property int selectionAnchor: -1
    property var selectedRows: []
    property int playlistDropTarget: -1
    property real volumeBeforeMute: 0.75
    property int mprisRaiseSerialSeen: 0
    property int notificationSerialSeen: 0
    property bool applicationQuitRequested: false
    property var treeSelectedPaths: []
    property int treeSelectionAnchorRow: -1
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
    readonly property bool compactToolbar: width < 980
    readonly property bool useMacWindowControls: Qt.platform.os === "osx"
    readonly property bool playerShowing: (root.visible
        && root.visibility !== Window.Hidden
        && root.visibility !== Window.Minimized)
        || (miniPlayer.visible
            && miniPlayer.visibility !== Window.Hidden
            && miniPlayer.visibility !== Window.Minimized)
        || (classicPlayer.visible && classicPlayer.visibility !== Window.Minimized)
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
            onActiveChanged: if (active) root.startSystemMove()
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
    }

    function useTreeRoot(path) {
        if (!fileTreeModel.is_path_directory(path))
            return
        appController.choose_directory(fileTreeModel.path_url(path))
        fileTreeModel.set_root_path_text(appController.directory_path)
        clearTreeSelection()
    }

    function showFromTray() {
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
            return
        }
        root.showFromTray()
    }

    function showMiniPlayer() {
        classicPlayer.hide()
        miniPlayer.show()
        miniPlayer.raise()
        miniPlayer.requestActivate()
        root.hide()
    }

    function showClassicPlayer() {
        classicPlayer.skin = JSON.parse(skinLibrary.active_json)
        if (!classicPlayer.skin.assets) return
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

    function playlistDropIndex(y) {
        const contentPosition = y + playlistView.contentY
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
        icon.source: Qt.resolvedUrl("icons/kog-symbolic.svg")
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
                enabled: appController.current_index >= 0
                    && appController.playback_state !== "stopped"
                onTriggered: appController.show_now_playing_notification()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: appController.playback_state === "playing"
                    ? qsTr("Pause") : qsTr("Play")
                icon.name: appController.playback_state === "playing"
                    ? "media-playback-pause" : "media-playback-start"
                enabled: appController.playlist_count > 0
                onTriggered: appController.play_pause()
            }
            Platform.MenuItem {
                text: qsTr("Stop")
                icon.name: "media-playback-stop"
                enabled: appController.current_index >= 0
                    && appController.playback_state !== "stopped"
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
                enabled: appController.playlist_count > 0
                onTriggered: appController.next()
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: qsTr("Quit Kog")
                icon.name: "application-exit"
                onTriggered: root.quitKog()
            }
        }
    }

    Component.onCompleted: fileTreeModel.set_root_path_text(appController.directory_path)

    Timer {
        interval: 200
        running: true
        repeat: true
        onTriggered: {
            appController.poll_playback()
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
            root.clearPlaylistSelection()
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
    SkinLibrary { id: skinLibrary }
    Timer { interval: 100; running: skinLibrary.busy; repeat: true; onTriggered: skinLibrary.poll() }
    SkinBrowser { id: skinBrowser; library: skinLibrary; onOpenClassic: root.showClassicPlayer() }
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
        MenuItem { action: toggleQueueAction }
        MenuItem { action: stopAfterSelectionAction }
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuSeparator {}
        MenuItem { action: saveSelectionAction }
        MenuItem { action: editTagsAction }
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
            enabled: fileTreeModel.is_path_directory(root.treeContextPath)
            onTriggered: root.useTreeRoot(root.treeContextPath)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Add to Playlist")
            icon.name: "list-add"
            onTriggered: root.addTreeSelection(root.treeContextPath, false)
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
            Action { text: qsTr("Classic Skins…"); icon.name: "preferences-desktop-theme"; onTriggered: skinBrowser.show() }
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
            MenuSeparator {}
            MenuItem { action: toggleQueueAction }
            MenuItem { action: stopAfterSelectionAction }
            MenuItem { action: clearQueueAction }
        }

        MenuSeparator {}
        Action { text: qsTr("Preferences…"); icon.name: "configure"; shortcut: "Ctrl+,"; onTriggered: preferences.show() }
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
            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "i"
                iconName: "dialog-information"
                toolTip: qsTr("Info Inspector")
                onClicked: infoInspector.show()
            }
            Image {
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
                Layout.leftMargin: 4
                Layout.rightMargin: 3
                source: Qt.resolvedUrl("icons/kog.svg")
                sourceSize.width: 56
                sourceSize.height: 56
                fillMode: Image.PreserveAspectFit
                mipmap: true
                Accessible.name: qsTr("Kog")

                TitleDragArea { anchors.fill: parent }
            }
            Item {
                id: nowPlayingTitle

                readonly property string displayTitle:
                    appController.now_title === "Not Playing"
                        ? qsTr("Kog") : appController.now_title
                readonly property bool overflowing:
                    nowPlayingLabel.implicitWidth > width

                Layout.preferredWidth: root.compactToolbar ? 82 : 156
                Layout.minimumWidth: root.compactToolbar ? 58 : 92
                Layout.maximumWidth: root.compactToolbar ? 112 : 210
                Layout.fillHeight: true
                clip: true

                Label {
                    id: nowPlayingLabel

                    anchors.verticalCenter: parent.verticalCenter
                    x: 0
                    text: nowPlayingTitle.displayTitle
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    color: root.palette.text
                    wrapMode: Text.NoWrap
                }

                SequentialAnimation {
                    id: nowPlayingMarquee

                    running: nowPlayingTitle.overflowing
                        && !nowPlayingHover.containsMouse
                    loops: Animation.Infinite

                    PauseAnimation { duration: 1200 }
                    NumberAnimation {
                        target: nowPlayingLabel
                        property: "x"
                        from: 0
                        to: Math.min(0, nowPlayingTitle.width
                            - nowPlayingLabel.implicitWidth)
                        duration: Math.max(2600,
                            Math.abs(to) * 28)
                        easing.type: Easing.Linear
                    }
                    PauseAnimation { duration: 900 }
                    NumberAnimation {
                        target: nowPlayingLabel
                        property: "x"
                        to: 0
                        duration: 320
                        easing.type: Easing.OutCubic
                    }

                    onStopped: nowPlayingLabel.x = 0
                }

                MouseArea {
                    id: nowPlayingHover

                    anchors.fill: parent
                    acceptedButtons: Qt.NoButton
                    hoverEnabled: true
                }
                ToolTip.visible: nowPlayingHover.containsMouse
                    && appController.now_title !== "Not Playing"
                ToolTip.delay: 500
                ToolTip.text: appController.now_title

                TitleDragArea { anchors.fill: parent }
            }

            TitleDragArea {
                Layout.preferredWidth: root.compactToolbar ? 0 : 6
                Layout.fillHeight: true
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
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: appController.playback_state === "playing" ? "Ⅱ" : "▶"
                iconName: appController.playback_state === "playing"
                    ? "media-playback-pause"
                    : "media-playback-start"
                toolTip: qsTr("Play/Pause")
                enabled: appController.playlist_count > 0
                onClicked: appController.play_pause()
            }
            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "■"
                iconName: "media-playback-stop"
                toolTip: qsTr("Stop")
                enabled: appController.current_index >= 0
                    && appController.playback_state !== "stopped"
                onClicked: appController.stop()
            }
            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "▶"
                iconName: "media-skip-forward"
                toolTip: qsTr("Next")
                enabled: appController.playlist_count > 0
                onClicked: appController.next()
            }
            ToolbarButton {
                id: volumeButton
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "♪"
                iconName: "audio-volume-high"
                toolTip: qsTr("Volume")
                onClicked: volumePopup.open()

                Popup {
                    id: volumePopup
                    x: (parent.width - width) / 2
                    y: parent.height + 4
                    width: 190
                    height: 54
                    padding: 10
                    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

                    RowLayout {
                        anchors.fill: parent
                        Label { text: qsTr("Volume") }
                        Slider {
                            Layout.fillWidth: true
                            from: 0
                            to: 1
                            value: appController.volume
                            onMoved: appController.set_volume_level(value)
                        }
                    }
                }
            }

            Frame {
                Layout.preferredWidth: 62
                Layout.preferredHeight: 30
                padding: 4
                Label {
                    anchors.centerIn: parent
                    text: root.timeLabel(appController.position_seconds)
                    color: root.palette.text
                    font.pixelSize: 12
                }
            }
            Slider {
                Layout.fillWidth: true
                Layout.minimumWidth: root.compactToolbar ? 72 : 140
                Layout.preferredWidth: root.compactToolbar ? 110 : 260
                Layout.maximumWidth: 460
                visible: !root.searchVisible || root.width >= 1080
                from: 0
                to: Math.max(1, appController.duration_seconds)
                value: appController.position_seconds
                enabled: appController.current_index >= 0
                Accessible.name: qsTr("Playback position")
                onMoved: appController.seek(value)
            }
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

            TitleDragArea {
                Layout.preferredWidth: root.compactToolbar ? 0 : 6
                Layout.fillHeight: true
            }

            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "×"
                iconName: "edit-clear-list"
                toolTip: qsTr("Clear Playlist")
                enabled: clearPlaylistAction.enabled
                onClicked: clearPlaylistAction.trigger()
            }

            TextField {
                id: searchField
                Layout.preferredWidth: root.compactToolbar ? 122 : 165
                visible: root.searchVisible
                placeholderText: qsTr("Search playlist")
                selectByMouse: true
                onTextChanged: {
                    if (text.length === 0) {
                        playlistSearchTimer.stop()
                        appController.filter_playlist("")
                        root.clearPlaylistSelection()
                    } else {
                        playlistSearchTimer.restart()
                    }
                }
                onVisibleChanged: if (visible) forceActiveFocus()
                Keys.onEscapePressed: {
                    text = ""
                    root.searchVisible = false
                }
            }
            ToolbarButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "⌕"
                iconName: "edit-find"
                checkable: true
                checked: root.searchVisible
                toolTip: root.searchVisible ? qsTr("Close Search") : qsTr("Search")
                onToggled: {
                    root.searchVisible = checked
                    if (!checked)
                        searchField.text = ""
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
        implicitHeight: 22
        color: root.palette.button
        border.color: root.palette.mid

        Label {
            anchors.centerIn: parent
            text: appController.total_duration
            font.pixelSize: 11
        }

        Label {
            anchors.right: parent.right
            anchors.rightMargin: 9
            anchors.verticalCenter: parent.verticalCenter
            visible: appController.status.length > 0
                && appController.status !== "Drop audio files here or use the Kog menu to add files"
            text: appController.status
            color: root.palette.placeholderText
            font.pixelSize: 10
            elide: Text.ElideLeft
            width: Math.min(320, implicitWidth)
        }
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal

        Rectangle {
            SplitView.preferredWidth: root.sidebarVisible ? 285 : 0
            SplitView.minimumWidth: root.sidebarVisible ? 170 : 0
            SplitView.maximumWidth: root.sidebarVisible ? 420 : 0
            visible: root.sidebarVisible
            color: root.palette.window
            border.color: root.palette.mid

            ColumnLayout {
                anchors.fill: parent
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
                        Label { Layout.fillWidth: true; text: appController.directory_path; font.bold: true; elide: Text.ElideMiddle }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 6
                    spacing: 4
                    TextField {
                        id: treeSearchField
                        objectName: "treeSearchField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search files and folders…")
                        Accessible.name: qsTr("Search music folder, subfolders, and archive contents")
                        selectByMouse: true
                        maximumLength: 200
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
                    CogButton {
                        Layout.preferredWidth: 28
                        Layout.preferredHeight: 28
                        iconName: "edit-clear"
                        glyph: "×"
                        enabled: treeSearchField.text.length > 0
                        toolTip: qsTr("Clear folder search")
                        onClicked: treeSearchField.clear()
                    }
                    BusyIndicator {
                        objectName: "treeSearchSpinner"
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        visible: fileTreeModel.searching
                        running: visible
                        Accessible.name: qsTr("Searching files and archives")
                    }
                }
                Timer {
                    id: treeSearchDebounce
                    interval: 250
                    onTriggered: fileTreeModel.searchText = treeSearchField.text
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
                    ToolTip.visible: hovered
                    ToolTip.delay: 700
                    ToolTip.text: fileTreeModel.parent_path
                    onClicked: {
                        appController.parent_directory()
                        fileTreeModel.set_root_path_text(
                            appController.directory_path)
                        root.clearTreeSelection()
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
                        readonly property string dragPath: filePath
                        width: Math.max(0,
                            directoryTree.width - directoryTree.scrollGutter)
                        implicitHeight: 26
                        icon.name: fileTreeModel.icon_name(filePath)
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
                            Label {
                                Layout.fillWidth: true
                                text: treeDelegate.fileName
                                elide: Text.ElideRight
                                color: treeDelegate.selected
                                    ? treeDelegate.palette.highlightedText
                                    : treeDelegate.palette.text
                            }
                        }
                        ToolTip.visible: treePointer.containsMouse
                        ToolTip.delay: 700
                        ToolTip.text: fileTreeModel.displayPath(treeDelegate.dragPath)
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
                                if (mouse.button === Qt.LeftButton)
                                    root.addTreeSelection(
                                        treeDelegate.dragPath, true)
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
        }

        Rectangle {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 560
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
                    onDraggingChanged: if (dragging)
                        playlistKineticWheel.stop()

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
                        onPressedChanged: if (pressed)
                            playlistKineticWheel.stop()
                    }
                    ScrollBar.horizontal: ScrollBar {
                        id: playlistHorizontalScrollBar
                        policy: ScrollBar.AsNeeded
                    }

                    footer: Item {
                        width: 1
                        height: playlistView.horizontalScrollGutter
                    }

                    // Physical mouse wheels use Kog's per-frame kinetic
                    // motion. Touchpad gestures still pass through to Qt so
                    // the platform can preserve their native pixel precision.
                    KineticWheelHandler {
                        id: playlistKineticWheel
                        view: playlistView
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
