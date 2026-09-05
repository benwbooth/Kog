import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app
    required property var mainWindow
    objectName: "kogMiniPlayer"

    width: 540
    height: 144
    minimumWidth: 540
    maximumWidth: 540
    minimumHeight: 144
    maximumHeight: 144
    flags: Qt.Window | Qt.FramelessWindowHint
    title: root.app.now_title === "Not Playing"
        ? qsTr("Kog Mini Player")
        : root.app.now_title + " — Kog"
    color: "transparent"

    readonly property real panelLuminance: 0.2126 * palette.window.r
        + 0.7152 * palette.window.g + 0.0722 * palette.window.b
    readonly property bool darkMode: panelLuminance < 0.5
    readonly property color raisedSurface: darkMode
        ? Qt.lighter(palette.window, 1.16)
        : Qt.darker(palette.window, 1.035)
    readonly property color outlineColor: darkMode
        ? Qt.lighter(palette.window, 1.52)
        : Qt.darker(palette.window, 1.22)
    readonly property string subtitle: {
        const artist = root.app.now_artist
        const album = root.app.current_album
        if (artist.length > 0 && album.length > 0)
            return artist + "  •  " + album
        if (artist.length > 0)
            return artist
        if (album.length > 0)
            return album
        return qsTr("Ready to play")
    }

    function timeLabel(seconds) {
        const value = Math.max(0, Math.floor(seconds))
        const hours = Math.floor(value / 3600)
        const minutes = Math.floor((value % 3600) / 60)
        const remaining = value % 60
        return hours > 0
            ? hours + ":" + String(minutes).padStart(2, "0")
                + ":" + String(remaining).padStart(2, "0")
            : minutes + ":" + String(remaining).padStart(2, "0")
    }

    function restoreFullPlayer() {
        root.hide()
        root.mainWindow.showFromTray()
    }

    onClosing: close => {
        if (!root.mainWindow.applicationQuitRequested) {
            close.accepted = false
            root.restoreFullPlayer()
        }
    }

    component MiniButton: CogButton {
        id: button

        property bool primary: false

        implicitWidth: primary ? 40 : 34
        implicitHeight: primary ? 40 : 34
        iconBackground: primary ? palette.highlight : root.palette.window
        icon.width: primary ? 20 : 18
        icon.height: primary ? 20 : 18

        // As in the notification controls, draw the bundled SVG explicitly.
        // Some desktop styles drop ToolButton's icon with a custom background.
        contentItem: Image {
            objectName: "miniButtonIcon"
            source: button.icon.source
            sourceSize: Qt.size(button.icon.width, button.icon.height)
            fillMode: Image.Pad
            opacity: button.enabled ? 1 : 0.4
        }

        background: Rectangle {
            radius: button.primary ? height / 2 : 7
            color: button.primary
                ? button.palette.highlight
                : (button.down
                    ? button.palette.mid
                    : (button.hovered ? root.raisedSurface : "transparent"))
            border.width: button.primary ? 1 : 0
            border.color: button.primary
                ? Qt.lighter(button.palette.highlight, 1.18)
                : "transparent"
            opacity: button.enabled ? 1 : 0.4
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: 11
        color: root.palette.window
        border.width: 1
        border.color: root.outlineColor
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.topMargin: 9
            Layout.bottomMargin: 10
            spacing: 11

            Rectangle {
                Layout.preferredWidth: 72
                Layout.preferredHeight: 72
                radius: 10
                color: root.raisedSurface
                border.width: 1
                border.color: root.outlineColor

                DragHandler {
                    target: null
                    acceptedButtons: Qt.LeftButton
                    onActiveChanged: if (active) root.startSystemMove()
                }

                Image {
                    anchors.centerIn: parent
                    width: 47
                    height: 47
                    source: Qt.resolvedUrl("icons/kog.svg")
                    sourceSize.width: 94
                    sourceSize.height: 94
                    fillMode: Image.PreserveAspectFit
                    mipmap: true
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 1

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 3
                    Label {
                        objectName: "miniTitle"
                        Layout.fillWidth: true
                        text: root.app.now_title
                        textFormat: Text.PlainText
                        color: root.palette.text
                        font.pixelSize: 14
                        font.bold: true
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: root.subtitle
                        textFormat: Text.PlainText
                        color: root.palette.placeholderText
                        font.pixelSize: 11
                        elide: Text.ElideRight
                    }
                    DragHandler {
                        target: null
                        acceptedButtons: Qt.LeftButton
                        onActiveChanged: if (active) root.startSystemMove()
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Slider {
                        Layout.fillWidth: true
                        from: 0
                        to: Math.max(1, root.app.duration_seconds)
                        value: root.app.position_seconds
                        enabled: root.app.current_index >= 0
                        Accessible.name: qsTr("Playback position")
                        onMoved: root.app.seek(value)
                    }
                    Label {
                        Layout.preferredWidth: 72
                        text: root.timeLabel(root.app.position_seconds)
                            + " / " + root.timeLabel(root.app.duration_seconds)
                        color: root.palette.placeholderText
                        horizontalAlignment: Text.AlignRight
                        font.pixelSize: 10
                        font.family: "monospace"
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 3

                    MiniButton {
                        objectName: "miniPrevious"
                        iconName: "media-skip-backward"
                        enabled: root.app.playlist_count > 0
                        toolTip: qsTr("Previous")
                        onClicked: root.app.previous()
                    }
                    MiniButton {
                        objectName: "miniPlayPause"
                        primary: true
                        iconName: root.app.playback_state === "playing"
                            ? "media-playback-pause"
                            : "media-playback-start"
                        enabled: root.app.playlist_count > 0
                        toolTip: root.app.playback_state === "playing"
                            ? qsTr("Pause") : qsTr("Play")
                        onClicked: root.app.play_pause()
                    }
                    MiniButton {
                        objectName: "miniStop"
                        iconName: "media-playback-stop"
                        enabled: root.app.current_index >= 0
                            && root.app.playback_state !== "stopped"
                        toolTip: qsTr("Stop")
                        onClicked: root.app.stop()
                    }
                    MiniButton {
                        objectName: "miniNext"
                        iconName: "media-skip-forward"
                        enabled: root.app.playlist_count > 0
                        toolTip: qsTr("Next")
                        onClicked: root.app.next()
                    }
                    MiniButton {
                        objectName: "miniRestore"
                        iconName: "view-restore"
                        toolTip: qsTr("Return to full player")
                        onClicked: root.restoreFullPlayer()
                    }

                    Item { Layout.fillWidth: true }

                    Label {
                        text: qsTr("Volume")
                        color: root.palette.placeholderText
                        font.pixelSize: 10
                    }
                    Slider {
                        Layout.preferredWidth: 82
                        from: 0
                        to: 1
                        value: root.app.volume
                        Accessible.name: qsTr("Volume")
                        onMoved: root.app.set_volume_level(value)
                    }
                }
            }
        }
    }
}
