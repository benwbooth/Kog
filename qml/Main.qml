import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

import org.kog.player 1.0

ApplicationWindow {
    id: root

    width: 1280
    height: 760
    minimumWidth: 900
    minimumHeight: 520
    visible: true
    title: appController.now_title === "Not Playing" ? qsTr("Kog") : appController.now_title + " — Kog"
    color: palette.window

    property bool sidebarVisible: true
    property int selectedRow: -1
    property bool repeatEnabled: false
    property bool shuffleEnabled: false
    readonly property bool useLightIcons: (0.2126 * palette.window.r
        + 0.7152 * palette.window.g
        + 0.0722 * palette.window.b) < 0.5

    function timeLabel(seconds) {
        const value = Math.max(0, Math.floor(seconds))
        const hours = Math.floor(value / 3600)
        const minutes = Math.floor((value % 3600) / 60)
        const remaining = value % 60
        return hours > 0
            ? hours + ":" + String(minutes).padStart(2, "0") + ":" + String(remaining).padStart(2, "0")
            : minutes + ":" + String(remaining).padStart(2, "0")
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

    FileDialog {
        id: addFilesDialog
        title: qsTr("Add Audio Files")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("Audio files (*)")]
        onAccepted: {
            for (let file of selectedFiles)
                appController.add_file(file)
        }
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Choose Music Folder")
        onAccepted: {
            appController.choose_directory(selectedFolder)
            fileTreeModel.set_root_url(selectedFolder)
        }
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

        Action { text: qsTr("Add Files…"); shortcut: StandardKey.Open; onTriggered: addFilesDialog.open() }
        MenuItem { text: qsTr("Choose Music Folder…"); onTriggered: folderDialog.open() }
        MenuSeparator {}
        MenuItem { action: removeSelectedAction }
        MenuItem { text: qsTr("Clear Playlist"); enabled: appController.playlist_count > 0; onTriggered: appController.clear_playlist() }
        MenuSeparator {}

        Menu {
            title: qsTr("View")
            Action {
                text: qsTr("Show File Tree")
                shortcut: "Ctrl+D"
                checkable: true
                checked: root.sidebarVisible
                onTriggered: root.sidebarVisible = checked
            }
            Action { text: qsTr("Show Info Inspector"); shortcut: "Ctrl+I"; onTriggered: infoInspector.show() }
            Action { text: qsTr("Show Lyrics"); shortcut: "Ctrl+Shift+L"; onTriggered: lyricsWindow.show() }
            Action { text: qsTr("Show Mini Player"); shortcut: "Ctrl+Shift+M"; onTriggered: miniPlayer.show() }
        }

        Menu {
            title: qsTr("Playback")
            Action { text: qsTr("Play/Pause"); shortcut: "Space"; onTriggered: appController.play_pause() }
            Action { text: qsTr("Stop"); shortcut: "Ctrl+."; onTriggered: appController.stop() }
            MenuSeparator {}
            Action { text: qsTr("Previous"); shortcut: "Ctrl+Left"; onTriggered: appController.previous() }
            Action { text: qsTr("Next"); shortcut: "Ctrl+Right"; onTriggered: appController.next() }
        }

        MenuSeparator {}
        Action { text: qsTr("Preferences…"); shortcut: "Ctrl+,"; onTriggered: preferences.show() }
        Action { text: qsTr("Quit Kog"); shortcut: StandardKey.Quit; onTriggered: Qt.quit() }
    }

    header: ToolBar {
        implicitHeight: 90
        padding: 8

        ColumnLayout {
            anchors.fill: parent
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 5

                CogButton {
                    id: hamburgerButton
                    glyph: "☰"
                    iconName: "application-menu"
                    toolTip: hamburgerMenu.visible ? "" : qsTr("Kog menu")
                    Accessible.name: qsTr("Kog menu")
                    onClicked: hamburgerMenu.popup(hamburgerButton, 0, hamburgerButton.height)
                }
                CogButton { glyph: "▣"; iconName: "folder-open"; toolTip: qsTr("Choose music folder"); onClicked: folderDialog.open() }

                ColumnLayout {
                    Layout.preferredWidth: 245
                    Layout.minimumWidth: 120
                    spacing: 0

                    Label { Layout.fillWidth: true; text: appController.now_title; font.pixelSize: 15; font.bold: true; elide: Text.ElideRight }
                    Label { Layout.fillWidth: true; text: appController.now_artist; color: root.palette.placeholderText; elide: Text.ElideRight }
                }

                Item { Layout.fillWidth: true }

                CogButton { glyph: "◀"; iconName: "media-skip-backward"; toolTip: qsTr("Previous"); onClicked: appController.previous() }
                CogButton {
                    glyph: appController.playback_state === "playing" ? "Ⅱ" : "▶"
                    iconName: appController.playback_state === "playing" ? "media-playback-pause" : "media-playback-start"
                    toolTip: qsTr("Play/Pause")
                    onClicked: appController.play_pause()
                }
                CogButton { glyph: "■"; iconName: "media-playback-stop"; toolTip: qsTr("Stop"); onClicked: appController.stop() }
                CogButton { glyph: "▶"; iconName: "media-skip-forward"; toolTip: qsTr("Next"); onClicked: appController.next() }

                ToolSeparator { Layout.fillHeight: true }

                CogButton {
                    glyph: "⇄"
                    iconName: "media-playlist-shuffle"
                    checkable: true
                    checked: root.shuffleEnabled
                    toolTip: qsTr("Shuffle")
                    onToggled: root.shuffleEnabled = checked
                }
                CogButton {
                    glyph: "↻"
                    iconName: "media-playlist-repeat"
                    checkable: true
                    checked: root.repeatEnabled
                    toolTip: qsTr("Repeat")
                    onToggled: root.repeatEnabled = checked
                }

                Item { Layout.fillWidth: true }

                Frame {
                    Layout.preferredWidth: 78
                    Layout.preferredHeight: 32
                    padding: 5
                    Label { anchors.centerIn: parent; text: root.timeLabel(appController.position_seconds); font.pixelSize: 14 }
                }

                Label { text: qsTr("Volume"); Accessible.name: qsTr("Volume") }
                Slider {
                    Layout.preferredWidth: 116
                    from: 0
                    to: 1
                    value: appController.volume
                    onMoved: appController.set_volume_level(value)
                }

                TextField {
                    id: searchField
                    Layout.preferredWidth: 170
                    placeholderText: qsTr("Search")
                    selectByMouse: true
                    onTextChanged: appController.filter_playlist(text)
                }
            }

            Slider {
                Layout.fillWidth: true
                Layout.preferredHeight: 15
                from: 0
                to: Math.max(1, appController.duration_seconds)
                value: appController.position_seconds
                enabled: appController.current_index >= 0
                onMoved: appController.seek(value)
            }
        }
    }

    footer: Rectangle {
        implicitHeight: 25
        color: root.palette.button
        border.color: root.palette.mid

        Label {
            anchors.centerIn: parent
            text: appController.total_duration
            font.pixelSize: 12
        }

        Label {
            anchors.right: parent.right
            anchors.rightMargin: 9
            anchors.verticalCenter: parent.verticalCenter
            text: appController.status
            color: root.palette.placeholderText
            font.pixelSize: 11
            elide: Text.ElideLeft
            width: Math.min(360, implicitWidth)
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
                            onClicked: folderDialog.open()
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
                    boundsBehavior: Flickable.StopAtBounds
                    columnWidthProvider: function(column) { return width }

                    delegate: TreeViewDelegate {
                        width: directoryTree.width
                        implicitHeight: 30
                        icon.source: {
                            const iconName = fileTreeModel.is_directory(treeView.index(row, column))
                                ? "folder"
                                : "audio-x-generic"
                            return Qt.resolvedUrl("icons/" + iconName
                                + (root.useLightIcons ? "-light" : "") + ".svg")
                        }
                        icon.color: "transparent"
                        icon.width: 18
                        icon.height: 18
                        ToolTip.visible: hovered
                        ToolTip.delay: 700
                        ToolTip.text: fileTreeModel.file_url(treeView.index(row, column)).toString()
                        onDoubleClicked: {
                            const itemIndex = treeView.index(row, column)
                            if (!fileTreeModel.is_directory(itemIndex))
                                appController.add_file(fileTreeModel.file_url(itemIndex))
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

                    Label {
                        anchors.centerIn: parent
                        visible: appController.playlist_count === 0
                        text: qsTr("Drop music here or use Add Files")
                        color: root.palette.placeholderText
                        font.pixelSize: 18
                    }
                }
            }

            DropArea {
                anchors.fill: parent
                onDropped: drop => {
                    if (!drop.hasUrls)
                        return
                    for (let url of drop.urls)
                        appController.add_file(url)
                    drop.acceptProposedAction()
                }
            }
        }
    }

}
