import QtQuick
import QtQuick.Controls

Window {
    id: root

    required property var app

    width: 480
    height: 270
    minimumWidth: 320
    minimumHeight: 180
    title: qsTr("Lyrics")
    color: palette.window

    ScrollView {
        anchors.fill: parent
        clip: true

        TextArea {
            id: lyricsText

            text: root.app.current_lyrics
            placeholderText: qsTr("No lyrics available for this track.")
            readOnly: true
            selectByMouse: true
            persistentSelection: true
            wrapMode: TextEdit.Wrap
            padding: 12
            font.pixelSize: 14
            Accessible.name: qsTr("Lyrics for the current track")
        }
    }
}
