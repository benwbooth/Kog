import QtQuick

SqliteSettings {
    id: settings
    category: "MainWindow"
    property bool sidebarVisible: true
    property real sidebarWidth: 285
    property bool treeSectionExpanded: true
    property bool playlistsSectionExpanded: true
    values: ({sidebarVisible: settings.sidebarVisible, sidebarWidth: settings.sidebarWidth,
        treeSectionExpanded: settings.treeSectionExpanded, playlistsSectionExpanded: settings.playlistsSectionExpanded})
}
