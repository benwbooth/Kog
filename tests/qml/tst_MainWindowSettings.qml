import QtCore
import QtQuick
import QtTest
import "../../qml"

TestCase {
    name: "MainWindowSettings"

    Component { id: settingsFactory; MainWindowSettings {} }

    function initTestCase() {
        failOnWarning(/Setting initial properties failed|Failed to initialize QSettings/)
    }

    function test_default_and_session_restore() {
        const path = StandardPaths.writableLocation(StandardPaths.TempLocation)
            + "/kog-main-window-settings-" + Date.now() + "-" + Math.random() + ".ini"
        const location = Qt.resolvedUrl(path)
        let settings = settingsFactory.createObject(null, { location: location })
        verify(settings !== null)
        compare(settings.location, location)
        compare(settings.sidebarVisible, true, "A fresh installation shows the file tree")

        settings.sidebarVisible = false
        settings.destroy()
        wait(0)
        settings = settingsFactory.createObject(null, { location: location })
        compare(settings.sidebarVisible, false, "Hidden state survives a new settings instance")

        settings.sidebarVisible = true
        settings.destroy()
        wait(0)
        settings = settingsFactory.createObject(null, { location: location })
        compare(settings.sidebarVisible, true, "Shown state survives a new settings instance")
        settings.destroy()
    }
}
