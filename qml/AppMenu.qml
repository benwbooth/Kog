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
    // Flat: every menu's items inline, a separator between menus (submenus
    // kept), as nixos-update-checker's hamburger does. For apps with short
    // menus; otherwise one submenu per menu.
    property bool flat: false

    function openBelow(item) {
        rebuild()
        popup(item, Math.max(0, item.width - implicitWidth), item.height + 4)
    }
    function openAt(item, x, y) {
        rebuild()
        popup(item, x, y)
    }

    property var _objects: []
    function rebuild() {
        while (count > 0) takeItem(0)
        for (var i = 0; i < _objects.length; i++) _objects[i].destroy()
        _objects = []
        var menus = model || []
        for (var m = 0; m < menus.length; m++) {
            if (flat) {
                if (m > 0) {
                    var sep = separatorComponent.createObject(null)
                    _objects.push(sep)
                    addItem(sep)
                }
                fill(root, menus[m].items || [])
                continue
            }
            var sub = submenuComponent.createObject(null, { title: menus[m].title })
            _objects.push(sub)
            fill(sub, menus[m].items || [])
            addMenu(sub)
        }
    }
    function fill(menu, items) {
        for (var i = 0; i < items.length; i++) {
            var it = items[i]
            var obj
            if (it.separator) {
                obj = separatorComponent.createObject(null)
                menu.addItem(obj)
            } else if (it.items) {
                obj = submenuComponent.createObject(null, { title: it.text, enabled: it.enabled !== false })
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
    }

    Component { id: separatorComponent; MenuSeparator {} }
    Component { id: submenuComponent; Menu {} }
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
