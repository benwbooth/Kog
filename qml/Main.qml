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

    Action {
        id: removeSelectedAction
        text: qsTr("Remove Selected")
        shortcut: StandardKey.Delete
        enabled: root.selectedRow >= 0
        onTriggered: {
            appController.remove_track(root.selectedRow)
            root.selectedRow = -1
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
        MenuItem {
            text: qsTr("Choose Music Folder…")
            icon.name: "folder-open"
            onTriggered: root.chooseMusicFolder()
        }
        MenuSeparator {}
        MenuItem { action: removeSelectedAction; icon.name: "edit-delete" }
        MenuItem {
            text: qsTr("Clear Playlist")
            icon.name: "edit-clear-list"
            enabled: appController.playlist_count > 0
            onTriggered: appController.clear_playlist()
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
                onTextChanged: appController.filter_playlist(text)
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
                    // Route pointer input through the handlers in the delegate.
                    // Otherwise TreeView's built-in double-tap expansion races
                    // our file activation and the native DragHandler.
                    pointerNavigationEnabled: false
                    selectionModel: ItemSelectionModel {
                        model: fileTreeModel
                    }
                    boundsBehavior: Flickable.StopAtBounds
                    columnWidthProvider: function(column) { return width }

                    delegate: TreeViewDelegate {
                        id: treeDelegate
                        readonly property url dragUrl: fileTreeModel.file_url(
                            treeView.index(row, column))
                        width: directoryTree.width
                        implicitHeight: 26
                        icon.width: 18
                        icon.height: 18
                        ToolTip.visible: hovered
                        ToolTip.delay: 700
                        ToolTip.text: fileTreeModel.file_url(treeView.index(row, column)).toString()
                        Drag.active: treeDrag.active
                        // A native drag follows the pointer while this delegate
                        // remains in place. Drag.Internal would leave the drag
                        // hotspot over the tree because treeDrag has no target.
                        Drag.dragType: Drag.Automatic
                        Drag.keys: ["kog-file-tree-entry"]
                        Drag.supportedActions: Qt.CopyAction
                        Drag.proposedAction: Qt.CopyAction
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

                        TapHandler {
                            id: treeTap
                            acceptedButtons: Qt.LeftButton
                            gesturePolicy: TapHandler.DragThreshold
                            grabPermissions: PointerHandler.CanTakeOverFromAnything
                                | PointerHandler.ApprovesTakeOverByAnything

                            onTapped: {
                                const itemIndex = treeView.index(treeDelegate.row, 0)
                                directoryTree.selectionModel.setCurrentIndex(
                                    itemIndex, ItemSelectionModel.ClearAndSelect)
                            }
                            onDoubleTapped: eventPoint => {
                                const indicator = treeDelegate.indicator
                                if (indicator
                                        && eventPoint.position.x >= indicator.x
                                        && eventPoint.position.x < indicator.x + indicator.width)
                                    return
                                const itemIndex = treeView.index(treeDelegate.row, 0)
                                if (fileTreeModel.is_directory(itemIndex))
                                    treeView.toggleExpanded(treeDelegate.row)
                                else
                                    appController.add_file(treeDelegate.dragUrl)
                            }
                        }

                        TapHandler {
                            parent: treeDelegate.indicator
                            enabled: treeDelegate.hasChildren
                            acceptedButtons: Qt.LeftButton
                            exclusiveSignals: TapHandler.SingleTap | TapHandler.DoubleTap

                            onSingleTapped: treeView.toggleExpanded(treeDelegate.row)
                            onDoubleTapped: treeView.toggleExpanded(treeDelegate.row)
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

                PlaylistHeader {
                    id: playlistHeader
                    Layout.fillWidth: true
                    theme: root.palette
                }

                ListView {
                    id: playlistView
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: appController.playlist_count
                    boundsBehavior: Flickable.StopAtBounds
                    currentIndex: root.selectedRow
                    keyNavigationEnabled: true
                    focus: true
                    highlightMoveDuration: 0

                    Keys.onReturnPressed: if (root.selectedRow >= 0)
                        appController.play_index(root.selectedRow)
                    Keys.onEnterPressed: if (root.selectedRow >= 0)
                        appController.play_index(root.selectedRow)
                    Keys.onDeletePressed: removeSelectedAction.trigger()

                    delegate: PlaylistRow {
                        required property int index
                        width: playlistView.width
                        app: appController
                        columns: playlistHeader
                        theme: root.palette
                        rowIndex: index
                        selected: root.selectedRow === index
                        onPressed: row => root.selectedRow = row
                        onActivated: row => {
                            root.selectedRow = row
                            appController.play_index(row)
                        }
                    }

                    ScrollBar.vertical: ScrollBar {}

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

            DropArea {
                anchors.fill: parent
                onDropped: drop => {
                    if (drop.source && drop.source.dragUrl) {
                        appController.add_file(drop.source.dragUrl)
                        drop.acceptProposedAction()
                        return
                    }
                    if (!drop.hasUrls)
                        return
                    for (let url of drop.urls)
                        appController.add_file(url)
                    drop.acceptProposedAction()
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
