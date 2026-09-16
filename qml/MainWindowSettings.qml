import QtCore

Settings {
    category: "MainWindow"
    property bool sidebarVisible: true
    property bool treeSectionExpanded: true
    property bool playlistsSectionExpanded: true

    onSidebarVisibleChanged: {
        // Persist the toggle immediately, including a quit directly afterward.
        setValue("sidebarVisible", sidebarVisible)
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
