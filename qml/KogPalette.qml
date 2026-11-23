import QtQuick

// Kog's colours for a window's controls: the application palette, which is
// Earshot's light or dark palette (native/kog_desktop_integration.cpp) or the
// desktop's with KOG_COLORS=system. The Basic style colours its controls
// from its own palette, not the application's, and windows don't inherit a
// palette from the window that opened them, so every window sets
//     palette: KogPalette {}
// and may override single roles inside the braces.
Palette {
    readonly property SystemPalette sys: SystemPalette { colorGroup: SystemPalette.Active }
    readonly property SystemPalette disabledColors: SystemPalette { colorGroup: SystemPalette.Disabled }

    window: sys.window
    windowText: sys.windowText
    base: sys.base
    alternateBase: sys.alternateBase
    text: sys.text
    button: sys.button
    buttonText: sys.buttonText
    brightText: "#ffffff"
    light: sys.light
    midlight: sys.midlight
    mid: sys.mid
    dark: sys.dark
    shadow: sys.shadow
    highlight: sys.highlight
    highlightedText: sys.highlightedText
    accent: sys.accent
    link: sys.accent
    toolTipBase: sys.button
    toolTipText: sys.text
    placeholderText: sys.placeholderText

    disabled.windowText: disabledColors.windowText
    disabled.text: disabledColors.text
    disabled.buttonText: disabledColors.buttonText
    disabled.highlight: disabledColors.highlight
}
