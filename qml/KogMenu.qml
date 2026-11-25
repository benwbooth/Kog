import QtQuick

// Kog's right-click and drop-down menus: the shared macOS 26 menu (MacMenu,
// from hearth/shared/app-menus, the same look as the ☰ and hearth's menus),
// in Kog's palette. Every menu in Kog is a KogMenu with KogMenuItem rows and
// KogMenuSeparator lines; nested KogMenus become submenus.
MacMenu {
    delegate: KogMenuItem {}
}
