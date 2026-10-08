import QtQuick

// Tracker text in the Spleen 6x12 pixel font, drawn at its native size with
// no smoothing so every glyph stays sharp.
Text {
    font.family: "Spleen 6x12"
    font.pixelSize: 12
    font.hintingPreference: Font.PreferNoHinting
    renderType: Text.NativeRendering
    antialiasing: false
    textFormat: Text.PlainText
    elide: Text.ElideRight
    verticalAlignment: Text.AlignVCenter
    color: "#d7e6ed"
}
