import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.platform as Platform

// Export the selected playlist rows to audio files: pick a format, a quality
// for lossy formats, whether to keep the equalizer and effects, and a folder.
ApplicationWindow {
    id: root
    required property var app
    property var rows: []
    readonly property var formats: JSON.parse(app.export_formats())
    property int formatIndex: 0
    readonly property var format: formats[formatIndex] || ({})
    property int bitrate: format.bitrate || 0
    property url folder: Platform.StandardPaths.writableLocation(Platform.StandardPaths.MusicLocation)
    property var state: ({ running: false, doneMs: 0, totalMs: 0, message: "" })
    property string error: ""

    title: qsTr("Export Tracks")
    width: 520
    height: 360
    minimumWidth: 420
    minimumHeight: 300

    function openFor(selection) {
        rows = selection
        error = ""
        refresh()
        show()
        raise()
        requestActivate()
    }
    function refresh() { state = JSON.parse(app.export_state()) }

    Timer { interval: 250; repeat: true; running: root.visible && root.state.running; onTriggered: root.refresh() }

    Platform.FolderDialog {
        id: folderDialog
        title: qsTr("Export to folder")
        folder: root.folder
        onAccepted: root.folder = folder
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        Label {
            text: root.rows.length === 1 ? qsTr("Export 1 track") : qsTr("Export %1 tracks").arg(root.rows.length)
            font.bold: true
        }
        GridLayout {
            columns: 2
            columnSpacing: 12
            rowSpacing: 8
            Layout.fillWidth: true
            enabled: !root.state.running

            Label { text: qsTr("Format") }
            ComboBox {
                objectName: "exportFormat"
                Layout.fillWidth: true
                model: root.formats.map(f => f.label)
                currentIndex: root.formatIndex
                onActivated: index => root.formatIndex = index
            }
            Label { text: qsTr("Bitrate"); visible: root.format.lossy === true }
            ComboBox {
                visible: root.format.lossy === true
                Layout.fillWidth: true
                readonly property var choices: [96, 128, 160, 192, 256, 320]
                model: choices.map(kbps => qsTr("%1 kbps").arg(kbps))
                currentIndex: Math.max(0, choices.indexOf(root.bitrate))
                onActivated: index => root.bitrate = choices[index]
            }
            Label { text: qsTr("Folder") }
            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: decodeURIComponent(String(root.folder).replace("file://", ""))
                    elide: Text.ElideMiddle
                }
                Button { text: qsTr("Choose…"); onClicked: folderDialog.open() }
            }
            Item { width: 1; height: 1 }
            CheckBox {
                objectName: "exportEffects"
                id: effectsBox
                text: qsTr("Include the equalizer and effects")
            }
            Item { width: 1; height: 1 }
            RowLayout {
                CheckBox {
                    id: playlistBox
                    objectName: "exportPlaylist"
                    text: qsTr("Also write an M3U playlist named")
                    checked: root.rows.length > 1
                }
                TextField {
                    id: playlistName
                    enabled: playlistBox.checked
                    Layout.fillWidth: true
                    text: qsTr("Kog export")
                }
            }
        }

        ProgressBar {
            Layout.fillWidth: true
            visible: root.state.running
            from: 0
            to: Math.max(1, root.state.totalMs)
            value: root.state.doneMs
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: root.error || root.state.message
            color: root.error ? "#d9534f" : palette.windowText
        }
        Item { Layout.fillHeight: true }
        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            Button {
                text: root.state.running ? qsTr("Cancel export") : qsTr("Close")
                onClicked: root.state.running ? root.app.cancel_export() : root.close()
            }
            Button {
                objectName: "exportStart"
                text: qsTr("Export")
                highlighted: true
                enabled: !root.state.running && root.rows.length > 0
                onClicked: {
                    root.error = root.app.start_export(root.rows.join(","), JSON.stringify({
                        format: root.format.id, bitrate: root.bitrate,
                        applyEffects: effectsBox.checked, folder: String(root.folder),
                        playlist: playlistBox.checked ? playlistName.text : ""
                    }))
                    root.refresh()
                }
            }
        }
    }
}
