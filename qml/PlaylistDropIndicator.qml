import QtQuick

// A viewport overlay: headers, tabs and horizontal scrolling must not move
// the marker away from the gap where the tracks will be inserted.
Item {
    id: indicator
    required property var view
    required property int target
    required property color color
    property real rowHeight: 24
    property real rightInset: 0
    property real bottomInset: 0
    parent: view
    width: view.width
    height: Math.max(0, view.height - bottomInset)
    z: 50
    clip: true
    visible: view.visible && target >= 0 && target <= view.count

    Rectangle {
        objectName: "insertionLine"
        x: 4
        y: Math.max(0, Math.min(indicator.height - height,
            indicator.target * indicator.rowHeight + indicator.view.originY
                - indicator.view.contentY - height / 2))
        width: Math.max(0, indicator.width - indicator.rightInset - 8)
        height: 3
        radius: 1
        color: indicator.color
    }
}
