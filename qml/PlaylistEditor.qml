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
    readonly property var tab: (workspaceState.tabs || []).find(item => item.key === workspaceState.active) || ({})
    readonly property var actions: workspaceState.actions || ({})
    readonly property var selection: workspaceState.selected || []
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function label(entry) {
        return entry.title || entry.name || ((entry.entry || entry.path || "").split(/[\\/]/).pop() + (entry.fragment ? " [" + entry.fragment + "]" : ""))
    }
    function choose(index, modifiers) {
        const range = !!(modifiers & Qt.ShiftModifier)
        const toggle = !!(modifiers & (Qt.ControlModifier | Qt.MetaModifier))
        send({op: "selection", command: {op: "choose", index: index,
            gesture: range ? (toggle ? "add_range" : "range") : (toggle ? "toggle" : "replace")}})
        forceActiveFocus()
    }
    Keys.onDeletePressed: send({op: "remove"})
    Shortcut { sequence: StandardKey.Save; enabled: editor.visible; onActivated: editor.send({op: "save"}) }
    Shortcut { sequence: StandardKey.Undo; enabled: editor.visible; onActivated: editor.send({op: "undo"}) }
    Shortcut { sequence: StandardKey.Redo; enabled: editor.visible; onActivated: editor.send({op: "redo"}) }
    ColumnLayout {
        anchors.fill: parent
        spacing: 4
        Flow {
            visible: !editor.compact
            Layout.fillWidth: true
            spacing: 4
            Button { text: qsTr("Play Now"); enabled: editor.actions.queue; onClicked: editor.send({op:"queue", action:"play_now"}) }
            Button { text: qsTr("Play Next"); enabled: editor.actions.queue; onClicked: editor.send({op:"queue", action:"play_next"}) }
            Button { text: qsTr("Add to Queue"); enabled: editor.actions.queue; onClicked: editor.send({op:"queue", action:"add_to_queue"}) }
            Button { text: editor.tab.saving ? qsTr("Saving…") : qsTr("Save"); enabled: editor.actions.save; onClicked: editor.send({op:"save"}) }
            Button { text: qsTr("Reload"); enabled: editor.actions.reload; onClicked: editor.send({op:"reload"}) }
        }
        Flow {
            Layout.fillWidth: true
            visible: !editor.compact && !editor.tab.readonly
            spacing: 4
            Button { text: qsTr("Add Play Queue"); enabled: editor.actions.add_play_queue; onClicked: editor.app.workspace_add_queue_selection("all") }
            Button { text: qsTr("Add Queue Selection"); enabled: editor.actions.add_queue_selection; onClicked: editor.app.workspace_add_queue_selection(editor.queueSelection.join(",")) }
            Button { text: qsTr("Remove"); enabled: editor.actions.remove; onClicked: editor.send({op:"remove"}) }
            Button { text: qsTr("Move Up"); enabled: editor.actions.move_up; onClicked: editor.send({op:"nudge", delta:-1}) }
            Button { text: qsTr("Move Down"); enabled: editor.actions.move_down; onClicked: editor.send({op:"nudge", delta:1}) }
            Button { text: qsTr("Undo"); enabled: editor.actions.undo; onClicked: editor.send({op:"undo"}) }
            Button { text: qsTr("Redo"); enabled: editor.actions.redo; onClicked: editor.send({op:"redo"}) }
        }
        Flow {
            visible: !editor.compact
            Layout.fillWidth: true; spacing: 4
            Button { text: qsTr("Select All"); enabled: editor.actions.select_all; onClicked: editor.send({op:"selection", command:{op:"all"}}) }
            Button { text: qsTr("Clear Selection"); enabled: editor.actions.clear_selection; onClicked: editor.send({op:"selection", command:{op:"clear"}}) }
        }
        RowLayout {
            visible: editor.compact
            Layout.fillWidth: true
            ToolButton {
                text: qsTr("Actions")
                onClicked: compactMenu.popup()
                Menu {
                    id: compactMenu
                    MenuItem { text: qsTr("Play Now"); enabled: editor.actions.queue; onTriggered: editor.send({op:"queue",action:"play_now"}) }
                    MenuItem { text: qsTr("Play Next"); enabled: editor.actions.queue; onTriggered: editor.send({op:"queue",action:"play_next"}) }
                    MenuItem { text: qsTr("Add to Queue"); enabled: editor.actions.queue; onTriggered: editor.send({op:"queue",action:"add_to_queue"}) }
                    MenuSeparator {}
                    MenuItem { text: qsTr("Add Play Queue"); enabled: editor.actions.add_play_queue; onTriggered: editor.app.workspace_add_queue_selection("all") }
                    MenuItem { text: qsTr("Add Queue Selection"); enabled: editor.actions.add_queue_selection; onTriggered: editor.app.workspace_add_queue_selection(editor.queueSelection.join(",")) }
                    MenuItem { text: qsTr("Remove"); enabled: editor.actions.remove; onTriggered: editor.send({op:"remove"}) }
                    MenuItem { text: qsTr("Move Up"); enabled: editor.actions.move_up; onTriggered: editor.send({op:"nudge",delta:-1}) }
                    MenuItem { text: qsTr("Move Down"); enabled: editor.actions.move_down; onTriggered: editor.send({op:"nudge",delta:1}) }
                    MenuItem { text: qsTr("Undo"); enabled: editor.actions.undo; onTriggered: editor.send({op:"undo"}) }
                    MenuItem { text: qsTr("Redo"); enabled: editor.actions.redo; onTriggered: editor.send({op:"redo"}) }
                    MenuItem { text: qsTr("Select All"); enabled: editor.actions.select_all; onTriggered: editor.send({op:"selection",command:{op:"all"}}) }
                    MenuItem { text: qsTr("Clear Selection"); enabled: editor.actions.clear_selection; onTriggered: editor.send({op:"selection",command:{op:"clear"}}) }
                    MenuItem { text: qsTr("Reload"); enabled: editor.actions.reload; onTriggered: editor.send({op:"reload"}) }
                }
            }
            Item { Layout.fillWidth: true }
            ToolButton { text: qsTr("Save"); enabled: editor.actions.save; onClicked: editor.send({op:"save"}) }
        }
        Label {
            Layout.fillWidth: true
            text: editor.tab.loading ? qsTr("Loading playlist…") : editor.workspaceState.error || (editor.tab.readonly
                ? qsTr("Favorites · use star controls to change this list")
                : editor.selection.length > 0 ? qsTr("%1 selected · playback actions use the selection").arg(editor.selection.length)
                : qsTr("%1 tracks · playback actions use the whole playlist").arg(editor.workspaceState.entries.length))
            wrapMode: Text.WordWrap
            color: editor.theme.text
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: editor.workspaceState.entries || []
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                required property int index
                required property var modelData
                width: ListView.view.width
                height: 36
                color: editor.selection.indexOf(index) >= 0 ? editor.theme.highlight : index % 2 ? editor.theme.alternateBase : editor.theme.base
                Label {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    verticalAlignment: Text.AlignVCenter
                    text: (index + 1) + ".  " + editor.label(modelData)
                    color: editor.selection.indexOf(index) >= 0 ? editor.theme.highlightedText : editor.theme.text
                    elide: Text.ElideRight
                }
                MouseArea { anchors.fill: parent; onClicked: mouse => editor.choose(index, mouse.modifiers) }
            }
        }
    }
}
