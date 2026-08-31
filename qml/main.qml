import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import org.kog.player 1.0

ApplicationWindow {
    id: root

    width: 900
    height: 600
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    title: qsTr("Kog")

    AppController {
        id: appController
    }

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 12

        Label {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Kog")
            font.pixelSize: 42
            font.bold: true
        }

        Label {
            Layout.alignment: Qt.AlignHCenter
            text: appController.status
            color: palette.mid
        }
    }
}
