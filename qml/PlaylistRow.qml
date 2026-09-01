pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

Item {
    id: root

    required property var app
    required property int rowIndex
    required property var columns
    required property var theme
    property bool selected: false
    property bool hovered: false
    property int revision: app.playlist_revision
    readonly property string statusMessage: {
        revision
        return app.track_status_message_at(rowIndex)
    }
    readonly property bool isCurrentTrack: app.current_index >= 0
        && Number(app.track_number_at(rowIndex)) === app.current_index + 1
    readonly property bool isPlaying: isCurrentTrack
        && app.playback_state === "playing"

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal dragStarted(int rowIndex)
    signal dragMoved(real viewX, real viewY)
    signal dragFinished(real viewX, real viewY)
    signal dragCanceled()

    implicitHeight: 24
    height: implicitHeight

    ListView.onPooled: {
        root.hovered = false
        rowPointer.manualDragging = false
        rowPointer.suppressNextClick = false
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        anchors.topMargin: 3
        anchors.bottomMargin: 3
        radius: 4
        color: root.selected
            ? root.theme.highlight
            : (root.hovered
                ? root.theme.button
                : (root.rowIndex % 2 ? root.theme.alternateBase : "transparent"))
    }

    component Cell: Item {
        id: cell

        required property var column
        property string text: ""

        width: column.width
        height: root.height

        Text {
            anchors.fill: parent
            leftPadding: 6
            rightPadding: 6
            text: cell.text
            visible: cell.column.id !== "status" || !root.isPlaying
            color: root.selected ? root.theme.highlightedText : root.theme.text
            font.pixelSize: 11
            horizontalAlignment: cell.column.alignment
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        Loader {
            anchors.centerIn: parent
            width: 16
            height: 14
            active: cell.column.id === "status" && root.isPlaying

            sourceComponent: Item {
                id: playingWaveform

                readonly property var levels: [
                    root.app.audio_level_low,
                    root.app.audio_level_low_mid,
                    root.app.audio_level_mid,
                    root.app.audio_level_high_mid,
                    root.app.audio_level_high
                ]
                readonly property var colors: root.selected
                    ? ["#8cbcff", "#64d8ff", "#47eee7", "#53edb4", "#82ef99"]
                    : ["#438cf5", "#32b8ed", "#20cbd2", "#27cf9c", "#55d979"]

                Rectangle {
                    anchors.fill: parent
                    radius: 4
                    visible: root.selected
                    color: Qt.rgba(0.02, 0.08, 0.11, 0.78)
                    border.width: 1
                    border.color: Qt.rgba(1, 1, 1, 0.18)
                }

                Repeater {
                    model: 5

                    Rectangle {
                        required property int index

                        x: 1 + index * 3
                        y: 1 + 12 - height
                        width: 2
                        height: 2 + 10 * Math.max(0, Math.min(1,
                            playingWaveform.levels[index]))
                        radius: 1
                        color: playingWaveform.colors[index]

                        Behavior on height {
                            NumberAnimation {
                                duration: 70
                                easing.type: Easing.OutCubic
                            }
                        }
                    }
                }
            }
        }
    }

    Row {
        anchors.fill: parent

        Repeater {
            model: root.columns.visibleColumns

            Cell {
                required property var modelData
                column: modelData
                text: {
                    root.revision
                    return root.app.track_value_at(root.rowIndex, modelData.id)
                }
            }
        }
    }

    MouseArea {
        id: rowPointer
        property real pressX: 0
        property real pressY: 0
        property bool manualDragging: false
        property bool suppressNextClick: false

        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true
        preventStealing: true
        onEntered: root.hovered = true
        onExited: root.hovered = false
        onPressed: mouse => {
            pressX = mouse.x
            pressY = mouse.y
            manualDragging = false
        }
        onPositionChanged: mouse => {
            if ((mouse.buttons & Qt.LeftButton) === 0)
                return
            if (!manualDragging
                    && (Math.abs(mouse.x - pressX)
                        >= Application.styleHints.startDragDistance
                        || Math.abs(mouse.y - pressY)
                        >= Application.styleHints.startDragDistance)) {
                manualDragging = true
                root.dragStarted(root.rowIndex)
            }
            if (!manualDragging)
                return
            const point = root.mapToItem(root.ListView.view,
                mouse.x, mouse.y)
            root.dragMoved(point.x, point.y)
        }
        onClicked: mouse => {
            if (suppressNextClick) {
                suppressNextClick = false
                return
            }
            root.pressed(root.rowIndex, mouse.modifiers, mouse.button)
        }
        onDoubleClicked: root.activated(root.rowIndex)
        onReleased: mouse => {
            if (manualDragging) {
                const point = root.mapToItem(root.ListView.view,
                    mouse.x, mouse.y)
                root.dragFinished(point.x, point.y)
                suppressNextClick = true
                mouse.accepted = true
            }
            manualDragging = false
        }
        onCanceled: {
            manualDragging = false
            suppressNextClick = false
            root.dragCanceled()
        }
        ToolTip.visible: containsMouse && root.statusMessage.length > 0
        ToolTip.delay: 650
        ToolTip.text: root.statusMessage
    }

}
