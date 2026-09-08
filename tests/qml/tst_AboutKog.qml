import QtQuick
import QtTest
import "../../qml"

TestCase {
    name: "AboutKog"
    AboutKog { id: about }
    function test_running_version_and_reopen() {
        compare(about.version, Qt.application.version)
        const field = findChild(about, "aboutVersion")
        verify(field !== null)
        compare(field.text, "Version " + Qt.application.version)
        verify(field.readOnly === undefined)
        about.open()
        tryCompare(about, "visible", true)
        about.hide()
        tryCompare(about, "visible", false)
        about.open()
        tryCompare(about, "visible", true)
        about.hide()
    }
}
