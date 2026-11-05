import QtQuick

// The playing song's six-bar meter, laid out like the Dynamic Island's
// waveform: pill bars centred in their slots, half a slot wide when quiet and
// up to 66% wider when loud, springing to each new height and shading from
// violet to blue. It sits straight on whatever is behind it.
Item {
    id: meter

    required property var app
    // White bars, for the blue of a selected row.
    property bool highlighted: false
    property color highlightColor: "white"
    // Bright bars for dark backgrounds, deeper ones for light.
    property bool dark: true

    implicitWidth: 24
    implicitHeight: 21

    readonly property var levels: [
        app.audio_level_low,
        app.audio_level_low_mid,
        app.audio_level_mid,
        app.audio_level_high_mid,
        app.audio_level_high,
        app.audio_level_air
    ]

    Item {
        id: area
        anchors.centerIn: parent
        width: meter.width - 4
        height: meter.height - 4

        Repeater {
            model: 6

            Rectangle {
                required property int index
                readonly property real slot: area.width / 6
                readonly property real base: slot / 2
                // Raised toward the top like kog_playback_policy's
                // waveform::displayed (gain 1.2, exponent 0.8).
                property real level: Math.min(1, 1.2 * Math.pow(
                    Math.max(0, Math.min(1, meter.levels[index])), 0.8))
                // The spring overshoots: the height may bounce a little past
                // the top (the meter has room for it).
                readonly property real shown: Math.max(0, Math.min(1, level))
                readonly property real bounce: Math.max(0, Math.min(1.15, level))

                width: base + 0.66 * shown * base
                height: Math.max(bounce * area.height, base)
                x: slot * (index + 0.5) - width / 2
                y: (area.height - height) / 2
                radius: width / 2
                antialiasing: true
                color: meter.highlighted ? meter.highlightColor
                    : Qt.hsla((262 - 50 * shown) / 360, 0.95, meter.dark ? 0.68 : 0.5, 1)

                Behavior on level {
                    // Stiff and lightly damped: a bar gets most of the way to
                    // each reading (20 a second) and springs past it, like the
                    // Dynamic Island's.
                    SpringAnimation {
                        spring: 15
                        damping: 0.45
                        epsilon: 0.005
                    }
                }
            }
        }
    }
}
