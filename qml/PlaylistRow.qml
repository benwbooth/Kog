pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: root

    required property var app
    required property int rowIndex
    required property var columns
    required property var theme
    property bool selected: false
    property bool hovered: false
    property string dragRows: ""
    readonly property bool playlistDrag: true
    property int revision: app.playlist_revision

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal dragStarted(int rowIndex)

    implicitHeight: 24
    height: implicitHeight

    Drag.active: rowDrag.active
    Drag.dragType: Drag.Automatic
    Drag.keys: ["kog-playlist-row"]
    Drag.supportedActions: Qt.MoveAction
    Drag.proposedAction: Qt.MoveAction
    Drag.source: root
    Drag.mimeData: ({
        "application/x-kog-playlist-rows": root.dragRows
    })
    Drag.hotSpot.x: width / 2
    Drag.hotSpot.y: height / 2

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
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true
        onEntered: root.hovered = true
        onExited: root.hovered = false
        onClicked: mouse => root.pressed(root.rowIndex, mouse.modifiers, mouse.button)
        onDoubleClicked: root.activated(root.rowIndex)
    }

    DragHandler {
        id: rowDrag
        target: null
        acceptedButtons: Qt.LeftButton
        grabPermissions: PointerHandler.CanTakeOverFromAnything
            | PointerHandler.ApprovesTakeOverByAnything
        onActiveChanged: if (active) root.dragStarted(root.rowIndex)
    }
}
