import QtQuick

// What a channel without musical keys (noise, samples) shows in place of the
// keyboard: one pad per noise rate or per sample, lit while it sounds.
Item {
    id: root
    property var pads: []
    property color accent: "#50c8ef"
    implicitWidth: 1520
    implicitHeight: 64
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Sounding: %1").arg(pads.filter(p => p.level > 0).map(p => p.label).join(", ") || qsTr("none"))

    readonly property real padWidth: Math.min(88, (width - 4 * Math.max(0, pads.length - 1)) / Math.max(1, pads.length))

    Row {
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            model: root.pads.length
            Rectangle {
                required property int index
                readonly property var pad: root.pads[index] || ({label: "", level: 0})
                objectName: "channelPad"
                width: root.padWidth
                height: Math.min(root.height, 44)
                radius: 5
                color: pad.level > 0
                    ? Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.35 + 0.65 * Math.min(1, pad.level))
                    : "#22323c"
                border.width: 1
                border.color: pad.level > 0 ? root.accent : "#33434d"
                Text {
                    anchors.centerIn: parent
                    width: parent.width - 6
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: parent.pad.label
                    color: parent.pad.level > 0 ? "#0c141b" : "#91aab8"
                    font.pixelSize: 11
                    font.family: "monospace"
                }
            }
        }
    }
}
