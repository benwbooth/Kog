import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root
    objectName: "aboutKogWindow"
    title: qsTr("About Kog")
    width: 400
    height: 280
    minimumWidth: 340
    minimumHeight: 260
    color: palette.window
    readonly property string version: Qt.application.version

    function open() {
        show()
        raise()
        requestActivate()
    }
    Shortcut { sequence: "Escape"; enabled: root.visible; onActivated: root.hide() }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 12
        Label {
            text: qsTr("Kog")
            font.pixelSize: 28
            font.bold: true
            Layout.alignment: Qt.AlignHCenter
        }
        Label {
            objectName: "aboutVersion"
            text: qsTr("Version %1").arg(root.version)
            horizontalAlignment: Text.AlignHCenter
            Accessible.name: text
            Layout.fillWidth: true
        }
        Label {
            text: qsTr("Music, chiptunes, and game audio.\nFree software licensed under GPL-3.0-or-later.")
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
        Item { Layout.fillHeight: true }
        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            Button {
                text: qsTr("Project Website")
                onClicked: Qt.openUrlExternally("https://github.com/benwbooth/Kog")
            }
            Button { text: qsTr("Close"); onClicked: root.hide() }
        }
    }
}
