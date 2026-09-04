import QtQuick
import org.kde.layershell as LayerShell

// Loaded only when the Linux layer-shell module is available. Other platforms
// keep using the regular window's position and available desktop geometry.
QtObject {
    required property var target
    readonly property var surface: target.LayerShell.Window

    Component.onCompleted: {
        surface.anchors = LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorRight
        surface.layer = LayerShell.Window.LayerOverlay
        surface.exclusionZone = 0 // Respect the panel's reserved area, reserve none ourselves.
        surface.keyboardInteractivity = LayerShell.Window.KeyboardInteractivityNone
        surface.activateOnShow = false
        surface.scope = "kog-notification"
        surface.margins = Qt.binding(() => ({
            left: 0, top: 0,
            right: Math.round(target.rightMargin), bottom: Math.round(target.bottomMargin)
        }))
    }
}
