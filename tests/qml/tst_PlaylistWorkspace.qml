import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtTest
import "../../qml" as Kog

TestCase {
    id: test
    name: "PlaylistWorkspace"
    width: 1000; height: 360
    visible: true
    when: windowShown
    SystemPalette { id: theme }
    property var state: ({active:"local:1", tabs:[{key:"queue",name:"Play Queue"},{key:"local:1",name:"test"}],
        entries:[{path:"one.flac"},{path:"two.flac"}], selected:[],
        actions:{append:true,queue:true,remove:false,save:false,select_all:true}})
    QtObject {
        id: backend
        property int workspace_revision: 0
        property int playlist_revision: 0
        property int current_index: -1
        property string playback_state: "paused"
        property string playlist_column_layout: ""
        property real audio_level_low: 0
        property real audio_level_low_mid: 0
        property real audio_level_mid: 0
        property real audio_level_high_mid: 0
        property real audio_level_high: 0
        property var commands: []
        function workspace_command(value) {
            const command = JSON.parse(value)
            commands.push(command)
            if (command.op === "focus") test.state = Object.assign({}, test.state, {active:command.key})
            if (command.op === "selection") test.state = Object.assign({}, test.state, {selected:[command.command.index]})
        }
        function workspace_track_value_at(row, column) {
            const values = {index:String(row+1),title:row ? "Second track" : "First track",artist:"Artist",album:"Album",length:"3:12",genre:"Ambient",date:"2026",path:row ? "two.flac" : "one.flac",filesize:"12.0 MiB"}
            return values[column] || ""
        }
        function workspace_current_index() { return -1 }
        function workspace_sort(column, descending) { commands.push({op:"sort",column:column,descending:descending}) }
        function workspace_toggle_stars(row) { commands.push({op:"star",row:row}) }
        function workspace_move(target) { commands.push({op:"move",target:target}) }
        function save_playlist_column_layout(layout) { playlist_column_layout = layout }
    }
    Rectangle {
        id: panel
        anchors.fill: parent
        color: theme.base
        ColumnLayout {
            anchors.fill: parent; spacing: 0
            Kog.PlaylistWorkspaceBar {
                id: tabs
                Layout.fillWidth: true
                app: backend; workspaceState: test.state
            }
            Kog.PlaylistEditor {
                id: editor
                Layout.fillWidth: true; Layout.fillHeight: true
                app: backend; workspaceState: test.state; theme: theme
            }
        }
    }
    function test_native_labels_and_shared_table() {
        const native = findChild(tabs, "playlistTabBar")
        const view = findChild(editor, "workspacePlaylistView")
        const header = findChild(editor, "workspacePlaylistHeader")
        compare(native.count, 2)
        compare(native.currentIndex, 1)
        compare(tabs.tabAtPoint(native.itemAt(0), 10, native.height / 2), "queue")
        compare(tabs.tabAtPoint(native.itemAt(1), 10, native.height / 2), "local:1")
        compare(tabs.tabAtPoint(tabs, 10, tabs.height + 30), "")
        compare(test.state.active, "local:1", "Finding a drop target must not switch tabs")
        for (let i = 0; i < native.count; ++i) {
            const tab = native.itemAt(i)
            if (tab.background && "text" in tab.background && tab.background.text.length)
                compare(tab.contentItem, null, "A style-painted tab label must not have a second label")
        }
        compare(view.count, 2)
        compare(header.height, 30)
        tryVerify(() => view.itemAtIndex(0) !== null)
        compare(view.itemAtIndex(0).height, 24)
        compare(view.y, header.height, "No toolbar rows between the header and tracks")
        const image = grabImage(panel)
        image.save("/tmp/kog-playlist-table-preview.png")
        mouseClick(native.itemAt(0), 30, native.height / 2)
        compare(test.state.active, "queue")
        mouseClick(native.itemAt(1), 10, native.height / 2)
        compare(test.state.active, "local:1")
        compare(findChild(native.itemAt(0), "closePlaylistTab").visible, false)
        const closeButton = findChild(native.itemAt(1), "closePlaylistTab")
        backend.commands = []
        mouseClick(closeButton, closeButton.width / 2, closeButton.height / 2)
        compare(backend.commands.length, 1)
        compare(backend.commands[0].op, "close")
        compare(backend.commands[0].key, "local:1")
        mouseClick(view.itemAtIndex(1), 200, 12)
        compare(test.state.selected[0], 1)
        mouseClick(view, 200, 150, Qt.RightButton)
        const menu = findChild(editor, "workspaceContextMenu")
        tryCompare(menu, "visible", true)
        menu.close()
        tryCompare(menu, "visible", false)
        test.state = Object.assign({}, test.state, {entries:[],selected:[]})
        tryCompare(view, "count", 0)
        waitForRendering(panel)
        grabImage(panel).save("/tmp/kog-playlist-empty-preview.png")
        test.state = {active:"queue",tabs:[{key:"queue",name:"Play Queue"}],entries:[],selected:[],actions:{}}
        tryCompare(tabs, "visible", false)
        compare(tabs.implicitHeight, 0)
        wait(0)
    }
}
