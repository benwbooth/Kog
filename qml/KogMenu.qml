import QtQuick
import QtQuick.Controls
import QtQuick.Templates as T

// Kog's right-click and drop-down menus, in the look they had under KDE's
// org.kde.desktop style (Breeze): the window colour on a rounded panel with
// a thin frame and a soft shadow, rows with an icon column, the shortcut at
// the right, and a highlight-tinted rounded row under the pointer. Kog runs
// the Basic style everywhere now (no Kirigami), whose menus are flat grey
// boxes, so every menu in Kog is a KogMenu with KogMenuItem rows and
// KogMenuSeparator lines. Nested KogMenus become submenus.
T.Menu {
    id: control

    readonly property color frameColor: Qt.tint(palette.window,
        Qt.rgba(palette.windowText.r, palette.windowText.g, palette.windowText.b, 0.2))

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding)

    // Kept inside the window; a menu taller than the room scrolls.
    margins: 0
    // Breeze's menu margins. The .001 keeps rows off exact half device
    // pixels at fractional scales, where Qt rounds each glyph on its own
    // and round letters drop a pixel (the overlay is outside the main
    // window's own nudge).
    topPadding: 4.001
    bottomPadding: 4
    leftPadding: 4.001
    rightPadding: 4
    overlap: 1
    cascade: true

    delegate: KogMenuItem {}

    contentItem: ListView {
        id: listview
        property bool hasCheckables: false
        property bool hasIcons: false

        implicitWidth: Math.max(1, contentItem.children.reduce(
            (w, child) => Math.max(w, child.implicitWidth), 0))
        implicitHeight: Math.max(1, contentItem.visibleChildren.reduce(
            (h, item) => h + item.implicitHeight, 1))

        model: control.contentModel
        spacing: 0
        interactive: Window.window
            ? contentHeight + control.topPadding + control.bottomPadding > Window.window.height
            : false
        clip: true
        currentIndex: control.currentIndex
        keyNavigationEnabled: true
        keyNavigationWraps: true

        ScrollIndicator.vertical: ScrollIndicator {}

        // As Qt Widgets menus do: leaving a row clears its highlight.
        Connections {
            target: listview.currentItem
            ignoreUnknownSignals: true
            function onHoveredChanged() {
                const item = listview.currentItem
                if (item instanceof T.MenuItem && item.highlighted && !item.subMenu && !item.hovered)
                    control.currentIndex = -1
            }
        }
    }

    // Line the rows' icons and check boxes up in columns when any row has one.
    Connections {
        target: listview.contentItem
        function onVisibleChildrenChanged() {
            let checkables = false, icons = false
            for (const child of listview.contentItem.visibleChildren) {
                if (child.checkable) checkables = true
                if (child.icon && (child.icon.name !== "" || child.icon.source.toString() !== ""))
                    icons = true
            }
            listview.hasCheckables = checkables
            listview.hasIcons = icons
        }
    }

    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; easing.type: Easing.InOutQuad; duration: 150 }
    }
    exit: Transition {
        NumberAnimation { property: "opacity"; from: 1; to: 0; easing.type: Easing.InOutQuad; duration: 150 }
    }

    background: Item {
        implicitWidth: 144
        // Shadow: a few widening, fading rings, offset down by 2.
        Repeater {
            model: 4
            Rectangle {
                required property int index
                anchors.fill: parent
                anchors.margins: -(index + 1)
                anchors.topMargin: -(index + 1) + 2
                anchors.bottomMargin: -(index + 1) - 2
                radius: 5 + index + 1
                color: "transparent"
                border.width: 1
                border.color: Qt.rgba(0, 0, 0, 0.13 - index * 0.03)
            }
        }
        Rectangle {
            anchors.fill: parent
            radius: 5
            color: control.palette.window
            border.width: 1
            border.color: control.frameColor
        }
    }
}
