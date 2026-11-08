import QtQuick
import QtQuick.Controls

// A scroll bar that keeps its space whenever the content overflows, so rows
// never shift, but shows only while the pointer is over it, while it is
// dragged, or for a moment after the view scrolls (call flash() when its
// view moves; Kog's wheel handling scrolls content directly, which the bar
// does not see as activity).
ScrollBar {
    id: bar
    policy: size < 1.0 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
    hoverEnabled: true
    property bool scrolling: false
    function flash() {
        if (size < 1.0) {
            scrolling = true
            idle.restart()
        }
    }
    Timer {
        id: idle
        interval: 900
        onTriggered: bar.scrolling = false
    }
    // Thin, as on macOS: a 4px pill that widens to 7px under the pointer,
    // in an 8px strip, so it stays clear of buttons at the rows' ends.
    padding: 2
    implicitWidth: orientation === Qt.Vertical ? 8 : 0
    implicitHeight: orientation === Qt.Horizontal ? 8 : 0
    background: Item {}
    contentItem: Rectangle {
        readonly property real thickness: bar.hovered || bar.pressed ? 7 : 4
        implicitWidth: bar.orientation === Qt.Vertical ? thickness : 0
        implicitHeight: bar.orientation === Qt.Horizontal ? thickness : 0
        x: bar.orientation === Qt.Vertical ? bar.width - width - 1 : 0
        radius: thickness / 2
        color: bar.palette.text
        opacity: bar.pressed ? 0.6 : 0.4
        Behavior on implicitWidth { NumberAnimation { duration: 120 } }
        Behavior on implicitHeight { NumberAnimation { duration: 120 } }
    }
    opacity: hovered || pressed || scrolling ? 1 : 0
    // Quick to appear, slow to fade away.
    Behavior on opacity {
        NumberAnimation {
            duration: opacity < 0.5 ? 350 : 1200
            easing.type: Easing.OutCubic
        }
    }
}
