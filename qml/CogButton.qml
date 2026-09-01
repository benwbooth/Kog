import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    property string glyph: ""
    property string iconName: ""
    property string toolTip: ""
    readonly property color iconBackground: ApplicationWindow.window
        ? ApplicationWindow.window.palette.window
        : palette.window
    readonly property bool useLightIcon: (0.2126 * iconBackground.r
        + 0.7152 * iconBackground.g
        + 0.0722 * iconBackground.b) < 0.5

    implicitWidth: 38
    implicitHeight: 38
    text: glyph
    icon.source: iconName.length > 0
        ? Qt.resolvedUrl("icons/" + iconName + (useLightIcon ? "-light" : "") + ".svg")
        : ""
    icon.color: "transparent"
    icon.width: 20
    icon.height: 20
    display: iconName.length > 0 ? AbstractButton.IconOnly : AbstractButton.TextOnly
    font.pixelSize: 18
    hoverEnabled: true

    ToolTip.visible: hovered && toolTip.length > 0
    ToolTip.text: toolTip
    ToolTip.delay: 450

}
