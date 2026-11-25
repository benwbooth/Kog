import QtQuick
import QtQuick.Controls.Basic

// A row of a MacComboPopup: a MacMenuItem's look, with a checkmark on the
// current choice (shared by Ben's Qt Quick apps; see MacMenuStyle.qml).
//     ComboBox { id: box; delegate: MacComboItem { combo: box }; popup: MacComboPopup { combo: box } }
ItemDelegate {
    id: row
    required property int index
    required property var model
    property ComboBox combo: null
    readonly property MacMenuStyle st: combo && combo.popup && combo.popup.macStyle ? combo.popup.macStyle : ownStyle
    MacMenuStyle { id: ownStyle; window: row.palette.window; highlight: row.palette.highlight }
    // The row's text: the model's textRole, or the item itself.
    readonly property string label: {
        if (!combo) return ""
        const m = row.model
        const role = combo.textRole
        if (role) {
            if (m && m[role] !== undefined) return String(m[role])
            if (m && m.modelData && m.modelData[role] !== undefined) return String(m.modelData[role])
        }
        if (m && m.modelData !== undefined) return String(m.modelData)
        return combo.textAt(index)
    }
    readonly property bool current: combo && combo.currentIndex === index
    readonly property bool lit: enabled && combo && combo.highlightedIndex === index
    width: ListView.view ? ListView.view.width : implicitWidth
    implicitWidth: st.gutter + lbl.implicitWidth + 2 * st.textInset
    implicitHeight: st.rowHeight
    padding: 0
    // (Basic sets these itself, for its own indicator and arrow.)
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    hoverEnabled: true
    highlighted: lit
    background: Rectangle {
        radius: row.st.selectionRadius
        color: row.st.accent
        opacity: row.lit ? 1 : 0
    }
    readonly property color textColor: !enabled ? st.fgDisabled : lit ? st.accentText : st.fg
    contentItem: Item {
        Text {
            x: Math.round(5 * row.st.pt) + (row.st.gutter - Math.round(5 * row.st.pt) - width) / 2 - 2
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            visible: row.current
            text: "✓"
            color: row.textColor
            font.family: row.st.family
            font.pixelSize: row.st.fontSize
            font.weight: Font.DemiBold
            renderType: Text.NativeRendering
        }
        Text {
            id: lbl
            x: row.st.gutter + 0.001
            width: parent.width - row.st.gutter - row.st.textInset
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: 0.001
            text: row.label
            color: row.textColor
            font.family: row.st.family
            font.pixelSize: row.st.fontSize
            font.letterSpacing: row.st.letterSpacing
            elide: Text.ElideRight
            textFormat: Text.PlainText
            renderType: Text.NativeRendering
        }
    }
}
