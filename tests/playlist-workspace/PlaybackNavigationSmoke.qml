import QtQuick

Item {
    id: smoke
    required property var window
    required property var app
    required property var editor
    required property string fixtures
    property int stage: 0
    property int attempts: 0
    property int playlistId: -1
    property real pausedPosition: 0
    function check(value, message) {
        if (!value) {
            app.create_playlist("PLAYBACK FAIL: " + message)
            Qt.exit(1)
            throw new Error(message)
        }
    }
    function childNamed(item, name) {
        if (item.objectName === name) return item
        for (const child of item.children || []) {
            const found = childNamed(child, name)
            if (found) return found
        }
        return null
    }
    function press(name) {
        const button = childNamed(window.footer, name)
        check(button && button.enabled, "Missing or disabled " + name)
        button.clicked()
    }
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function entry(name) { return {kind:"local", path:fixtures + "/" + name + ".wav", entry:"", fragment:null} }
    Timer {
        interval: 100; running: true; repeat: true
        onTriggered: {
            try {
                smoke.check(++smoke.attempts < 200, "timeout at stage " + smoke.stage + ": " + app.status)
                if (smoke.stage === 0) {
                    app.set_volume_level(0)
                    app.select_repeat_mode("off")
                    app.select_shuffle_mode("off")
                    const id = JSON.parse(app.create_playlist("Main queue source")).id
                    app.open_playlist_tab(id, "Main queue source")
                    smoke.send({op:"append", entries:[smoke.entry("outside-1"), smoke.entry("outside-2")]})
                    smoke.send({op:"queue", action:"add_to_queue"})
                    smoke.stage++
                } else if (smoke.stage === 1 && app.playlist_count === 2) {
                    smoke.playlistId = JSON.parse(app.create_playlist("Playing tab")).id
                    app.open_playlist_tab(smoke.playlistId, "Playing tab")
                    smoke.send({op:"append", entries:[smoke.entry("first"), smoke.entry("middle"), smoke.entry("last")]})
                    smoke.send({op:"save"})
                    smoke.editor.activate(1)
                    smoke.stage++
                } else if (smoke.stage === 2 && app.playback_state === "playing") {
                    smoke.check(app.playlist_count === 5 && app.current_index === 3, "activation did not start at the middle of the complete playlist")
                    app.seek(4)
                    smoke.stage = 20
                } else if (smoke.stage === 20 && app.position_seconds >= 3.5) {
                    smoke.editor.activate(1)
                    smoke.stage++
                } else if (smoke.stage === 21 && app.playback_state === "paused") {
                    smoke.pausedPosition = app.position_seconds
                    smoke.check(smoke.pausedPosition >= 3.5 && app.playlist_count === 5, "current row restarted instead of pausing")
                    smoke.editor.activate(1)
                    smoke.stage++
                } else if (smoke.stage === 22 && app.playback_state === "playing") {
                    smoke.check(app.position_seconds >= smoke.pausedPosition - 0.1 && app.playlist_count === 5, "current row restarted instead of resuming")
                    smoke.press("nextTrackButton")
                    smoke.stage = 3
                } else if (smoke.stage === 3 && app.current_index === 4 && app.playback_state === "playing") {
                    smoke.press("previousTrackButton")
                    smoke.stage++
                } else if (smoke.stage === 4 && app.current_index === 3 && app.playback_state === "playing") {
                    smoke.send({op:"focus", key:"queue"})
                    app.select_repeat_mode("all")
                    smoke.press("nextTrackButton")
                    smoke.stage++
                } else if (smoke.stage === 5 && app.current_index === 4 && app.playback_state === "playing") {
                    app.seek(Math.max(0, app.duration_seconds - 0.25))
                    smoke.stage++
                } else if (smoke.stage === 6 && app.current_index === 2 && app.playback_state === "playing") {
                    smoke.check(app.playlist_count === 5, "transport changed the queue contents")
                    smoke.check(JSON.parse(app.workspace_json()).active === "queue", "transport switched the viewed tab")
                    app.stop()
                    app.create_playlist("Playlist navigation complete")
                    console.warn("PLAYLIST NAVIGATION PASS: native middle-row activation and pause/resume without restart, actual Next/Previous buttons, end of stream and Repeat All stay in the playing tab while another tab is visible")
                    Qt.quit()
                }
            } catch (error) { smoke.check(false, "stage " + smoke.stage + ": " + error) }
        }
    }
}
