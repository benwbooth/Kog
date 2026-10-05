import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

FocusScope {
    id: row
    required property int pid
    required property string name
    required property int entryCount
    required property var theme
    property bool selected: false
    property bool renaming: false
    property bool canAppend: true
    property string destinationName: qsTr("Play Queue")
    readonly property bool favorite: pid === 0
    readonly property bool dragging: pointer.dragging
    signal selectedWithModifiers(int modifiers)
    signal opened()
    signal appendRequested()
    signal contextRequested()
    signal renameAccepted(string name)
    signal renameCancelled()
    signal navigationRequested(int delta, int modifiers)
    signal dragStarted()
    signal dragMoved(real x, real y)
    signal dragFinished(real x, real y)
    signal dragCancelled()
    implicitHeight: 30
    Accessible.role: Accessible.ListItem
    Accessible.name: name
    Accessible.selected: selected
    Keys.onReturnPressed: opened()
    Keys.onEnterPressed: opened()
    Keys.onUpPressed: event => { navigationRequested(-1, event.modifiers); event.accepted = true }
    Keys.onDownPressed: event => { navigationRequested(1, event.modifiers); event.accepted = true }
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && event.modifiers & Qt.ShiftModifier)) {
            contextRequested()
            event.accepted = true
        }
    }
    Rectangle {
        anchors.fill: parent
        radius: 4
        visible: row.selected || hover.hovered || row.activeFocus
        color: row.selected ? row.theme.highlight : row.theme.button
        border.width: row.activeFocus ? 1 : 0
        border.color: row.theme.highlight
    }
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 8
        anchors.rightMargin: 36
        spacing: 6
        Image {
            visible: row.favorite
            Layout.preferredWidth: 14
            Layout.preferredHeight: 14
            source: Qt.resolvedUrl("icons/star-filled.svg")
            sourceSize.width: 28
            sourceSize.height: 28
            fillMode: Image.PreserveAspectFit
            mipmap: true
        }
        Label {
            Layout.fillWidth: true
            visible: !row.renaming
            text: row.name
            font.pixelSize: 12
            color: row.selected ? row.theme.highlightedText : row.theme.text
            elide: Text.ElideRight
        }
        TextField {
            Layout.fillWidth: true
            visible: row.renaming
            text: row.name
            font.pixelSize: 12
            selectByMouse: true
            maximumLength: 120
            background: Item {}
            onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
            onAccepted: row.renameAccepted(text)
            Keys.onEscapePressed: row.renameCancelled()
            onActiveFocusChanged: if (!activeFocus && row.renaming) row.renameAccepted(text)
        }
        Label {
            visible: !row.renaming
            text: String(row.entryCount)
            font.pixelSize: 11
            color: row.selected ? row.theme.highlightedText : row.theme.placeholderText
        }
    }
    HoverHandler { id: hover }
    MouseArea {
        id: pointer
        anchors.fill: parent
        enabled: !row.renaming
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        preventStealing: dragging
        property real pressX: 0
        property real pressY: 0
        property bool dragging: false
        // Keep the release after a drag from also selecting/opening the row.
        property bool dragged: false
        onPressed: mouse => {
            pressX = mouse.x; pressY = mouse.y
            dragging = false; dragged = false
            row.forceActiveFocus()
            if (mouse.button === Qt.RightButton) row.contextRequested()
        }
        onPositionChanged: mouse => {
            if (!(mouse.buttons & Qt.LeftButton)) return
            if (!dragging && (Math.abs(mouse.x - pressX) >= Application.styleHints.startDragDistance
                    || Math.abs(mouse.y - pressY) >= Application.styleHints.startDragDistance)) {
                dragging = true; dragged = true
                row.dragStarted()
            }
            if (dragging) row.dragMoved(mouse.x, mouse.y)
        }
        onClicked: mouse => {
            if (mouse.button === Qt.LeftButton && !dragged) row.selectedWithModifiers(mouse.modifiers)
        }
        onDoubleClicked: mouse => {
            if (mouse.button === Qt.LeftButton && !dragged) row.opened()
        }
        onReleased: mouse => {
            if (dragging) row.dragFinished(mouse.x, mouse.y)
            dragging = false
        }
        onCanceled: { dragging = false; dragged = false; row.dragCancelled() }
    }
    ToolButton {
        id: appendButton
        objectName: "appendPlaylistButton"
        anchors.right: parent.right
        anchors.rightMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        width: 26; height: 26
        visible: !row.renaming && (hover.hovered || row.selected || row.activeFocus)
        enabled: row.canAppend && row.entryCount > 0
        text: "+"
        font.pixelSize: 18
        display: AbstractButton.TextOnly
        flat: true
        Accessible.name: qsTr("Append to “%1”").arg(row.destinationName)
        ToolTip.visible: hovered
        ToolTip.delay: 500
        ToolTip.text: row.entryCount === 0 ? qsTr("This playlist is empty")
            : (enabled ? Accessible.name : qsTr("“%1” cannot be edited").arg(row.destinationName))
        onClicked: row.appendRequested()
    }
}
