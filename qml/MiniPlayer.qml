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
    color: palette.window

    RowLayout {
        anchors.fill: parent
        anchors.margins: 10
        spacing: 8

        CogButton { glyph: "◀"; iconName: "media-skip-backward"; toolTip: qsTr("Previous"); onClicked: root.app.previous() }
        CogButton {
            glyph: root.app.playback_state === "playing" ? "Ⅱ" : "▶"
            iconName: root.app.playback_state === "playing" ? "media-playback-pause" : "media-playback-start"
            toolTip: qsTr("Play/Pause")
            onClicked: root.app.play_pause()
        }
        CogButton { glyph: "■"; iconName: "media-playback-stop"; toolTip: qsTr("Stop"); onClicked: root.app.stop() }
        CogButton { glyph: "▶"; iconName: "media-skip-forward"; toolTip: qsTr("Next"); onClicked: root.app.next() }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Label { Layout.fillWidth: true; text: root.app.now_title; font.bold: true; elide: Text.ElideRight }
            Label { Layout.fillWidth: true; text: root.app.now_artist; color: root.palette.placeholderText; elide: Text.ElideRight }
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
