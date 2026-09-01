import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 620
    height: 560
    minimumWidth: 500
    minimumHeight: 320
    title: qsTr("Kog Preferences")
    color: palette.window

    FileDialog {
        id: soundfontDialog
        title: qsTr("Choose an SF2 SoundFont")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("SoundFont 2 banks (*.sf2)")]
        onAccepted: root.app.set_soundfont(selectedFile)
    }

    FolderDialog {
        id: sc55RomDialog
        title: qsTr("Choose the folder containing your Roland ROMs")
        onAccepted: root.app.set_sc55_rom_directory(selectedFolder)
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
                            qsTr("OPL3Windows (Nuked OPL3)"),
                            qsTr("Nuked SC-55 (Roland ROMs)")
                        ]
                        currentIndex: root.app.midi_engine === "nuked-sc55"
                            ? 2
                            : (root.app.midi_engine === "opl3windows" ? 1 : 0)
                        onActivated: root.app.select_midi_engine(
                            currentIndex === 2
                                ? "nuked-sc55"
                                : (currentIndex === 1 ? "opl3windows" : "rustysynth-sf2"))
                    }
                }

                Label {
                    text: qsTr("SoundFont:")
                    font.bold: true
                    visible: midiEngine.currentIndex === 0
                }

                Label {
                    Layout.fillWidth: true
                    visible: midiEngine.currentIndex === 0
                    text: root.app.soundfont_path.length > 0
                        ? root.app.soundfont_path
                        : qsTr("No SoundFont selected")
                    elide: Text.ElideMiddle
                    color: root.app.soundfont_path.length > 0
                        ? root.palette.text
                        : root.palette.placeholderText
                }

                RowLayout {
                    visible: midiEngine.currentIndex === 0
                    Button { text: qsTr("Choose SoundFont…"); onClicked: soundfontDialog.open() }
                    Button {
                        text: qsTr("Clear")
                        enabled: root.app.soundfont_path.length > 0
                        onClicked: root.app.clear_soundfont()
                    }
                    Item { Layout.fillWidth: true }
                }

                Label {
                    text: qsTr("Roland ROM directory:")
                    font.bold: true
                    visible: midiEngine.currentIndex === 2
                }

                Label {
                    Layout.fillWidth: true
                    visible: midiEngine.currentIndex === 2
                    text: root.app.sc55_rom_path.length > 0
                        ? root.app.sc55_rom_path
                        : qsTr("No ROM directory selected")
                    elide: Text.ElideMiddle
                    color: root.app.sc55_rom_path.length > 0
                        ? root.palette.text
                        : root.palette.placeholderText
                }

                RowLayout {
                    visible: midiEngine.currentIndex === 2
                    Button {
                        text: qsTr("Choose ROM Folder…")
                        onClicked: sc55RomDialog.open()
                    }
                    Button {
                        text: qsTr("Clear")
                        enabled: root.app.sc55_rom_path.length > 0
                        onClicked: root.app.clear_sc55_rom_directory()
                    }
                    Item { Layout.fillWidth: true }
                }

                Label {
                    Layout.fillWidth: true
                    text: root.app.midi_status
                    wrapMode: Text.Wrap
                    color: root.palette.placeholderText
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("SF2 SoundFonts and OPL3 need no proprietary firmware. Nuked SC-55 runs as a separate optional helper and requires ROM images from hardware you own; Kog does not include Roland ROMs. SF3 and MT-32 remain separate milestones.")
            wrapMode: Text.Wrap
            color: root.palette.placeholderText
        }

        Item { Layout.fillHeight: true }

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button { text: qsTr("Close"); onClicked: root.close() }
        }
    }
}
