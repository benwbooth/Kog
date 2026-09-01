import QtQuick

Rectangle {
    id: root

    required property var theme

    readonly property int numberWidth: 54
    readonly property int ratingWidth: 78
    readonly property int lengthWidth: 70
    readonly property int yearWidth: 58
    readonly property int trackWidth: 54
    readonly property real flexibleWidth: Math.max(0,
        width - numberWidth - ratingWidth - lengthWidth - yearWidth - trackWidth)
    readonly property int titleWidth: flexibleWidth * 0.28
    readonly property int artistWidth: flexibleWidth * 0.23
    readonly property int albumWidth: flexibleWidth * 0.28
    readonly property int genreWidth: width - numberWidth - ratingWidth
        - titleWidth - artistWidth - albumWidth - lengthWidth - yearWidth - trackWidth

    implicitHeight: 30
    color: theme.window
    border.color: theme.mid

    component HeaderCell: Rectangle {
        required property string label
        property int alignment: Text.AlignLeft

        height: root.height
        color: "transparent"

        Rectangle {
            anchors.right: parent.right
            width: 1
            height: parent.height * 0.56
            anchors.verticalCenter: parent.verticalCenter
            color: root.theme.mid
        }

        Text {
            anchors.fill: parent
            anchors.leftMargin: 7
            anchors.rightMargin: 7
            text: parent.label
            color: root.theme.buttonText
            font.pixelSize: 11
            horizontalAlignment: parent.alignment
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }

    Row {
        anchors.fill: parent

        HeaderCell { width: root.numberWidth; label: "#"; alignment: Text.AlignHCenter }
        HeaderCell { width: root.ratingWidth; label: qsTr("Rating") }
        HeaderCell { width: root.titleWidth; label: qsTr("Title") }
        HeaderCell { width: root.artistWidth; label: qsTr("Artist") }
        HeaderCell { width: root.albumWidth; label: qsTr("Album") }
        HeaderCell { width: root.lengthWidth; label: qsTr("Length"); alignment: Text.AlignRight }
        HeaderCell { width: root.yearWidth; label: qsTr("Year"); alignment: Text.AlignRight }
        HeaderCell { width: root.genreWidth; label: qsTr("Genre") }
        HeaderCell { width: root.trackWidth; label: "№"; alignment: Text.AlignHCenter }
    }
}
