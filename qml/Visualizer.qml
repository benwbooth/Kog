import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.settings

ApplicationWindow {
    id: root
    required property var app
    title: qsTr("Kog — Visualizer")
    width: 760
    height: 480
    minimumWidth: 440
    minimumHeight: 300
    color: "#10191f"
    property string settingsFile: ""
    Settings {
        category: "Visualizer"
        fileName: root.settingsFile
        property alias mode: mode.currentIndex
    }
    Shortcut { sequence: "Escape"; onActivated: root.visibility === Window.FullScreen ? root.showNormal() : root.hide() }
    Shortcut { sequence: "F11"; onActivated: root.visibility === Window.FullScreen ? root.showNormal() : root.showFullScreen() }
    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            Label { text: qsTr("Visualizations"); font.bold: true; Layout.fillWidth: true }
            ComboBox { id: mode; objectName: "visualizerMode"; model: [qsTr("Spectrum"), qsTr("Oscilloscope")] }
            ToolButton {
                text: qsTr("Full screen")
                icon.name: "view-fullscreen"
                onClicked: root.visibility === Window.FullScreen ? root.showNormal() : root.showFullScreen()
            }
        }
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        AudioVisualization {
            id: visualization
            objectName: "audioVisualization"
            app: root.app
            active: root.visible && root.visibility !== Window.Minimized
            waveform: mode.currentIndex === 1
            Layout.fillWidth: true
            Layout.fillHeight: true
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 20
            ColumnLayout {
                Layout.fillWidth: true
                Label { text: root.app.now_title; textFormat: Text.PlainText; color: "#e6f0f5"; font.pixelSize: 18; elide: Text.ElideRight; Layout.fillWidth: true }
                Label { text: root.app.now_artist || qsTr("Live audio • 40-band spectrum / PCM oscilloscope"); textFormat: Text.PlainText; color: "#98afb9"; elide: Text.ElideRight; Layout.fillWidth: true }
            }
            Button { text: root.app.playback_state === "playing" ? qsTr("Pause") : qsTr("Play"); onClicked: root.app.play_pause() }
            Button { text: qsTr("Next"); onClicked: root.app.next() }
        }
    }
}
