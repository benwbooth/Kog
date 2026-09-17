import QtCore

Settings {
    category: "MainWindow"
    property bool sidebarVisible: true
    property real sidebarWidth: 285
    property bool treeSectionExpanded: true
    property bool playlistsSectionExpanded: true

    onSidebarVisibleChanged: {
        // Persist the toggle immediately, including a quit directly afterward.
        setValue("sidebarVisible", sidebarVisible)
        sync()
    }
    onSidebarWidthChanged: {
        setValue("sidebarWidth", sidebarWidth)
        sync()
    }
    onTreeSectionExpandedChanged: {
        setValue("treeSectionExpanded", treeSectionExpanded)
        sync()
    }
    onPlaylistsSectionExpandedChanged: {
        setValue("playlistsSectionExpanded", playlistsSectionExpanded)
        sync()
    }
}
