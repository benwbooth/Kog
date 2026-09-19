pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Layouts
import Qt.labs.platform as Platform

Window {
    id: root

    required property var app

    // Roomy enough for the densest page (Server, with its groups of fields);
    // every page scrolls, so a smaller window still reaches the rest.
    width: 880
    height: 660
    minimumWidth: 680
    minimumHeight: 520
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
    readonly property var filteredFormatGroups: {
        const groups = []
        for (const group of supportedFormatCatalog.groups) {
            const extensions = matchingFormatExtensions(group)
            if (extensions.length > 0) {
                groups.push({
                    name: group.name,
                    detail: group.detail,
                    extensions: extensions
                })
            }
        }
        return groups
    }

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

    function reloadBlacklist() {
        blacklistModel.clear()
        try {
            const parsed = JSON.parse(app.blacklist_json())
            if (parsed && parsed.ok && Array.isArray(parsed.entries)) {
                for (const entry of parsed.entries) {
                    blacklistModel.append({
                        entryId: entry.id,
                        entryKind: entry.kind,
                        entryPath: entry.path,
                        entryMember: entry.entry || ""
                    })
                }
            }
        } catch (e) {}
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
        if (visible) {
            app.refresh_output_devices()
            reloadBlacklist()
        }
    }

    onCurrentPageChanged: {
        if (visible && currentPage === 0)
            reloadBlacklist()
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
                        { title: qsTr("Formats"), iconName: "audio-x-generic" },
                        { title: qsTr("Server"), iconName: "network-server" }
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

                    PreferenceGroup {
                        title: qsTr("Blacklisted from Random Radio")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 6

                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: qsTr("Blacklisted songs and folders never come up in Random Radio. Right-click songs or folders to blacklist them.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: blacklistModel.count === 0
                                text: qsTr("Nothing blacklisted.")
                                color: root.palette.placeholderText
                            }
                            ListView {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.min(contentHeight, 220)
                                visible: blacklistModel.count > 0
                                clip: true
                                model: ListModel { id: blacklistModel }
                                delegate: RowLayout {
                                    required property int entryId
                                    required property string entryKind
                                    required property string entryPath
                                    required property string entryMember
                                    width: ListView.view.width
                                    spacing: 8

                                    Label {
                                        Layout.fillWidth: true
                                        text: entryKind === "folder"
                                            ? qsTr("Folder: %1").arg(entryPath)
                                            : (entryMember.length > 0
                                                ? qsTr("Song: %1 :: %2").arg(entryPath).arg(entryMember)
                                                : qsTr("Song: %1").arg(entryPath))
                                        color: root.foregroundColor
                                        elide: Text.ElideMiddle
                                    }
                                    Button {
                                        text: qsTr("Remove")
                                        onClicked: {
                                            root.app.remove_blacklist_entry(entryId)
                                            root.reloadBlacklist()
                                        }
                                    }
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

                    PreferenceGroup {
                        title: qsTr("System tray")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 6

                            PreferenceCheckBox {
                                text: qsTr("Show Kog in the system tray")
                                checked: root.app.show_tray_icon
                                onToggled: root.app.update_show_tray_icon(checked)
                            }
                            PreferenceCheckBox {
                                text: qsTr("Close the main window to the tray")
                                enabled: root.app.show_tray_icon
                                checked: root.app.close_to_tray
                                onToggled: root.app.update_close_to_tray(checked)
                            }
                            PreferenceCheckBox {
                                text: qsTr("Minimize the main window to the tray")
                                enabled: root.app.show_tray_icon
                                checked: root.app.minimize_to_tray
                                onToggled: root.app.update_minimize_to_tray(checked)
                            }
                            PreferenceCheckBox {
                                text: qsTr("Show a notification when a new track starts")
                                checked: root.app.track_notifications
                                onToggled: root.app.update_track_notifications(checked)
                            }
                            PreferenceCheckBox {
                                text: qsTr("Automatically download missing album covers")
                                checked: root.app.download_cover_art
                                onToggled: root.app.update_download_cover_art(checked)
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: qsTr("Covers come from embedded tags first, then Deezer, iTunes, MusicBrainz, and DuckDuckGo, and are cached on disk.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                            text: qsTr("A compact now-playing popup includes Previous, Play/Pause, Stop, and Next controls. Hover to keep it open; it otherwise dismisses after eight seconds. Drag its header to move it; your position is remembered. Right-click the header to reset it above the tray.")
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
                                    text: qsTr("Import Archive…")
                                    icon.name: "archive-insert"
                                    onClicked: root.app.choose_sc55_rom_archive()
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
                                    text: qsTr("Import Archive…")
                                    icon.name: "archive-insert"
                                    onClicked: root.app.choose_mt32_rom_archive()
                                }
                                Button {
                                    text: qsTr("Clear")
                                    icon.name: "edit-clear"
                                    enabled: root.app.mt32_rom_path.length > 0
                                    onClicked: root.app.clear_mt32_rom_directory()
                                }
                                Item { Layout.fillWidth: true }
                            }

                            PreferenceCheckBox {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: qsTr("Map General MIDI programs to MT-32 patches")
                                checked: root.app.mt32_gm_program_mapping
                                onToggled: root.app.update_mt32_gm_program_mapping(checked)
                            }

                            PreferenceLabel {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: qsTr("Enabled by default for ordinary General MIDI files. Turn this off for music authored specifically for native MT-32 program numbers and custom timbres.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }

                            PreferenceLabel {
                                Layout.fillWidth: true
                                visible: midiEngine.currentIndex === 3
                                text: qsTr("Munt is built into Kog. Select a folder or import a ZIP, 7Z, RAR, TAR, or compressed ROM archive. ROMs are recognized by content, so filenames do not matter.")
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
                        text: qsTr("SF2 SoundFonts and OPL3 need no proprietary firmware. Nuked SC-55 is built into Kog but requires a supported user-supplied ROM set. Imported archives are unpacked safely into Kog's private data directory; Kog does not include Roland ROMs.")
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
                                model: root.filteredFormatGroups
                                boundsBehavior: Flickable.StopAtBounds
                                readonly property real scrollGutter:
                                    formatScrollBar.visible
                                        ? formatScrollBar.implicitWidth + 4 : 0

                                delegate: PreferenceGroup {
                                    id: formatGroup

                                    required property var modelData

                                    width: Math.max(0,
                                        formatList.width - formatList.scrollGutter)
                                    title: modelData.name + "  ·  "
                                        + qsTr("%1 extensions").arg(modelData.extensions.length)

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
                                            text: formatGroup.modelData.extensions
                                                .map(extension => "." + extension).join("  ")
                                            wrapMode: Text.Wrap
                                            textFormat: Text.PlainText
                                        }
                                    }
                                }

                                ScrollBar.vertical: ScrollBar {
                                    id: formatScrollBar
                                    policy: ScrollBar.AsNeeded
                                }
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
            ScrollView {
                clip: true
                contentWidth: availableWidth
                ScrollBar.vertical.policy: ScrollBar.AsNeeded
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                // Copies tokens and addresses to the system clipboard.
                TextEdit {
                    id: clipboardHelper
                    visible: false
                    width: 0
                    height: 0
                }

                // One dialog serves both halves of the pair: the key is
                // requested immediately after the certificate.
                Platform.FileDialog {
                    id: certificateDialog
                    title: qsTr("Choose a certificate (PEM)")
                    nameFilters: [qsTr("Certificates (*.pem *.crt *.cer)"), qsTr("All files (*)")]
                    onAccepted: {
                        chosenCertificate = file.toString()
                        keyDialog.open()
                    }
                }
                Platform.FileDialog {
                    id: keyDialog
                    title: qsTr("Choose the matching private key (PEM)")
                    nameFilters: [qsTr("Private keys (*.pem *.key)"), qsTr("All files (*)")]
                    onAccepted: {
                        let result = null
                        try {
                            result = JSON.parse(root.app.import_server_certificate(
                                chosenCertificate, file.toString()))
                        } catch (error) {
                            result = null
                        }
                        serverState.detail = result && result.ok
                            ? qsTr("Certificate imported")
                            : qsTr("Could not import that certificate pair")
                        serverState.load()
                    }
                }

                property string chosenCertificate: ""

                onVisibleChanged: if (visible && serverState.loaded === false)
                    serverState.load()

                ColumnLayout {
                    x: 22
                    width: parent.width - 44
                    spacing: 18

                    PreferenceLabel {
                        text: qsTr("Server")
                        font.pixelSize: 22
                        font.bold: true
                    }

                    QtObject {
                        id: serverState
                        property bool loaded: false
                        property bool enabled: false
                        property string address: "127.0.0.1"
                        property int port: 8420
                        property string auth: "token"
                        property string token: ""
                        property string username: ""
                        property string password: ""
                        property bool hasPassword: false
                        property string tls: "off"
                        property string certificatePath: ""
                        property string codec: "aac"
                        property int cacheMegabytes: 2048
                        property var problems: []
                        property var status: null
                        property string detail: ""

                        function load() {
                            let payload = null
                            try {
                                payload = JSON.parse(root.app.server_settings_json())
                            } catch (error) {
                                payload = null
                            }
                            if (!payload)
                                return
                            enabled = payload.enabled
                            address = payload.address
                            port = payload.port
                            auth = payload.auth
                            token = payload.token
                            username = payload.username
                            hasPassword = payload.hasPassword
                            password = ""
                            tls = payload.tls
                            certificatePath = payload.certificatePath
                            codec = payload.defaultCodec
                            cacheMegabytes = Math.round((payload.cacheBytes || 0) / (1024 * 1024))
                            problems = payload.problems || []
                            status = payload.status || null
                            loaded = true
                        }

                        function payload() {
                            const body = {
                                "enabled": enabled,
                                "address": address,
                                "port": port,
                                "auth": auth,
                                "token": token,
                                "username": username,
                                "tls": tls,
                                "defaultCodec": codec,
                                "cacheBytes": cacheMegabytes * 1024 * 1024
                            }
                            if (password.length > 0)
                                body["password"] = password
                            return JSON.stringify(body)
                        }

                        function save() {
                            let result = null
                            try {
                                result = JSON.parse(root.app.save_server_settings(payload()))
                            } catch (error) {
                                result = null
                            }
                            password = ""
                            load()
                            detail = result && result.ok
                                ? qsTr("Saved") : qsTr("Could not save the settings")
                        }

                        // The run switch is persistent: it saves the setting
                        // and starts or stops the server to match, so there is
                        // no separate start/stop step to forget.
                        function applyEnabled() {
                            save()
                            const running = !!(status && status.running)
                            if (enabled && !running) {
                                start()
                            } else if (!enabled && running) {
                                root.app.stop_api_server()
                                detail = qsTr("Server stopped")
                                load()
                            }
                        }

                        function start() {
                            let result = null
                            try {
                                result = JSON.parse(root.app.start_api_server())
                            } catch (error) {
                                result = null
                            }
                            if (result && result.ok) {
                                detail = qsTr("Serving at %1").arg(result.url || "")
                            } else if (result && result.error) {
                                detail = result.error
                            } else {
                                detail = qsTr("Could not start the server")
                            }
                            load()
                        }
                    }

                    PreferenceGroup {
                        title: qsTr("Web API and streaming")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: qsTr("Serve your library to other devices. Streams are transcoded per client, so each listener gets their own copy. The server binds to loopback unless you choose otherwise.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                            PreferenceCheckBox {
                                text: qsTr("Run the API server")
                                checked: serverState.enabled
                                onToggled: {
                                    serverState.enabled = checked
                                    serverState.applyEnabled()
                                }
                            }

                            GridLayout {
                                Layout.fillWidth: true
                                columns: 4
                                columnSpacing: 8

                                PreferenceLabel { text: qsTr("Address") }
                                TextField {
                                    Layout.preferredWidth: 160
                                    text: serverState.address
                                    selectByMouse: true
                                    onTextChanged: serverState.address = text.trim()
                                }
                                PreferenceLabel { text: qsTr("Port") }
                                SpinBox {
                                    from: 1
                                    to: 65535
                                    value: serverState.port
                                    editable: true
                                    onValueModified: serverState.port = value
                                }
                            }
                        }
                    }

                    PreferenceGroup {
                        title: qsTr("Authentication")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Require") }
                                ComboBox {
                                    Layout.fillWidth: true
                                    model: [qsTr("An API token"), qsTr("Username and password"), qsTr("Nothing (loopback only)")]
                                    currentIndex: serverState.auth === "basic" ? 1
                                        : (serverState.auth === "none" ? 2 : 0)
                                    onActivated: serverState.auth =
                                        ["token", "basic", "none"][currentIndex]
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                visible: serverState.auth === "token"
                                TextField {
                                    Layout.fillWidth: true
                                    placeholderText: qsTr("API token")
                                    text: serverState.token
                                    selectByMouse: true
                                    onTextChanged: serverState.token = text
                                }
                                Button {
                                    text: qsTr("Generate")
                                    onClicked: {
                                        let result = null
                                        try {
                                            result = JSON.parse(root.app.generate_api_token())
                                        } catch (error) {
                                            result = null
                                        }
                                        if (result && result.ok)
                                            serverState.token = result.token
                                    }
                                }
                                Button {
                                    text: qsTr("Copy")
                                    enabled: serverState.token.length > 0
                                    onClicked: {
                                        clipboardHelper.text = serverState.token
                                        clipboardHelper.selectAll()
                                        clipboardHelper.copy()
                                        serverState.detail = qsTr("Token copied")
                                    }
                                }
                            }
                            GridLayout {
                                Layout.fillWidth: true
                                visible: serverState.auth === "basic"
                                columns: 2
                                columnSpacing: 8
                                PreferenceLabel { text: qsTr("Username") }
                                TextField {
                                    Layout.fillWidth: true
                                    text: serverState.username
                                    selectByMouse: true
                                    onTextChanged: serverState.username = text.trim()
                                }
                                PreferenceLabel { text: qsTr("Password") }
                                TextField {
                                    Layout.fillWidth: true
                                    placeholderText: serverState.hasPassword
                                        ? qsTr("Leave blank to keep the current password")
                                        : qsTr("Set a password")
                                    echoMode: TextInput.Password
                                    selectByMouse: true
                                    onTextChanged: serverState.password = text
                                }
                            }
                        }
                    }

                    PreferenceGroup {
                        title: qsTr("Encryption and codec")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("HTTPS") }
                                ComboBox {
                                    Layout.fillWidth: true
                                    model: [qsTr("Off (plain HTTP)"), qsTr("Self-signed certificate"),
                                        qsTr("My own certificate")]
                                    currentIndex: serverState.tls === "selfSigned" ? 1
                                        : (serverState.tls === "pem" ? 2 : 0)
                                    onActivated: serverState.tls =
                                        ["off", "selfSigned", "pem"][currentIndex]
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                visible: serverState.tls === "pem"
                                Button {
                                    text: qsTr("Choose certificate…")
                                    onClicked: certificateDialog.open()
                                }
                                PreferenceLabel {
                                    Layout.fillWidth: true
                                    text: serverState.certificatePath.length > 0
                                        ? serverState.certificatePath : qsTr("No certificate chosen")
                                    elide: Text.ElideMiddle
                                    color: root.palette.placeholderText
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Stream format") }
                                ComboBox {
                                    Layout.fillWidth: true
                                    model: ["aac", "opus", "flac"]
                                    currentIndex: Math.max(0, model.indexOf(serverState.codec))
                                    onActivated: serverState.codec = model[currentIndex]
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                PreferenceLabel { text: qsTr("Stream cache (MB)") }
                                SpinBox {
                                    from: 0
                                    to: 102400
                                    stepSize: 256
                                    value: serverState.cacheMegabytes
                                    editable: true
                                    onValueModified: serverState.cacheMegabytes = value
                                }
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: qsTr("AAC plays everywhere. Opus is smaller but not supported by every iOS browser. FLAC is lossless and best on a home network.")
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                        }
                    }

                    PreferenceGroup {
                        title: qsTr("Status")
                        Layout.fillWidth: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            PreferenceLabel {
                                Layout.fillWidth: true
                                visible: serverState.problems.length > 0
                                text: serverState.problems.join("\n")
                                wrapMode: Text.Wrap
                                color: "#e05c5c"
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: serverState.status && serverState.status.running
                                    ? qsTr("Running at %1").arg(serverState.status.url)
                                    : qsTr("Not running")
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                visible: serverState.status && serverState.status.running
                                    && serverState.status.certificatePath
                                text: serverState.status && serverState.status.certificatePath
                                    ? qsTr("Certificate: %1").arg(serverState.status.certificatePath) : ""
                                elide: Text.ElideMiddle
                                color: root.palette.placeholderText
                            }
                            RowLayout {
                                Button {
                                    text: qsTr("Save Settings")
                                    onClicked: serverState.save()
                                }
                                Button {
                                    text: qsTr("Copy Address")
                                    onClicked: {
                                        const payload = JSON.parse(root.app.server_addresses_json())
                                        if (payload.addresses && payload.addresses.length > 0) {
                                            clipboardHelper.text = payload.addresses[0]
                                            clipboardHelper.selectAll()
                                            clipboardHelper.copy()
                                            serverState.detail = qsTr("Address copied: %1")
                                                .arg(payload.addresses[0])
                                        }
                                    }
                                }
                            }
                            PreferenceLabel {
                                Layout.fillWidth: true
                                text: serverState.detail
                                wrapMode: Text.Wrap
                                color: root.palette.placeholderText
                            }
                        }
                    }

                    Item { Layout.fillHeight: true }
                }
            }

        }
    }
}
