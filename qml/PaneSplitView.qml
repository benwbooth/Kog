import QtQuick
import QtQuick.Controls

SplitView {
    id: root
    // Some platform styles extend the splitter hit area into adjacent panes.
    // Keep it inside this gutter so it cannot steal scrollbar presses.
    handle: Rectangle {
        objectName: "paneDivider"
        implicitWidth: 8
        implicitHeight: 8
        color: SplitHandle.pressed ? root.palette.highlight
            : (SplitHandle.hovered ? root.palette.mid : root.palette.window)
        Rectangle {
            anchors.centerIn: parent
            width: root.orientation === Qt.Horizontal ? 1 : parent.width
            height: root.orientation === Qt.Horizontal ? parent.height : 1
            color: root.palette.mid
        }
    }
}
