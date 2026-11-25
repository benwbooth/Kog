import QtQuick
import QtQuick.Controls.Basic

// The window's menu button, grove's "⋮" toolbar button, shared by Ben's Qt
// Quick apps (see MacMenuStyle.qml): three vertical dots, drawn
// pixel-exact, on a rounded fill that shows on hover and stays down while its
// menu is open. Set `menu` (an AppMenu or MacMenu) and clicks open it below
// the button and close it again; leave it null and handle onClicked.
AbstractButton {
    id: b
    property bool dark: true
    property color fg: dark ? "#ffffff" : "#1d1d1f"
    property string tip: qsTr("Menu")
    property var menu: null
    // (Down while its menu is open; set it when the app opens the menu itself.)
    property bool menuOpen: !!menu && menu.visible
    readonly property real pt: 1.125

    implicitHeight: Math.round(28 * pt)
    implicitWidth: implicitHeight + 4
    hoverEnabled: true
    focusPolicy: Qt.NoFocus
    opacity: enabled ? 1 : 0.35

    // A second click closes the menu: the press outside already closed it,
    // so don't open it again at once.
    property double _closedAt: 0
    Connections {
        target: b.menu
        ignoreUnknownSignals: true
        function onClosed() { b._closedAt = Date.now() }
    }
    // Just before the button opens its menu (to note the focus to give back).
    signal aboutToOpen()
    function toggle() {
        if (!menu) return
        if (menu.visible || Date.now() - _closedAt < 250) { menu.close(); return }
        aboutToOpen()
        if (typeof menu.openBelow === "function") menu.openBelow(b)
        else menu.popup(b, 0, b.height)
    }
    onClicked: if (menu) toggle()

    background: Rectangle {
        radius: Math.round(6 * b.pt)
        color: b.down || b.menuOpen ? (b.dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.10))
             : b.hovered ? (b.dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(0, 0, 0, 0.05))
             : "transparent"
        Behavior on color { ColorAnimation { duration: 90 } }
    }
    contentItem: Item {
        // Three dots, drawn here (not an icon-theme symbol: Kog has no icon
        // theme, and Breeze's art differs from size to size), each on whole
        // device pixels: 3 px dots, one dot apart, at 4/3.
        Item {
            id: icon
            readonly property real dpr: {
                const w = b.Window.window
                return w && w.devicePixelRatio > 0 ? w.devicePixelRatio : (Screen.devicePixelRatio || 1)
            }
            readonly property int dot: Math.max(2, Math.round(2.25 * dpr))
            readonly property int step: 2 * dot
            width: dot / dpr
            height: (2 * step + dot) / dpr
            x: Math.round((parent.width - width) / 2)
            y: Math.round((parent.height - height) / 2)
            Repeater {
                model: 3
                Rectangle {
                    y: index * icon.step / icon.dpr
                    width: icon.dot / icon.dpr
                    height: width
                    radius: width / 2
                    color: b.fg
                    antialiasing: true
                }
            }
            transform: Translate { id: gridSnap }
            function snap() {
                if (!visible) return
                const p = mapToItem(null, 0, 0)
                const rx = p.x - gridSnap.x, ry = p.y - gridSnap.y
                gridSnap.x = Math.round(rx * dpr) / dpr - rx
                gridSnap.y = Math.round(ry * dpr) / dpr - ry
            }
            onDprChanged: Qt.callLater(snap)
            Component.onCompleted: Qt.callLater(snap)
        }
    }
    // (The dots sit on the device pixel grid: re-snap them when the button
    // moves, which their own x/y don't always show.)
    onXChanged: Qt.callLater(icon.snap)
    onYChanged: Qt.callLater(icon.snap)
    onWidthChanged: Qt.callLater(icon.snap)
    onHoveredChanged: icon.snap()

    ToolTip.visible: hovered && tip !== "" && !menuOpen
    ToolTip.delay: 700
    ToolTip.text: tip
}
