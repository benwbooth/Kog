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

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal dragStarted(int rowIndex)
    signal dragMoved(real viewX, real viewY)
    signal dragFinished(real viewX, real viewY)
    signal dragCanceled()

    implicitHeight: 24
    height: implicitHeight

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

    component Cell: Text {
        required property var column

        width: column.width
        height: root.height
        leftPadding: 6
        rightPadding: 6
        color: root.selected ? root.theme.highlightedText : root.theme.text
        font.pixelSize: 11
        horizontalAlignment: column.alignment
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
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
