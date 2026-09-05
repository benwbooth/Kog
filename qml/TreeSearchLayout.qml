import QtQuick

// Hide query resets until the first layout settles, then keep existing rows
// visible while new matches arrive. Opacity allows hidden layouts to polish.
QtObject {
    id: layout
    required property var view
    required property var model
    property bool ready: true
    property int generation: 0
    property int settlingFrames: 0
    property var openedAncestors: ({})

    property Connections results: Connections {
        target: layout.model
        function onSearchResultsChanged() { layout.prepare(true) }
        function onSearchBatchChanged() { layout.prepare(false) }
    }
    property Connections frames: Connections {
        target: layout.view ? layout.view.Window.window : null
        function onAfterAnimating() {
            if (layout.settlingFrames <= 0) return
            if (--layout.settlingFrames === 0)
                layout.ready = true
            else
                layout.view.Window.window.update()
        }
    }

    function prepare(reset) {
        const current = ++generation
        if (reset) {
            ready = false
            openedAncestors = ({})
        }
        settlingFrames = 0
        if (reset && model.searching) return
        Qt.callLater(function() { expandBatch(current, 0) })
    }

    function expandBatch(current, firstRow) {
        if (current !== generation) return
        view.forceLayout()
        let row = firstRow
        // Yield during large batches. Never reset existing rows or reopen an
        // ancestor the user has deliberately collapsed between batches.
        for (let count = 0; row < view.rows && count < 64; ++row, ++count) {
            if (model.searchText.trim().length && model.isSearchAncestor(view.index(row, 0))) {
                const key = model.filePath(view.index(row, 0))
                if (!openedAncestors[key]) {
                    openedAncestors[key] = true
                    view.expand(row)
                    view.forceLayout()
                }
            }
        }
        if (row < view.rows) {
            Qt.callLater(function() { expandBatch(current, row) })
            return
        }
        view.forceLayout()
        const window = view.Window.window
        if (window && window.visible) {
            settlingFrames = 2
            window.update()
        } else {
            ready = true
        }
    }
}
