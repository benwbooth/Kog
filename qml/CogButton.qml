import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    property string glyph: ""
    property string iconName: ""
    property string toolTip: ""
    property string badgeText: ""
    property bool modeActive: false
    property color iconBackground: palette.window
    property bool forceLightIcon: false
    readonly property bool useLightIcon: (0.2126 * iconBackground.r
        + 0.7152 * iconBackground.g
        + 0.0722 * iconBackground.b) < 0.5

    implicitWidth: 38
    implicitHeight: 38
    text: glyph
    icon.name: forceLightIcon ? "" : iconName
    icon.source: iconName.length > 0
        ? Qt.resolvedUrl("icons/" + iconName
            + (forceLightIcon || useLightIcon ? "-light" : "") + ".svg")
        : ""
    icon.color: "transparent"
    icon.width: 20
    icon.height: 20
    display: iconName.length > 0 ? AbstractButton.IconOnly : AbstractButton.TextOnly
    font.pixelSize: 18
    hoverEnabled: true
    Accessible.name: toolTip

    ToolTip.visible: hovered && toolTip.length > 0
    ToolTip.text: toolTip
    ToolTip.delay: 450

    Label {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: 2
        anchors.bottomMargin: 1
        visible: control.badgeText.length > 0
        text: control.badgeText
        color: control.palette.highlightedText
        font.pixelSize: 8
        font.bold: true
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter

        background: Rectangle {
            radius: 5
            color: control.palette.highlight
            border.width: 1
            border.color: control.palette.window
        }
        leftPadding: 3
        rightPadding: 3
        topPadding: 1
        bottomPadding: 1
    }

}
