import QtQuick
import QtQuick.Controls.impl

// A menu icon drawn pixel-exact (shared by Ben's Qt Quick apps; see
// MacMenuStyle.qml). At a fractional scale (4/3) a 16 pt icon is 21.33
// device pixels, so the 16 px symbol was stretched between pixels and its
// strokes went soft. Instead: an icon-theme design size (16/22/24/32 px)
// whose size in points is whole (Qt rounds sourceSize to whole points, then
// multiplies by the scale: 22 px can't be asked for at 4/3), the one nearest
// the wanted size, placed on the device pixel grid. At 4/3 that is 24 px
// (18 pt); at 1x and 2x, 16 and 32 px (16 pt). Call snap() when it moves in
// the window without its own x/y changing (its menu opening or moving).
Item {
    id: ci
    // A freedesktop icon name ("name|fallback": the first is looked up in
    // the theme; an app's provider gets the whole string).
    property string name: ""
    property real size: 16
    // An app's image provider, (name, device px) => URL, loaded off the UI
    // thread; null: the icon theme, looked up here.
    property var iconUrl: null
    // Recolour a one-colour icon (transparent: as drawn; theme icons only).
    property color color: "transparent"
    readonly property real dpr: {
        const w = ci.Window.window
        return w && w.devicePixelRatio > 0 ? w.devicePixelRatio : (Screen.devicePixelRatio || 1)
    }
    readonly property int px: {
        const want = size * dpr
        let best = 0
        for (const d of [16, 22, 24, 32]) {
            const pt = d / dpr
            if (Math.abs(pt - Math.round(pt)) > 0.01) continue
            if (!best || Math.abs(d - want) < Math.abs(best - want)) best = d
        }
        return best
    }
    // (No exact size at this scale: the plain size.)
    readonly property int pt: px > 0 ? Math.round(px / dpr) : Math.round(size)
    width: px > 0 ? px / dpr : size
    height: width
    // (Only the one in use is made: an IconImage looks its name up even
    // when hidden or empty.)
    Loader {
        anchors.fill: parent
        active: ci.name !== ""
        sourceComponent: ci.iconUrl ? providerIcon : themeIcon
    }
    Component {
        id: themeIcon
        IconImage {
            name: ci.name.split("|")[0]
            sourceSize: Qt.size(ci.pt, ci.pt)
            // (Setting a transparent colour would draw nothing.)
            Binding on color { when: ci.color.a > 0; value: ci.color }
        }
    }
    Component {
        id: providerIcon
        // (An Image: IconImage doesn't load from image providers. The
        // provider draws the icon in the right colours itself.)
        Image {
            source: ci.iconUrl(ci.name, ci.px > 0 ? ci.px : Math.round(ci.size * ci.dpr))
            sourceSize: Qt.size(ci.pt, ci.pt)
            asynchronous: true
            cache: true
        }
    }
    transform: Translate { id: gridSnap }
    function snap() {
        if (!visible || px <= 0) return
        const p = mapToItem(null, 0, 0)
        const rx = p.x - gridSnap.x, ry = p.y - gridSnap.y
        gridSnap.x = Math.round(rx * dpr) / dpr - rx
        gridSnap.y = Math.round(ry * dpr) / dpr - ry
    }
    onVisibleChanged: Qt.callLater(snap)
    onXChanged: Qt.callLater(snap)
    onYChanged: Qt.callLater(snap)
    onDprChanged: Qt.callLater(snap)
    Component.onCompleted: Qt.callLater(snap)
}
