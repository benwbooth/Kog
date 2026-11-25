import QtQuick
import QtQuick.Controls.Basic

// The draggable part of a window's own title bar (toolbar), behaving as
// arbor's title bars do (shared by Ben's Qt Quick apps; see ChromeButtons):
// drag to move (a maximized window is restored first), double-click to
// maximize or restore, right-click for the
// window menu (Minimize, Maximize/Restore, Keep Above Others, Close).
// Put it behind the bar's controls: they take their own clicks.
//     ChromeTitleArea { anchors.fill: parent; window: win }
Item {
    id: ta
    required property var window
    // The window menu's look (MacMenu): set from the app's theme.
    property alias menuDark: winMenu.dark
    property alias menuAccent: winMenu.accent
    readonly property bool maximized: window && window.visibility === Window.Maximized
    function toggleMaximized() { maximized ? window.showNormal() : window.showMaximized() }

    DragHandler {
        target: null
        acceptedButtons: Qt.LeftButton
        grabPermissions: PointerHandler.CanTakeOverFromAnything
        // (Only a real drag activates it, so this never races the
        // double-click. A maximized window is restored first, then moved.)
        onActiveChanged: {
            if (!active) return
            if (ta.maximized) ta.window.showNormal()
            ta.window.startSystemMove()
        }
    }
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onDoubleTapped: ta.toggleMaximized()
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (point) => winMenu.popup(ta, point.position.x, point.position.y)
    }
    MacMenu {
        id: winMenu
        MacMenuItem { text: qsTr("Minimize"); icon.name: "window-minimize"; onTriggered: ta.window.showMinimized() }
        MacMenuItem {
            text: ta.maximized ? qsTr("Restore") : qsTr("Maximize")
            icon.name: ta.maximized ? "window-restore" : "window-maximize"
            onTriggered: ta.toggleMaximized()
        }
        MacMenuItem {
            text: qsTr("Keep Above Others")
            icon.name: "window-keep-above"
            checkable: true
            checked: ta.window && (ta.window.flags & Qt.WindowStaysOnTopHint) !== 0
            onTriggered: ta.window.flags = checked ? (ta.window.flags | Qt.WindowStaysOnTopHint)
                                                   : (ta.window.flags & ~Qt.WindowStaysOnTopHint)
        }
        MacMenuSeparator {}
        MacMenuItem { text: qsTr("Close"); icon.name: "window-close"; onTriggered: ta.window.close() }
    }
}
