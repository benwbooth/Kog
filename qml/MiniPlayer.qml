import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 440
    height: 112
    minimumWidth: 380
    maximumHeight: 112
    minimumHeight: 112
    title: qsTr("Kog Mini Player")
    color: "#f4f4f4"
    palette.windowText: "#303030"
    palette.text: "#303030"

    RowLayout {
        anchors.fill: parent
        anchors.margins: 10
        spacing: 8

        CogButton { glyph: "◀"; toolTip: qsTr("Previous"); onClicked: root.app.previous() }
        CogButton {
            glyph: root.app.playback_state === "playing" ? "Ⅱ" : "▶"
            toolTip: qsTr("Play/Pause")
            onClicked: root.app.play_pause()
        }
        CogButton { glyph: "■"; toolTip: qsTr("Stop"); onClicked: root.app.stop() }
        CogButton { glyph: "▶"; toolTip: qsTr("Next"); onClicked: root.app.next() }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Label { Layout.fillWidth: true; text: root.app.now_title; font.bold: true; elide: Text.ElideRight }
            Label { Layout.fillWidth: true; text: root.app.now_artist; color: "#777777"; elide: Text.ElideRight }
            Slider {
                Layout.fillWidth: true
                from: 0
                to: Math.max(1, root.app.duration_seconds)
                value: root.app.position_seconds
                onMoved: root.app.seek(value)
            }
        }
    }
}
