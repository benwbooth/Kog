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
    property bool applyClose: false
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
            if (command.op === "focus") test.state = JSON.parse(JSON.stringify(Object.assign({}, test.state, {active:command.key})))
            if (command.op === "close" && test.applyClose) {
                const next = JSON.parse(JSON.stringify(test.state))
                next.tabs = next.tabs.filter(entry => entry.key !== command.key)
                if (next.active === command.key) next.active = "queue"
                test.state = next
            }
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
    Kog.PlaylistSidebarRow {
        id: source
        x: 20; y: 250; width: 300; height: 30
        visible: false
        pid: 2; name: "Playlist " + pid; entryCount: 1
        theme: theme
        onOpened: {
            const next = JSON.parse(JSON.stringify(test.state))
            const key = "local:" + pid
            if (!next.tabs.some(entry => entry.key === key))
                next.tabs.push({key:key,name:name})
            next.active = key
            test.state = next
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
    function test_repeated_double_click_close_and_reopen() {
        const native = findChild(tabs, "playlistTabBar")
        test.state = {active:"queue",tabs:[{key:"queue",name:"Play Queue"}],entries:[],selected:[],actions:{}}
        source.visible = true
        test.applyClose = true
        for (let pid = 2; pid <= 4; ++pid) {
            source.pid = pid
            mouseDoubleClickSequence(source, 80, 15)
            tryCompare(native, "count", pid)
            compare(native.currentIndex, pid - 1)
            verify(tabs.visible && tabs.height > 20)
            const added = native.itemAt(pid - 1)
            mouseDoubleClickSequence(source, 80, 15)
            compare(native.count, pid, "Reopening the playlist must focus its existing tab")
            compare(native.itemAt(pid - 1), added, "Snapshot refresh must retain the native control")
        }
        const retained = native.itemAt(3)
        const close = findChild(native.itemAt(2), "closePlaylistTab")
        mouseClick(close, close.width / 2, close.height / 2)
        tryCompare(native, "count", 3)
        compare(native.itemAt(2), retained, "Closing a middle tab must keep neighboring controls")
        compare(test.state.active, "local:4")
        source.pid = 3
        mouseDoubleClickSequence(source, 80, 15)
        tryCompare(native, "count", 4)
        compare(native.itemAt(2), retained)
        compare(native.currentIndex, 3)
        const renamed = JSON.parse(JSON.stringify(test.state))
        renamed.tabs[3].name = "Renamed playlist"
        renamed.tabs[3].dirty = true
        test.state = renamed
        compare(native.itemAt(3).text, "Renamed playlist •")
        source.visible = false
        test.applyClose = false
    }
    function test_snapshot_replacement_keeps_tabs_visible() {
        const native = findChild(tabs, "playlistTabBar")
        const entries = [{key:"queue",name:"Play Queue"}]
        for (let index = 1; index <= 6; ++index) {
            entries.push({key:"local:" + index,name:"Playlist " + index})
            // Native workspace_json returns a fresh array on every revision.
            test.state = {active:"local:" + index,tabs:JSON.parse(JSON.stringify(entries)),entries:[],selected:[],actions:{}}
            wait(20)
            tryCompare(native, "count", entries.length)
            verify(tabs.visible && tabs.height > 20, "New tab collapsed the tab strip: " + tabs.height)
            for (let tabIndex = 0; tabIndex < native.count; ++tabIndex) {
                const item = native.itemAt(tabIndex)
                verify(item.visible && item.width > 20 && item.height > 20)
            }
            for (let refresh = 0; refresh < 3; ++refresh) {
                test.state = JSON.parse(JSON.stringify(test.state))
                tryCompare(native, "count", entries.length)
                wait(20)
                verify(tabs.height > 20, "Snapshot refresh collapsed the tab strip: " + tabs.height)
            }
        }
        grabImage(panel).save("/tmp/kog-playlist-refreshed-tabs.png")
    }
    function test_track_drag_insertion_marker() {
        const view = findChild(editor, "workspacePlaylistView")
        const marker = findChild(editor, "workspaceDropIndicator")
        const line = findChild(marker, "insertionLine")
        test.state = {active:"local:1",tabs:[{key:"queue",name:"Play Queue"},{key:"local:1",name:"Mix"}],
            entries:Array.from({length:20}, (_, i) => ({path:"track" + i + ".flac"})),selected:[0,1],actions:{append:true}}
        tryCompare(view, "count", 20)
        view.positionViewAtBeginning()
        wait(20)
        const first = view.itemAtIndex(0)
        backend.commands = []
        mousePress(first, 200, 12)
        mouseMove(first, 200, 62, 20)
        compare(marker.target, 3)
        verify(marker.visible)
        fuzzyCompare(line.mapToItem(view, 0, 0).y, 72 - line.height / 2, 0.5)
        compare(test.state.selected.length, 2, "Dragging selected rows preserves the multi-selection")
        grabImage(panel).save("/tmp/kog-playlist-insertion-marker.png")
        mouseRelease(first, 200, 62)
        compare(marker.visible, false)
        compare(backend.commands[backend.commands.length - 1].op, "move")
        compare(backend.commands[backend.commands.length - 1].target, 3)

        view.positionViewAtEnd()
        wait(20)
        const last = view.itemAtIndex(19)
        mousePress(last, 200, 4)
        const end = view.mapToItem(last, 200, view.height - view.horizontalScrollGutter - 2)
        mouseMove(last, end.x, end.y, 20)
        compare(marker.target, 20)
        verify(marker.visible)
        verify(line.y + line.height <= marker.height)
        const x = line.mapToItem(view, 0, 0).x
        view.contentX = 80
        compare(line.mapToItem(view, 0, 0).x, x, "Marker stays across the viewport while columns scroll")
        mouseMove(last, -30, end.y, 20)
        compare(marker.visible, false, "Leaving the pane removes the marker")
        backend.commands = []
        mouseRelease(last, -30, end.y)
        compare(backend.commands.length, 0, "Dropping outside must not reorder")
        view.contentX = 0

        // Filtered visual gaps must point to the matching underlying entry.
        editor.searchQuery = "Second"
        tryCompare(view, "count", 19)
        view.positionViewAtBeginning()
        wait(20)
        const filtered = view.itemAtIndex(0)
        mousePress(filtered, 200, 18)
        mouseMove(filtered, 200, 1, 20)
        compare(marker.target, 0)
        compare(line.y, 0, "The first insertion line must not be clipped")
        mouseRelease(filtered, 200, 1)
        compare(backend.commands[backend.commands.length - 1].target, 1)
        test.state = Object.assign({}, test.state, {actions:{append:false}})
        filtered.dragMoved(200, 60)
        compare(marker.visible, false, "Read-only playlists have no reorder marker")
        filtered.dragCanceled()
        editor.searchQuery = ""
    }

}
