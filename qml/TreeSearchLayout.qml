import QtQuick

// Keep intermediate model resets/ancestor expansion out of the rendered pane.
// Opacity (not visibility) lets TreeView finish polishing while it is covered.
QtObject {
    id: layout
    required property var view
    required property var model
    property bool ready: true
    property int generation: 0
    property int settlingFrames: 0

    property Connections results: Connections {
        target: layout.model
        function onSearchResultsChanged() { layout.prepare() }
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

    function prepare() {
        const current = ++generation
        ready = false
        settlingFrames = 0
        if (model.searching) return
        Qt.callLater(function() { expandBatch(current, 0) })
    }

    function expandBatch(current, firstRow) {
        if (current !== generation || model.searching) return
        view.forceLayout()
        let row = firstRow
        // Yield during large searches, keeping the partially expanded tree
        // hidden while the search field and the rest of Kog remain responsive.
        for (let count = 0; row < view.rows && count < 64; ++row, ++count) {
            if (model.searchText.trim().length && model.isSearchAncestor(view.index(row, 0))) {
                view.expand(row)
                view.forceLayout()
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
