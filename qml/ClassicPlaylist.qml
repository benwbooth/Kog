import QtQuick
import QtQuick.Controls

Item {
    id: root
    required property var app
    required property var skin
    signal closeRequested()
    property var selectedRows: []
    property int selectionAnchor: -1
    readonly property string sheet: (skin.assets || {}).pledit || ""
    readonly property var colors: skin.playlistColors || ({})
    readonly property color normal: colors.normal || "#00ff00"
    readonly property color current: colors.current || "#ffffff"
    readonly property color normalBg: colors.normalbg || "#000000"
    readonly property color selectedBg: colors.selectedbg || "#0000c6"
    function selectRow(row, modifiers) {
        if ((modifiers & Qt.ShiftModifier) && selectionAnchor >= 0) {
            const rows = []
            for (let i = Math.min(row, selectionAnchor); i <= Math.max(row, selectionAnchor); ++i) rows.push(i)
            selectedRows = rows
        } else if (modifiers & Qt.ControlModifier) {
            selectedRows = selectedRows.includes(row) ? selectedRows.filter(i => i !== row) : selectedRows.concat([row])
            selectionAnchor = row
        } else { selectedRows = [row]; selectionAnchor = row }
        list.currentIndex = row
        list.forceActiveFocus()
    }
    function removeSelected() {
        if (selectedRows.length) app.remove_tracks(selectedRows.join(","))
        selectedRows = []
    }
    Connections {
        target: root.app
        function onPlaylist_countChanged() {
            root.selectedRows = root.selectedRows.filter(row => row < root.app.playlist_count)
            if (root.selectionAnchor >= root.app.playlist_count) root.selectionAnchor = -1
        }
    }
    Rectangle { anchors.fill: parent; color: "#252a3d" }
    // Tile the original bitmap borders without smoothing or stretching them.
    Repeater {
        model: 11
        SkinSprite { required property int index; x: index * 25; width: 25; height: 20; source: root.sheet; sheetX: 127 }
    }
    SkinSprite { width: 25; height: 20; source: root.sheet }
    SkinSprite { x: 88; width: 100; height: 20; source: root.sheet; sheetX: 26 }
    SkinSprite { x: 250; width: 25; height: 20; source: root.sheet; sheetX: 153 }
    Text { anchors.horizontalCenter: parent.horizontalCenter; y: 4; text: qsTr("PLAYLIST"); font.pixelSize: 9; color: "white"; visible: !root.sheet }
    MouseArea { x: 262; y: 3; width: 10; height: 11; onClicked: root.closeRequested() }
    Item {
        y: 20; width: 275; height: root.height - 58; clip: true
        Repeater {
            model: Math.ceil(parent.height / 29)
            Item {
                required property int index
                y: index * 29; width: 275; height: 29
                SkinSprite { width: 12; height: 29; source: root.sheet; sheetY: 42 }
                SkinSprite { x: 255; width: 20; height: 29; source: root.sheet; sheetX: 31; sheetY: 42 }
            }
        }
    }
    SkinSprite { y: root.height - 38; width: 125; height: 38; source: root.sheet; sheetY: 72 }
    SkinSprite { x: 125; y: root.height - 38; width: 150; height: 38; source: root.sheet; sheetX: 126; sheetY: 72 }
    Rectangle { x: 12; y: 20; width: 243; height: root.height - 58; color: root.normalBg }
    ListView {
        id: list
        objectName: "classicPlaylist"
        x: 12; y: 20; width: 243; height: root.height - 58
        clip: true
        model: root.app.playlist_count
        reuseItems: true
        boundsBehavior: Flickable.StopAtBounds
        Keys.onReturnPressed: if (currentIndex >= 0) root.app.activate_playlist_index(currentIndex)
        Keys.onDeletePressed: root.removeSelected()
        Keys.onPressed: event => {
            if (event.key === Qt.Key_A && (event.modifiers & Qt.ControlModifier)) {
                root.selectedRows = Array.from({length: count}, (_, i) => i)
                event.accepted = true
            }
        }
        delegate: Rectangle {
            id: row
            required property int index
            width: list.width; height: 13
            readonly property bool selected: root.selectedRows.includes(index)
            readonly property int revision: root.app.playlist_revision
            readonly property string number: { revision; return root.app.track_number_at(index) }
            readonly property string title: { revision; return root.app.track_value_at(index, "title") }
            readonly property string duration: { revision; return root.app.track_value_at(index, "length") }
            color: selected ? root.selectedBg : root.normalBg
            Text {
                x: 2; y: 1; width: parent.width - 40; height: 12
                text: row.number + ". " + row.title; textFormat: Text.PlainText
                font.pixelSize: 9; elide: Text.ElideRight
                color: Number(row.number) === root.app.current_index + 1 ? root.current : root.normal
            }
            Text { anchors.right: parent.right; anchors.rightMargin: 2; y: 1; text: row.duration; font.pixelSize: 9; color: root.normal }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                property real startY: 0
                property bool moving: false
                onPressed: event => {
                    startY = mapToItem(list, event.x, event.y).y
                    moving = false
                    if (!row.selected || event.modifiers) root.selectRow(row.index, event.modifiers)
                }
                onPositionChanged: event => {
                    if ((pressedButtons & Qt.LeftButton) && Math.abs(mapToItem(list, event.x, event.y).y - startY) > 8)
                        moving = true
                }
                onReleased: event => {
                    if (moving) {
                        const point = mapToItem(list.contentItem, event.x, event.y)
                        const target = Math.max(0, Math.min(list.count, Math.floor(point.y / 13)))
                        root.app.move_tracks(root.selectedRows.join(","), target)
                        root.selectedRows = []
                    } else if (event.button === Qt.RightButton) editMenu.popup()
                    moving = false
                }
                onDoubleClicked: root.app.activate_playlist_index(row.index)
            }
        }
        ScrollBar.vertical: ScrollBar {
            parent: root
            x: 259; y: 20; width: 8; height: list.height
            policy: ScrollBar.AlwaysOn
            minimumSize: Math.min(1, 18 / height)
            contentItem: Item {
                SkinSprite { anchors.centerIn: parent; width: 8; height: 18; source: root.sheet; sheetX: 52; sheetY: 53; visible: !!root.sheet }
                Rectangle { anchors.fill: parent; color: root.normal; visible: !root.sheet }
            }
            background: Item {}
        }
        KineticWheelHandler { view: list }
    }
    DropArea {
        x: 12; y: 20; width: 243; height: root.height - 58
        onDropped: drop => { if (drop.hasUrls) { root.app.enqueue_urls_json(JSON.stringify(drop.urls)); drop.acceptProposedAction() } }
    }
    Menu {
        id: editMenu
        MenuItem { text: qsTr("Add files…"); icon.name: "document-open"; onTriggered: root.app.open_audio_files() }
        MenuItem { text: qsTr("Remove selected"); enabled: root.selectedRows.length > 0; icon.name: "edit-delete"; onTriggered: root.removeSelected() }
        MenuItem { text: qsTr("Save playlist…"); icon.name: "document-save"; onTriggered: root.app.save_playlist() }
        MenuItem { text: qsTr("Clear playlist"); icon.name: "edit-clear-list"; onTriggered: root.app.clear_playlist() }
    }
    Item {
        y: root.height - 30; width: 275; height: 18
        Repeater {
            model: [qsTr("ADD"), qsTr("REM"), qsTr("SEL"), qsTr("MISC"), qsTr("LIST")]
            Item {
                required property int index
                required property string modelData
                x: index === 4 ? 231 : 14 + index * 29
                width: 22; height: 18
                Text { anchors.centerIn: parent; text: modelData; font.pixelSize: 7; color: "white"; visible: !root.sheet }
                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        if (index === 0) root.app.open_audio_files()
                        else if (index === 1) root.removeSelected()
                        else if (index === 2) root.selectedRows = Array.from({length: list.count}, (_, i) => i)
                        else editMenu.popup()
                    }
                }
            }
        }
    }
    Text { x: 132; y: root.height - 28; width: 88; text: qsTr("%1 tracks").arg(list.count); font.pixelSize: 8; color: root.normal; elide: Text.ElideRight }
    Row {
        x: 128; y: root.height - 16
        Repeater {
            model: ["⏮", "▶", "Ⅱ", "■", "⏭", "+"]
            Item {
                required property int index
                required property string modelData
                width: 10; height: 10
                Text { anchors.centerIn: parent; text: modelData; font.pixelSize: 7; color: root.normal; visible: !root.sheet }
                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        if (index === 0) root.app.previous()
                        else if (index === 1 && root.app.playback_state !== "playing") root.app.play_pause()
                        else if (index === 2 && root.app.playback_state === "playing") root.app.play_pause()
                        else if (index === 3) root.app.stop()
                        else if (index === 4) root.app.next()
                        else if (index === 5) root.app.open_audio_files()
                    }
                }
            }
        }
    }
}
