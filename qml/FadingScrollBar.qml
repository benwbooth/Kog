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
    // Space kept between the pill and the bar's outer edge, for a bar at
    // the window's edge, where the window's resize border takes the pointer.
    property real edgeInset: 0
    implicitWidth: orientation === Qt.Vertical ? 8 + edgeInset : 0
    implicitHeight: orientation === Qt.Horizontal ? 8 : 0
    background: Item {}
    // The control sizes contentItem to the whole handle area; the pill is
    // drawn inside it, against the side away from the edge inset.
    contentItem: Item {
        implicitWidth: 4
        implicitHeight: 4
        Rectangle {
            readonly property real thickness: bar.hovered || bar.pressed ? 7 : 4
            readonly property bool vertical: bar.orientation === Qt.Vertical
            width: vertical ? thickness : parent.width
            height: vertical ? parent.height : thickness
            x: vertical ? parent.width - width - bar.edgeInset : 0
            y: vertical ? 0 : parent.height - height
            radius: thickness / 2
            color: bar.palette.text
            opacity: bar.pressed ? 0.6 : 0.4
            Behavior on width { enabled: bar.orientation === Qt.Vertical; NumberAnimation { duration: 120 } }
            Behavior on height { enabled: bar.orientation === Qt.Horizontal; NumberAnimation { duration: 120 } }
        }
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
