import QtQuick

Rectangle {
    id: root

    readonly property int numberWidth: 44
    readonly property int statusWidth: 28
    readonly property int trackWidth: 38
    readonly property int titleWidth: Math.max(150, (width - 110) * 0.25)
    readonly property int artistWidth: Math.max(125, (width - 110) * 0.20)
    readonly property int albumWidth: Math.max(135, (width - 110) * 0.22)
    readonly property int lengthWidth: 62
    readonly property int yearWidth: 54
    readonly property int genreWidth: Math.max(110, width - numberWidth - statusWidth - trackWidth - titleWidth - artistWidth - albumWidth - lengthWidth - yearWidth)

    implicitHeight: 27
    color: "#f6f6f6"
    border.color: "#d3d3d3"

    component HeaderCell: Rectangle {
        required property string label
        property int alignment: Text.AlignLeft

        height: root.height
        color: "transparent"
        border.color: "#dedede"

        Text {
            anchors.fill: parent
            anchors.leftMargin: 7
            anchors.rightMargin: 7
            text: parent.label
            color: "#4b4b4b"
            font.pixelSize: 12
            font.bold: true
            horizontalAlignment: parent.alignment
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }

    Row {
        anchors.fill: parent

        HeaderCell { width: root.numberWidth; label: "№"; alignment: Text.AlignRight }
        HeaderCell { width: root.statusWidth; label: "" }
        HeaderCell { width: root.trackWidth; label: "#"; alignment: Text.AlignRight }
        HeaderCell { width: root.titleWidth; label: qsTr("Title") }
        HeaderCell { width: root.artistWidth; label: qsTr("Artist") }
        HeaderCell { width: root.albumWidth; label: qsTr("Album") }
        HeaderCell { width: root.lengthWidth; label: qsTr("Length"); alignment: Text.AlignRight }
        HeaderCell { width: root.yearWidth; label: qsTr("Year"); alignment: Text.AlignRight }
        HeaderCell { width: root.genreWidth; label: qsTr("Genre") }
    }
}
