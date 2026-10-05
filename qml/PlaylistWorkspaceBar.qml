import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: bar
    required property var app
    required property var workspaceState
    property string appendTarget: ""
    readonly property var entries: workspaceState.tabs || []
    readonly property bool multipleTabs: entries.length > 1
    visible: multipleTabs
    implicitHeight: multipleTabs ? tabs.implicitHeight : 0
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function tabAtPoint(from, x, y) {
        const point = from.mapToItem(tabs, x, y)
        if (!visible || point.x < 0 || point.x >= tabs.width || point.y < 0 || point.y >= tabs.height)
            return ""
        for (let index = 0; index < tabs.count; ++index) {
            const item = tabs.itemAt(index)
            const local = from.mapToItem(item, x, y)
            if (local.x >= 0 && local.x < item.width && local.y >= 0 && local.y < item.height)
                return item.modelData.key
        }
        return ""
    }
    TabBar {
        id: tabs
        objectName: "playlistTabBar"
        anchors.fill: parent
        clip: true
        currentIndex: Math.max(0, bar.entries.findIndex(tab => tab.key === bar.workspaceState.active))
        // KDE's default ListView dereferences item zero while a Repeater is
        // being rebuilt. Keep its native buttons but allow an empty model.
        contentItem: ListView {
            implicitWidth: contentWidth
            implicitHeight: tabs.count > 0 && tabs.itemAt(0) ? tabs.itemAt(0).implicitHeight : 0
            model: tabs.contentModel
            currentIndex: tabs.currentIndex
            orientation: ListView.Horizontal
            spacing: tabs.spacing
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.AutoFlickIfNeeded
            snapMode: ListView.SnapToItem
            highlightMoveDuration: 0
            highlightRangeMode: ListView.ApplyRange
            preferredHighlightBegin: 40
            preferredHighlightEnd: width - 40
        }
        onCurrentIndexChanged: Qt.callLater(function() {
            // Model updates can change the current index while bindings are
            // evaluating. Dispatch keyboard navigation after they settle.
            const entry = bar.entries[tabs.currentIndex]
            if (tabs.activeFocus && entry && tabs.count === bar.entries.length
                    && entry.key !== bar.workspaceState.active)
                bar.send({op: "focus", key: entry.key})
        })
        Repeater {
            model: bar.entries
            TabButton {
                id: tab
                required property var modelData
                text: modelData.name + (modelData.dirty ? " •" : "")
                // Keep the style's own label. KDE paints its text in the
                // background, so replacing contentItem draws it twice.
                width: implicitWidth + (closeButton.visible ? closeButton.width : 0)
                rightPadding: leftPadding + (closeButton.visible ? closeButton.width + 6 : 0)
                onClicked: bar.send({op: "focus", key: modelData.key})
                Rectangle {
                    anchors.fill: parent
                    color: "transparent"
                    border.width: 2
                    border.color: tab.palette.highlight
                    visible: bar.appendTarget === tab.modelData.key
                    radius: 3
                }
                Accessible.name: text + (modelData.dirty ? qsTr("; unsaved changes") : "")
                ToolTip.visible: hovered
                ToolTip.delay: 700
                ToolTip.text: text
                ToolButton {
                    id: closeButton
                    objectName: "closePlaylistTab"
                    visible: tab.modelData.key !== "queue"
                    anchors.right: parent.right
                    anchors.rightMargin: 5
                    anchors.verticalCenter: parent.verticalCenter
                    width: 24; height: 24
                    text: "×"
                    font.pixelSize: 16
                    display: AbstractButton.TextOnly
                    flat: true
                    Accessible.name: qsTr("Close %1").arg(tab.modelData.name)
                    ToolTip.visible: hovered
                    ToolTip.delay: 700
                    ToolTip.text: Accessible.name
                    onClicked: bar.send({op: "close", key: tab.modelData.key})
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
