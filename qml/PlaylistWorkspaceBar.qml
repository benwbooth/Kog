import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: bar
    required property var app
    required property var workspaceState
    property string appendTarget: ""
    property string draggedKey: ""
    property real dragX: 0
    property real dragY: 0
    property var dropBefore: undefined
    property real dropX: -1
    readonly property var entries: workspaceState.tabs || []
    readonly property bool multipleTabs: entries.length > 1
    visible: multipleTabs
    implicitHeight: multipleTabs ? tabs.implicitHeight : 0
    function send(command) { app.workspace_command(JSON.stringify(command)) }
    function updateTabDrop(x, y) {
        dragX = x; dragY = y; dropX = -1; dropBefore = undefined
        if (x < 0 || x > width || y < 0 || y > height) return
        for (let index = 0; index < tabs.count; ++index) {
            const item = tabs.itemAt(index)
            if (item.tabKey === draggedKey) continue
            const point = item.mapToItem(bar, 0, 0)
            if (x < point.x + item.width / 2) {
                dropBefore = item.tabKey; dropX = Math.max(1, point.x); return
            }
        }
        dropBefore = null
        const last = tabs.itemAt(tabs.count - 1)
        dropX = Math.min(width - 2, last.mapToItem(bar, last.width, 0).x)
    }
    function cancelTabDrag() { draggedKey = ""; dropBefore = undefined; dropX = -1 }
    Timer {
        interval: 40; repeat: true; running: bar.draggedKey !== ""
        onTriggered: {
            if (bar.dragY < 0 || bar.dragY > bar.height) return
            const view = tabs.contentItem
            const delta = bar.dragX < 28 ? -12 : bar.dragX > bar.width - 28 ? 12 : 0
            view.contentX = Math.max(0, Math.min(Math.max(0, view.contentWidth - view.width), view.contentX + delta))
            bar.updateTabDrop(bar.dragX, bar.dragY)
        }
    }
    Rectangle {
        z: 10; x: bar.dropX; y: 2; width: 3; height: parent.height - 4
        color: tabs.palette.highlight; visible: bar.draggedKey !== "" && bar.dropX >= 0
    }
    function tabAtPoint(from, x, y) {
        const point = from.mapToItem(tabs, x, y)
        if (!visible || point.x < 0 || point.x >= tabs.width || point.y < 0 || point.y >= tabs.height)
            return ""
        for (let index = 0; index < tabs.count; ++index) {
            const item = tabs.itemAt(index)
            const local = from.mapToItem(item, x, y)
            if (local.x >= 0 && local.x < item.width && local.y >= 0 && local.y < item.height)
                return item.tabKey
        }
        return ""
    }
    // Workspace snapshots replace their array on every revision. Reconcile
    // native controls by key so TabBar keeps ownership of existing buttons.
    function syncTabs() {
        if (!tabs) return
        for (let index = 0; index < entries.length; ++index) {
            const entry = entries[index]
            let existing = index
            while (existing < tabs.count && tabs.itemAt(existing).tabKey !== entry.key)
                ++existing
            if (existing === tabs.count)
                tabs.insertItem(index, tabButton.createObject(bar, {
                    tabKey: entry.key, title: entry.name, dirty: !!entry.dirty
                }))
            else {
                if (existing !== index) tabs.moveItem(existing, index)
                const item = tabs.itemAt(index)
                item.title = entry.name
                item.dirty = !!entry.dirty
            }
        }
        while (tabs.count > entries.length) {
            const item = tabs.takeItem(tabs.count - 1)
            item.destroy()
        }
    }
    onEntriesChanged: syncTabs()
    Component.onCompleted: syncTabs()
    TabButton {
        id: tabMetrics
        visible: false
        text: qsTr("Play Queue")
        font: tabs.font
    }
    TabBar {
        id: tabs
        objectName: "playlistTabBar"
        anchors.fill: parent
        clip: true
        currentIndex: Math.max(0, bar.entries.findIndex(tab => tab.key === bar.workspaceState.active))
        // Keep native styling with a stable height even while tabs are added
        // or removed; KDE's default view assumes item zero already exists.
        contentItem: ListView {
            function revealCurrent() {
                if (bar.draggedKey === "" && width > 0 && currentIndex >= 0 && currentIndex < count)
                    positionViewAtIndex(currentIndex, ListView.Contain)
            }
            implicitWidth: contentWidth
            implicitHeight: tabMetrics.implicitHeight
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
            onWidthChanged: Qt.callLater(revealCurrent)
            onContentWidthChanged: Qt.callLater(revealCurrent)
            onCurrentIndexChanged: Qt.callLater(revealCurrent)
        }
        onCurrentIndexChanged: Qt.callLater(function() {
            // Model updates can change the current index while bindings are
            // evaluating. Dispatch keyboard navigation after they settle.
            const entry = bar.entries[tabs.currentIndex]
            if (tabs.activeFocus && entry && tabs.count === bar.entries.length
                    && entry.key !== bar.workspaceState.active)
                bar.send({op: "focus", key: entry.key})
        })
    }
    Component {
        id: tabButton
        TabButton {
            id: tab
            required property string tabKey
            required property string title
            required property bool dirty
            readonly property bool nativePaintedLabel: !!background && "text" in background
            text: title + (dirty ? " •" : "")
            width: implicitWidth
            implicitWidth: Math.ceil(implicitContentWidth) + leftPadding + rightPadding
            leftPadding: 12
            rightPadding: tabKey !== "queue" ? closeButton.width + 12 : leftPadding
            // KDE normally paints its label inside the native background,
            // centered across the whole tab regardless of content padding.
            // Draw just the label ourselves so it shares the close button's
            // layout, retaining the native frame, hover and selection states.
            Binding {
                target: tab.nativePaintedLabel ? tab.background : null
                property: "text"
                value: ""
            }
            Binding {
                target: tab
                property: "contentItem"
                value: tabLabel
                when: tab.nativePaintedLabel
            }
            Text {
                id: tabLabel
                objectName: "playlistTabLabel"
                visible: tab.nativePaintedLabel
                text: tab.text
                font: tab.font
                color: tab.palette.buttonText
                horizontalAlignment: Text.AlignLeft
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
            onClicked: bar.send({op: "focus", key: tab.tabKey})
            opacity: bar.draggedKey === tab.tabKey ? 0.55 : 1
            MouseArea {
                anchors.fill: parent
                anchors.rightMargin: closeButton.visible ? closeButton.width + 6 : 0
                enabled: !bar.workspaceState.pending_close
                preventStealing: true
                property real startX: 0
                property real startY: 0
                onPressed: mouse => { startX = mouse.x; startY = mouse.y }
                onPositionChanged: mouse => {
                    if (!pressed) return
                    if (!bar.draggedKey && Math.abs(mouse.x - startX) + Math.abs(mouse.y - startY) > Qt.styleHints.startDragDistance)
                        bar.draggedKey = tab.tabKey
                    if (bar.draggedKey) {
                        const point = mapToItem(bar, mouse.x, mouse.y)
                        bar.updateTabDrop(point.x, point.y)
                    }
                }
                onReleased: mouse => {
                    if (bar.draggedKey) {
                        const point = mapToItem(bar, mouse.x, mouse.y)
                        bar.updateTabDrop(point.x, point.y)
                        if (bar.dropBefore !== undefined)
                            bar.send({op: "move_tab", key: bar.draggedKey, before: bar.dropBefore})
                    } else bar.send({op: "focus", key: tab.tabKey})
                    bar.cancelTabDrag()
                }
                onCanceled: bar.cancelTabDrag()
            }
            Rectangle {
                anchors.fill: parent
                color: "transparent"
                border.width: 2
                border.color: tab.palette.highlight
                visible: bar.appendTarget === tab.tabKey
                radius: 3
            }
            Accessible.name: text + (tab.dirty ? qsTr("; unsaved changes") : "")
            ToolTip.visible: hovered
            ToolTip.delay: 700
            ToolTip.text: text
            ToolButton {
                id: closeButton
                objectName: "closePlaylistTab"
                visible: tab.tabKey !== "queue"
                anchors.right: parent.right
                anchors.rightMargin: 6
                anchors.verticalCenter: parent.verticalCenter
                width: 24; height: 24
                text: "×"
                font.pixelSize: 16
                display: AbstractButton.TextOnly
                flat: true
                Accessible.name: qsTr("Close %1").arg(tab.title)
                ToolTip.visible: hovered
                ToolTip.delay: 700
                ToolTip.text: Accessible.name
                onClicked: bar.send({op: "close", key: tab.tabKey})
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
