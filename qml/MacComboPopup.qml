import QtQuick
import QtQuick.Controls.Basic

// The list a pop-up button (ComboBox) opens, macOS style: a glass menu
// placed so the current choice sits over the button, its text lined up with
// the button's (shared by Ben's Qt Quick apps; see MacMenuStyle.qml).
//     ComboBox { id: box; delegate: MacComboItem { combo: box }; popup: MacComboPopup { combo: box } }
Popup {
    id: pop
    property ComboBox combo: null
    property alias dark: style.dark
    property alias accent: style.accent
    readonly property MacMenuStyle macStyle: MacMenuStyle {
        id: style
        window: pop.combo ? pop.combo.palette.window : "#2a2a2e"
        highlight: pop.combo ? pop.combo.palette.highlight : "#0a84ff"
    }
    // Where the button's text starts (to line the rows' text up with it).
    property real textX: combo ? combo.leftPadding : 0
    popupType: Popup.Item
    padding: style.padding
    topPadding: style.padding + 0.001
    leftPadding: style.padding + 0.001
    margins: Math.round(4 * style.pt)
    x: below ? 0 : Math.round(textX - style.gutter - style.padding)
    // Below the button instead (an editable combo: the list mustn't cover
    // what is being typed).
    property bool below: combo ? combo.editable : false
    // At most this many rows; more scroll.
    property int maxRows: 16
    y: !combo ? 0 : below ? combo.height + 2
        : Math.round((combo.height - style.rowHeight) / 2 - style.padding - Math.max(0, combo.currentIndex) * style.rowHeight)
    width: Math.max(combo ? combo.width - x : 0, list.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.min(list.contentHeight, maxRows * style.rowHeight) + topPadding + bottomPadding
    font.family: style.family
    font.pixelSize: style.fontSize
    background: MacMenuBackground { style: pop.macStyle; popup: pop }
    contentItem: ListView {
        id: list
        clip: true
        implicitHeight: contentHeight
        implicitWidth: {
            void count
            let w = 0
            for (let i = 0; i < count; i++) {
                const it = itemAtIndex(i)
                if (it) w = Math.max(w, it.implicitWidth)
            }
            return w
        }
        model: pop.combo && pop.visible ? pop.combo.delegateModel : null
        currentIndex: pop.combo ? pop.combo.highlightedIndex : -1
        highlightMoveDuration: 0
        boundsBehavior: Flickable.StopAtBounds
        ScrollIndicator.vertical: ScrollIndicator {}
    }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: style.inMs; easing.type: Easing.OutCubic } }
    exit: Transition {
        SequentialAnimation {
            PauseAnimation { duration: style.blinkMs * 2 }
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: style.outMs; easing.type: Easing.InCubic }
        }
    }
}
