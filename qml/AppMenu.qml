import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The app's menus inside the window, from the same model GlobalMenuBar
// exports to the desktop's global menu bar: one submenu per menu (File ▸,
// Edit ▸ ...), shortcuts shown on the right. Rebuilt each time it opens so
// texts and states are current. Opened by the window's ☰ button:
//     AppMenu { id: appMenu; model: root.menuModel; onTriggered: id => root.menuAction(id) }
//     onClicked: appMenu.openBelow(button)
//
// Shared by Ben's Qt Quick apps (keep the copies the same).
Menu {
    id: root
    property var model: []
    signal triggered(string id)
    // The menu item type: a MenuItem with a `shortcutText` property (the
    // app's own look, e.g. with icons); the default shows the shortcut at
    // the right.
    property Component itemDelegate: defaultItem
    // The submenus' type (a Menu in the app's own look: background,
    // padding, and `delegate` for its rows that open further submenus) and
    // the separators'. Without them, the style's plain ones. Set the root's
    // own `delegate` too, for its rows (each opens a submenu).
    property Component submenuDelegate: plainSubmenu
    property Component separatorDelegate: plainSeparator
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
                if (it.icon) obj.icon.name = it.icon
                const id = it.id || ""
                obj.triggered.connect(function() { root.triggered(id) })
                menu.addItem(obj)
            }
            _objects.push(obj)
        }
        fitWidth(menu)
    }

    Component { id: plainSeparator; MenuSeparator {} }
    Component { id: plainSubmenu; Menu {} }
    Component {
        id: defaultItem
        MenuItem {
            id: mi
            // The key is the app's own Shortcut; this only shows it.
            property string shortcutText
            contentItem: RowLayout {
                spacing: 24
                Label {
                    text: mi.text
                    font: mi.font
                    color: mi.palette.windowText
                    opacity: mi.enabled ? 1 : 0.45
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    renderType: Text.NativeRendering
                }
                Label {
                    visible: mi.shortcutText !== ""
                    text: mi.shortcutText
                    font: mi.font
                    color: mi.palette.windowText
                    opacity: mi.enabled ? 0.55 : 0.3
                    renderType: Text.NativeRendering
                }
            }
        }
    }
}
