import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 620
    height: 360
    minimumWidth: 500
    minimumHeight: 320
    title: qsTr("Kog Preferences")
    color: "#f4f4f4"
    palette.windowText: "#303030"
    palette.text: "#303030"
    palette.button: "#f2f2f2"
    palette.buttonText: "#303030"

    FileDialog {
        id: soundfontDialog
        title: qsTr("Choose an SF2 SoundFont")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("SoundFont 2 banks (*.sf2)")]
        onAccepted: root.app.set_soundfont(selectedFile)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 14

        Label {
            text: qsTr("MIDI")
            font.pixelSize: 20
            font.bold: true
        }

        GroupBox {
            title: qsTr("SoundFont synthesis")
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    Label { text: qsTr("Backend:"); font.bold: true }
                    Label { text: qsTr("RustySynth (SF2)") }
                    Item { Layout.fillWidth: true }
                }

                Label {
                    Layout.fillWidth: true
                    text: root.app.soundfont_path.length > 0
                        ? root.app.soundfont_path
                        : qsTr("No SoundFont selected")
                    elide: Text.ElideMiddle
                    color: root.app.soundfont_path.length > 0 ? "#303030" : "#777777"
                }

                RowLayout {
                    Button { text: qsTr("Choose SoundFont…"); onClicked: soundfontDialog.open() }
                    Button {
                        text: qsTr("Clear")
                        enabled: root.app.soundfont_path.length > 0
                        onClicked: root.app.clear_soundfont()
                    }
                    Item { Layout.fillWidth: true }
                }

                Label {
                    Layout.fillWidth: true
                    text: root.app.midi_status
                    wrapMode: Text.Wrap
                    color: "#555555"
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("SF3, OPL3, MT-32, and SC-55 rendering are tracked separately and are not enabled in this milestone.")
            wrapMode: Text.Wrap
            color: "#666666"
        }

        Item { Layout.fillHeight: true }

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button { text: qsTr("Close"); onClicked: root.close() }
        }
    }
}
