// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

ListView {
    id: view

    property int rowCount: 0
    model: 0

    function syncRowCount() {
        if (model === rowCount)
            return

        // Assigning a new integer model resets ListView to its first row.
        // Restore the viewport in the same turn, before the next frame can
        // render the reset position.
        const previousY = contentY
        const previousX = contentX
        model = rowCount
        forceLayout()
        contentY = Math.min(previousY, Math.max(0, contentHeight - height))
        contentX = previousX
    }

    onRowCountChanged: syncRowCount()
    Component.onCompleted: syncRowCount()
}
