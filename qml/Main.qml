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
    color: "#ededed"
    palette.window: "#f4f4f4"
    palette.windowText: "#303030"
    palette.base: "#ffffff"
    palette.text: "#303030"
    palette.button: "#f2f2f2"
    palette.buttonText: "#303030"
    palette.highlight: "#c95f00"
    palette.highlightedText: "#ffffff"
    palette.placeholderText: "#777777"

    property bool sidebarVisible: true
    property int selectedRow: -1
    property bool repeatEnabled: false
    property bool shuffleEnabled: false

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
        onAccepted: appController.choose_directory(selectedFolder)
    }

    InfoInspector { id: infoInspector; app: appController }
    MiniPlayer { id: miniPlayer; app: appController }

    menuBar: MenuBar {
        background: Rectangle { color: "#f7f7f7"; border.color: "#d3d3d3" }

        Menu {
            title: qsTr("File")
            Action { text: qsTr("Add Files…"); shortcut: StandardKey.Open; onTriggered: addFilesDialog.open() }
            Action { text: qsTr("Choose Music Folder…"); onTriggered: folderDialog.open() }
            MenuSeparator {}
            Action { text: qsTr("Clear Playlist"); onTriggered: appController.clear_playlist() }
            MenuSeparator {}
            Action { text: qsTr("Quit"); shortcut: StandardKey.Quit; onTriggered: Qt.quit() }
        }
        Menu {
            title: qsTr("Edit")
            Action {
                text: qsTr("Remove Selected")
                shortcut: StandardKey.Delete
                enabled: root.selectedRow >= 0
                onTriggered: {
                    appController.remove_track(root.selectedRow)
                    root.selectedRow = -1
                }
            }
        }
        Menu {
            title: qsTr("View")
            Action { text: qsTr("Show File Tree"); checkable: true; checked: root.sidebarVisible; onTriggered: root.sidebarVisible = checked }
            Action { text: qsTr("Show Info Inspector"); shortcut: "Ctrl+I"; onTriggered: infoInspector.show() }
            Action { text: qsTr("Show Mini Player"); onTriggered: miniPlayer.show() }
        }
        Menu {
            title: qsTr("Control")
            Action { text: qsTr("Play/Pause"); shortcut: "Space"; onTriggered: appController.play_pause() }
            Action { text: qsTr("Stop"); shortcut: "Ctrl+."; onTriggered: appController.stop() }
            Action { text: qsTr("Previous"); shortcut: "Ctrl+Left"; onTriggered: appController.previous() }
            Action { text: qsTr("Next"); shortcut: "Ctrl+Right"; onTriggered: appController.next() }
        }
    }

    header: ToolBar {
        implicitHeight: 90
        padding: 8
        background: Rectangle {
            color: "#fafafa"
            border.color: "#d3d3d3"
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 5

                CogButton { glyph: "▣"; toolTip: qsTr("Choose music folder"); onClicked: folderDialog.open() }

                ColumnLayout {
                    Layout.preferredWidth: 245
                    Layout.minimumWidth: 120
                    spacing: 0

                    Label { Layout.fillWidth: true; text: appController.now_title; color: "#303030"; font.pixelSize: 15; font.bold: true; elide: Text.ElideRight }
                    Label { Layout.fillWidth: true; text: appController.now_artist; color: "#757575"; elide: Text.ElideRight }
                }

                Item { Layout.fillWidth: true }

                CogButton { glyph: "◀"; toolTip: qsTr("Previous"); onClicked: appController.previous() }
                CogButton {
                    glyph: appController.playback_state === "playing" ? "Ⅱ" : "▶"
                    toolTip: qsTr("Play/Pause")
                    onClicked: appController.play_pause()
                }
                CogButton { glyph: "■"; toolTip: qsTr("Stop"); onClicked: appController.stop() }
                CogButton { glyph: "▶"; toolTip: qsTr("Next"); onClicked: appController.next() }

                ToolSeparator { Layout.fillHeight: true }

                CogButton {
                    glyph: "⇄"
                    checkable: true
                    checked: root.shuffleEnabled
                    toolTip: qsTr("Shuffle")
                    onToggled: root.shuffleEnabled = checked
                }
                CogButton {
                    glyph: "↻"
                    checkable: true
                    checked: root.repeatEnabled
                    toolTip: qsTr("Repeat")
                    onToggled: root.repeatEnabled = checked
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    Layout.preferredWidth: 78
                    Layout.preferredHeight: 32
                    color: "#ffffff"
                    radius: 6
                    border.color: "#dddddd"
                    Label { anchors.centerIn: parent; text: root.timeLabel(appController.position_seconds); color: "#303030"; font.pixelSize: 14 }
                }

                Label { text: "🔊"; color: "#606060" }
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
                    leftPadding: 27
                    onTextChanged: appController.filter_playlist(text)

                    Label {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: "⌕"
                        color: "#777777"
                        font.pixelSize: 18
                    }
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
        color: "#e5e5e5"
        border.color: "#cfcfcf"

        Label {
            anchors.centerIn: parent
            text: appController.total_duration
            color: "#454545"
            font.pixelSize: 12
        }

        Label {
            anchors.right: parent.right
            anchors.rightMargin: 9
            anchors.verticalCenter: parent.verticalCenter
            text: appController.status
            color: "#707070"
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
            color: "#e9e9e9"
            border.color: "#c8c8c8"

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 35
                    color: "#e1e1e1"
                    border.color: "#c7c7c7"

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 7
                        anchors.rightMargin: 7
                        spacing: 5

                        CogButton { Layout.preferredWidth: 28; Layout.preferredHeight: 28; glyph: "‹"; toolTip: qsTr("Parent folder"); onClicked: appController.parent_directory() }
                        Label { text: "📁"; font.pixelSize: 16 }
                        Label { Layout.fillWidth: true; text: appController.directory_path; font.bold: true; elide: Text.ElideMiddle }
                    }
                }

                ListView {
                    id: directoryList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: appController.directory_count
                    currentIndex: -1
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: ItemDelegate {
                        required property int index
                        width: directoryList.width
                        height: 29
                        leftPadding: 10
                        text: (appController.directory_is_folder_at(index) ? "📁  " : "♫  ") + appController.directory_name_at(index)
                        font.pixelSize: 12
                        contentItem: Text {
                            text: parent.text
                            color: "#303030"
                            font: parent.font
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }
                        background: Rectangle {
                            color: parent.hovered ? "#d9e8f6" : "transparent"
                        }
                        onDoubleClicked: appController.activate_directory_entry(index)
                    }

                    ScrollBar.vertical: ScrollBar {}
                }
            }
        }

        Rectangle {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 560
            color: "white"

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                PlaylistHeader {
                    id: playlistHeader
                    Layout.fillWidth: true
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
                        text: qsTr("Drop music here")
                        color: "#a0a0a0"
                        font.pixelSize: 21
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
