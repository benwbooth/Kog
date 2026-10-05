import QtQuick

Item {
    id: smoke
    required property var window
    required property var app
    required property string fixture
    property int stage: 0
    property int attempts: 0
    property int sourceId: -1
    property int destinationId: -1
    readonly property string destinationKey: "local:" + destinationId
    function check(value, message) {
        if (!value) {
            app.create_playlist("SIDEBAR FAIL: " + message)
            Qt.exit(1)
            throw new Error(message)
        }
    }
    function find(item, name) {
        if (item.objectName === name) return item
        for (const child of item.children || []) { const found = find(child, name); if (found) return found }
        return null
    }
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function drag(row, target) {
        const point = target.mapToItem(row, target.width / 2, target.height / 2)
        row.dragStarted(); row.dragMoved(point.x, point.y)
        row.dragFinished(point.x, point.y)
    }
    Timer {
        interval: 100; running: true; repeat: true
        onTriggered: {
            smoke.attempts++
            smoke.check(smoke.attempts < 200, "timeout at stage " + smoke.stage)
            const w = smoke.window
            if (smoke.stage === 0) {
                w.sidebarVisible = true; w.playlistsSectionExpanded = true
                smoke.sourceId = JSON.parse(app.create_playlist("Source songs")).id
                smoke.destinationId = JSON.parse(app.create_playlist("Current mix")).id
                app.open_playlist_tab(smoke.sourceId, "Source songs")
                const entry = {kind:"local",path:smoke.fixture,entry:"",fragment:null}
                smoke.send({op:"append",entries:[entry,entry,entry]}); smoke.send({op:"save"})
                app.open_playlist_tab(smoke.destinationId, "Current mix")
                smoke.send({op:"focus",key:"queue"})
                smoke.stage=1
                return
            }
            const row = smoke.find(w.contentItem, "sidebarPlaylist_" + smoke.sourceId)
            if (!row || row.entryCount !== 3) return
            const bar = smoke.find(w.contentItem, "playlistTabBar")
            smoke.check(bar && bar.count === w.playlistWorkspace.tabs.length, "visible tab count " + (bar ? bar.count : "missing") + " differs from workspace " + w.playlistWorkspace.tabs.length + " at stage " + smoke.stage)
            smoke.check(bar.visible && bar.height > 20, "tab bar disappeared")
            for (let i = 0; i < bar.count; ++i) {
                const tab = bar.itemAt(i)
                smoke.check(tab.visible && tab.width > 20 && tab.height > 20, "tab button disappeared")
            }
            if (smoke.stage === 1) {
                row.selectedWithModifiers(Qt.NoModifier)
                smoke.check(w.playlistWorkspace.active === "queue", "single click changed the pane")
                row.opened(); row.opened()
                smoke.check(w.playlistWorkspace.active === "local:" + smoke.sourceId && w.playlistWorkspace.tabs.length === 3, "open should reuse its tab")
                smoke.send({op:"focus",key:smoke.destinationKey})
                row.appendRequested()
                smoke.check(w.playlistWorkspace.entries.length === 3 && app.playlist_count === 0, "plus did not append to active draft")
                smoke.check(w.appendPlaylistLabel === "Append to “Current mix”", "destination label")
                smoke.send({op:"undo"}); smoke.check(w.playlistWorkspace.entries.length === 0, "draft undo")
                const queueTab = bar.itemAt(0)
                const point = queueTab.mapToItem(row, 10, queueTab.height / 2)
                smoke.check(w.playlistDragTarget(row, point.x, point.y) === "queue", "queue tab drop target")
                smoke.drag(row, queueTab)
                smoke.check(w.playlistWorkspace.active === smoke.destinationKey, "drop switched tabs")
                smoke.stage=2
            } else if (smoke.stage === 2 && app.playlist_count === 3) {
                smoke.check(app.playback_state === "stopped", "append started playback")
                smoke.send({op:"focus",key:"queue"}); smoke.send({op:"undo"})
                smoke.check(app.playlist_count === 0, "queue append undo")
                smoke.send({op:"focus",key:smoke.destinationKey})
                smoke.drag(row, smoke.find(w.contentItem, "playlistPane"))
                smoke.check(w.playlistWorkspace.entries.length === 3 && app.playlist_count === 0, "pane drop targets active draft")
                app.open_playlist_tab(0, "Favorites")
                smoke.check(!w.canAppendToTab("local:0") && !row.canAppend, "read-only target availability")
                row.appendRequested()
                smoke.check(w.playlistWorkspace.entries.length === 0 && app.playlist_count === 0, "read-only destination changed")
                smoke.send({op:"focus",key:smoke.destinationKey})
                // The context-menu path handles all selected sources as a single append.
                w.setPlaylistSelectionById([smoke.sourceId])
                w.appendPlaylists(w.orderedSelectedPlaylistIds(), w.playlistWorkspace.active)
                smoke.check(w.playlistWorkspace.entries.length === 6, "context append routing")
                smoke.send({op:"undo"}); smoke.check(w.playlistWorkspace.entries.length === 3, "context append undo")
                smoke.stage=3
            } else if (smoke.stage === 3) {
                smoke.check(bar.count === 4, "opening Favorites should retain all tabs")
                smoke.check(JSON.parse(app.create_playlist("Sidebar routing complete")).ok, "completion marker")
                Qt.quit()
            }
        }
    }
}
