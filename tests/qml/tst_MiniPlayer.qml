import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    name: "MiniPlayer"
    when: windowShown
    width: 600
    height: 250

    QtObject {
        id: playback
        property string now_title: "日本語 <Live> — a long track title"
        property string now_artist: "Artist"
        property string current_album: "Album"
        property string playback_state: "playing"
        property int current_index: 0
        property int playlist_count: 3
        property real position_seconds: 45
        property real duration_seconds: 180
        property real volume: 0.5
        property int previousCalls: 0
        property int nextCalls: 0
        function previous() { previousCalls++ }
        function next() { nextCalls++ }
        function stop() { playback_state = "stopped" }
        function play_pause() {
            playback_state = playback_state === "playing" ? "paused" : "playing"
        }
        function seek(value) { position_seconds = value }
        function set_volume_level(value) { volume = value }
    }
    QtObject {
        id: main
        property bool applicationQuitRequested: false
        property int restoreCalls: 0
        function showFromTray() { restoreCalls++ }
    }
    Kog.MiniPlayer { id: mini; app: playback; mainWindow: main }
    readonly property var buttons: ["miniPrevious", "miniPlayPause", "miniStop", "miniNext", "miniRestore"]

    function init() {
        failOnWarning(/TypeError|ReferenceError|Unable to assign|Binding loop|Cannot open/)
        playback.playback_state = "playing"
        playback.playlist_count = 3
        playback.current_index = 0
        mini.show()
        verify(waitForRendering(mini.contentItem))
    }
    function cleanup() { mini.hide() }

    function test_transport_and_restore() {
        const toggle = findChild(mini, "miniPlayPause")
        compare(toggle.iconName, "media-playback-pause")
        compare(toggle.toolTip, "Pause")
        mouseClick(toggle)
        compare(playback.playback_state, "paused")
        compare(toggle.iconName, "media-playback-start")
        compare(toggle.toolTip, "Play")
        mouseClick(toggle)
        compare(playback.playback_state, "playing")
        const previous = playback.previousCalls
        mouseClick(findChild(mini, "miniPrevious"))
        compare(playback.previousCalls, previous + 1)
        const next = playback.nextCalls
        mouseClick(findChild(mini, "miniNext"))
        compare(playback.nextCalls, next + 1)
        const stop = findChild(mini, "miniStop")
        mouseClick(stop)
        compare(playback.playback_state, "stopped")
        compare(stop.enabled, false)
        compare(toggle.iconName, "media-playback-start")
        const restored = main.restoreCalls
        mouseClick(findChild(mini, "miniRestore"))
        compare(main.restoreCalls, restored + 1)
        compare(mini.visible, false)
    }

    function test_icons_and_layout_data() {
        return [
            {tag: "dark-fixed", background: "#202428", highlight: "#296f9d"},
            {tag: "light-fixed", background: "#f4f4f4", highlight: "#83c4f1"}
        ]
    }
    function test_icons_and_layout(data) {
        mini.palette.window = data.background
        mini.palette.highlight = data.highlight
        const dark = data.tag.indexOf("dark") === 0
        mini.palette.text = dark ? "#eff0f1" : "#232629"
        mini.palette.windowText = mini.palette.text
        mini.palette.buttonText = mini.palette.text
        mini.palette.placeholderText = dark ? "#a4afb6" : "#636b71"
        wait(80)
        compare(mini.height, 144)
        verify((mini.flags & Qt.FramelessWindowHint) !== 0)
        const title = findChild(mini, "miniTitle")
        // Leave room for different Qt/KDE font metrics, but not a separate
        // title-bar row above the track information.
        verify(title.mapToItem(mini.contentItem, 0, 0).y < mini.height / 4,
            "No title bar above the track")
        let captured = false
        mini.contentItem.grabToImage(function(result) {
            verify(result.saveToFile("/tmp/kog-mini-" + data.tag + ".png"))
            captured = true
        })
        tryVerify(() => captured)
        const pixels = grabImage(mini.contentItem)
        const scaleX = pixels.width / mini.width
        const scaleY = pixels.height / mini.height
        let right = 0
        for (const name of buttons) {
            const button = findChild(mini, name)
            const icon = findChild(button, "miniButtonIcon")
            tryCompare(icon, "status", Image.Ready)
            verify(icon.source.toString().endsWith(".svg"))
            const point = button.mapToItem(mini.contentItem, 0, 0)
            verify(point.x >= right, "Transport buttons must not overlap")
            verify(point.x + button.width <= mini.width)
            verify(point.y >= 0 && point.y + button.height <= mini.height)
            right = point.x + button.width
            // Check the rendered glyph, not merely the presence of an icon name.
            // Inspect the final window image: grabbing the Image item alone
            // can return a transparent texture under the Basic control style.
            const left = Math.round((point.x + (button.width - button.icon.width) / 2) * scaleX)
            const top = Math.round((point.y + (button.height - button.icon.height) / 2) * scaleY)
            const colors = {}
            for (let y = top; y < top + Math.floor(button.icon.height * scaleY); ++y)
                for (let x = left; x < left + Math.floor(button.icon.width * scaleX); ++x)
                    colors[pixels.pixel(x, y).toString()] = true
            verify(Object.keys(colors).length > 1, name + " must not render as a blank button ("
                + icon.width + " x " + icon.height + ", " + icon.source + ")")
        }
    }

    function test_empty_queue_keeps_restore_available() {
        playback.playlist_count = 0
        playback.current_index = -1
        playback.playback_state = "stopped"
        for (const name of buttons.slice(0, 4))
            compare(findChild(mini, name).enabled, false)
        compare(findChild(mini, "miniRestore").enabled, true)
    }

    function test_fixed_window_size() {
        compare(mini.width, 540)
        compare(mini.height, 144)
        compare(mini.minimumWidth, mini.width)
        compare(mini.maximumWidth, mini.width)
        compare(mini.minimumHeight, mini.height)
        compare(mini.maximumHeight, mini.height)
    }

    function test_close_restores_full_player() {
        const restored = main.restoreCalls
        mini.close()
        compare(main.restoreCalls, restored + 1)
        compare(mini.visible, false)
    }
}
