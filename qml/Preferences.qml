pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
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
    readonly property real windowLuminance: 0.2126 * palette.window.r
        + 0.7152 * palette.window.g
        + 0.0722 * palette.window.b
    readonly property color foregroundColor: windowLuminance < 0.5
        ? Qt.lighter(palette.placeholderText, 1.35)
        : Qt.darker(palette.text, 1.2)
    palette.windowText: foregroundColor
    palette.buttonText: foregroundColor

    property int currentPage: 0
    readonly property var outputDevices: JSON.parse(app.output_devices_json)
    readonly property var supportedFormatCatalog: JSON.parse(app.supported_formats_json)
    property string formatSearchText: ""

    component PreferenceLabel: Label {
        color: root.foregroundColor
    }

    component PreferenceGroup: GroupBox {
        id: preferenceGroup

        palette.windowText: root.foregroundColor
        palette.buttonText: root.foregroundColor
        label: PreferenceLabel {
            x: preferenceGroup.leftPadding
            width: preferenceGroup.availableWidth
            text: preferenceGroup.title
            elide: Text.ElideRight
        }
    }

    component PreferenceCheckBox: CheckBox {
        id: preferenceCheckBox

        contentItem: PreferenceLabel {
            leftPadding: preferenceCheckBox.indicator.width + preferenceCheckBox.spacing
            text: preferenceCheckBox.text
            font: preferenceCheckBox.font
            verticalAlignment: Text.AlignVCenter
        }
    }

    function matchingFormatExtensions(group) {
        const query = formatSearchText.trim().toLowerCase().replace(/^\./, "")
        if (query.length === 0
                || group.name.toLowerCase().includes(query)
                || group.detail.toLowerCase().includes(query))
            return group.extensions
        return group.extensions.filter(extension =>
            extension.toLowerCase().includes(query))
    }

    function outputDeviceIndex(id) {
        if (id.length === 0)
            return 0
        for (let index = 0; index < outputDevices.length; ++index) {
            if (outputDevices[index].id === id)
                return index + 1
        }
        return 0
    }

    onVisibleChanged: {
        if (visible)
            app.refresh_output_devices()
    }

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

                PreferenceLabel {
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
                        { title: qsTr("Synthesis"), iconName: "audio-midi" },
                        { title: qsTr("Formats"), iconName: "audio-x-generic" }
                    ]

                    ItemDelegate {
                        id: navigationDelegate

                        required property int index
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.title
                        icon.name: modelData.iconName
                        palette.text: root.foregroundColor
                        palette.windowText: root.foregroundColor
                        palette.buttonText: root.foregroundColor
                        highlighted: root.currentPage === index
                        contentItem: ControlsImpl.IconLabel {
                            spacing: navigationDelegate.spacing
                            mirrored: navigationDelegate.mirrored
                            display: navigationDelegate.display
                            alignment: Qt.AlignLeft | Qt.AlignVCenter
                            icon: navigationDelegate.icon
                            text: navigationDelegate.text
                            font: navigationDelegate.font
                            color: navigationDelegate.highlighted
                                ? navigationDelegate.palette.highlightedText
                                : root.foregroundColor
                        }
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

                    PreferenceLabel {
                        text: qsTr("Playlist")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    PreferenceGroup {
                        title: qsTr("When opening files")
                        Layout.fillWidth: true

                        RowLayout {
                            anchors.fill: parent
                            PreferenceLabel { text: qsTr("Normally:") }
                            ComboBox {
                                Layout.fillWidth: true
                                model: [
                                    qsTr("Clear playlist and play"),
                                    qsTr("Enqueue"),
                                    qsTr("Enqueue and play")
                                ]
                                currentIndex: root.app.opening_files_behavior === "clearAndPlay"
                                    ? 0
                                    : (root.app.opening_files_behavior === "enqueue" ? 1 : 2)
                                onActivated: root.app.select_opening_files_behavior(
                                    ["clearAndPlay", "enqueue", "enqueueAndPlay"][currentIndex])
                            }
                        }
                    }

                    PreferenceGroup {
                        title: qsTr("When adding folders")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            PreferenceCheckBox {
                                text: qsTr("Read CUE sheets")
                                checked: root.app.read_cue_sheets_in_folders
                                onToggled: root.app.set_folder_cue_mode(checked)
                            }
                            PreferenceCheckBox {
                                text: qsTr("Read M3U and PLS playlist files")
                                checked: root.app.read_playlists_in_folders
                                onToggled: root.app.set_folder_playlist_mode(checked)
                            }
                            PreferenceLabel {
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

                    PreferenceLabel {
                        text: qsTr("Output")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    PreferenceGroup {
                        title: qsTr("Audio output")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 10
                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Device:") }
                                ComboBox {
                                    id: outputDeviceSelector

                                    Layout.fillWidth: true
                                    model: [{
                                        id: "",
                                        label: qsTr("System Default Device")
                                    }].concat(root.outputDevices)
                                    textRole: "label"
                                    valueRole: "id"
                                    currentIndex: root.outputDeviceIndex(
                                        root.app.output_device_id)
                                    onActivated: index => root.app.select_output_device(
                                        index === 0
                                            ? ""
                                            : root.outputDevices[index - 1].id)
                                    Accessible.name: qsTr("Audio output device")
                                }
                                Button {
                                    text: qsTr("Refresh")
                                    icon.name: "view-refresh"
                                    onClicked: root.app.refresh_output_devices()
                                }
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: root.app.output_device_status
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Volume:") }
                                Slider {
                                    Layout.fillWidth: true
                                    from: 0
                                    to: 1
                                    value: root.app.volume
                                    onMoved: root.app.set_volume_level(value)
                                }
                                PreferenceLabel {
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

                    PreferenceLabel {
                        text: qsTr("General")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    PreferenceGroup {
                        title: qsTr("Music folder")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 10
                            PreferenceLabel {
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

                    PreferenceGroup {
                        title: qsTr("Appearance")
                        Layout.fillWidth: true

                        PreferenceLabel {
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

                    PreferenceLabel {
                        text: qsTr("Synthesis")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    PreferenceGroup {
                        title: qsTr("MIDI synthesis")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Backend:") }
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

                            PreferenceLabel {
                                text: qsTr("SoundFont:")
                                font.bold: true
                                visible: midiEngine.currentIndex === 0
                            }
                            PreferenceLabel {
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

                            PreferenceLabel {
                                text: qsTr("Roland SC-55 ROM directory:")
                                font.bold: true
                                visible: midiEngine.currentIndex === 2
                            }
                            PreferenceLabel {
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

                            PreferenceLabel {
                                text: qsTr("MT-32 / CM-32L ROM directory:")
                                font.bold: true
                                visible: midiEngine.currentIndex === 3
                            }
                            PreferenceLabel {
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

                            PreferenceLabel {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: qsTr("Munt is built into Kog. Roland control and PCM ROM images must be supplied from hardware you own.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }

                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: root.app.midi_status
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                        }
                    }

                    PreferenceLabel {
                        Layout.fillWidth: true
                        text: qsTr("SF2 SoundFonts and OPL3 need no proprietary firmware. Nuked SC-55 is built into Kog but requires ROM images dumped from supported hardware; Kog does not include Roland ROMs.")
                        wrapMode: Text.Wrap
                        color: root.palette.placeholderText
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            Loader {
                Layout.fillWidth: true
                Layout.fillHeight: true
                active: root.currentPage === 4

                sourceComponent: Component {
                    Item {
                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 22
                            spacing: 12

                            PreferenceLabel {
                                text: qsTr("Supported Formats")
                                font.pixelSize: 22
                                font.bold: true
                            }

                            Frame {
                                Layout.fillWidth: true
                                padding: 14

                                background: Rectangle {
                                    radius: 8
                                    color: root.palette.alternateBase
                                    border.color: root.palette.mid
                                }

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 4

                                    PreferenceLabel {
                                        text: qsTr("%1 recognized file extensions")
                                            .arg(root.supportedFormatCatalog.uniqueExtensionCount)
                                        font.pixelSize: 17
                                        font.bold: true
                                    }
                                    PreferenceLabel {
                                        Layout.fillWidth: true
                                        text: qsTr("This list comes from the decoders bundled into this build of Kog. HTTP and HTTPS audio streams are supported too.")
                                        wrapMode: Text.Wrap
                                        color: root.palette.placeholderText
                                    }
                                }
                            }

                            TextField {
                                Layout.fillWidth: true
                                placeholderText: qsTr("Search extensions or decoder names")
                                text: root.formatSearchText
                                selectByMouse: true
                                onTextChanged: root.formatSearchText = text
                                Accessible.name: qsTr("Search supported formats")
                            }

                            ListView {
                                id: formatList

                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 8
                                model: root.supportedFormatCatalog.groups
                                boundsBehavior: Flickable.StopAtBounds

                                delegate: PreferenceGroup {
                                    id: formatGroup

                                    required property var modelData
                                    readonly property var matchingExtensions:
                                        root.matchingFormatExtensions(modelData)

                                    width: formatList.width
                                    visible: matchingExtensions.length > 0
                                    height: visible ? implicitHeight : 0
                                    title: modelData.name + "  ·  "
                                        + qsTr("%1 extensions").arg(matchingExtensions.length)

                                    ColumnLayout {
                                        anchors.fill: parent
                                        spacing: 7

                                        PreferenceLabel {
                                            Layout.fillWidth: true
                                            visible: formatGroup.modelData.detail.length > 0
                                            text: formatGroup.modelData.detail
                                            color: root.palette.placeholderText
                                            font.pixelSize: 11
                                            wrapMode: Text.Wrap
                                        }
                                        PreferenceLabel {
                                            Layout.fillWidth: true
                                            text: formatGroup.matchingExtensions
                                                .map(extension => "." + extension).join("  ")
                                            wrapMode: Text.Wrap
                                            textFormat: Text.PlainText
                                        }
                                    }
                                }

                                ScrollBar.vertical: ScrollBar {}
                            }

                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: qsTr("Kog validates each file through its decoder, including companion files and subsongs where supported.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                                font.pixelSize: 11
                            }
                        }
                    }
                }
            }
        }
    }
}
