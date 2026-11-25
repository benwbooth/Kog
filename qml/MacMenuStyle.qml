import QtQuick

// The look of the Mac* menu components (shared by Ben's Qt Quick apps; the
// canonical copy is in hearth/shared/app-menus, scripts/sync-app-menus.sh
// copies it into each app: keep the copies the same).
//
// macOS 26 menus, as hearth draws them (shell/Theme.qml "macOS 26 metrics"):
// 12 pt corners, 4 pt around the rows, 24 pt rows with 13 pt text, the
// selection an accent-filled 8 pt rounded rectangle, a checkmark/icon column,
// grey shortcuts at the right, thin inset separators. One hearth point is
// 1.125 logical px.
QtObject {
    // Inputs: the app's palette (MacMenu/MacComboBox pass theirs in); an app
    // may set `dark` and `accent` itself.
    property color window: "#2a2a2e"
    property color highlight: "#0a84ff"
    property bool dark: window.hslLightness < 0.5
    property color accent: highlight

    readonly property real pt: 1.125
    readonly property string family: {
        const f = Qt.fontFamilies()
        for (const n of ["SF Pro", "SF Pro Text", "Inter", "Inter Variable"]) if (f.indexOf(n) >= 0) return n
        return Qt.application.font.family
    }
    // 13 pt (Inter runs ~4% larger than SF Pro at the same size).
    readonly property real fontSize: Math.round(13 * pt * (family.indexOf("Inter") === 0 ? 0.962 : 1) * 10) / 10
    readonly property real letterSpacing: -0.09
    // (Shortcuts: a little looser, as macOS spaces its key glyphs.)
    readonly property real keySpacing: 0.6

    readonly property color fg: dark ? "#f5f5f7" : "#1d1d1f"
    readonly property color fgSecondary: dark ? Qt.rgba(1, 1, 1, 0.55) : Qt.rgba(0, 0, 0, 0.5)
    readonly property color fgDisabled: dark ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(0, 0, 0, 0.26)
    readonly property color accentText: "#ffffff"
    readonly property color accentDim: Qt.rgba(accent.r, accent.g, accent.b, dark ? 0.35 : 0.25)
    readonly property color separator: dark ? Qt.rgba(1, 1, 1, 0.13) : Qt.rgba(0, 0, 0, 0.11)

    // Glass: a translucent tint over the blurred window behind the menu
    // (vibrancy), a thin bright rim, a darker outline and a soft shadow.
    readonly property color glassTint: dark ? Qt.rgba(0.13, 0.135, 0.15, 0.56) : Qt.rgba(0.985, 0.985, 0.99, 0.68)
    // (Where nothing behind can be blurred: nearly opaque.)
    readonly property color glassSolid: dark ? Qt.rgba(0.17, 0.175, 0.19, 0.985) : Qt.rgba(0.975, 0.975, 0.98, 0.985)
    readonly property color glassRim: dark ? Qt.rgba(1, 1, 1, 0.05) : Qt.rgba(1, 1, 1, 0.40)
    // The specular edge: brightest along the top, fading down the sides.
    readonly property color glassSpecular: dark ? Qt.rgba(1, 1, 1, 0.20) : Qt.rgba(1, 1, 1, 0.85)
    // How the backdrop is treated: blurred well past recognition, its
    // colours lifted (macOS vibrancy), darkened a touch in dark mode.
    readonly property real backdropBlur: 64
    readonly property real backdropSaturation: 0.75
    readonly property real backdropBrightness: dark ? -0.08 : 0.06
    // The outline: a faint hairline, just enough to part the menu from what
    // is behind (macOS: no hard frame).
    readonly property color glassEdge: dark ? Qt.rgba(0, 0, 0, 0.28) : Qt.rgba(0, 0, 0, 0.09)
    readonly property color shadow: Qt.rgba(0, 0, 0, dark ? 0.32 : 0.2)
    readonly property real shadowBlur: 26
    readonly property real shadowY: 9

    readonly property int radius: Math.round(12 * pt)
    readonly property int padding: Math.round(4 * pt)
    readonly property int rowHeight: Math.round(24 * pt)
    readonly property int selectionRadius: Math.round(8 * pt)
    readonly property int textInset: Math.round(10 * pt)
    readonly property int gutter: Math.round(24 * pt)   // checkmark / icon column
    readonly property int separatorHeight: Math.round(9 * pt)
    readonly property int minWidth: Math.round(180 * pt)

    // Motion: a quick fade on open, a slower fade
    // on close; a chosen row blinks once first (macOS).
    readonly property int inMs: 100
    readonly property int outMs: 180
    readonly property int blinkMs: 55

    // Shortcuts the PC way, with words, not macOS glyphs (as hearth spells
    // them): "ctrl+shift+n" -> "Ctrl+Shift+N", modifiers in Ctrl, Alt,
    // Shift, Super order.
    function pcShortcut(s) {
        s = s ? String(s) : ""
        if (!s) return ""
        const parts = s.split("+").map(x => x.trim()).filter(x => x !== "")
        if (s.endsWith("++")) parts.push("+")
        const mods = { ctrl: "Ctrl", control: "Ctrl", alt: "Alt", option: "Alt", shift: "Shift",
                       super: "Super", meta: "Super", cmd: "Super", win: "Super", logo: "Super", mod: "Super" }
        const order = ["Ctrl", "Alt", "Shift", "Super"]
        const keys = { return: "Enter", enter: "Enter", backspace: "Backspace", delete: "Del", del: "Del",
                       escape: "Esc", esc: "Esc", tab: "Tab", left: "←", right: "→", up: "↑", down: "↓",
                       pgup: "PgUp", pageup: "PgUp", pgdown: "PgDn", pagedown: "PgDn", page_up: "PgUp", page_down: "PgDn",
                       home: "Home", end: "End", space: "Space", print: "PrtSc", ins: "Ins", insert: "Ins" }
        let m = [], k = []
        for (const p of parts) {
            const l = p.toLowerCase()
            if (mods[l]) m.push(mods[l])
            else k.push(keys[l] || (p.length === 1 ? p.toUpperCase() : p))
        }
        m = [...new Set(m)].sort((a, b) => order.indexOf(a) - order.indexOf(b))
        return m.concat(k).join("+")
    }
}
