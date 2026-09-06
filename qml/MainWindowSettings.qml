import QtCore

Settings {
    category: "MainWindow"
    property bool sidebarVisible: true

    onSidebarVisibleChanged: {
        // Persist the toggle immediately, including a quit directly afterward.
        setValue("sidebarVisible", sidebarVisible)
        sync()
    }
}
