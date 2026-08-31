import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 330
    height: 570
    minimumWidth: 280
    minimumHeight: 420
    title: qsTr("Info Inspector")
    color: "#8c8c8c"

    function timeLabel(seconds) {
        const value = Math.max(0, Math.floor(seconds))
        const hours = Math.floor(value / 3600)
        const minutes = Math.floor((value % 3600) / 60)
        const remaining = value % 60
        return hours > 0
            ? hours + ":" + String(minutes).padStart(2, "0") + ":" + String(remaining).padStart(2, "0")
            : minutes + ":" + String(remaining).padStart(2, "0")
    }

    function fileName(path) {
        const normalized = path.replace(/\\/g, "/")
        return normalized.substring(normalized.lastIndexOf("/") + 1)
    }

    component InfoLine: RowLayout {
        required property string name
        required property string value

        Layout.fillWidth: true
        spacing: 12
        visible: value.length > 0

        Label {
            Layout.preferredWidth: 112
            horizontalAlignment: Text.AlignRight
            text: parent.name + ":"
            color: "white"
            font.bold: true
        }
        Label {
            Layout.fillWidth: true
            text: parent.value
            color: "white"
            font.bold: true
            elide: Text.ElideMiddle
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 10

        Label {
            Layout.alignment: Qt.AlignHCenter
            text: root.app.now_title
            color: "white"
            font.pixelSize: 16
            font.bold: true
            elide: Text.ElideRight
        }

        Item { Layout.preferredHeight: 4 }
        InfoLine { name: qsTr("Artist"); value: root.app.now_artist }
        InfoLine { name: qsTr("Album"); value: root.app.current_album }
        InfoLine { name: qsTr("Title"); value: root.app.now_title === "Not Playing" ? "" : root.app.now_title }
        InfoLine { name: qsTr("Track"); value: root.app.current_track_number }
        InfoLine { name: qsTr("Length"); value: root.app.duration_seconds > 0 ? root.timeLabel(root.app.duration_seconds) : "" }
        InfoLine { name: qsTr("Date"); value: root.app.current_year }
        InfoLine { name: qsTr("Genre"); value: root.app.current_genre }
        InfoLine { name: qsTr("Filename"); value: root.fileName(root.app.current_file) }
        InfoLine { name: qsTr("Format"); value: root.app.current_codec }
        InfoLine { name: qsTr("Sample Rate"); value: root.app.current_sample_rate }
        InfoLine { name: qsTr("Channels"); value: root.app.current_channels }
        InfoLine { name: qsTr("Bitrate"); value: root.app.current_bitrate }
        InfoLine { name: qsTr("Bits Per Sample"); value: root.app.current_bits_per_sample }

        Item { Layout.fillHeight: true }

        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 176
            Layout.preferredHeight: 176
            color: "#c8c8c8"

            Text {
                anchors.centerIn: parent
                text: "♫"
                color: "#808080"
                font.pixelSize: 78
            }
        }

        Label {
            Layout.fillWidth: true
            text: root.app.current_file
            color: "#eeeeee"
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideMiddle
        }
    }
}
