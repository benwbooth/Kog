import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

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
    // Kog's own icons, drawn in one consistent line style as in the Earshot
    // look. Only names Kog has no art for fall back to the desktop's theme.
    readonly property var ownIcons: [
        "application-menu", "audio-volume-high", "audio-x-generic", "edit-clear-list",
        "edit-find", "folder", "folder-open", "gear", "go-up", "media-playback-pause",
        "media-playback-start", "media-playback-stop", "media-playlist-random",
        "media-playlist-repeat", "media-playlist-shuffle", "media-skip-backward",
        "media-skip-forward", "view-list-tree", "view-refresh", "view-restore",
        "window-close"
    ]
    readonly property bool ownIcon: ownIcons.indexOf(iconName) >= 0
    icon.name: forceLightIcon || ownIcon ? "" : iconName
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

    // Flat, softly rounded buttons as in Earshot. The desktop style paints a
    // button's icon as part of its background, so the icon is drawn here too.
    background: Rectangle {
        radius: 8
        color: Qt.rgba(control.palette.buttonText.r, control.palette.buttonText.g,
            control.palette.buttonText.b,
            control.down || control.checked ? 0.14 : (control.hovered && control.enabled ? 0.08 : 0))
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    contentItem: Item {
        implicitWidth: control.icon.width
        implicitHeight: control.icon.height
        opacity: control.enabled ? 1 : 0.38

        ControlsImpl.IconImage {
            anchors.centerIn: parent
            visible: control.iconName.length > 0
            width: control.icon.width
            height: control.icon.height
            sourceSize: Qt.size(control.icon.width, control.icon.height)
            name: control.icon.name
            source: control.icon.name.length > 0 ? "" : control.icon.source
            color: control.icon.name.length > 0 ? control.palette.buttonText : "transparent"
        }
        Label {
            anchors.centerIn: parent
            visible: control.iconName.length === 0
            text: control.glyph
            font.pixelSize: control.font.pixelSize
            color: control.palette.buttonText
        }
    }

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
