// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: testCase
    name: "CountPreservingListView"
    width: 320
    height: 200
    visible: true
    when: windowShown

    property int rows: 100

    CountPreservingListView {
        id: view
        width: 320
        height: 200
        rowCount: testCase.rows
        delegate: Rectangle { width: 320; height: 20 }
    }

    function test_appendingRowsKeepsViewport() {
        compare(view.count, 100)
        view.positionViewAtIndex(90, ListView.Beginning)
        const before = view.contentY
        verify(before > 0)

        rows = 101
        compare(view.count, 101)
        compare(view.contentY, before)
        wait(30)
        compare(view.contentY, before)

        rows = 10
        compare(view.count, 10)
        verify(view.contentY <= Math.max(0, view.contentHeight - view.height))

        rows = 0
        compare(view.count, 0)
        compare(view.contentY, 0)
    }
}
