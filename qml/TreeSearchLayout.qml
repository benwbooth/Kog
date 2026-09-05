import QtQuick

// Hide query resets until the first layout settles, then keep existing rows
// visible while new matches arrive. Opacity allows hidden layouts to polish.
QtObject {
    id: layout
    required property var view
    required property var model
    property bool ready: true
    property bool busy: false
    property int nextRow: -1
    property bool anotherPass: false
    property int settlingFrames: 0
    property var openedAncestors: ({})
    property Timer work: Timer {
        interval: 16
        repeat: true
        onTriggered: layout.expandBatch()
    }

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
        if (reset) {
            work.stop()
            busy = false
            nextRow = -1
            ready = false
            openedAncestors = ({})
            settlingFrames = 0
        }
        if (reset && model.searching) return
        anotherPass = true
        busy = true
        work.start()
    }

    function expandBatch() {
        // Polish once per frame, never once per expanded ancestor. Walking
        // backwards keeps newly inserted descendants below unvisited rows.
        view.forceLayout()
        if (nextRow < 0) {
            nextRow = view.rows - 1
            anotherPass = false
        }
        const started = Date.now()
        for (let count = 0; nextRow >= 0 && count < 512 && Date.now() - started < 3; ++count) {
            const row = nextRow--
            if (model.searchText.trim().length && model.isSearchAncestor(view.index(row, 0))) {
                const key = model.filePath(view.index(row, 0))
                if (!openedAncestors[key]) {
                    view.expand(row)
                    // A batch can end with a parent whose children arrive in
                    // the next batch. Retry it then if expansion was premature.
                    if (view.isExpanded(row)) {
                        openedAncestors[key] = true
                        anotherPass = true
                    }
                }
            }
        }
        // Reveal the first stable frame while expansion continues. New
        // streamed batches do not restart a pass or hide existing matches.
        const window = view.Window.window
        if (!ready && settlingFrames === 0 && window && window.visible) {
            settlingFrames = 2
            window.update()
        } else if (!window || !window.visible) {
            ready = true
        }
        if (nextRow < 0 && !anotherPass) {
            busy = false
            work.stop()
        }
    }
}
