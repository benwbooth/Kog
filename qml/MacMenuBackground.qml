import QtQuick
import QtQuick.Effects

// The glass behind a Mac* menu or pop-up list: the window behind it blurred
// (vibrancy), a translucent tint, a thin bright rim, a darker outline and a
// soft shadow. Shared by Ben's Qt Quick apps (see MacMenuStyle.qml).
Item {
    id: bg
    property MacMenuStyle style: MacMenuStyle {}
    // The popup it sits in: its moves re-aim the blur.
    property var popup: null
    implicitWidth: style.minWidth
    implicitHeight: style.rowHeight + 2 * style.padding

    // What's behind: the window's content (the popup is drawn above it, in
    // the window's overlay, so the capture never includes the menu itself).
    readonly property Item behind: Window.window ? Window.window.contentItem : null
    // (Blurring needs shaders: with the software renderer the tint stays
    // nearly opaque instead.)
    readonly property bool vibrancy: behind !== null && visible && width > 0
        && GraphicsInfo.api !== GraphicsInfo.Software && GraphicsInfo.api !== GraphicsInfo.Unknown
    property rect area: Qt.rect(0, 0, 0, 0)
    function aim() {
        if (!behind) return
        const p = bg.mapToItem(behind, 0, 0)
        area = Qt.rect(p.x, p.y, bg.width, bg.height)
    }
    onWidthChanged: Qt.callLater(aim)
    onHeightChanged: Qt.callLater(aim)
    onVisibleChanged: if (visible) Qt.callLater(aim)
    Component.onCompleted: Qt.callLater(aim)
    Connections {
        target: bg.popup
        ignoreUnknownSignals: true
        function onXChanged() { Qt.callLater(bg.aim) }
        function onYChanged() { Qt.callLater(bg.aim) }
        function onOpened() { Qt.callLater(bg.aim) }
        function onAboutToShow() { Qt.callLater(bg.aim) }
    }

    RectangularShadow {
        anchors.fill: parent
        offset.y: bg.style.shadowY
        radius: bg.style.radius
        blur: bg.style.shadowBlur
        spread: -4
        color: bg.style.shadow
    }

    ShaderEffectSource {
        id: grab
        sourceItem: bg.vibrancy ? bg.behind : null
        sourceRect: bg.area
        anchors.fill: parent
        live: true
        hideSource: false
        visible: false
    }
    Rectangle {
        id: mask
        anchors.fill: parent
        radius: bg.style.radius
        visible: false
        layer.enabled: true
    }
    MultiEffect {
        anchors.fill: parent
        visible: bg.vibrancy
        source: grab
        blurEnabled: true
        blur: 1
        blurMax: bg.style.backdropBlur
        autoPaddingEnabled: false
        saturation: bg.style.backdropSaturation
        brightness: bg.style.backdropBrightness
        maskEnabled: true
        maskSource: mask
    }
    Rectangle {
        anchors.fill: parent
        radius: bg.style.radius
        color: bg.vibrancy ? bg.style.glassTint : bg.style.glassSolid
        border.width: 1
        border.color: bg.style.glassEdge
    }
    Rectangle {   // a little light in the top part
        anchors.fill: parent
        anchors.margins: 1
        radius: bg.style.radius - 1
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, bg.style.dark ? 0.06 : 0.20) }
            GradientStop { position: 0.3; color: Qt.rgba(1, 1, 1, 0) }
        }
    }
    Rectangle {   // the rim, all round
        anchors.fill: parent
        anchors.margins: 1
        radius: bg.style.radius - 1
        color: "transparent"
        border.width: 1
        border.color: bg.style.glassRim
    }
    // The specular edge: a bright rim along the top that fades down the
    // sides (a rim ring, masked by a top-to-bottom fade).
    Item {
        anchors.fill: parent
        anchors.margins: 1
        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: specFade
            maskThresholdMin: 0.0
            maskSpreadAtMin: 1.0
        }
        Rectangle {
            anchors.fill: parent
            radius: bg.style.radius - 1
            color: "transparent"
            border.width: 1
            border.color: bg.style.glassSpecular
        }
    }
    Rectangle {
        id: specFade
        anchors.fill: parent
        visible: false
        layer.enabled: true
        gradient: Gradient {
            GradientStop { position: 0.0; color: "white" }
            GradientStop { position: 0.45; color: "transparent" }
        }
    }
}
