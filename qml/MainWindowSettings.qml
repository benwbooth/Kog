import QtQuick

SqliteSettings {
    category: "MainWindow"
    property bool sidebarVisible: true
    property real sidebarWidth: 285
    property bool treeSectionExpanded: true
    property bool playlistsSectionExpanded: true
    values: ({sidebarVisible, sidebarWidth, treeSectionExpanded, playlistsSectionExpanded})
}
