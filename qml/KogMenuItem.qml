import QtQuick
import QtQuick.Templates as T

// A row of a KogMenu, in the ☰ menu's glass look (AppMenu.qml's glassItem):
// a 16 pt icon, the text, the action's key at the right, a › for a submenu;
// the accent colour fills the row under the pointer. A checked row gets a
// soft accent square behind its icon, or a ✓ when it has none. The look's
// colours and sizes come from the KogMenu it is in.
T.MenuItem {
    id: control

    // The key shown at the right: the action's shortcut unless set.
    property string shortcutText: itemShortcut.nativeText
    // "name|fallback": the first name is used.
    readonly property string iconName: (icon.name || "").split("|")[0]

    readonly property var look: menu && menu.rowHeight !== undefined ? menu : null
    readonly property bool dark: look ? look.dark : palette.window.hslLightness < 0.5
    readonly property color accent: look ? look.accent : palette.highlight
    readonly property real pt: 1.125
    readonly property color fg: look ? look.fg : (dark ? "#f5f5f7" : "#1d1d1f")
    readonly property color fgSecondary: look ? look.fgSecondary
        : (dark ? Qt.rgba(1, 1, 1, 0.55) : Qt.rgba(0, 0, 0, 0.5))
    readonly property color fgDisabled: look ? look.fgDisabled
        : (dark ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(0, 0, 0, 0.26))
    readonly property bool lit: highlighted && enabled

    implicitHeight: visible ? (look ? look.rowHeight : Math.round(24 * pt)) : 0
    implicitWidth: Math.max(Math.round(200 * pt), lbl.implicitWidth + sc.implicitWidth + 70)
    // Hidden rows take no room.
    height: visible ? undefined : 0
    leftPadding: 8
    rightPadding: 8
    hoverEnabled: true

    background: Rectangle {
        radius: Math.round(8 * control.pt)
        color: control.lit ? control.accent : "transparent"
    }

    indicator: Item {}

    arrow: Text {
        visible: control.subMenu !== null
        x: control.width - width - 8
        anchors.verticalCenter: parent.verticalCenter
        text: "›"
        font.family: lbl.font.family
        font.pixelSize: Math.round(15 * control.pt)
        renderType: Text.NativeRendering
        color: control.lit ? "#ffffff" : control.fgSecondary
    }

    contentItem: Item {
        implicitWidth: lbl.implicitWidth + sc.implicitWidth + 60
        // A toggle that's on: a soft accent square behind its icon (a ✓
        // when it has none).
        Rectangle {
            visible: control.checkable && control.checked && control.iconName !== ""
            x: -1
            width: 22; height: 22; radius: 5
            anchors.verticalCenter: parent.verticalCenter
            color: control.lit ? Qt.rgba(1, 1, 1, 0.25)
                : Qt.rgba(control.accent.r, control.accent.g, control.accent.b, 0.28)
        }
        GlassText {
            visible: control.checkable && control.checked && control.iconName === ""
            x: 4
            anchors.verticalCenter: parent.verticalCenter
            text: "✓"
            color: control.lit ? "#ffffff" : control.fg
        }
        AppMenu.CrispIcon {
            id: rowIcon
            visible: control.iconName !== ""
            name: control.iconName
            x: 2 + (16 - width) / 2
            anchors.verticalCenter: parent.verticalCenter
            opacity: control.enabled ? 1 : 0.4
            // Placed on the pixel grid again whenever the menu opens or
            // moves (it opens at a new spot, then is moved to stay inside
            // the window).
            Connections {
                target: control.menu
                ignoreUnknownSignals: true
                function onOpened() { Qt.callLater(rowIcon.snap) }
                function onXChanged() { Qt.callLater(rowIcon.snap) }
                function onYChanged() { Qt.callLater(rowIcon.snap) }
            }
        }
        GlassText {
            id: lbl
            x: 24
            width: parent.width - x - sc.width - 8
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            text: control.text
            color: !control.enabled ? control.fgDisabled : control.lit ? "#ffffff" : control.fg
        }
        GlassText {
            id: sc
            anchors.right: parent.right
            anchors.rightMargin: control.subMenu ? 18 : 4
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            text: control.shortcutText
            color: control.lit ? "#ffffff" : control.fgSecondary
        }
    }

    // Only reads the action's key for display; the action owns the shortcut.
    Shortcut {
        id: itemShortcut
        enabled: false
        sequence: control.action && control.action.shortcut !== undefined
            ? control.action.shortcut : ""
    }

    component GlassText: Text {
        font.family: control.look ? control.look.family : control.font.family
        font.pixelSize: control.look ? control.look.fontSize : Math.round(13 * control.pt * 10) / 10
        renderType: Text.NativeRendering
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        textFormat: Text.PlainText
    }
}
