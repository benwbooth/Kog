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
    // How songs are shown: "list" (the column table), "rows" (two-line
    // rows with album art) or "grid" (cover tiles).
    property string playlistViewMode: "list"
    // The rows view's four lines as JSON: one list of field ids per line.
    // Empty means the default lines.
    property string playlistRowLines: ""
    values: ({playlistViewMode: settings.playlistViewMode, playlistRowLines: settings.playlistRowLines, sidebarVisible: settings.sidebarVisible, sidebarWidth: settings.sidebarWidth,
        treeSectionExpanded: settings.treeSectionExpanded, playlistsSectionExpanded: settings.playlistsSectionExpanded,
        playlistsHeight: settings.playlistsHeight})
}
