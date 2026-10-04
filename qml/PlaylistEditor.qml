import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FocusScope {
    id: editor
    required property var app
    required property var workspaceState
    required property var theme
    property var queueSelection: []
    readonly property var tab: (workspaceState.tabs || []).find(item => item.key === workspaceState.active) || ({})
    readonly property var selection: workspaceState.selected || []
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function label(entry) {
        return entry.title || entry.name || ((entry.entry || entry.path || "").split(/[\\/]/).pop() + (entry.fragment ? " [" + entry.fragment + "]" : ""))
    }
    function choose(index, modifiers) {
        let selected = selection.slice()
        if (modifiers & Qt.ControlModifier) {
            const at = selected.indexOf(index)
            if (at >= 0) selected.splice(at, 1); else selected.push(index)
        } else if ((modifiers & Qt.ShiftModifier) && selected.length > 0) {
            const first = Math.min(selected[0], index), last = Math.max(selected[0], index)
            selected = []
            for (let row = first; row <= last; row++) selected.push(row)
        } else selected = [index]
        send({op: "select", indices: selected})
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
            Layout.fillWidth: true
            spacing: 4
            Button { text: qsTr("Play Now"); enabled: !editor.tab.loading && editor.workspaceState.entries.length > 0; onClicked: editor.send({op:"queue", action:"play_now"}) }
            Button { text: qsTr("Play Next"); enabled: !editor.tab.loading && editor.workspaceState.entries.length > 0; onClicked: editor.send({op:"queue", action:"play_next"}) }
            Button { text: qsTr("Add to Queue"); enabled: !editor.tab.loading && editor.workspaceState.entries.length > 0; onClicked: editor.send({op:"queue", action:"add_to_queue"}) }
            Button { text: editor.tab.saving ? qsTr("Saving…") : qsTr("Save"); enabled: !!editor.tab.dirty && !editor.tab.saving; onClicked: editor.send({op:"save"}) }
            Button { text: qsTr("Reload"); enabled: !editor.tab.dirty && !editor.tab.loading; onClicked: editor.send({op:"reload"}) }
        }
        Flow {
            Layout.fillWidth: true
            visible: !editor.tab.readonly
            spacing: 4
            Button { text: qsTr("Add Play Queue"); onClicked: editor.app.workspace_add_queue_selection("all") }
            Button { text: qsTr("Add Queue Selection"); enabled: editor.queueSelection.length > 0; onClicked: editor.app.workspace_add_queue_selection(editor.queueSelection.join(",")) }
            Button { text: qsTr("Remove"); enabled: editor.selection.length > 0; onClicked: editor.send({op:"remove"}) }
            Button { text: qsTr("Move Up"); enabled: editor.selection.length > 0; onClicked: editor.send({op:"nudge", delta:-1}) }
            Button { text: qsTr("Move Down"); enabled: editor.selection.length > 0; onClicked: editor.send({op:"nudge", delta:1}) }
            Button { text: qsTr("Undo"); enabled: !!editor.workspaceState.can_undo; onClicked: editor.send({op:"undo"}) }
            Button { text: qsTr("Redo"); enabled: !!editor.workspaceState.can_redo; onClicked: editor.send({op:"redo"}) }
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
