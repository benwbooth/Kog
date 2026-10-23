import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FocusScope {
    id: editor
    required property var app
    required property var workspaceState
    required property var theme
    property var queueSelection: []
    property bool compact: false
    property var searchModel: fallbackSearch
    property string searchQuery: ""
    property string sortColumn: "index"
    property bool sortAscending: true
    property int dropTarget: -1
    // Main window: one editor per open tab, each kept alive so switching tabs
    // shows it as it was left. Empty means follow whichever tab is active.
    property string tabKey: ""
    readonly property bool live: tabKey.length === 0 || workspaceState.active === tabKey
    property var frozenState: ({})
    // Checks the new state itself: `live` may not have caught up with it yet.
    onWorkspaceStateChanged: if (tabKey.length === 0 || workspaceState.active === tabKey) frozenState = workspaceState
    readonly property var shownState: live ? workspaceState : frozenState
    readonly property string activeKey: tabKey.length > 0 ? tabKey : (workspaceState.active || "")
    // Moves only when this tab's rows, their metadata or stars changed, and
    // only while it is in front, so switching tabs never re-renders them.
    property int revision: 0
    property int seenGeneration: -1
    function bump() {
        revision += 1
        seenGeneration = app.workspace_tab_generation(activeKey)
    }
    Connections {
        target: editor.app
        enabled: editor.live
        function onWorkspace_revisionChanged() { editor.bumpIfChanged() }
        function onPlaylist_revisionChanged() { editor.bumpIfChanged() }
        // Metadata for a big playlist arrives for a long time; refreshing
        // every visible cell for each batch made scrolling stutter.
        function onWorkspace_metadata_revisionChanged() { if (!metadataBump.running) metadataBump.start() }
    }
    Timer {
        id: metadataBump
        interval: 1000
        // Never mid-scroll: wait until the list has been still for a moment.
        onTriggered: if (scrollSave.running) restart(); else editor.bumpIfChanged()
    }
    function bumpIfChanged() {
        if (app.workspace_tab_generation(activeKey) !== seenGeneration) bump()
    }
    onLiveChanged: if (live) bumpIfChanged()
    onActiveKeyChanged: {
        dropTarget = -1
        scrollSave.stop()
        Qt.callLater(restoreScroll)
    }
    // Each tab keeps where it was scrolled to. Saving waits until scrolling
    // settles so the jump while switching tabs is never recorded.
    property var scrollPositions: ({})
    Timer {
        id: scrollSave
        interval: 150
        onTriggered: {
            editor.scrollPositions[editor.activeKey] = Qt.point(list.contentX, list.contentY)
            // Missing song info loads from the top of the view on.
            const top = list.indexAt(0, list.contentY + 1)
            if (top >= 0 && top < editor.rows.length)
                editor.app.workspace_tab_first_visible(editor.activeKey, editor.rows[top])
        }
    }
    function restoreScroll() {
        const saved = scrollPositions[activeKey]
        list.contentX = saved ? saved.x : 0
        list.contentY = saved ? saved.y : list.originY
        scrollSave.stop()
    }
    onVisibleChanged: if (!visible) dropTarget = -1
    signal openVisualizer()
    readonly property var tab: (shownState.tabs || []).find(item => item.key === shownState.active) || ({})
    readonly property var actions: shownState.actions || ({})
    readonly property var selection: shownState.selected || []
    // Replaced only when the shown rows really change: every row and cell
    // binds to it.
    property var rows: []
    function updateRows() {
        const words = searchQuery.toLowerCase().trim().split(/\s+/).filter(word => word.length > 0)
        const next = (shownState.entries || []).map((entry, index) => index).filter(index => {
            if (words.length === 0) return true
            const text = ["title", "artist", "albumartist", "album", "genre", "composer", "filename"]
                .map(column => String(app.workspace_tab_value_at(activeKey, index, column))).join("\n").toLowerCase()
            return words.every(word => text.indexOf(word) >= 0)
        })
        if (next.length !== rows.length || next.some((value, index) => value !== rows[index]))
            rows = next
    }
    onRevisionChanged: updateRows()
    onShownStateChanged: updateRows()
    Component.onCompleted: updateRows()
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function dropIndex(x, y) {
        if (!actions.append || x < 0 || x >= list.width - list.verticalScrollGutter
                || y < 0 || y >= list.height - list.horizontalScrollGutter)
            return -1
        return Math.max(0, Math.min(rows.length,
            Math.floor((y + list.contentY - list.originY + 12) / 24)))
    }
    function choose(index, modifiers) {
        const range = !!(modifiers & Qt.ShiftModifier)
        const toggle = !!(modifiers & (Qt.ControlModifier | Qt.MetaModifier))
        send({op: "selection", command: {op: "choose", index: index,
            gesture: range ? (toggle ? "add_range" : "range") : (toggle ? "toggle" : "replace")}})
        list.currentIndex = rows.indexOf(index)
        list.forceActiveFocus()
    }
    function selectAll() { send({op: "select", indices: rows}) }
    function activate(index) {
        send({op: "activate", index: index})
        list.currentIndex = rows.indexOf(index)
        list.forceActiveFocus()
    }
    function moveCursor(delta, modifiers) {
        if (!rows.length) return
        const next = Math.max(0, Math.min(rows.length - 1, list.currentIndex + delta))
        choose(rows[next], modifiers)
        list.positionViewAtIndex(next, ListView.Contain)
    }
    function openContext(index, modifiers) {
        if (index >= 0 && selection.indexOf(index) < 0) choose(index, modifiers)
        list.forceActiveFocus()
        contextMenu.popup()
    }
    onSearchQueryChanged: {
        updateRows()
        if (visible) send({op: "selection", command: {op: "clear"}})
    }
    Keys.onDeletePressed: send({op: "remove"})

    // The same row and column controls as the play queue, backed by draft
    // metadata. Editing this model never changes the playback queue.
    QtObject {
        id: rowData
        readonly property int playlist_revision: editor.revision
        readonly property int playlist_count: editor.rows.length
        // The playing row moves without the queue index changing (a new
        // playlist start lands at the same index), so also follow the
        // workspace, whose revision moves whenever its playing row does.
        readonly property int current_index: {
            playlist_revision
            editor.app.current_index
            editor.app.workspace_revision
            return editor.live ? editor.app.workspace_current_index() : -1
        }
        readonly property string playback_state: editor.app.playback_state
        readonly property real audio_level_low: editor.app.audio_level_low
        readonly property real audio_level_low_mid: editor.app.audio_level_low_mid
        readonly property real audio_level_mid: editor.app.audio_level_mid
        readonly property real audio_level_high_mid: editor.app.audio_level_high_mid
        readonly property real audio_level_high: editor.app.audio_level_high
        readonly property real audio_level_air: editor.app.audio_level_air
        function track_value_at(row, column) { return editor.app.workspace_tab_value_at(editor.activeKey, editor.rows[row], column) }
        function track_number_at(row) { return String(editor.rows[row] + 1) }
        function track_status_message_at(row) { return track_value_at(row, "status_message") }
        function track_missing_at(row) { return track_value_at(row, "missing") === "true" }
        function toggle_stars(row) { editor.app.workspace_toggle_stars(String(editor.rows[Number(row)])) }
    }
    QtObject {
        id: fallbackSearch
        function highlightedName(text) { return text }
        function icon_name() { return "kog-format-audio" }
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Item {
            id: headerViewport
            visible: !editor.compact
            Layout.fillWidth: true
            Layout.rightMargin: list.verticalScrollGutter
            Layout.preferredHeight: header.implicitHeight
            clip: true
            PlaylistHeader {
                id: header
                objectName: "workspacePlaylistHeader"
                x: -list.contentX
                width: Math.max(headerViewport.width, totalWidth)
                height: implicitHeight
                availableWidth: headerViewport.width
                theme: editor.theme
                app: rowData
                cacheKey: editor.activeKey
                savedLayout: editor.app.playlist_column_layout
                sortColumn: editor.sortColumn
                sortAscending: editor.sortAscending
                onSortRequested: column => {
                    if (!editor.actions.append) return
                    editor.sortAscending = column === editor.sortColumn ? !editor.sortAscending : true
                    editor.sortColumn = column
                    editor.app.workspace_sort(column, !editor.sortAscending)
                }
                onColumnLayoutChanged: layout => editor.app.save_playlist_column_layout(layout)
            }
        }
        Label {
            visible: text.length > 0
            Layout.fillWidth: true
            padding: 8
            text: editor.tab.loading ? qsTr("Loading playlist…") : editor.shownState.error || ""
            wrapMode: Text.WordWrap
            color: editor.theme.text
        }
        ListView {
            id: list
            objectName: "workspacePlaylistView"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: editor.rows.length
            reuseItems: true
            cacheBuffer: Math.min(height * 1.5, 1200)
            boundsBehavior: Flickable.StopAtBounds
            readonly property real verticalScrollGutter: verticalBar.visible ? verticalBar.implicitWidth + 4 : 0
            readonly property real horizontalScrollGutter: horizontalBar.visible ? horizontalBar.implicitHeight + 4 : 0
            contentWidth: editor.compact ? width : Math.max(width, header.totalWidth + verticalScrollGutter)
            flickableDirection: Flickable.AutoFlickDirection
            maximumFlickVelocity: 12000
            flickDeceleration: 2200
            keyNavigationEnabled: false
            Keys.onReturnPressed: if (currentIndex >= 0) editor.activate(editor.rows[currentIndex])
            Keys.onEnterPressed: if (currentIndex >= 0) editor.activate(editor.rows[currentIndex])
            Keys.onUpPressed: event => { editor.moveCursor(-1, event.modifiers); event.accepted = true }
            Keys.onDownPressed: event => { editor.moveCursor(1, event.modifiers); event.accepted = true }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
                    editor.openContext(-1, Qt.NoModifier)
                    event.accepted = true
                } else if (event.key === Qt.Key_A && event.modifiers & (Qt.ControlModifier | Qt.MetaModifier)) {
                    editor.selectAll()
                    event.accepted = true
                }
            }
            delegate: editor.compact ? classicRow : tableRow
            onContentYChanged: scrollSave.restart()
            onContentXChanged: scrollSave.restart()
            ScrollBar.vertical: ScrollBar { id: verticalBar; policy: ScrollBar.AsNeeded }
            ScrollBar.horizontal: ScrollBar { id: horizontalBar; policy: ScrollBar.AsNeeded }
            footer: Item { width: 1; height: list.horizontalScrollGutter }
            KineticWheelHandler { view: list }
            KineticWheelHandler { view: list; orientation: Qt.Horizontal }
            Item {
                anchors.fill: parent
                z: -1
                Repeater {
                    model: list.count === 0 && !editor.compact ? Math.ceil(list.height / 24) : 0
                    Rectangle {
                        required property int index
                        x: 6; y: index * 24 + 3
                        width: Math.max(0, list.width - 12 - list.verticalScrollGutter)
                        height: 18; radius: 4
                        visible: index % 2 === 1
                        color: editor.theme.alternateBase
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.RightButton
                    onClicked: editor.openContext(-1, Qt.NoModifier)
                }
            }
        }
    }
    Component {
        id: tableRow
        PlaylistRow {
            required property int index
            width: Math.max(0, list.contentWidth - list.verticalScrollGutter)
            app: rowData
            columns: header
            searchModel: editor.searchModel
            searchQuery: editor.searchQuery
            theme: editor.theme
            rowIndex: index
            selected: editor.selection.indexOf(editor.rows[index]) >= 0
            onOpenVisualizer: editor.openVisualizer()
            onPressed: (row, modifiers, button) => {
                if (button === Qt.RightButton) editor.openContext(editor.rows[row], modifiers)
                else editor.choose(editor.rows[row], modifiers)
            }
            onActivated: row => editor.activate(editor.rows[row])
            onDragStarted: row => {
                if (editor.actions.append && editor.selection.indexOf(editor.rows[row]) < 0)
                    editor.choose(editor.rows[row], Qt.NoModifier)
            }
            onDragMoved: (viewX, viewY) => editor.dropTarget = editor.dropIndex(viewX, viewY)
            onDragFinished: (viewX, viewY) => {
                const target = editor.dropIndex(viewX, viewY)
                editor.dropTarget = -1
                if (target < 0) return
                editor.app.workspace_move(target === editor.rows.length ? editor.shownState.entries.length : editor.rows[target])
            }
            onDragCanceled: editor.dropTarget = -1
        }
    }
    PlaylistDropIndicator {
        objectName: "workspaceDropIndicator"
        view: list
        target: editor.dropTarget
        color: editor.theme.highlight
        rightInset: list.verticalScrollGutter
        bottomInset: list.horizontalScrollGutter
    }
    Component {
        id: classicRow
        Rectangle {
            required property int index
            readonly property bool selected: editor.selection.indexOf(editor.rows[index]) >= 0
            readonly property int revision: rowData.playlist_revision
            width: list.width; height: 13
            color: selected ? editor.theme.highlight : editor.theme.base
            Text {
                x: 2; y: 1; width: parent.width - 40; height: 12
                text: { parent.revision; return rowData.track_number_at(index) + ". " + rowData.track_value_at(index, "title") }
                textFormat: Text.PlainText; font.pixelSize: 9; elide: Text.ElideRight
                color: parent.selected ? editor.theme.highlightedText : editor.theme.text
            }
            Text {
                anchors.right: parent.right; anchors.rightMargin: 2; y: 1
                text: { parent.revision; return rowData.track_value_at(index, "length") }
                font.pixelSize: 9; color: editor.theme.text
            }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: event => event.button === Qt.RightButton
                    ? editor.openContext(editor.rows[index], event.modifiers) : editor.choose(editor.rows[index], event.modifiers)
                onDoubleClicked: editor.activate(editor.rows[index])
            }
        }
    }
    Menu {
        id: contextMenu
        objectName: "workspaceContextMenu"
        MenuItem { text: qsTr("Play Now"); icon.name: "media-playback-start"; enabled: !!editor.actions.queue; onTriggered: editor.send({op:"queue", action:"play_now"}) }
        MenuItem { text: qsTr("Play Next"); enabled: !!editor.actions.queue; onTriggered: editor.send({op:"queue", action:"play_next"}) }
        MenuItem { text: qsTr("Add to Queue"); enabled: !!editor.actions.queue; onTriggered: editor.send({op:"queue", action:"add_to_queue"}) }
        MenuSeparator {}
        MenuItem { text: qsTr("Add Play Queue"); enabled: !!editor.actions.add_play_queue; onTriggered: editor.app.workspace_add_queue_selection("all") }
        MenuItem { text: qsTr("Add Queue Selection"); enabled: !!editor.actions.add_queue_selection; onTriggered: editor.app.workspace_add_queue_selection(editor.queueSelection.join(",")) }
        MenuItem { text: qsTr("Remove Selected"); icon.name: "edit-delete"; enabled: !!editor.actions.remove; onTriggered: editor.send({op:"remove"}) }
        MenuItem { text: qsTr("Move Up"); enabled: !!editor.actions.move_up; onTriggered: editor.send({op:"nudge", delta:-1}) }
        MenuItem { text: qsTr("Move Down"); enabled: !!editor.actions.move_down; onTriggered: editor.send({op:"nudge", delta:1}) }
        MenuSeparator {}
        MenuItem { text: qsTr("Undo"); icon.name: "edit-undo"; enabled: !!editor.actions.undo; onTriggered: editor.send({op:"undo"}) }
        MenuItem { text: qsTr("Redo"); icon.name: "edit-redo"; enabled: !!editor.actions.redo; onTriggered: editor.send({op:"redo"}) }
        MenuItem { text: qsTr("Select All"); enabled: !!editor.actions.select_all; onTriggered: editor.selectAll() }
        MenuItem { text: qsTr("Clear Selection"); enabled: !!editor.actions.clear_selection; onTriggered: editor.send({op:"selection", command:{op:"clear"}}) }
        MenuSeparator {}
        MenuItem { text: qsTr("Save"); icon.name: "document-save"; enabled: !!editor.actions.save; onTriggered: editor.send({op:"save"}) }
        MenuItem { text: qsTr("Reload"); icon.name: "view-refresh"; enabled: !!editor.actions.reload; onTriggered: editor.send({op:"reload"}) }
    }
}
