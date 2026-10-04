import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: bar
    required property var app
    required property var workspaceState
    implicitHeight: 38
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    ScrollView {
        anchors.fill: parent
        contentWidth: tabs.implicitWidth
        ScrollBar.vertical.policy: ScrollBar.AlwaysOff
        Row {
            id: tabs
            spacing: 2
            Repeater {
                model: bar.workspaceState.tabs || []
                Row {
                    required property var modelData
                    Button {
                        text: modelData.name + (modelData.dirty ? " •" : "")
                        checkable: true
                        checked: bar.workspaceState.active === modelData.key
                        onClicked: bar.send({op: "focus", key: modelData.key})
                        Accessible.name: text + (modelData.dirty ? qsTr("; unsaved changes") : "")
                    }
                    ToolButton {
                        visible: modelData.key !== "queue"
                        text: "×"
                        Accessible.name: qsTr("Close %1").arg(modelData.name)
                        onClicked: bar.send({op: "close", key: modelData.key})
                    }
                }
            }
        }
    }
    Dialog {
        id: closeDialog
        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        title: qsTr("Save playlist changes?")
        visible: bar.visible && !!bar.Window.window && bar.Window.window.visible && !!bar.workspaceState.pending_close
        closePolicy: Popup.NoAutoClose
        Label { text: qsTr("The playlist has unsaved changes.") }
        footer: DialogButtonBox {
            Button { text: qsTr("Save"); onClicked: bar.send({op: "resolve_close", choice: "save"}) }
            Button { text: qsTr("Discard"); onClicked: bar.send({op: "resolve_close", choice: "discard"}) }
            Button { text: qsTr("Cancel"); onClicked: bar.send({op: "resolve_close", choice: "cancel"}) }
        }
    }
}
