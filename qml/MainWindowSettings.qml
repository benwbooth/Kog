import QtQuick

SqliteSettings {
    id: settings
    category: "MainWindow"
    property bool sidebarVisible: true
    property real sidebarWidth: 285
    property bool treeSectionExpanded: true
    property bool playlistsSectionExpanded: true
    // The Playlists section's height while Files is open too.
    property real playlistsHeight: 260
    values: ({sidebarVisible: settings.sidebarVisible, sidebarWidth: settings.sidebarWidth,
        treeSectionExpanded: settings.treeSectionExpanded, playlistsSectionExpanded: settings.playlistsSectionExpanded,
        playlistsHeight: settings.playlistsHeight})
}
