import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Shapes

// One row of a MacMenu, macOS 26 style (shared by Ben's Qt Quick apps; see
// MacMenuStyle.qml): 24 pt rows, 13 pt text, the selection an accent-filled
// rounded rectangle, a checkmark/icon column, the key at the right (spelled
// the PC way), a chevron for submenus, disabled rows greyed out. A chosen row
// blinks once while the menu fades (macOS).
//
//     MacMenuItem { text: "Copy"; icon.name: "edit-copy"; shortcutText: "Ctrl+C"; onTriggered: ... }
//
// The key shown comes from `shortcutText`, else the Action's shortcut when
// it is a string. It is only shown: the app's own Shortcut/Action does the work.
MenuItem {
    id: mi
    property string shortcutText: ""
    // (Qt.labs.platform's MenuItem name for it: a key sequence string.)
    property var shortcut: undefined
    // "name|fallback" (the first name is looked up in the icon theme).
    property string iconName: icon.name || ""
    // A radio item: a dot instead of a checkmark.
    property bool radio: false
    // A colour swatch instead of an icon (tags, labels); transparent: none.
    property color swatch: "transparent"
    // Hidden rows take no room.
    implicitHeight: visible ? st.rowHeight : 0
    height: visible ? implicitHeight : 0

    readonly property MacMenuStyle st: menu && menu.macStyle ? menu.macStyle : ownStyle
    MacMenuStyle { id: ownStyle; window: mi.palette.window; highlight: mi.palette.highlight }
    readonly property bool gutter: menu && menu.gutter !== undefined ? menu.gutter : true
    // "Text\tCtrl+X" (platform menus' way of giving the key): shown apart.
    readonly property string label: text.split("\t")[0]
    readonly property string tabKey: text.indexOf("\t") >= 0 ? text.split("\t")[1] : ""
    readonly property string keyText: subMenu ? ""
        : st.pcShortcut(shortcutText !== "" ? shortcutText : tabKey !== "" ? tabKey : keyReader.nativeText)
    // Only reads the key (a string or a StandardKey, of `shortcut` or the
    // Action) for display; the app's Shortcut/Action owns it.
    Shortcut {
        id: keyReader
        enabled: false
        // (`sequences`: a StandardKey with several bindings warns as `sequence`.)
        sequences: {
            const k = mi.shortcut !== undefined && mi.shortcut !== null && mi.shortcut !== "" ? mi.shortcut
                : mi.action && mi.action.shortcut !== undefined ? mi.action.shortcut : ""
            return k === "" || k === undefined || k === null ? [] : [k]
        }
    }

    // Choosing: lit through the blink, whatever the pointer does meanwhile.
    property bool chosen: false
    property bool blinkOff: false
    readonly property bool lit: enabled && !blinkOff && (highlighted || chosen)
    onTriggered: if (!subMenu && menu && menu.macStyle) { chosen = true; blink.restart(); if (menu.choose) menu.choose() }
    SequentialAnimation {
        id: blink
        PropertyAction { target: mi; property: "blinkOff"; value: true }
        PauseAnimation { duration: mi.st.blinkMs }
        PropertyAction { target: mi; property: "blinkOff"; value: false }
        PauseAnimation { duration: mi.st.blinkMs * 4 }
        PropertyAction { target: mi; property: "chosen"; value: false }
    }

    readonly property color textColor: !enabled ? st.fgDisabled : lit ? st.accentText : st.fg

    implicitWidth: Math.max(st.minWidth - 2 * st.padding,
                            (gutter ? st.gutter : st.textInset) + label.implicitWidth
                            + (keyText !== "" ? key.implicitWidth + Math.round(28 * st.pt) : 0)
                            + (subMenu ? Math.round(26 * st.pt) : 0) + st.textInset)
    padding: 0
    // (Basic sets these itself, for its own indicator and arrow.)
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    spacing: 0
    hoverEnabled: true
    font.family: st.family
    font.pixelSize: st.fontSize
    indicator: Item {}
    arrow: Item {}
    background: Rectangle {
        radius: mi.st.selectionRadius
        color: mi.st.accent
        opacity: mi.lit ? 1 : 0
        Behavior on opacity { enabled: !mi.lit && !mi.blinkOff; NumberAnimation { duration: 70 } }
    }
    contentItem: Item {
        // Checkmark / icon column.
        Item {
            id: column
            x: Math.round(5 * mi.st.pt)
            width: mi.st.gutter - x
            height: parent.height
            visible: mi.gutter
            // A checked toggle with an icon: a soft accent square behind it.
            Rectangle {
                x: (parent.width - width) / 2 - 2
                y: (parent.height - height) / 2
                width: 22; height: 22; radius: 5
                color: mi.lit ? Qt.rgba(1, 1, 1, 0.25) : mi.st.accentDim
                visible: mi.checkable && mi.checked && mi.iconName !== ""
            }
            Text {
                x: (parent.width - width) / 2 - 2
                anchors.verticalCenter: parent.verticalCenter
                anchors.verticalCenterOffset: 0.001
                visible: mi.checkable && mi.checked && mi.iconName === "" && mi.swatch.a === 0
                text: mi.radio ? "●" : "✓"
                color: mi.textColor
                font.family: mi.st.family
                font.pixelSize: mi.radio ? Math.round(mi.st.fontSize * 0.6) : mi.st.fontSize
                font.weight: Font.DemiBold
                renderType: Text.NativeRendering
            }
            Rectangle {
                x: (parent.width - width) / 2 - 2
                y: (parent.height - height) / 2
                width: 11; height: 11; radius: 6
                visible: mi.swatch.a > 0
                color: mi.swatch
                border.width: mi.checkable && mi.checked ? 2 : 1
                border.color: mi.checkable && mi.checked ? mi.textColor : Qt.rgba(0, 0, 0, 0.2)
            }
            MacMenuIcon {
                id: rowIcon
                x: (parent.width - width) / 2 - 2
                y: (parent.height - height) / 2
                visible: mi.iconName !== "" && mi.swatch.a === 0
                name: mi.menu && mi.menu.iconUrl ? mi.iconName : mi.iconName.split("|")[0]
                iconUrl: mi.menu && mi.menu.iconUrl ? mi.menu.iconUrl : null
                color: mi.menu && mi.menu.monoIcon && mi.menu.monoIcon(mi.iconName) ? mi.textColor : "transparent"
                opacity: mi.enabled ? 1 : 0.4
                // Placed again whenever the menu opens or moves (it opens at
                // a new spot, then is moved to stay inside the window).
                Connections {
                    target: mi.menu
                    ignoreUnknownSignals: true
                    function onOpened() { Qt.callLater(rowIcon.snap) }
                    function onXChanged() { Qt.callLater(rowIcon.snap) }
                    function onYChanged() { Qt.callLater(rowIcon.snap) }
                }
            }
        }
        Text {
            id: label
            readonly property int textLeft: mi.gutter ? mi.st.gutter : mi.st.textInset
            x: textLeft + 0.001
            // (From `textLeft`: the 0.001 nudge would make it a hair narrower
            // than its text, and it would be cut off.)
            width: parent.width - textLeft - (key.visible ? key.implicitWidth + 16 : 0) - (mi.subMenu ? 20 : 0) - mi.st.textInset
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            text: mi.label
            color: mi.textColor
            font.family: mi.st.family
            font.pixelSize: mi.st.fontSize
            font.letterSpacing: mi.st.letterSpacing
            elide: Text.ElideRight
            textFormat: Text.PlainText
            renderType: Text.NativeRendering
        }
        Text {
            id: key
            anchors.right: parent.right
            anchors.rightMargin: mi.st.textInset
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            visible: mi.keyText !== ""
            text: mi.keyText
            color: !mi.enabled ? mi.st.fgDisabled : mi.lit ? mi.st.accentText : mi.st.fgSecondary
            font.family: mi.st.family
            font.pixelSize: mi.st.fontSize
            font.letterSpacing: mi.st.keySpacing
            textFormat: Text.PlainText
            renderType: Text.NativeRendering
        }
        // Submenu chevron (SF Symbol chevron.right, ~9 pt).
        Shape {
            visible: mi.subMenu !== null
            anchors.right: parent.right
            anchors.rightMargin: mi.st.textInset
            anchors.verticalCenter: parent.verticalCenter
            width: 6
            height: 11
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: mi.textColor
                strokeWidth: 1.6
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                startX: 0.8; startY: 0.8
                PathLine { x: 5.2; y: 5.5 }
                PathLine { x: 0.8; y: 10.2 }
            }
        }
    }
}
