import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kog.player 1.0
import "../../qml" as Source

ApplicationWindow {
    id: root
    width: 1000; height: 580; visible: true
    AppController { id: controller }
    property int stage: 0
    property int playlistId: 0
    property int attempts: 0
    readonly property var snapshot: { const revision = controller.workspace_revision; return JSON.parse(controller.workspace_json()) }
    function send(value) { controller.workspace_command(JSON.stringify(value)) }
    function check(value, message) { if (!value) { console.error("WORKSPACE FAIL: " + message); Qt.exit(1); throw new Error(message) } }
    ColumnLayout {
        anchors.fill: parent
        Source.PlaylistWorkspaceBar { Layout.fillWidth: true; app: controller; workspaceState: root.snapshot }
        Source.PlaylistEditor { Layout.fillWidth: true; Layout.fillHeight: true; app: controller; workspaceState: root.snapshot; theme: root.palette }
    }
    Component.onCompleted: {
        const created = JSON.parse(controller.create_playlist("Workspace smoke")); check(created.ok, "create"); playlistId = created.id
        controller.open_playlist_tab(playlistId, "Workspace smoke")
        check(snapshot.tabs.length === 2 && controller.playlist_count === 0, "open must leave queue alone")
        send({op:"append", entries:[{kind:"local", path:Qt.resolvedUrl("../fixtures/codec-libs/tone.wav").toString().replace("file://", ""), entry:"", fragment:null}]})
        check(snapshot.tabs[1].dirty && snapshot.entries.length === 1, "edit draft")
        send({op:"save"}); check(!snapshot.tabs[1].dirty, "save acknowledgement")
        send({op:"queue",action:"add_to_queue"})
        stage = 1
    }
    Timer {
        interval: 100; running: true; repeat: true
        onTriggered: {
            controller.poll_workspace()
            root.attempts++
            root.check(root.attempts < 150, "async queue timeout: " + controller.status)
            if (root.stage === 1 && controller.playlist_count === 1) {
                root.check(controller.playback_state !== "playing", "add should not play")
                root.send({op:"select",indices:[0]}); root.send({op:"remove"})
                root.check(controller.playlist_count === 1 && root.snapshot.entries.length === 0, "draft must not change queue")
                root.send({op:"close",key:root.snapshot.active}); root.check(!!root.snapshot.pending_close,"dirty close confirmation")
                root.send({op:"resolve_close",choice:"cancel"}); root.send({op:"undo"})
                root.check(root.snapshot.entries.length === 1 && !root.snapshot.tabs[1].dirty,"undo restores clean draft")
                controller.rename_playlist(root.playlistId,"Renamed smoke")
                root.check(root.snapshot.tabs[1].name === "Renamed smoke", "rename tab")
                root.send({op:"queue",action:"play_next"}); root.stage=2
            } else if (root.stage === 2 && controller.playlist_count === 2) {
                root.check(controller.playback_state !== "playing", "play next should not interrupt")
                root.send({op:"focus",key:"queue"}); root.check(root.snapshot.active === "queue","pinned queue")
                controller.open_playlist_tab(root.playlistId,"Renamed smoke")
                root.check(root.snapshot.tabs.length === 2,"focus existing tab")
                root.stage=3
                root.check(JSON.parse(controller.create_playlist("Workspace smoke complete")).ok, "completion marker")
                console.warn("WORKSPACE PASS: open/edit/save/queue/close/undo/rename/focus")
                Qt.quit()
            }
        }
    }
}
