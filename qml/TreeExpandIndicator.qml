import QtQuick
import QtQuick.Controls

// Fixed-size tree expander, independent of the active Qt Controls style.
Item {
    id: mark
    required property TreeViewDelegate control
    objectName: "treeExpandIndicator"
    implicitWidth: 16
    implicitHeight: 16
    width: 16
    height: 16
    x: control.leftMargin + control.depth * control.indentation
    y: (control.height - height) / 2
    visible: control.isTreeNode && control.hasChildren

    readonly property color stroke: control.selected
        ? control.palette.highlightedText : control.palette.text

    Rectangle {
        anchors.centerIn: parent
        width: 12
        height: 12
        radius: 2
        color: "transparent"
        border.width: 1
        border.color: mark.stroke
    }
    Rectangle {
        anchors.centerIn: parent
        width: 6
        height: 1
        color: mark.stroke
    }
    Rectangle {
        anchors.centerIn: parent
        width: 1
        height: 6
        color: mark.stroke
        visible: !mark.control.expanded
    }
}
