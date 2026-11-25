import QtQuick
import QtQuick.Controls.Basic

// The window's menu button, grove's "⋮" toolbar button, shared by Ben's Qt
// Quick apps (see MacMenuStyle.qml): a 16 pt vertical-dots symbol, drawn
// pixel-exact, on a rounded fill that shows on hover and stays down while its
// menu is open. Set `menu` (an AppMenu or MacMenu) and clicks open it below
// the button and close it again; leave it null and handle onClicked.
AbstractButton {
    id: b
    property bool dark: true
    property color fg: dark ? "#ffffff" : "#1d1d1f"
    property string tip: qsTr("Menu")
    property string iconName: "overflow-menu-symbolic|view-more-symbolic|application-menu"
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
        MacMenuIcon {
            id: icon
            name: b.iconName
            size: 16
            color: b.fg
            x: Math.round((parent.width - width) / 2)
            y: Math.round((parent.height - height) / 2)
        }
    }
    // (The icon sits on the device pixel grid: re-snap it when the button
    // moves, which its own x/y don't always show.)
    onXChanged: Qt.callLater(icon.snap)
    onYChanged: Qt.callLater(icon.snap)
    onWidthChanged: Qt.callLater(icon.snap)
    onHoveredChanged: icon.snap()

    ToolTip.visible: hovered && tip !== "" && !menuOpen
    ToolTip.delay: 700
    ToolTip.text: tip
}
