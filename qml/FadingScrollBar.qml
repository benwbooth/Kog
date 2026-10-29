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
    opacity: hovered || pressed || scrolling ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 150 } }
}
