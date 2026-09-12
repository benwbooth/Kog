import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml"

TestCase {
    name: "PaneSplitView"
    width: 640
    height: 360
    visible: true
    when: windowShown

    PaneSplitView {
        id: split
        anchors.fill: parent
        Item {
            id: leftPane
            SplitView.preferredWidth: 220
            SplitView.minimumWidth: 100
            Flickable {
                id: view
                anchors.fill: parent
                contentHeight: 2400
                contentWidth: width
                ScrollBar.vertical: ScrollBar {
                    id: bar
                    policy: ScrollBar.AlwaysOn
                }
            }
        }
        Item { SplitView.fillWidth: true }
    }

    function test_scrollbar_edge_and_divider_have_separate_grabs() {
        waitForPolish(split)
        const divider = findChild(split, "paneDivider")
        verify(divider !== null)
        compare(divider.containmentMask, null)
        const originalWidth = leftPane.width
        // Hover the divider first: KDE's old handle grows its hit margin here.
        mouseMove(divider, divider.width / 2, 30)
        const x = bar.width - 1
        const y = bar.topPadding + bar.size * bar.availableHeight / 2
        mousePress(bar, x, y)
        mouseMove(bar, x, y + 60, 50)
        mouseRelease(bar, x, y + 60)
        verify(view.contentY > 0, "Dragging the scrollbar scrolls the tree pane")
        compare(leftPane.width, originalWidth, "Scrollbar drag must not resize the pane")

        const start = divider.mapToItem(split, divider.width / 2, 30)
        mousePress(split, start.x, start.y)
        mouseMove(split, start.x + 40, start.y, 50)
        mouseRelease(split, start.x + 40, start.y)
        verify(leftPane.width > originalWidth + 20, "The divider still resizes the pane")
    }
}
