pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

Item {
    id: root

    required property var app
    required property int rowIndex
    required property var columns
    required property var theme
    required property var searchModel
    property string searchQuery: ""
    property bool selected: false
    property bool hovered: false
    property int revision: app.playlist_revision
    readonly property string statusMessage: {
        revision
        return app.track_status_message_at(rowIndex)
    }
    readonly property bool isCurrentTrack: app.current_index >= 0
        && Number(app.track_number_at(rowIndex)) === app.current_index + 1
    readonly property bool isPlaying: isCurrentTrack
        && app.playback_state === "playing"
    readonly property bool isActiveTrack: isCurrentTrack
        && app.playback_state !== "stopped"

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal dragStarted(int rowIndex)
    signal dragMoved(real viewX, real viewY)
    signal dragFinished(real viewX, real viewY)
    signal dragCanceled()

    // True when the x position (in row coordinates) lands inside the
    // star column. Cells start at x=0 with no row-level margins, so the
    // hit test just walks the visible column widths.
    function starColumnHit(x) {
        const column = columnAt(x)
        return column !== null && column.id === "star"
    }

    function columnAt(x) {
        let offset = 0
        const cols = columns.visibleColumns
        for (let i = 0; i < cols.length; i++) {
            const column = cols[i]
            if (x >= offset && x < offset + column.width)
                return column
            offset += column.width
        }
        return null
    }

    // Grayed out once playback proves the file is gone; skipped over
    // by next/previous/auto-advance from then on.
    readonly property bool isMissing: {
        root.revision
        return root.app.track_missing_at(root.rowIndex)
    }

    // Per-row star state for the unified tooltip below.
    readonly property bool rowStarred: {
        root.revision
        return String(root.app.track_value_at(root.rowIndex, "star")).length > 0
    }

    // Single tooltip for the whole row, driven by the proven rowPointer
    // hover path: field contents under the cursor, Star/Unstar over the
    // star column, and the playback status only over empty cells. One
    // tooltip means no competing popups and no hover-event races.
    readonly property string hoverTip: {
        root.revision
        if (rowPointer.hoverX < 0)
            return ""
        const column = root.columnAt(rowPointer.hoverX)
        if (column === null)
            return root.statusMessage
        if (column.id === "star")
            return root.rowStarred ? qsTr("Unstar") : qsTr("Star")
        const value = String(root.app.track_value_at(root.rowIndex, column.id))
        return value.length > 0 ? value : root.statusMessage
    }

    implicitHeight: 24
    height: implicitHeight

    ListView.onPooled: {
        root.hovered = false
        rowPointer.manualDragging = false
        rowPointer.suppressNextClick = false
        rowPointer.hoverX = -1
        rowPointer.hoverViewX = -1
        rowPointer.hoverViewY = -1
        tipTimer.stop()
        fieldTip.visible = false
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        anchors.topMargin: 3
        anchors.bottomMargin: 3
        radius: 4
        color: root.selected
            ? root.theme.highlight
            : (root.hovered
                ? root.theme.button
                : (root.rowIndex % 2 ? root.theme.alternateBase : "transparent"))
    }

    component Cell: Item {
        id: cell

        required property var column
        property string text: ""

        width: column.width
        height: root.height

        SearchHighlightLabel {
            id: cellLabel
            anchors.fill: parent
            leftPadding: 6
            rightPadding: 6
            sourceText: cell.text
            query: root.searchQuery
            searchModel: root.searchModel
            wholeQuery: true
            visible: (cell.column.id !== "status" || !root.isActiveTrack)
                && cell.column.id !== "star"
            color: root.selected
                ? root.theme.highlightedText
                : (root.isMissing ? root.theme.placeholderText : root.theme.text)
            font.pixelSize: 11
            horizontalAlignment: cell.column.alignment
            verticalAlignment: Text.AlignVCenter
        }

        readonly property bool starred: cell.text.length > 0

        Image {
            anchors.centerIn: parent
            width: 16
            height: 16
            visible: cell.column.id === "star"
            source: Qt.resolvedUrl(cell.starred
                ? "icons/star-filled.svg"
                : "icons/star-outline.svg")
            opacity: cell.starred ? 1 : (starMouse.containsMouse ? 0.9 : 0.45)
            fillMode: Image.PreserveAspectFit
            mipmap: true
            Accessible.name: qsTr("Star")
        }

        MouseArea {
            id: starMouse
            anchors.fill: parent
            visible: cell.column.id === "star"
            // Hover and cursor only: clicks are handled by rowPointer's
            // star hit test, which sits above this area and would
            // otherwise swallow them (and double-toggle if both fired).
            acceptedButtons: Qt.NoButton
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            Accessible.name: cell.starred ? qsTr("Unstar") : qsTr("Star")
            Accessible.role: Accessible.Button
        }

        Loader {
            anchors.centerIn: parent
            width: 34
            height: 14
            active: cell.column.id === "status" && root.isActiveTrack

            sourceComponent: Item {
                id: playbackIndicator

                ControlsImpl.IconImage {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: 13
                    height: 13
                    sourceSize.width: 13
                    sourceSize.height: 13
                    name: root.isPlaying
                        ? "media-playback-start"
                        : "media-playback-pause"
                    color: root.selected
                        ? root.theme.highlightedText
                        : root.theme.text
                }

                Item {
                    id: playingWaveform
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: 16
                    height: 14
                    visible: root.isPlaying

                    readonly property var levels: [
                        root.app.audio_level_low,
                        root.app.audio_level_low_mid,
                        root.app.audio_level_mid,
                        root.app.audio_level_high_mid,
                        root.app.audio_level_high
                    ]
                    readonly property var colors: root.selected
                        ? ["#8cbcff", "#64d8ff", "#47eee7", "#53edb4", "#82ef99"]
                        : ["#438cf5", "#32b8ed", "#20cbd2", "#27cf9c", "#55d979"]

                    Rectangle {
                        anchors.fill: parent
                        radius: 4
                        visible: root.selected
                        color: Qt.rgba(0.02, 0.08, 0.11, 0.78)
                        border.width: 1
                        border.color: Qt.rgba(1, 1, 1, 0.18)
                    }

                    Repeater {
                        model: 5

                        Rectangle {
                            required property int index

                            x: 1 + index * 3
                            y: 1 + 12 - height
                            width: 2
                            height: 2 + 10 * Math.max(0, Math.min(1,
                                playingWaveform.levels[index]))
                            radius: 1
                            color: playingWaveform.colors[index]

                            Behavior on height {
                                NumberAnimation {
                                    duration: 70
                                    easing.type: Easing.OutCubic
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Row {
        anchors.fill: parent

        Repeater {
            model: root.columns.visibleColumns

            Cell {
                required property var modelData
                column: modelData
                text: {
                    root.revision
                    return root.app.track_value_at(root.rowIndex, modelData.id)
                }
            }
        }
    }

    MouseArea {
        id: rowPointer
        property real pressX: 0
        property real pressY: 0
        property bool manualDragging: false
        property bool suppressNextClick: false

        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true
        preventStealing: true
        scrollGestureEnabled: false
        property real hoverX: -1
        // The cursor in view coordinates, kept as properties so the tooltip's
        // bindings re-evaluate on every hover move. A mapToItem call inside a
        // binding is opaque to QML: it evaluates once, goes stale when the
        // view scrolls or the pooled delegate is reused, and pinned the tip
        // to wherever the row was first created.
        property real hoverViewX: -1
        property real hoverViewY: -1
        onEntered: {
            root.hovered = true
            hoverX = -1
        }
        onExited: {
            root.hovered = false
            hoverX = -1
            hoverViewX = -1
            hoverViewY = -1
        }
        onPressed: mouse => {
            pressX = mouse.x
            pressY = mouse.y
            manualDragging = false
        }
        onPositionChanged: mouse => {
            hoverX = mouse.x
            const view = root.ListView.view
            const viewPoint = view ? root.mapToItem(view, mouse.x, mouse.y)
                : Qt.point(-1, -1)
            hoverViewX = viewPoint.x
            hoverViewY = viewPoint.y
            if ((mouse.buttons & Qt.LeftButton) === 0)
                return
            if (!manualDragging
                    && (Math.abs(mouse.x - pressX)
                        >= Application.styleHints.startDragDistance
                        || Math.abs(mouse.y - pressY)
                        >= Application.styleHints.startDragDistance)) {
                manualDragging = true
                root.dragStarted(root.rowIndex)
            }
            if (!manualDragging)
                return
            const point = root.mapToItem(root.ListView.view,
                mouse.x, mouse.y)
            root.dragMoved(point.x, point.y)
        }
        onClicked: mouse => {
            if (suppressNextClick) {
                suppressNextClick = false
                return
            }
            // Star column toggles the star without touching selection:
            // rowPointer sits above the star cell, so this is the only
            // click path that reliably reaches it.
            if (root.starColumnHit(mouse.x)) {
                root.app.toggle_stars(String(root.rowIndex))
                return
            }
            root.pressed(root.rowIndex, mouse.modifiers, mouse.button)
        }
        onDoubleClicked: mouse => {
            if (root.starColumnHit(mouse.x))
                return
            root.activated(root.rowIndex)
        }
        onReleased: mouse => {
            if (manualDragging) {
                const point = root.mapToItem(root.ListView.view,
                    mouse.x, mouse.y)
                root.dragFinished(point.x, point.y)
                suppressNextClick = true
                mouse.accepted = true
            }
            manualDragging = false
        }
        onCanceled: {
            manualDragging = false
            suppressNextClick = false
            root.dragCanceled()
        }
        onHoverXChanged: {
            if (hoverX < 0 || manualDragging || root.hoverTip.length === 0) {
                tipTimer.stop()
                fieldTip.visible = false
            } else {
                tipTimer.restart()
            }
        }
    }

    Timer {
        id: tipTimer
        interval: 650
        repeat: false
        onTriggered: {
            if (rowPointer.hoverX >= 0 && !rowPointer.manualDragging
                    && root.hoverTip.length > 0)
                fieldTip.visible = true
        }
    }

    // Field tooltip as an explicitly positioned popup: the attached
    // ToolTip cannot take coordinates, and its default placement lands
    // mid-row. This one follows the cursor: just above it, flipped below
    // when the row hugs the pane top.
    Popup {
        id: fieldTip
        // Parent to the view, not the row: the row lives inside the
        // horizontally scrolled content, so row-local coordinates inherit
        // contentX and push the tip off the left edge of the pane.
        parent: root.ListView.view
        readonly property Item viewport: root.ListView.view
        // Wide enough for whatever the cell elided — the whole point of the
        // tip is the full text — with wrapping only past this cap.
        width: Math.min(tipLabel.implicitWidth + 18, 560)
        height: tipLabel.implicitHeight + 12
        x: {
            if (!viewport || rowPointer.hoverViewX < 0)
                return 0
            return Math.round(Math.max(4, Math.min(
                rowPointer.hoverViewX - width / 2,
                viewport.width - width - 4)))
        }
        y: {
            if (!viewport || rowPointer.hoverViewY < 0)
                return 0
            const above = rowPointer.hoverViewY - height - 6
            if (above >= 4)
                return Math.round(above)
            return Math.round(Math.min(viewport.height - height - 4,
                rowPointer.hoverViewY + 20))
        }
        modal: false
        focus: false
        closePolicy: Popup.NoAutoClose
        padding: 0
        opacity: 0.96

        background: Rectangle {
            radius: 5
            color: root.theme.window
            border.width: 1
            border.color: root.theme.mid
        }
        contentItem: Label {
            id: tipLabel
            width: fieldTip.availableWidth
            leftPadding: 9
            rightPadding: 9
            topPadding: 6
            bottomPadding: 6
            text: root.hoverTip
            color: root.theme.text
            font.pixelSize: 12
            wrapMode: Text.Wrap
            verticalAlignment: Text.AlignVCenter
        }
    }

}
