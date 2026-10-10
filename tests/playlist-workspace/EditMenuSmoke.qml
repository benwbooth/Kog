import QtQuick

Item {
    id: smoke
    required property var window
    required property var app
    required property string fixture
    required property var controls
    property int stage: 0
    property int attempts: 0
    property int playlistId: -1
    function check(value, message) {
        if (!value) {
            app.create_playlist("EDIT FAIL: " + message)
            Qt.exit(1)
            throw new Error(message)
        }
    }
    function control(name) {
        const item = controls[name]
        check(!!item, "Missing control " + name)
        return item
    }
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function snapshot() { return JSON.parse(app.workspace_json()) }
    Timer {
        interval: 100; running: true; repeat: true
        onTriggered: {
          try {
            smoke.check(++smoke.attempts < 160, "timeout at stage " + smoke.stage)
            const state = smoke.snapshot()
            if (smoke.stage === 0) {
                smoke.check(smoke.control("editMenu").title === "Edit", "Edit submenu title")
                smoke.check(!smoke.control("editUndoAction").enabled && !smoke.control("editClearAction").enabled, "Empty queue actions enabled")
                smoke.playlistId = JSON.parse(app.create_playlist("Edit test")).id
                app.open_playlist_tab(smoke.playlistId, "Edit test")
                const entry = {kind:"local", path:smoke.fixture, entry:"", fragment:null}
                smoke.send({op:"append", entries:[entry, entry, entry]})
                smoke.send({op:"save"})
                smoke.stage++
            } else if (smoke.stage === 1) {
                if (state.tabs[1].saving || state.tabs[1].dirty || state.entries.length !== 3) return
                smoke.send({op:"select", indices:[0]})
                smoke.check(smoke.control("editMoveDown").enabled, "Draft Move Down disabled")
                smoke.control("editMoveDown").triggered()
                smoke.check(smoke.snapshot().selected[0] === 1, "Move Down did not follow draft selection")
                smoke.check(smoke.control("editUndoAction").text === "Undo", "Draft undo label")
                smoke.control("editUndoAction").trigger()
                smoke.check(smoke.snapshot().selected[0] === 0, "Undo did not restore draft selection")
                smoke.control("editRedoAction").trigger()
                smoke.check(smoke.snapshot().selected[0] === 1, "Redo did not move draft selection")
                smoke.control("editClearAction").trigger()
                smoke.check(smoke.snapshot().entries.length === 0 && app.playlist_count === 0, "Clear targeted hidden queue")
                smoke.control("editUndoAction").trigger()
                smoke.check(smoke.snapshot().entries.length === 3, "Clear was not undoable")
                smoke.control("editSelectAllAction").trigger()
                smoke.check(smoke.snapshot().selected.length === 3, "Select All failed")
                smoke.control("editRemoveAction").trigger()
                smoke.check(smoke.snapshot().entries.length === 0, "Remove Selected failed")
                smoke.control("editUndoAction").trigger()
                smoke.control("editClearSelection").triggered()
                smoke.check(smoke.snapshot().selected.length === 0, "Clear Selection failed")
                smoke.control("editSaveAction").trigger()
                smoke.stage++
            } else if (smoke.stage === 2) {
                if (state.tabs[1].saving || state.tabs[1].dirty) return
                smoke.check(smoke.control("editReload").enabled, "Reload disabled on saved draft")
                smoke.send({op:"queue", action:"add_to_queue"})
                smoke.send({op:"focus", key:"queue"})
                smoke.stage++
            } else if (smoke.stage === 3) {
                if (app.playlist_count !== 3) return
                smoke.check(smoke.control("editUndoAction").text === "Undo", "Queue undo label")
                smoke.control("editUndoAction").trigger()
                smoke.check(app.playlist_count === 0, "Queue undo failed")
                smoke.control("editRedoAction").trigger()
                smoke.stage++
            } else if (smoke.stage === 4) {
                if (app.playlist_count !== 3) return
                smoke.control("editSelectAllAction").trigger()
                smoke.check(smoke.window.selectedRows.length === 3, "Queue Select All failed")
                smoke.control("editClearAction").trigger()
                smoke.check(app.playlist_count === 0 && smoke.window.selectedRows.length === 0, "Queue Clear failed")
                app.open_playlist_tab(0, "Favorites")
                smoke.stage++
            } else if (smoke.stage === 5) {
                if (state.tabs.find(tab => tab.key === state.active).loading) return
                smoke.check(!smoke.control("editSaveAction").enabled && !smoke.control("editClearAction").enabled
                    && !smoke.control("editUndoAction").enabled && !smoke.control("editRemoveAction").enabled, "Read-only actions enabled")
                app.create_playlist("Edit menu complete")
                Qt.quit()
            }
          } catch (error) { smoke.check(false, "stage " + smoke.stage + ": " + error + " " + error.stack) }
        }
    }
}
