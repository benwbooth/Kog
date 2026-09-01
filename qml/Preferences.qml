pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 760
    height: 520
    minimumWidth: 660
    minimumHeight: 430
    title: qsTr("Kog Preferences")
    color: palette.window

    property int currentPage: 0

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Pane {
            Layout.preferredWidth: 184
            Layout.fillHeight: true
            padding: 8

            background: Rectangle {
                color: root.palette.alternateBase
                border.color: root.palette.mid
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                Label {
                    Layout.leftMargin: 10
                    Layout.topMargin: 8
                    Layout.bottomMargin: 6
                    text: qsTr("Preferences")
                    font.pixelSize: 17
                    font.bold: true
                }

                Repeater {
                    model: [
                        { title: qsTr("Playlist"), iconName: "view-media-playlist" },
                        { title: qsTr("Output"), iconName: "audio-volume-high" },
                        { title: qsTr("General"), iconName: "configure" },
                        { title: qsTr("Synthesis"), iconName: "audio-midi" }
                    ]

                    ItemDelegate {
                        required property int index
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.title
                        icon.name: modelData.iconName
                        highlighted: root.currentPage === index
                        onClicked: root.currentPage = index
                    }
                }

                Item { Layout.fillHeight: true }
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: root.currentPage

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    x: 22
                    width: parent.width - 44
                    spacing: 18

                    Label {
                        text: qsTr("Playlist")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    GroupBox {
                        title: qsTr("When opening files")
                        Layout.fillWidth: true

                        RowLayout {
                            anchors.fill: parent
                            Label { text: qsTr("Normally:") }
                            ComboBox {
                                Layout.fillWidth: true
                                model: [qsTr("Add to the current playlist"), qsTr("Replace the current playlist")]
                                currentIndex: root.app.opening_files_behavior === "replace" ? 1 : 0
                                onActivated: root.app.select_opening_files_behavior(
                                    currentIndex === 1 ? "replace" : "add")
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("When adding folders")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            CheckBox {
                                text: qsTr("Read CUE sheets")
                                checked: root.app.read_cue_sheets_in_folders
                                onToggled: root.app.set_folder_cue_mode(checked)
                            }
                            CheckBox {
                                text: qsTr("Read M3U and PLS playlist files")
                                checked: root.app.read_playlists_in_folders
                                onToggled: root.app.set_folder_playlist_mode(checked)
                            }
                            Label {
                                Layout.fillWidth: true
                                text: qsTr("Folders dropped onto the playlist are scanned recursively. Unsupported files are ignored.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    x: 22
                    width: parent.width - 44
                    spacing: 18

                    Label {
                        text: qsTr("Output")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    GroupBox {
                        title: qsTr("Audio output")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 10
                            RowLayout {
                                Layout.fillWidth: true
                                Label { text: qsTr("Device:") }
                                Label {
                                    Layout.fillWidth: true
                                    text: qsTr("System default")
                                    color: root.palette.placeholderText
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                Label { text: qsTr("Volume:") }
                                Slider {
                                    Layout.fillWidth: true
                                    from: 0
                                    to: 1
                                    value: root.app.volume
                                    onMoved: root.app.set_volume_level(value)
                                }
                                Label {
                                    Layout.preferredWidth: 44
                                    horizontalAlignment: Text.AlignRight
                                    text: Math.round(root.app.volume * 100) + "%"
                                }
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    x: 22
                    width: parent.width - 44
                    spacing: 18

                    Label {
                        text: qsTr("General")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    GroupBox {
                        title: qsTr("Music folder")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 10
                            Label {
                                Layout.fillWidth: true
                                text: root.app.directory_path
                                elide: Text.ElideMiddle
                            }
                            RowLayout {
                                Button {
                                    text: qsTr("Choose…")
                                    icon.name: "folder-open"
                                    onClicked: root.app.choose_music_folder()
                                }
                                Item { Layout.fillWidth: true }
                            }
                        }
                    }

                    GroupBox {
                        title: qsTr("Appearance")
                        Layout.fillWidth: true

                        Label {
                            anchors.fill: parent
                            text: qsTr("Kog follows the current Qt platform theme, color scheme, fonts, controls, and icon theme.")
                            wrapMode: Text.Wrap
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    x: 22
                    width: parent.width - 44
                    spacing: 18

                    Label {
                        text: qsTr("Synthesis")
                        font.pixelSize: 22
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
                                Label { text: qsTr("Backend:") }
                                ComboBox {
                                    id: midiEngine
                                    Layout.fillWidth: true
                                    model: [
                                        qsTr("RustySynth (SF2)"),
                                        qsTr("OPL3Windows (Nuked OPL3)"),
                                        qsTr("Nuked SC-55"),
                                        qsTr("Munt (MT-32 / CM-32L)")
                                    ]
                                    currentIndex: root.app.midi_engine === "munt-mt32"
                                        ? 3
                                        : (root.app.midi_engine === "nuked-sc55"
                                            ? 2
                                            : (root.app.midi_engine === "opl3windows" ? 1 : 0))
                                    onActivated: root.app.select_midi_engine(
                                        currentIndex === 3
                                            ? "munt-mt32"
                                            : (currentIndex === 2
                                                ? "nuked-sc55"
                                                : (currentIndex === 1 ? "opl3windows" : "rustysynth-sf2")))
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
                                Button {
                                    text: qsTr("Choose SoundFont…")
                                    icon.name: "document-open"
                                    onClicked: root.app.choose_soundfont_file()
                                }
                                Button {
                                    text: qsTr("Clear")
                                    icon.name: "edit-clear"
                                    enabled: root.app.soundfont_path.length > 0
                                    onClicked: root.app.clear_soundfont()
                                }
                                Item { Layout.fillWidth: true }
                            }

                            Label {
                                text: qsTr("Roland SC-55 ROM directory:")
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
                                    icon.name: "folder-open"
                                    onClicked: root.app.choose_sc55_rom_folder()
                                }
                                Button {
                                    text: qsTr("Clear")
                                    icon.name: "edit-clear"
                                    enabled: root.app.sc55_rom_path.length > 0
                                    onClicked: root.app.clear_sc55_rom_directory()
                                }
                                Item { Layout.fillWidth: true }
                            }

                            Label {
                                text: qsTr("MT-32 / CM-32L ROM directory:")
                                font.bold: true
                                visible: midiEngine.currentIndex === 3
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: root.app.mt32_rom_path.length > 0
                                    ? root.app.mt32_rom_path
                                    : qsTr("No ROM directory selected")
                                elide: Text.ElideMiddle
                                color: root.app.mt32_rom_path.length > 0
                                    ? root.palette.text
                                    : root.palette.placeholderText
                            }
                            RowLayout {
                                visible: midiEngine.currentIndex === 3
                                Button {
                                    text: qsTr("Choose ROM Folder…")
                                    icon.name: "folder-open"
                                    onClicked: root.app.choose_mt32_rom_folder()
                                }
                                Button {
                                    text: qsTr("Clear")
                                    icon.name: "edit-clear"
                                    enabled: root.app.mt32_rom_path.length > 0
                                    onClicked: root.app.clear_mt32_rom_directory()
                                }
                                Item { Layout.fillWidth: true }
                            }

                            Label {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: qsTr("Munt is built into Kog. Roland control and PCM ROM images must be supplied from hardware you own.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
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
                        text: qsTr("SF2 SoundFonts and OPL3 need no proprietary firmware. Nuked SC-55 is built into Kog but requires ROM images dumped from supported hardware; Kog does not include Roland ROMs.")
                        wrapMode: Text.Wrap
                        color: root.palette.placeholderText
                    }

                    Item { Layout.fillHeight: true }
                }
            }
        }
    }
}
