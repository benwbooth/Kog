import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    property string glyph: ""
    property string toolTip: ""

    implicitWidth: 38
    implicitHeight: 38
    text: glyph
    font.pixelSize: 19
    hoverEnabled: true

    ToolTip.visible: hovered && toolTip.length > 0
    ToolTip.text: toolTip
    ToolTip.delay: 450

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.enabled ? (control.down ? "#222222" : "#555555") : "#aaaaaa"
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        radius: 6
        color: control.down ? "#d5d5d5" : (control.hovered ? "#e9e9e9" : "transparent")
        border.color: control.activeFocus ? "#6aa9e9" : "transparent"
    }
}
