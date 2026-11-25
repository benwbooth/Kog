import QtQuick
import QtQuick.Controls.Basic

// A menu in the macOS 26 look, as hearth draws its menus (shared by Ben's Qt
// Quick apps; the canonical copy is hearth/shared/app-menus, copied into
// each app by hearth's scripts/sync-app-menus.sh: keep the copies the same).
// Rows: MacMenuItem, MacMenuSeparator; submenus: nested MacMenus.
//
//     MacMenu {
//         id: menu
//         MacMenuItem { text: "Copy"; icon.name: "edit-copy"; shortcutText: "Ctrl+C"; onTriggered: copy() }
//         MacMenuSeparator {}
//         MacMenu { title: "Sort By"; MacMenuItem { text: "Name"; checkable: true } }
//     }
//     menu.popup()            // at the pointer; popup(item, x, y) next to an item
//
// Glass with vibrancy (the window behind is blurred), 12 pt corners, 4 pt
// around the rows, a soft shadow; a quick fade on open; a chosen row
// blinks once while the menu fades.
Menu {
    id: menu
    // The look: the app's palette by default; set `dark` / `accent` to the
    // app's own theme.
    property alias dark: style.dark
    property alias accent: style.accent
    readonly property MacMenuStyle macStyle: MacMenuStyle {
        id: style
        window: menu.palette.window
        highlight: menu.palette.highlight
    }
    // Icons: by default each name is looked up in the icon theme on the UI
    // thread; an app with its own image provider gives (name, device px) =>
    // URL instead (loaded off the UI thread).
    property var iconUrl: null
    // name => whether that icon is one colour (then drawn in the text's
    // colour: without a KDE platform theme, Breeze's monochrome icons are black).
    property var monoIcon: null
    // The checkmark/icon column: shown when any row has an icon, is
    // checkable or has a swatch (macOS keeps the text aligned), else the text
    // starts at the left inset.
    property bool gutter: {
        void count
        for (let i = 0; i < count; i++) {
            const it = itemAt(i)
            if (!it || it.visible === false) continue
            if ((it.icon && it.icon.name) || it.iconName || it.checkable || (it.swatch && it.swatch.a > 0)) return true
        }
        return false
    }

    // Separators don't know their menu: hand them its look.
    function _adopt() {
        for (let i = 0; i < count; i++) {
            const it = itemAt(i)
            if (it && it.macSeparator) it.st = macStyle
        }
    }
    onCountChanged: Qt.callLater(_adopt)
    Component.onCompleted: _adopt()

    // Set by a chosen row: the menu waits for its blink before fading.
    property bool _chosen: false
    function choose() { _chosen = true }
    onClosed: _chosen = false

    popupType: Popup.Item
    topPadding: style.padding + 0.001
    bottomPadding: style.padding
    leftPadding: style.padding + 0.001
    rightPadding: style.padding
    overlap: style.padding + 1
    cascade: true
    // Kept inside the window: a menu taller than the room left scrolls.
    margins: Math.round(4 * style.pt)
    implicitWidth: Math.max(style.minWidth, contentItem ? contentItem.implicitWidth + leftPadding + rightPadding : 0)
    font.family: style.family
    font.pixelSize: style.fontSize
    delegate: MacMenuItem {}
    background: MacMenuBackground { style: menu.macStyle; popup: menu }
    contentItem: ListView {
        implicitHeight: contentHeight
        implicitWidth: {
            let w = 0
            for (let i = 0; i < count; i++) {
                const it = menu.itemAt(i)
                if (it && it.visible !== false) w = Math.max(w, it.implicitWidth)
            }
            return w
        }
        model: menu.contentModel
        interactive: Window.window ? contentHeight + menu.topPadding + menu.bottomPadding > menu.height : false
        clip: interactive
        currentIndex: menu.currentIndex
        ScrollIndicator.vertical: ScrollIndicator {}
    }
    // A quick fade in (no grow: scaling resampled the icons, and a longer
    // fade made every right-click feel slow, in grove).
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: style.inMs; easing.type: Easing.OutCubic } }
    exit: Transition {
        SequentialAnimation {
            PauseAnimation { duration: menu._chosen ? style.blinkMs * 2 : 0 }
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: style.outMs; easing.type: Easing.InCubic }
        }
    }
}
