import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 620
    height: 410
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
            title: qsTr("MIDI synthesis")
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    Label { text: qsTr("Backend:"); font.bold: true }
                    ComboBox {
                        id: midiEngine
                        Layout.fillWidth: true
                        model: [
                            qsTr("RustySynth (SF2)"),
                            qsTr("OPL3Windows (Nuked OPL3)")
                        ]
                        currentIndex: root.app.midi_engine === "opl3windows" ? 1 : 0
                        onActivated: root.app.select_midi_engine(
                            currentIndex === 1 ? "opl3windows" : "rustysynth-sf2")
                    }
                }

                Label { text: qsTr("SoundFont:"); font.bold: true }

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
            text: qsTr("OPL3Windows uses Cog's General MIDI timbre table and Nuked OPL3 engine. SF3, MT-32, SC-55, and additional OPL banks remain separate milestones.")
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
