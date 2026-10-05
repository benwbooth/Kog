import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml" as Kog

TestCase {
    id: test
    name: "PlaylistSidebar"
    width: 640; height: 160
    visible: true
    when: windowShown
    SystemPalette { id: theme }
    Kog.PlaylistSidebarRow {
        id: row
        x: 20; y: 20; width: 300; height: 30
        pid: 7; name: "Road trip"; entryCount: 12
        theme: theme
        destinationName: "Current mix"
        onSelectedWithModifiers: selected = true
    }
    SignalSpy { id: selection; target: row; signalName: "selectedWithModifiers" }
    SignalSpy { id: opened; target: row; signalName: "opened" }
    SignalSpy { id: appended; target: row; signalName: "appendRequested" }
    SignalSpy { id: menu; target: row; signalName: "contextRequested" }
    SignalSpy { id: navigation; target: row; signalName: "navigationRequested" }
    SignalSpy { id: dragStart; target: row; signalName: "dragStarted" }
    SignalSpy { id: dragEnd; target: row; signalName: "dragFinished" }
    function init() {
        row.pid = 7; row.selected = false; row.canAppend = true
        row.entryCount = 12; row.renaming = false
        selection.clear(); opened.clear(); appended.clear(); menu.clear()
        navigation.clear(); dragStart.clear(); dragEnd.clear()
        mouseMove(test, 500, 130)
    }
    function test_single_double_and_keyboard() {
        mouseClick(row, 80, 15)
        compare(selection.count, 1)
        compare(opened.count, 0, "Selecting a source must keep the destination pane open")
        mouseDoubleClickSequence(row, 80, 15)
        compare(opened.count, 1)
        compare(appended.count, 0)
        row.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(opened.count, 2)
        keyClick(Qt.Key_Down)
        compare(navigation.count, 1)
        compare(navigation.signalArguments[0][0], 1)
        keyClick(Qt.Key_F10, Qt.ShiftModifier)
        compare(menu.count, 1)
    }
    function test_append_button_is_a_separate_action() {
        row.selected = true
        const button = findChild(row, "appendPlaylistButton")
        verify(button.visible)
        compare(button.Accessible.name, "Append to “Current mix”")
        mouseClick(button, button.width / 2, button.height / 2)
        compare(appended.count, 1)
        compare(selection.count, 0)
        compare(opened.count, 0)
        row.canAppend = false
        verify(!button.enabled)
        mouseClick(button, button.width / 2, button.height / 2)
        compare(appended.count, 1)
        row.canAppend = true; row.entryCount = 0
        verify(!button.enabled)
    }
    function test_drag_does_not_open_or_click_the_source() {
        // Favorites can be dragged out too, although its sidebar position is pinned.
        row.pid = 0
        mousePress(row, 80, 15)
        mouseMove(row, 420, 80, 20)
        mouseRelease(row, 420, 80)
        compare(dragStart.count, 1)
        compare(dragEnd.count, 1)
        compare(selection.count, 0)
        compare(opened.count, 0)
        compare(appended.count, 0)
    }
}
