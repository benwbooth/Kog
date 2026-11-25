import QtQuick
import QtQuick.Templates as T

// A line between groups of rows in a KogMenu (the ☰ menu's look).
T.MenuSeparator {
    id: control

    readonly property bool dark: palette.window.hslLightness < 0.5

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: visible ? Math.max(implicitBackgroundHeight + topInset + bottomInset,
                                       implicitContentHeight + topPadding + bottomPadding) : 0

    topPadding: 4
    bottomPadding: 4
    leftPadding: 10
    rightPadding: 10
    hoverEnabled: false
    focusPolicy: Qt.NoFocus

    contentItem: Rectangle {
        implicitHeight: 1
        color: control.dark ? Qt.rgba(1, 1, 1, 0.10) : Qt.rgba(0, 0, 0, 0.09)
    }
}
