import QtQuick

Item {
    id: root

    required property var app
    required property int rowIndex
    required property var columns
    required property var theme
    property bool selected: false
    property bool hovered: false
    property string dragRows: ""
    readonly property bool playlistDrag: true
    property int revision: app.playlist_revision

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal dragStarted(int rowIndex)

    implicitHeight: 24

    Drag.active: rowDrag.active
    Drag.dragType: Drag.Automatic
    Drag.keys: ["kog-playlist-row"]
    Drag.supportedActions: Qt.MoveAction
    Drag.proposedAction: Qt.MoveAction
    Drag.mimeData: ({
        "application/x-kog-playlist-rows": root.dragRows
    })
    Drag.hotSpot.x: width / 2
    Drag.hotSpot.y: height / 2

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

    component Cell: Text {
        required property int cellWidth
        property int alignment: Text.AlignLeft

        width: cellWidth
        height: root.height
        leftPadding: 6
        rightPadding: 6
        color: root.selected ? root.theme.highlightedText : root.theme.text
        font.pixelSize: 11
        horizontalAlignment: alignment
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    Row {
        anchors.fill: parent

        Cell {
            cellWidth: columns.numberWidth
            alignment: Text.AlignHCenter
            text: {
                root.revision
                const status = app.track_status_at(root.rowIndex)
                return status.length > 0 ? status : app.track_number_at(root.rowIndex)
            }
        }
        Cell {
            cellWidth: columns.ratingWidth
            text: { root.revision; return app.track_rating_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.titleWidth
            text: { root.revision; return app.track_title_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.artistWidth
            text: { root.revision; return app.track_artist_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.albumWidth
            text: { root.revision; return app.track_album_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.lengthWidth
            alignment: Text.AlignRight
            text: { root.revision; return app.track_length_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.yearWidth
            alignment: Text.AlignRight
            text: { root.revision; return app.track_year_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.genreWidth
            text: { root.revision; return app.track_genre_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.trackWidth
            alignment: Text.AlignHCenter
            text: { root.revision; return app.track_metadata_number_at(root.rowIndex) }
        }
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true
        onEntered: root.hovered = true
        onExited: root.hovered = false
        onClicked: mouse => root.pressed(root.rowIndex, mouse.modifiers, mouse.button)
        onDoubleClicked: root.activated(root.rowIndex)
    }

    DragHandler {
        id: rowDrag
        target: null
        acceptedButtons: Qt.LeftButton
        grabPermissions: PointerHandler.CanTakeOverFromAnything
            | PointerHandler.ApprovesTakeOverByAnything
        onActiveChanged: if (active) root.dragStarted(root.rowIndex)
    }
}
