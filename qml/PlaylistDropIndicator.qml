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
        // Rows can vary in height: the gap is at the laid out row's edge
        // when there is one, else estimated from rowHeight.
        y: {
            const view = indicator.view
            const count = view.count
            const item = count > 0 && view.itemAtIndex
                ? view.itemAtIndex(Math.min(indicator.target, count - 1)) : null
            const edge = item ? (indicator.target < count ? item.y : item.y + item.height)
                : indicator.target * indicator.rowHeight + view.originY
            return Math.max(0, Math.min(indicator.height - height,
                edge - view.contentY - height / 2))
        }
        width: Math.max(0, indicator.width - indicator.rightInset - 8)
        height: 3
        radius: 1
        color: indicator.color
    }
}
