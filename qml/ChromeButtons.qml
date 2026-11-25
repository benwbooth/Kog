import QtQuick
import QtQuick.Controls.Basic

// Minimize, maximize/restore and close for a window that draws its own title
// bar, exactly as arbor draws Breeze's on its title bars (shared by Ben's Qt
// Quick apps; canonical copy in hearth/shared/app-menus, copied into each app
// by hearth's scripts/sync-app-menus.sh): 18 px circles 24 px apart, close
// last at the right, thin stroked icons (⌄ minimize, ⌃ maximize, ◇ restore,
// ✕ close); the one under the pointer gets a round highlight in the text
// colour, close a red one (#da4453) with a white ✕.
//     ChromeButtons { window: win; fg: Theme.fg; bg: Theme.toolbar; anchors.right: parent.right }
Row {
    id: wc
    required property var window
    // The icons' colour, and the hover highlight's.
    property color fg: palette.windowText
    // The icon's colour on that highlight (the bar's own colour).
    property color bg: palette.window
    property bool showTips: true
    readonly property bool maximized: window && (window.visibility === Window.Maximized || window.visibility === Window.FullScreen)
    spacing: 6
    // (Close's centre 15 px from the right edge, as arbor's: anchor the row to
    // the bar's right edge with no margin.)
    rightPadding: 6

    // One 1.1 px round-capped stroke from (x1, y1) to (x2, y2) on the
    // 18-unit grid (Rectangles: Shapes don't draw on every backend).
    component Stroke: Rectangle {
        property real x1; property real y1; property real x2; property real y2
        property color ink
        readonly property real len: Math.hypot(x2 - x1, y2 - y1)
        width: len + 1.1
        height: 1.1
        radius: 0.55
        antialiasing: true
        color: ink
        x: (x1 + x2) / 2 - width / 2
        y: (y1 + y2) / 2 - height / 2
        rotation: Math.atan2(y2 - y1, x2 - x1) * 180 / Math.PI
    }

    component WinButton: Item {
        id: b
        property string glyph
        property bool danger: false
        property string tip
        signal hit()
        width: 18
        height: 18
        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        readonly property bool hot: area.containsMouse
        readonly property bool down: area.pressed
        Rectangle {
            anchors.fill: parent
            radius: width / 2
            visible: b.hot || b.down
            color: b.danger ? (b.down ? "#c03045" : "#da4453") : wc.fg
            opacity: b.danger ? 1 : (b.down ? 0.75 : 1)
        }
        Item {
            id: icon
            anchors.fill: parent
            opacity: !wc.window || wc.window.active || b.hot ? 1 : 0.55
            // (Flattened first: overlapping strokes would show darker joints when dimmed.)
            layer.enabled: opacity < 1
            readonly property color ink: (b.hot || b.down) ? (b.danger ? "white" : wc.bg) : wc.fg
            Stroke { ink: icon.ink; visible: b.glyph === "minimize"; x1: 4; y1: 7; x2: 9; y2: 12 }
            Stroke { ink: icon.ink; visible: b.glyph === "minimize"; x1: 9; y1: 12; x2: 14; y2: 7 }
            Stroke { ink: icon.ink; visible: b.glyph === "maximize"; x1: 4; y1: 11; x2: 9; y2: 6 }
            Stroke { ink: icon.ink; visible: b.glyph === "maximize"; x1: 9; y1: 6; x2: 14; y2: 11 }
            Stroke { ink: icon.ink; visible: b.glyph === "close"; x1: 5; y1: 5; x2: 13; y2: 13 }
            Stroke { ink: icon.ink; visible: b.glyph === "close"; x1: 13; y1: 5; x2: 5; y2: 13 }
            Rectangle {
                visible: b.glyph === "restore"
                x: 9 - width / 2; y: 9 - height / 2
                width: 7.07 + 1.1; height: width
                rotation: 45
                antialiasing: true
                color: "transparent"
                border.width: 1.1
                border.color: icon.ink
            }
        }
        // (A little slack around the circle, as arbor's hit radius.)
        MouseArea { id: area; anchors.fill: parent; anchors.margins: -3; hoverEnabled: true; onClicked: b.hit() }
        ToolTip.visible: wc.showTips && b.hot && b.tip !== ""
        ToolTip.delay: 700
        ToolTip.text: b.tip
    }

    WinButton { glyph: "minimize"; tip: qsTr("Minimize"); onHit: wc.window.showMinimized() }
    WinButton {
        glyph: wc.maximized ? "restore" : "maximize"
        tip: wc.maximized ? qsTr("Restore") : qsTr("Maximize")
        onHit: wc.maximized ? wc.window.showNormal() : wc.window.showMaximized()
    }
    WinButton { glyph: "close"; danger: true; tip: qsTr("Close"); onHit: wc.window.close() }
}
