import QtQuick
import QtQuick.Controls.Basic

// The app's menus inside the window, from the same model GlobalMenuBar
// exports to the desktop's global menu bar: one submenu per menu (File ▸,
// Edit ▸ ...), shortcuts shown on the right. Rows are made once and updated
// in place each time it opens, so texts and states are current. Opened by
// the window's ☰ button:
//     AppMenu { id: appMenu; model: root.menuModel; onTriggered: id => root.menuAction(id) }
//     onClicked: appMenu.openBelow(button)
//
// The look is MacMenu's (macOS 26, as hearth draws its menus), in the app's
// light/dark palette and accent colour (set `dark` and `accent` from the
// app's theme). Give every model item an `icon` (a freedesktop icon name).
//
// Shared by Ben's Qt Quick apps: the canonical copy is in
// hearth/shared/app-menus; hearth's scripts/sync-app-menus.sh copies it
// into each app (keep the copies the same).
MacMenu {
    id: root
    property var model: []
    signal triggered(string id)

    // The row types (defaults: the Mac ones). An app may give its own: a
    // MenuItem with a `shortcutText` property; a Menu for submenus; a
    // MenuSeparator.
    property Component itemDelegate: macItem
    property Component submenuDelegate: macSubmenu
    property Component separatorDelegate: macSeparator
    Component { id: macItem; MacMenuItem {} }
    Component { id: macSeparator; MacMenuSeparator {} }
    Component { id: macSubmenu; MacMenu {} }
    // Submenus get the root's look and icon settings.
    function _share(sub) {
        if (sub.dark !== undefined) sub.dark = Qt.binding(() => root.dark)
        if (sub.accent !== undefined) sub.accent = Qt.binding(() => root.accent)
        if (sub.iconUrl !== undefined) sub.iconUrl = Qt.binding(() => root.iconUrl)
        if (sub.monoIcon !== undefined) sub.monoIcon = Qt.binding(() => root.monoIcon)
    }
    // Flat: every menu's items inline, a separator between menus (submenus
    // kept), as nixos-update-checker's hamburger does. For apps with short
    // menus; otherwise one submenu per menu (see the model options below).
    property bool flat: false

    function openBelow(item) {
        refresh()
        popup(item, Math.max(0, item.width - implicitWidth), item.height + 4)
    }
    function openAt(item, x, y) {
        refresh()
        popup(item, x, y)
    }

    // Model options for the ☰ (they don't change the global menu bar):
    // a menu or item with `inWindow: false` is left out here; a menu with
    // `inline: true` shows its items flat, between separators;
    // `appMenuAtEnd` puts the first menu's (the app-name menu's) items flat
    // at the bottom. Menus and items may carry `icon`.
    property bool appMenuAtEnd: false

    // The rows are built once and kept: each open (and a moment after the
    // model changes, so the click itself has nothing to build) only updates
    // texts, states and icons in place. Rows are made again only when the
    // menus' shape changes (items added, removed or moved).
    property var _objects: []
    property var _entries: []     // [obj, kind] in walk order, for updates
    property string _shape: ""
    // (While it's open, only texts and states follow the model.)
    onModelChanged: prepare.restart()
    Timer {
        id: prepare
        interval: 250
        onTriggered: {
            if (!root.visible) root.refresh()
            else if (JSON.stringify(root.walk(null)) === root._shape) root.update()
        }
    }
    Component.onCompleted: prepare.restart()

    function refresh() {
        var shape = JSON.stringify(walk(null))
        if (shape !== _shape || _entries.length === 0) { rebuild(); _shape = shape }
        else update()
    }

    // The ☰'s view of the model: menus and items as shown, with the
    // options applied. `visit(kind, node, depth)` sees each in order.
    function walk(visit) {
        var out = []
        var menus = (model || []).filter(function(m) { return m.inWindow !== false })
        var tail = null
        if (appMenuAtEnd && menus.length > 1) { tail = menus[0]; menus = menus.slice(1) }
        function items(list, depth) {
            list = list.filter(function(it) { return it.inWindow !== false })
            // No separator first, last or twice in a row (hidden items can leave one).
            list = list.filter(function(it, i) {
                return !it.separator || (i > 0 && i < list.length - 1 && !list[i - 1].separator)
            })
            for (var i = 0; i < list.length; i++) {
                var it = list[i]
                if (it.separator) { out.push("-"); if (visit) visit("sep", it, depth) }
                else if (it.items) {
                    out.push("<" + (it.id || it.text)); if (visit) visit("sub", it, depth)
                    items(it.items, depth + 1)
                    out.push(">"); if (visit) visit("end", it, depth)
                } else { out.push(it.id || it.text); if (visit) visit("item", it, depth) }
            }
        }
        var sepNext = false, any = false
        function group(list) {
            if (any) { out.push("-"); if (visit) visit("sep", null, 0) }
            items(list, 0)
            sepNext = any = true
        }
        for (var m = 0; m < menus.length; m++) {
            if (flat || menus[m].inline) { group(menus[m].items || []); continue }
            if (sepNext) { out.push("-"); if (visit) visit("sep", null, 0); sepNext = false }
            out.push("<" + menus[m].title); if (visit) visit("sub", { text: menus[m].title, icon: menus[m].icon }, 0)
            items(menus[m].items || [], 1)
            out.push(">"); if (visit) visit("end", menus[m], 0)
            any = true
        }
        if (tail) group(tail.items || [])
        return out
    }

    // The menu's style for a row made for it (rows made with createObject()
    // don't know their menu yet: without it each made a style of its own,
    // ~3x the time per row). Only for the Mac delegates, which take it.
    function _hint(menu, delegate) {
        return menu && menu.macStyle && (delegate === macItem || delegate === macSeparator) ? { styleHint: menu.macStyle } : {}
    }

    function rebuild() {
        while (count > 0) takeItem(0)
        for (var i = 0; i < _objects.length; i++) _objects[i].destroy()
        _objects = []
        var entries = []
        var stack = [root]
        walk(function(kind, it, depth) {
            var menu = stack[stack.length - 1]
            var obj
            if (kind === "sep") {
                obj = separatorDelegate.createObject(null, root._hint(menu, separatorDelegate))
                menu.addItem(obj)
            } else if (kind === "sub") {
                obj = submenuDelegate.createObject(null, { title: it.text, enabled: it.enabled !== false })
                root._share(obj)
                if (it.icon && obj.icon) obj.icon.name = it.icon
                stack.push(obj)
                entries.push([obj, kind])
                _objects.push(obj)
                return
            } else if (kind === "end") {
                var sub = stack.pop()
                fitWidth(sub)
                stack[stack.length - 1].addMenu(sub)
                return
            } else {
                obj = itemDelegate.createObject(null, Object.assign({
                    text: it.text, shortcutText: it.shortcut || "",
                    enabled: it.enabled !== false,
                    checkable: !!it.checkable, checked: !!it.checkable && !!it.checked }, root._hint(menu, itemDelegate)))
                setIcon(obj, it.icon || "")
                const id = it.id || ""
                obj.triggered.connect(function() { root.triggered(id) })
                menu.addItem(obj)
            }
            entries.push([obj, kind])
            _objects.push(obj)
        })
        _entries = entries
        fitWidth(root)
    }

    // Same shape: put the current texts, states and icons on the rows.
    function update() {
        var i = 0, touched = [root]
        walk(function(kind, it) {
            if (kind === "end") return
            var e = _entries[i++]
            if (!e) return
            var obj = e[0]
            if (kind === "sub") {
                if (obj.title !== it.text) obj.title = it.text
                obj.enabled = it.enabled !== false
                touched.push(obj)
            } else if (kind === "item") {
                if (obj.text !== it.text) obj.text = it.text
                if (obj.shortcutText !== (it.shortcut || "")) obj.shortcutText = it.shortcut || ""
                obj.enabled = it.enabled !== false
                obj.checkable = !!it.checkable
                obj.checked = !!it.checkable && !!it.checked
                setIcon(obj, it.icon || "")
            }
        })
        for (var k = 0; k < touched.length; k++) fitWidth(touched[k])
    }

    function setIcon(obj, name) {
        if (obj.icon && obj.icon.name !== name) obj.icon.name = name
        // (A delegate with its own iconName property, e.g. grove's GMenuItem.)
        if (typeof obj.iconName === "string" && obj.iconName !== name) try { obj.iconName = name } catch (e) {}
    }

    // Wide enough for the longest item and its shortcut (the style's
    // default is a fixed width that cuts long items off).
    function fitWidth(menu) {
        // (A MacMenu sizes itself to its widest row; other menus are made
        // wide enough here.)
        if (menu.macStyle) return
        var w = 0
        for (var i = 0; i < menu.count; i++) {
            var it = menu.itemAt(i)
            if (it) w = Math.max(w, it.implicitWidth)
        }
        menu.width = Math.max(menu.implicitWidth, Math.ceil(w + menu.leftPadding + menu.rightPadding))
    }
}
