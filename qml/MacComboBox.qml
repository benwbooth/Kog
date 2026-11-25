import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Shapes

// A pop-up button, macOS style (shared by Ben's Qt Quick apps; see
// MacMenuStyle.qml): a rounded button with the choice and a ⌃⌄ mark, which
// opens a glass list with the current choice over the button. Apps with
// their own button look keep it and set only
//     delegate: MacComboItem { combo: box }; popup: MacComboPopup { combo: box }
ComboBox {
    id: box
    property alias dark: pop.dark
    property alias accent: pop.accent
    readonly property MacMenuStyle st: pop.macStyle
    implicitHeight: Math.round(22 * st.pt)
    implicitWidth: Math.max(Math.round(90 * st.pt), implicitContentWidth + leftPadding + rightPadding)
    leftPadding: Math.round(9 * st.pt)
    rightPadding: Math.round(24 * st.pt)
    font.family: st.family
    font.pixelSize: st.fontSize
    delegate: MacComboItem { combo: box }
    popup: MacComboPopup { id: pop; combo: box }
    background: Rectangle {
        radius: Math.round(6 * box.st.pt)
        color: box.st.dark ? Qt.rgba(1, 1, 1, box.pressed ? 0.20 : box.hovered ? 0.16 : 0.12)
                           : (box.pressed ? Qt.rgba(0, 0, 0, 0.06) : "#ffffff")
        border.width: 1
        border.color: box.st.dark ? Qt.rgba(1, 1, 1, 0.08) : Qt.rgba(0, 0, 0, 0.14)
        opacity: box.enabled ? 1 : 0.5
    }
    contentItem: Text {
        text: box.displayText
        color: box.enabled ? box.st.fg : box.st.fgDisabled
        font.family: box.st.family
        font.pixelSize: box.st.fontSize
        font.letterSpacing: box.st.letterSpacing
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        textFormat: Text.PlainText
        renderType: Text.NativeRendering
        transform: Translate { x: 0.001; y: 0.001 }
    }
    // ⌃⌄ in an accent square at the right (macOS pop-up button).
    indicator: Rectangle {
        x: box.width - width - 3
        y: (box.height - height) / 2
        width: Math.round(16 * box.st.pt)
        height: box.height - 6
        radius: Math.round(4 * box.st.pt)
        color: box.st.accent
        opacity: box.enabled ? 1 : 0.5
        Shape {
            anchors.centerIn: parent
            width: 7; height: 11
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: "#ffffff"; strokeWidth: 1.5; fillColor: "transparent"
                capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin
                startX: 0.8; startY: 4
                PathLine { x: 3.5; y: 1.2 }
                PathLine { x: 6.2; y: 4 }
            }
            ShapePath {
                strokeColor: "#ffffff"; strokeWidth: 1.5; fillColor: "transparent"
                capStyle: ShapePath.RoundCap; joinStyle: ShapePath.RoundJoin
                startX: 0.8; startY: 7
                PathLine { x: 3.5; y: 9.8 }
                PathLine { x: 6.2; y: 7 }
            }
        }
    }
}
