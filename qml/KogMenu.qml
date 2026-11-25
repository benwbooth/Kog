import QtQuick
import QtQuick.Controls
import QtQuick.Templates as T

// Kog's right-click and drop-down menus, in the same glass look as the ☰
// menu (AppMenu.qml, grove's and hearth's menus): a 12 pt rounded panel
// with a soft rim and shadow, 24 pt rows, icons in a column, the accent
// colour on the highlighted row, the key at the right. Every menu in Kog is
// a KogMenu with KogMenuItem rows and KogMenuSeparator lines; nested
// KogMenus become submenus in the same look.
//
// Drawn inside the window (Popup.Item): a separate popup window gets
// another scale from arbor at 4/3 and blurs.
T.Menu {
    id: control

    // ---- look, shared with the rows (KogMenuItem reads it from `menu`) ----
    property bool dark: palette.window.hslLightness < 0.5
    property color accent: palette.highlight
    readonly property real pt: 1.125   // hearth's point: 1.125 logical px
    readonly property string family: {
        const f = Qt.fontFamilies()
        for (const n of ["SF Pro", "SF Pro Text", "Inter", "Inter Variable"]) if (f.indexOf(n) >= 0) return n
        return font.family
    }
    readonly property real fontSize: Math.round(13 * pt * (family.indexOf("Inter") === 0 ? 0.962 : 1) * 10) / 10
    readonly property color fg: dark ? "#f5f5f7" : "#1d1d1f"
    readonly property color fgSecondary: dark ? Qt.rgba(1, 1, 1, 0.55) : Qt.rgba(0, 0, 0, 0.5)
    readonly property color fgDisabled: dark ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(0, 0, 0, 0.26)
    readonly property color separatorColor: dark ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(0, 0, 0, 0.09)
    readonly property color glassTint: dark ? Qt.rgba(0.17, 0.175, 0.19, 0.995) : Qt.rgba(0.975, 0.975, 0.98, 0.995)
    readonly property color glassRim: dark ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(1, 1, 1, 0.85)
    readonly property color glassEdge: dark ? Qt.rgba(0, 0, 0, 0.6) : Qt.rgba(0, 0, 0, 0.13)
    readonly property int radius: Math.round(12 * pt)
    readonly property int rowHeight: Math.round(24 * pt)
    readonly property int menuPad: Math.round(5 * pt)

    popupType: Popup.Item

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding)

    // The .001 keeps rows off exact half device pixels at fractional scales
    // (the overlay is outside the window's own nudge).
    topPadding: menuPad + 0.001
    bottomPadding: menuPad
    leftPadding: menuPad + 0.001
    rightPadding: menuPad
    overlap: 3
    cascade: true
    // Kept inside the window; a menu taller than the room scrolls.
    margins: Math.round(4 * pt)

    delegate: KogMenuItem {}

    contentItem: ListView {
        id: listview
        implicitWidth: Math.max(1, contentItem.children.reduce(
            (w, child) => Math.max(w, child.implicitWidth), 0))
        implicitHeight: Math.max(1, contentItem.visibleChildren.reduce(
            (h, item) => h + item.implicitHeight, 1))
        model: control.contentModel
        spacing: 0
        interactive: Window.window
            ? contentHeight + control.topPadding + control.bottomPadding > Window.window.height
            : false
        clip: true
        currentIndex: control.currentIndex
        keyNavigationEnabled: true
        keyNavigationWraps: true
        ScrollIndicator.vertical: ScrollIndicator {}
    }

    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120; easing.type: Easing.OutCubic } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 140 } }

    background: Item {
        implicitWidth: Math.round(200 * control.pt)
        Rectangle {   // shadow
            anchors.fill: parent
            anchors.topMargin: 3
            anchors.margins: -1
            radius: control.radius + 1
            color: "transparent"
            border.width: 3
            border.color: Qt.rgba(0, 0, 0, control.dark ? 0.22 : 0.06)
        }
        Rectangle {
            anchors.fill: parent
            radius: control.radius
            color: control.glassTint
            border.width: 1
            border.color: control.glassEdge
        }
        Rectangle {   // sheen in the top part
            anchors.fill: parent
            anchors.margins: 1
            radius: control.radius - 1
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, control.dark ? 0.05 : 0.25) }
                GradientStop { position: 0.35; color: Qt.rgba(1, 1, 1, 0) }
            }
        }
        Rectangle {   // specular rim, lit from the top left
            anchors.fill: parent
            anchors.margins: 1
            radius: control.radius - 1
            color: "transparent"
            border.width: 1
            border.color: control.glassRim
            opacity: 0.9
        }
    }
}
