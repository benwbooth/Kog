import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.settings

Window {
    id: root

    required property var app
    signal openPlayer()
    property int displayDuration: 8000
    property alias settingsFile: placement.fileName
    // Saved top-left position; -1 means auto-place bottom-right. Only
    // meaningful where the platform lets clients position windows (X11
    // and offscreen); Wayland compositors place new windows themselves.
    property real posX: placement.posX
    property real posY: placement.posY
    readonly property bool dragging: moveHandler.active
    readonly property bool pointerInside: hover.hovered
    readonly property bool playing: app.playback_state === "playing"
    readonly property string detail: [app.now_artist, app.current_album]
        .filter(value => value.length > 0).join("  ·  ")
    readonly property real progress: app.duration_seconds > 0
        ? Math.max(0, Math.min(1, app.position_seconds / app.duration_seconds)) : 0

    objectName: "kogNowPlayingNotification"
    title: qsTr("Kog — Now Playing")
    width: 420
    height: 186
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
        | Qt.WindowDoesNotAcceptFocus
    // A notification must also work while the main player is hidden to tray.
    transientParent: null

    Settings {
        id: placement
        category: "NowPlayingNotification"
        property real posX: -1
        property real posY: -1
    }

    function applyPosition() {
        const availX = screen.virtualX
        const availY = screen.virtualY
        const availW = screen.desktopAvailableWidth
        const availH = screen.desktopAvailableHeight
        if (posX >= 0 && posY >= 0) {
            x = Math.max(availX, Math.min(posX, availX + availW - width))
            y = Math.max(availY, Math.min(posY, availY + availH - height))
        } else {
            x = availX + availW - width - 16
            y = availY + availH - height - 16
        }
    }

    function resetPosition() {
        posX = -1
        posY = -1
        placement.setValue("posX", -1)
        placement.setValue("posY", -1)
        placement.sync()
        applyPosition()
    }

    function savePosition() {
        posX = x
        posY = y
        // Settings batches property writes; persist immediately so quitting
        // straight after a drag does not lose the new position.
        placement.setValue("posX", posX)
        placement.setValue("posY", posY)
        placement.sync()
    }

    function timeLabel(seconds) {
        const total = Math.max(0, Math.floor(seconds))
        const minutes = Math.floor(total / 60)
        return minutes + ":" + String(total % 60).padStart(2, "0")
    }

    function present() {
        if (app.current_index < 0)
            return
        applyPosition()
        show()
        if (!pointerInside && !dragging)
            dismissTimer.restart()
    }

    function dismiss() {
        dismissTimer.stop()
        hide()
    }

    onClosing: event => {
        event.accepted = false
        dismiss()
    }
    onVisibleChanged: {
        if (visible)
            entrance.restart()
        else
            dismissTimer.stop()
    }

    Timer {
        id: dismissTimer
        interval: root.displayDuration
        onTriggered: root.dismiss()
    }

    HoverHandler {
        id: hover
        parent: root.contentItem
        onHoveredChanged: {
            if (hovered || root.dragging)
                dismissTimer.stop()
            else if (root.visible)
                dismissTimer.restart()
        }
    }

    component TransportButton: CogButton {
        id: button
        property bool primary: false
        focusPolicy: Qt.NoFocus
        implicitWidth: primary ? 38 : 32
        implicitHeight: primary ? 38 : 32
        iconBackground: primary ? palette.highlight : root.palette.window

        // Render the bundled SVG directly: some desktop ToolButton styles
        // discard their icon content when given a custom background.
        contentItem: Image {
            objectName: "notificationButtonIcon"
            source: button.icon.source
            sourceSize: Qt.size(button.icon.width, button.icon.height)
            fillMode: Image.Pad
            opacity: button.enabled ? 1 : 0.4
        }

        background: Rectangle {
            radius: button.primary ? height / 2 : 6
            color: button.primary ? button.palette.highlight
                : (button.down ? button.palette.mid
                    : (button.hovered ? button.palette.alternateBase : "transparent"))
            opacity: button.enabled ? 1 : 0.4
        }
    }

    Rectangle {
        id: card
        anchors.fill: parent
        anchors.margins: 4
        radius: 12
        color: root.palette.window
        border.color: root.palette.mid
        border.width: 1

        NumberAnimation {
            id: entrance
            target: card
            property: "opacity"
            from: 0
            to: 1
            duration: 140
            easing.type: Easing.OutCubic
        }
        DragHandler {
            id: moveHandler
            objectName: "notificationDragHandler"
            target: null
            acceptedButtons: Qt.LeftButton
            onActiveChanged: {
                // Active requires a real drag: presses and clicks never
                // activate the handler. The compositor moves the window
                // itself, so there is no follow math to fight over: this
                // is the same mechanism as the main window's title bar.
                if (active) {
                    dismissTimer.stop()
                    root.startSystemMove()
                } else {
                    root.savePosition()
                    if (!root.pointerInside && root.visible)
                        dismissTimer.restart()
                }
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 10

            RowLayout {
                id: header
                objectName: "notificationHeader"
                Layout.fillWidth: true
                spacing: 7
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: positionMenu.popup()
                }
                Menu {
                    id: positionMenu
                    MenuItem {
                        text: qsTr("Reset position above tray")
                        onTriggered: root.resetPosition()
                    }
                }
                Image {
                    source: Qt.resolvedUrl("icons/kog.svg")
                    sourceSize: Qt.size(20, 20)
                    Layout.preferredWidth: 18
                    Layout.preferredHeight: 18
                }
                Label {
                    text: qsTr("Kog")
                    font.bold: true
                    font.pixelSize: 11
                }
                Label {
                    text: root.playing ? qsTr("Now playing")
                        : (root.app.playback_state === "paused" ? qsTr("Paused") : qsTr("Stopped"))
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }
                Item { Layout.fillWidth: true }
                TransportButton {
                    objectName: "dismissNotification"
                    iconName: "window-close"
                    toolTip: qsTr("Dismiss")
                    implicitWidth: 24
                    implicitHeight: 24
                    icon.width: 14
                    icon.height: 14
                    onClicked: root.dismiss()
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 3
                Label {
                    objectName: "notificationTitle"
                    Layout.fillWidth: true
                    text: root.app.now_title
                    textFormat: Text.PlainText
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    ToolTip.visible: titleHover.hovered && truncated
                    ToolTip.text: text
                    ToolTip.delay: 550
                    HoverHandler { id: titleHover }
                    TapHandler { onTapped: { root.openPlayer(); root.dismiss() } }
                }
                Label {
                    Layout.fillWidth: true
                    text: root.detail.length > 0 ? root.detail : qsTr("Local music · Kog")
                    textFormat: Text.PlainText
                    color: root.palette.placeholderText
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 4
                Label {
                    text: root.timeLabel(root.app.position_seconds)
                        + " / " + root.timeLabel(root.app.duration_seconds)
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                    Layout.fillWidth: true
                }
                TransportButton {
                    objectName: "notificationPrevious"
                    iconName: "media-skip-backward"
                    toolTip: qsTr("Previous")
                    enabled: root.app.playlist_count > 0
                    onClicked: root.app.previous()
                }
                TransportButton {
                    objectName: "notificationPlayPause"
                    primary: true
                    iconName: root.playing ? "media-playback-pause" : "media-playback-start"
                    toolTip: root.playing ? qsTr("Pause") : qsTr("Play")
                    enabled: root.app.playlist_count > 0
                    onClicked: root.app.play_pause()
                }
                TransportButton {
                    objectName: "notificationStop"
                    iconName: "media-playback-stop"
                    toolTip: qsTr("Stop")
                    enabled: root.app.current_index >= 0 && root.app.playback_state !== "stopped"
                    onClicked: root.app.stop()
                }
                TransportButton {
                    objectName: "notificationNext"
                    iconName: "media-skip-forward"
                    toolTip: qsTr("Next")
                    enabled: root.app.playlist_count > 0
                    onClicked: root.app.next()
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 3
                radius: 1.5
                color: root.palette.alternateBase
                Rectangle {
                    width: parent.width * root.progress
                    height: parent.height
                    radius: parent.radius
                    color: root.palette.highlight
                }
            }
        }
    }
}
