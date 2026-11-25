import QtQuick
import QtQuick.Templates as T

// A line between groups of rows in a KogMenu (Breeze's look).
T.MenuSeparator {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: visible ? Math.max(implicitBackgroundHeight + topInset + bottomInset,
                                       implicitContentHeight + topPadding + bottomPadding) : 0
    width: parent?.width

    verticalPadding: 2
    horizontalPadding: 0
    hoverEnabled: false
    focusPolicy: Qt.NoFocus

    contentItem: Rectangle {
        implicitWidth: 144
        implicitHeight: 1
        color: Qt.tint(control.palette.window,
            Qt.rgba(control.palette.windowText.r, control.palette.windowText.g,
                    control.palette.windowText.b, 0.2))
    }
}
