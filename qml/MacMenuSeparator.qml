import QtQuick
import QtQuick.Controls.Basic

// A divider between groups of MacMenuItems: a 1 px line, inset like the row
// text (macOS 26 menus). Shared by Ben's Qt Quick apps (see MacMenuStyle.qml).
MenuSeparator {
    id: sep
    readonly property bool macSeparator: true
    // (Set by its MacMenu to the menu's look; `styleHint` gives it when it's
    // made, as for MacMenuItem. One of its own is made only when neither does.)
    property MacMenuStyle styleHint: null
    property MacMenuStyle st: styleHint ? styleHint : ownStyle()
    readonly property var _own: ({})
    function ownStyle() {
        if (!_own.style)
            _own.style = ownStyleMaker.createObject(sep, { window: sep.palette.window, highlight: sep.palette.highlight })
        return _own.style
    }
    Component { id: ownStyleMaker; MacMenuStyle {} }
    padding: 0
    topPadding: 0
    bottomPadding: 0
    implicitHeight: visible ? st.separatorHeight : 0
    height: visible ? implicitHeight : 0
    contentItem: Item {
        implicitWidth: 40
        implicitHeight: sep.st.separatorHeight
        Rectangle {
            x: sep.st.textInset
            width: parent.width - 2 * sep.st.textInset
            height: 1
            anchors.verticalCenter: parent.verticalCenter
            color: sep.st.separator
        }
    }
}
