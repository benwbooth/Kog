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
    // The fields the rows view shows, as a JSON list of field ids (lists per
    // line from an earlier picker are read as the fields they name). Empty
    // means the default fields.
    property string playlistRowLines: ""
    values: ({playlistViewMode: settings.playlistViewMode, playlistRowLines: settings.playlistRowLines, sidebarVisible: settings.sidebarVisible, sidebarWidth: settings.sidebarWidth,
        treeSectionExpanded: settings.treeSectionExpanded, playlistsSectionExpanded: settings.playlistsSectionExpanded,
        playlistsHeight: settings.playlistsHeight})
}
