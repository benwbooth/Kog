import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl

// The app's menus inside the window, from the same model GlobalMenuBar
// exports to the desktop's global menu bar: one submenu per menu (File ▸,
// Edit ▸ ...), shortcuts shown on the right. Rebuilt each time it opens so
// texts and states are current. Opened by the window's ☰ button:
//     AppMenu { id: appMenu; model: root.menuModel; onTriggered: id => root.menuAction(id) }
//     onClicked: appMenu.openBelow(button)
//
// The look is grove's glass menu (hearth's: 12 pt corners, 24 pt rows, an
// icon on every row, the accent colour on the highlighted row, the key at
// the right, a fade), in the app's light/dark palette and accent colour.
// Give every model item an `icon` (a freedesktop icon name).
//
// Shared by Ben's Qt Quick apps (keep the copies the same).
Menu {
    id: root
    property var model: []
    signal triggered(string id)

    // ---- look (override per app if needed) ----
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

    // The menu item type: a MenuItem with a `shortcutText` property (the
    // app's own look); the default is the glass row above.
    property Component itemDelegate: glassItem
    // The submenus' type (a Menu in the app's own look: background,
    // padding, and `delegate` for its rows that open further submenus) and
    // the separators'. Without them, the style's plain ones. Set the root's
    // own `delegate` too, for its rows (each opens a submenu).
    property Component submenuDelegate: glassSubmenu
    property Component separatorDelegate: glassSeparator

    // The root menu in the same look (submenus get it from glassSubmenu).
    topPadding: menuPad + 0.001
    bottomPadding: menuPad
    leftPadding: menuPad + 0.001
    rightPadding: menuPad
    overlap: 3
    cascade: true
    // Kept inside the window: a menu taller than the room left scrolls
    // instead of running off the edge (without margins a Popup.Item menu
    // isn't constrained at all).
    margins: Math.round(4 * pt)
    delegate: glassItem
    background: GlassBack {}
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120; easing.type: Easing.OutCubic } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 140 } }
    // Flat: every menu's items inline, a separator between menus (submenus
    // kept), as nixos-update-checker's hamburger does. For apps with short
    // menus; otherwise one submenu per menu (see the model options below).
    property bool flat: false

    function openBelow(item) {
        rebuild()
        popup(item, Math.max(0, item.width - implicitWidth), item.height + 4)
    }
    function openAt(item, x, y) {
        rebuild()
        popup(item, x, y)
    }

    // Model options for the ☰ (they don't change the global menu bar):
    // a menu or item with `inWindow: false` is left out here; a menu with
    // `inline: true` shows its items flat, between separators;
    // `appMenuAtEnd` puts the first menu's (the app-name menu's) items flat
    // at the bottom. Menus and items may carry `icon`.
    property bool appMenuAtEnd: false

    property var _objects: []
    function rebuild() {
        while (count > 0) takeItem(0)
        for (var i = 0; i < _objects.length; i++) _objects[i].destroy()
        _objects = []
        var menus = (model || []).filter(function(m) { return m.inWindow !== false })
        var tail = null
        if (appMenuAtEnd && menus.length > 1) { tail = menus[0]; menus = menus.slice(1) }
        // A flat group sits between separators: one before it unless it's
        // first, one after it before whatever follows.
        var sepNext = false
        function group(items) {
            if (count > 0) addSep(root)
            fill(root, items)
            sepNext = true
        }
        for (var m = 0; m < menus.length; m++) {
            if (flat || menus[m].inline) { group(menus[m].items || []); continue }
            if (sepNext) { addSep(root); sepNext = false }
            var sub = submenuDelegate.createObject(null, { title: menus[m].title })
            if (menus[m].icon && sub.icon) sub.icon.name = menus[m].icon
            _objects.push(sub)
            fill(sub, menus[m].items || [])
            addMenu(sub)
        }
        if (tail) group(tail.items || [])
        fitWidth(root)
    }
    function addSep(menu) {
        var sep = separatorDelegate.createObject(null)
        _objects.push(sep)
        menu.addItem(sep)
    }
    // Wide enough for the longest item and its shortcut (the style's
    // default is a fixed width that cuts long items off).
    function fitWidth(menu) {
        var w = 0
        for (var i = 0; i < menu.count; i++) {
            var it = menu.itemAt(i)
            if (it) w = Math.max(w, it.implicitWidth)
        }
        menu.width = Math.max(menu.implicitWidth, Math.ceil(w + menu.leftPadding + menu.rightPadding))
    }
    function fill(menu, items) {
        // No separator first, last or twice in a row (hidden items can leave one).
        items = items.filter(function(it) { return it.inWindow !== false })
        items = items.filter(function(it, i) {
            return !it.separator || (i > 0 && i < items.length - 1 && !items[i - 1].separator)
        })
        for (var i = 0; i < items.length; i++) {
            var it = items[i]
            var obj
            if (it.separator) {
                obj = separatorDelegate.createObject(null)
                menu.addItem(obj)
            } else if (it.items) {
                obj = submenuDelegate.createObject(null, { title: it.text, enabled: it.enabled !== false })
                if (it.icon && obj.icon) obj.icon.name = it.icon
                fill(obj, it.items)
                menu.addMenu(obj)
            } else {
                obj = itemDelegate.createObject(null, {
                    text: it.text, shortcutText: it.shortcut || "",
                    enabled: it.enabled !== false,
                    checkable: !!it.checkable, checked: !!it.checkable && !!it.checked })
                if (it.icon) { obj.icon.name = it.icon; if (obj.iconName !== undefined && typeof obj.iconName === "string") try { obj.iconName = it.icon } catch (e) {} }
                const id = it.id || ""
                obj.triggered.connect(function() { root.triggered(id) })
                menu.addItem(obj)
            }
            _objects.push(obj)
        }
        fitWidth(menu)
    }

    // ---- the glass look (grove's GMenu / GMenuItem / GlassPanel) ----
    component GlassBack: Item {
        implicitWidth: Math.round(200 * root.pt)
        Rectangle {   // shadow
            anchors.fill: parent
            anchors.topMargin: 3
            anchors.margins: -1
            radius: root.radius + 1
            color: "transparent"
            border.width: 3
            border.color: Qt.rgba(0, 0, 0, root.dark ? 0.22 : 0.06)
        }
        Rectangle {
            anchors.fill: parent
            radius: root.radius
            color: root.glassTint
            border.width: 1
            border.color: root.glassEdge
        }
        Rectangle {   // sheen in the top part
            anchors.fill: parent
            anchors.margins: 1
            radius: root.radius - 1
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, root.dark ? 0.05 : 0.25) }
                GradientStop { position: 0.35; color: Qt.rgba(1, 1, 1, 0) }
            }
        }
        Rectangle {   // specular rim, lit from the top left
            anchors.fill: parent
            anchors.margins: 1
            radius: root.radius - 1
            color: "transparent"
            border.width: 1
            border.color: root.glassRim
            opacity: 0.9
        }
    }
    component GlassText: Text {
        color: root.fg
        font.family: root.family
        font.pixelSize: root.fontSize
        renderType: Text.NativeRendering
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        textFormat: Text.PlainText
    }
    Component {
        id: glassItem
        MenuItem {
            id: mi
            // The key is the app's own Shortcut; this only shows it.
            property string shortcutText: ""
            // "name|fallback": the first name is used.
            readonly property string iconName: (icon.name || "").split("|")[0]
            implicitHeight: root.rowHeight
            implicitWidth: Math.max(Math.round(200 * root.pt), lbl.implicitWidth + sc.implicitWidth + 70)
            hoverEnabled: true
            background: Rectangle {
                radius: Math.round(8 * root.pt)
                color: mi.highlighted && mi.enabled ? root.accent : "transparent"
            }
            indicator: Item {}
            arrow: GlassText {
                visible: mi.subMenu !== null
                x: mi.width - width - 8
                anchors.verticalCenter: parent.verticalCenter
                text: "›"
                font.pixelSize: Math.round(15 * root.pt)
                color: mi.highlighted ? "#ffffff" : root.fgSecondary
            }
            contentItem: Item {
                implicitWidth: lbl.implicitWidth + sc.implicitWidth + 60
                // A toggle that's on: a soft accent square behind its icon
                // (a ✓ when it has none).
                Rectangle {
                    visible: mi.checkable && mi.checked && mi.iconName !== ""
                    x: -1
                    width: 22; height: 22; radius: 5
                    anchors.verticalCenter: parent.verticalCenter
                    color: mi.highlighted ? Qt.rgba(1, 1, 1, 0.25) : Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.28)
                }
                GlassText {
                    visible: mi.checkable && mi.checked && mi.iconName === ""
                    x: 4
                    anchors.verticalCenter: parent.verticalCenter
                    text: "✓"
                    color: mi.highlighted ? "#ffffff" : root.fg
                }
                IconImage {
                    visible: mi.iconName !== ""
                    name: mi.iconName
                    x: 2
                    width: 16; height: 16
                    sourceSize: Qt.size(32, 32)
                    anchors.verticalCenter: parent.verticalCenter
                    opacity: mi.enabled ? 1 : 0.4
                }
                GlassText {
                    id: lbl
                    x: 24
                    width: parent.width - x - sc.width - 8
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: 0.001
                    text: mi.text
                    color: !mi.enabled ? root.fgDisabled : mi.highlighted ? "#ffffff" : root.fg
                }
                GlassText {
                    id: sc
                    anchors.right: parent.right
                    anchors.rightMargin: mi.subMenu ? 18 : 4
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: 0.001
                    text: mi.shortcutText
                    color: mi.highlighted ? "#ffffff" : root.fgSecondary
                }
            }
        }
    }
    Component {
        id: glassSubmenu
        Menu {
            topPadding: root.menuPad + 0.001
            bottomPadding: root.menuPad
            leftPadding: root.menuPad + 0.001
            rightPadding: root.menuPad
            overlap: 3
            cascade: true
            margins: Math.round(4 * root.pt)
            delegate: glassItem
            background: GlassBack {}
            enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120; easing.type: Easing.OutCubic } }
            exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 140 } }
        }
    }
    Component {
        id: glassSeparator
        MenuSeparator {
            topPadding: 4
            bottomPadding: 4
            leftPadding: 10
            rightPadding: 10
            contentItem: Rectangle { implicitHeight: 1; color: root.separatorColor }
        }
    }
}
