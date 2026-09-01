pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml.Models

import org.kog.player 1.0

ApplicationWindow {
    id: root

    width: 1120
    height: 540
    minimumWidth: 800
    minimumHeight: 380
    visible: true
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
    readonly property bool compactToolbar: width < 980
    readonly property bool useLightIcons: (0.2126 * palette.window.r
        + 0.7152 * palette.window.g
        + 0.0722 * palette.window.b) < 0.5

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

    function isPlaylistDrag(drag) {
        return (drag.source && drag.source.playlistDrag)
            || drag.formats.indexOf("application/x-kog-playlist-rows") !== -1
    }

    function playlistDragRows(drop) {
        if (drop.source && drop.source.playlistDrag)
            return drop.source.dragRows
        return drop.getDataAsString("application/x-kog-playlist-rows").trim()
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

    Component.onCompleted: fileTreeModel.set_root_path_text(appController.directory_path)

    Timer {
        interval: 200
        running: true
        repeat: true
        onTriggered: appController.poll_playback()
    }

    InfoInspector { id: infoInspector; app: appController }
    Lyrics { id: lyricsWindow; app: appController }
    MiniPlayer { id: miniPlayer; app: appController }
    Preferences { id: preferences; app: appController }

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

    Menu {
        id: playlistContextMenu
        MenuItem {
            text: qsTr("Play")
            icon.name: "media-playback-start"
            enabled: root.selectedRow >= 0
            onTriggered: appController.play_index(root.selectedRow)
        }
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuSeparator {}
        MenuItem { action: saveSelectionAction }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Select All")
            icon.name: "edit-select-all"
            enabled: appController.playlist_count > 0
            onTriggered: selectAllAction.trigger()
        }
        MenuItem {
            text: qsTr("Clear Playlist")
            icon.name: "edit-clear-list"
            enabled: appController.playlist_count > 0
            onTriggered: {
                appController.clear_playlist()
                root.clearPlaylistSelection()
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
            text: qsTr("Choose Music Folder…")
            icon.name: "folder-open"
            onTriggered: root.chooseMusicFolder()
        }
        MenuSeparator {}
        MenuItem { action: savePlaylistAction }
        MenuItem { action: saveSelectionAction }
        MenuSeparator {}
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuItem {
            text: qsTr("Clear Playlist")
            icon.name: "edit-clear-list"
            enabled: appController.playlist_count > 0
            onTriggered: {
                appController.clear_playlist()
                root.clearPlaylistSelection()
            }
        }
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
            Action { text: qsTr("Show Lyrics"); icon.name: "view-media-lyrics"; shortcut: "Ctrl+Shift+L"; onTriggered: lyricsWindow.show() }
            Action { text: qsTr("Show Mini Player"); icon.name: "view-restore"; shortcut: "Ctrl+Shift+M"; onTriggered: miniPlayer.show() }
        }

        Menu {
            title: qsTr("Playback")
            icon.name: "media-playback-start"
            Action { text: qsTr("Play/Pause"); icon.name: "media-playback-start"; shortcut: "Space"; onTriggered: appController.play_pause() }
            Action { text: qsTr("Stop"); icon.name: "media-playback-stop"; shortcut: "Ctrl+."; onTriggered: appController.stop() }
            MenuSeparator {}
            Action { text: qsTr("Previous"); icon.name: "media-skip-backward"; shortcut: "Ctrl+Left"; onTriggered: appController.previous() }
            Action { text: qsTr("Next"); icon.name: "media-skip-forward"; shortcut: "Ctrl+Right"; onTriggered: appController.next() }
        }

        MenuSeparator {}
        Action { text: qsTr("Preferences…"); icon.name: "configure"; shortcut: "Ctrl+,"; onTriggered: preferences.show() }
        Action { text: qsTr("Quit Kog"); icon.name: "application-exit"; shortcut: StandardKey.Quit; onTriggered: Qt.quit() }
    }

    header: ToolBar {
        implicitHeight: 48
        padding: 5

        RowLayout {
            anchors.fill: parent
            spacing: 2

            RowLayout {
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

            CogButton {
                id: hamburgerButton
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "☰"
                iconName: "application-menu"
                toolTip: hamburgerMenu.visible ? "" : qsTr("Kog menu")
                onClicked: hamburgerMenu.popup(hamburgerButton, 0, hamburgerButton.height)
            }
            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "i"
                iconName: "dialog-information"
                toolTip: qsTr("Info Inspector")
                onClicked: infoInspector.show()
            }
            Label {
                Layout.preferredWidth: root.compactToolbar ? 92 : 176
                Layout.minimumWidth: 72
                text: appController.now_title === "Not Playing"
                    ? qsTr("Kog")
                    : appController.now_title
                font.pixelSize: 14
                font.bold: true
                elide: Text.ElideRight

                TitleDragArea { anchors.fill: parent }
            }

            TitleDragArea { Layout.fillWidth: true; Layout.fillHeight: true }

            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "◀"
                iconName: "media-skip-backward"
                toolTip: qsTr("Previous")
                enabled: appController.playlist_count > 0
                onClicked: appController.previous()
            }
            CogButton {
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
            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "▶"
                iconName: "media-skip-forward"
                toolTip: qsTr("Next")
                enabled: appController.playlist_count > 0
                onClicked: appController.next()
            }
            CogButton {
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
                    font.pixelSize: 12
                }
            }
            Slider {
                Layout.preferredWidth: 106
                visible: !root.compactToolbar
                from: 0
                to: Math.max(1, appController.duration_seconds)
                value: appController.position_seconds
                enabled: appController.current_index >= 0
                Accessible.name: qsTr("Playback position")
                onMoved: appController.seek(value)
            }
            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "⇄"
                iconName: "media-playlist-shuffle"
                checkable: true
                checked: appController.shuffle_enabled
                toolTip: qsTr("Shuffle")
                enabled: appController.playlist_count > 1
                onToggled: appController.set_shuffle_mode(checked)
            }
            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: "↻"
                iconName: "media-playlist-repeat"
                checkable: true
                checked: appController.repeat_enabled
                toolTip: qsTr("Repeat playlist")
                enabled: appController.playlist_count > 0
                onToggled: appController.set_repeat_mode(checked)
            }

            TitleDragArea { Layout.fillWidth: true; Layout.fillHeight: true }

            CogButton {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                glyph: root.sidebarVisible ? "«" : "»"
                checkable: true
                checked: root.sidebarVisible
                toolTip: root.sidebarVisible ? qsTr("Hide File Tree") : qsTr("Show File Tree")
                onToggled: root.sidebarVisible = checked
            }
            TextField {
                id: searchField
                Layout.preferredWidth: 165
                visible: root.searchVisible
                placeholderText: qsTr("Search playlist")
                selectByMouse: true
                onTextChanged: {
                    appController.filter_playlist(text)
                    root.clearPlaylistSelection()
                }
                onVisibleChanged: if (visible) forceActiveFocus()
                Keys.onEscapePressed: {
                    text = ""
                    root.searchVisible = false
                }
            }
            CogButton {
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
                            glyph: "↑"
                            iconName: "go-up"
                            toolTip: qsTr("Use parent folder as the tree root")
                            onClicked: {
                                appController.parent_directory()
                                fileTreeModel.set_root_path_text(appController.directory_path)
                            }
                        }
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

                TreeView {
                    id: directoryTree
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: fileTreeModel
                    rootIndex: fileTreeModel.root_index
                    alternatingRows: true
                    selectionBehavior: TableView.SelectRows
                    selectionMode: TableView.SingleSelection
                    // TreeView owns row selection and folder expansion. The
                    // delegate only adds Cog's file activation behavior.
                    pointerNavigationEnabled: true
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
                    columnWidthProvider: function(column) { return width }

                    delegate: TreeViewDelegate {
                        id: treeDelegate
                        // Bind the QFileSystemModel role as a required delegate
                        // property. Accessing it through the transient `model`
                        // object leaves recycled TreeView delegates displaying
                        // and activating the preceding row after expansion.
                        required property string filePath
                        readonly property string dragPath: filePath
                        readonly property url dragUrl: fileTreeModel.path_url(dragPath)
                        width: directoryTree.width
                        implicitHeight: 26
                        icon.width: 18
                        icon.height: 18
                        ToolTip.visible: hovered
                        ToolTip.delay: 700
                        ToolTip.text: treeDelegate.dragPath
                        Drag.active: treeDrag.active
                        // A native drag follows the pointer while this delegate
                        // remains in place. Drag.Internal would leave the drag
                        // hotspot over the tree because treeDrag has no target.
                        Drag.dragType: Drag.Automatic
                        Drag.keys: ["kog-file-tree-entry"]
                        Drag.supportedActions: Qt.CopyAction
                        Drag.proposedAction: Qt.CopyAction
                        Drag.source: treeDelegate
                        Drag.mimeData: ({
                            "text/uri-list": treeDelegate.dragUrl.toString() + "\r\n"
                        })
                        Drag.hotSpot.x: 20
                        Drag.hotSpot.y: height / 2

                        DragHandler {
                            id: treeDrag
                            target: null
                            acceptedButtons: Qt.LeftButton
                            grabPermissions: PointerHandler.CanTakeOverFromAnything
                                | PointerHandler.ApprovesTakeOverByAnything
                        }

                        onDoubleClicked: {
                            const indicatorItem = treeDelegate.indicator
                            if (indicatorItem
                                    && treeDelegate.pressX >= indicatorItem.x
                                    && treeDelegate.pressX < indicatorItem.x
                                        + indicatorItem.width)
                                return
                            if (!fileTreeModel.is_path_directory(treeDelegate.dragPath))
                                appController.activate_local_path(treeDelegate.dragPath)
                        }
                    }

                    ScrollBar.vertical: ScrollBar {}
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
                    boundsBehavior: Flickable.StopAtBounds
                    contentWidth: Math.max(width, playlistHeader.totalWidth)
                    flickableDirection: Flickable.AutoFlickDirection
                    currentIndex: root.selectedRow
                    keyNavigationEnabled: true
                    focus: true
                    highlightMoveDuration: 0

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
                        width: playlistView.contentWidth
                        app: appController
                        columns: playlistHeader
                        theme: root.palette
                        rowIndex: index
                        selected: root.isPlaylistRowSelected(index)
                        dragRows: root.selectedRows.length > 0
                            ? root.selectedRows.join(",")
                            : String(index)
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
                            appController.play_index(row)
                        }
                        onDragStarted: row => {
                            if (!root.isPlaylistRowSelected(row))
                                root.setPlaylistSelection([row], row, row)
                        }
                    }

                    ScrollBar.vertical: ScrollBar {}
                    ScrollBar.horizontal: ScrollBar {}

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
                                width: parent.width - 12
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
                    if (root.isPlaylistDrag(drag))
                        root.playlistDropTarget = root.playlistDropIndex(drag.y)
                }
                onPositionChanged: drag => {
                    if (root.isPlaylistDrag(drag))
                        root.playlistDropTarget = root.playlistDropIndex(drag.y)
                }
                onExited: root.playlistDropTarget = -1
                onDropped: drop => {
                    if (root.isPlaylistDrag(drop)) {
                        const dragRows = root.playlistDragRows(drop)
                        if (dragRows.length === 0)
                            return
                        const target = root.playlistDropTarget >= 0
                            ? root.playlistDropTarget
                            : root.playlistDropIndex(drop.y)
                        const moved = appController.move_tracks(
                            dragRows, target)
                        root.applyMovedSelection(moved)
                        root.playlistDropTarget = -1
                        drop.acceptProposedAction()
                        return
                    }
                    if (drop.source && drop.source.dragUrl) {
                        if (drop.source.dragPath)
                            appController.add_local_path(drop.source.dragPath)
                        else
                            appController.add_file(drop.source.dragUrl)
                        drop.acceptProposedAction()
                        return
                    }
                    if (!drop.hasUrls)
                    {
                        if (drop.formats.indexOf("text/uri-list") === -1)
                            return
                        const uriList = drop.getDataAsString("text/uri-list")
                            .split(/\r?\n/).filter(value => value.length > 0)
                        for (const url of uriList) {
                            if (/^https?:\/\//i.test(url))
                                appController.enqueue_url(url)
                            else
                                appController.add_file(url)
                        }
                        drop.acceptProposedAction()
                        root.playlistDropTarget = -1
                        return
                    }
                    for (let url of drop.urls) {
                        const value = url.toString()
                        if (/^https?:\/\//i.test(value))
                            appController.enqueue_url(value)
                        else
                            appController.add_file(url)
                    }
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
