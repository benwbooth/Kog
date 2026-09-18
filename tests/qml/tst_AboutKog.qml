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
    function test_build_stamp_shows_only_when_known() {
        const field = findChild(about, "aboutBuild")
        verify(field !== null)
        compare(field.visible, false, "hidden without a stamp")
        about.buildStamp = "abc1234 · 2026-01-01 09:00"
        compare(field.visible, true)
        verify(field.text.indexOf("abc1234") >= 0)
        about.buildStamp = ""
        compare(field.visible, false)
    }
}
