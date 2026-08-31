import QtQuick

Item {
    id: root

    required property var app
    required property int rowIndex
    required property var columns
    property bool selected: false
    property int revision: app.playlist_revision

    signal pressed(int rowIndex)
    signal activated(int rowIndex)

    implicitHeight: 29

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 3
        anchors.rightMargin: 3
        radius: 5
        color: root.selected ? "#c95f00" : (root.rowIndex % 2 ? "#f1f1f1" : "#fafafa")
    }

    component Cell: Text {
        required property int cellWidth
        property int alignment: Text.AlignLeft

        width: cellWidth
        height: root.height
        leftPadding: 7
        rightPadding: 7
        color: root.selected ? "white" : "#303030"
        font.pixelSize: 12
        font.bold: root.selected
        horizontalAlignment: alignment
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    Row {
        anchors.fill: parent

        Cell {
            cellWidth: columns.numberWidth
            alignment: Text.AlignRight
            text: { root.revision; return app.track_number_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.statusWidth
            alignment: Text.AlignHCenter
            text: { root.revision; return app.track_status_at(root.rowIndex) }
        }
        Cell {
            cellWidth: columns.trackWidth
            alignment: Text.AlignRight
            text: { root.revision; return app.track_number_at(root.rowIndex) }
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
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: root.pressed(root.rowIndex)
        onDoubleClicked: root.activated(root.rowIndex)
    }
}
