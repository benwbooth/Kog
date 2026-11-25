import QtQuick
import QtQuick.Layouts
import QtQuick.Shapes
import QtQuick.Templates as T

// A row of a KogMenu, in Breeze's look: a check box column (when the menu
// has checkable rows), a 16 px icon column (when it has icons), the text,
// the action's shortcut at the right, and a ▸ for a submenu. Under the
// pointer the row gets a rounded highlight-tinted box with a highlight
// border; pressed, it fills with the highlight colour.
T.MenuItem {
    id: control

    readonly property var view: ListView.view
    readonly property bool columnForChecks: checkable || (view && view.hasCheckables === true)
    readonly property bool columnForIcons: icon.name !== "" || icon.source.toString() !== ""
        || (view && view.hasIcons === true)
    // (Disabled: the text faded into the menu, as Breeze's inactive text.
    // The palette's disabled group isn't reliable here: a window that sets
    // windowText after disabled.windowText overwrites it.)
    readonly property color textColor: !enabled ? Qt.tint(palette.window, Qt.alpha(palette.windowText, 0.45))
        : pressed ? palette.highlightedText : palette.windowText

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding
                                + (subMenu ? arrow.implicitWidth + 4 : 0))
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)
    // Hidden rows take no room.
    height: visible ? undefined : 0

    padding: 4
    verticalPadding: 4
    hoverEnabled: true

    contentItem: RowLayout {
        spacing: 5
        Item {
            Layout.preferredWidth: control.columnForChecks ? control.indicator.width : 4
        }
        AppMenu.CrispIcon {
            id: rowIcon
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: width
            Layout.preferredHeight: height
            visible: control.columnForIcons
            size: 16
            name: control.icon.name
            source: control.icon.name === "" ? control.icon.source : ""
            color: control.icon.color
            opacity: control.enabled ? 1 : 0.4
            // Placed on the pixel grid again whenever the menu opens or moves.
            Connections {
                target: control.menu
                ignoreUnknownSignals: true
                function onOpened() { Qt.callLater(rowIcon.snap) }
                function onXChanged() { Qt.callLater(rowIcon.snap) }
                function onYChanged() { Qt.callLater(rowIcon.snap) }
            }
        }
        Text {
            id: label
            Layout.alignment: Qt.AlignVCenter
            Layout.fillWidth: true
            text: control.text
            font: control.font
            color: control.textColor
            elide: Text.ElideRight
            visible: control.text !== ""
            textFormat: Text.PlainText
            renderType: Text.NativeRendering
            verticalAlignment: Text.AlignVCenter
        }
        Text {
            id: shortcutLabel
            Layout.alignment: Qt.AlignVCenter
            Layout.leftMargin: 12
            visible: text !== ""
            text: itemShortcut.nativeText
            font: control.font
            color: control.textColor
            opacity: control.pressed ? 1 : 0.7
            renderType: Text.NativeRendering
            horizontalAlignment: Text.AlignRight
            verticalAlignment: Text.AlignVCenter
            // Only reads the action's key for display; the action itself
            // owns the shortcut.
            Shortcut {
                id: itemShortcut
                enabled: false
                sequence: control.action && control.action.shortcut !== undefined
                    ? control.action.shortcut : ""
            }
        }
        Item {
            Layout.preferredWidth: 4
        }
    }

    arrow: Shape {
        x: control.mirrored ? control.padding : control.width - width - control.padding - 2
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitWidth: 8
        implicitHeight: 10
        visible: control.subMenu
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: control.textColor
            strokeWidth: 1.5
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            startX: control.mirrored ? 6 : 2; startY: 1
            PathLine { x: control.mirrored ? 2 : 6; y: 5 }
            PathLine { x: control.mirrored ? 6 : 2; y: 9 }
        }
    }

    // Breeze's check box.
    indicator: Rectangle {
        x: control.mirrored ? control.width - width - control.rightPadding : control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        implicitWidth: 16
        implicitHeight: 16
        visible: control.checkable
        radius: 3
        color: control.palette.base
        border.width: 1
        border.color: control.checked || control.highlighted
            ? control.palette.highlight
            : Qt.tint(control.palette.window,
                Qt.rgba(control.palette.windowText.r, control.palette.windowText.g,
                        control.palette.windowText.b, 0.35))
        Shape {
            anchors.fill: parent
            visible: control.checked
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: control.palette.highlight
                strokeWidth: 2
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                startX: 4; startY: 8.5
                PathLine { x: 7; y: 11.5 }
                PathLine { x: 12.5; y: 5 }
            }
        }
    }

    background: Rectangle {
        implicitWidth: 144
        opacity: (control.highlighted || control.down) && control.enabled ? 1 : 0
        color: control.pressed ? control.palette.highlight
            : Qt.rgba(control.palette.highlight.r, control.palette.highlight.g,
                      control.palette.highlight.b, 0.3)
        border.color: control.palette.highlight
        border.width: 1
        radius: 5
    }
}
