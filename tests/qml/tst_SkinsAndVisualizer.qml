import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    name: "SkinsAndVisualizer"
    when: windowShown
    width: 880; height: 670
    QtObject {
        id: app
        property string now_title: "音楽 <Live>"
        property string now_artist: "Artist"
        property string playback_state: "playing"
        property string shuffle_mode: "off"
        property string repeat_mode: "off"
        property real position_seconds: 42
        property real duration_seconds: 180
        property real volume: 0.5
        property int nextCount: 0
        property int frameCount: 0
        function next() { nextCount++ }
        function previous() {}
        function play_pause() { playback_state = playback_state === "playing" ? "paused" : "playing" }
        function stop() { playback_state = "stopped" }
        function visualizer_frame() { frameCount++; return JSON.stringify({wave: [0, 0.5, -0.5, 0], spectrum: [0.2, 0.7, 0.4]}) }
        function seek(value) { position_seconds = value }
        function set_volume_level(value) { volume = value }
    }
    QtObject {
        id: main
        property bool applicationQuitRequested: false
        property int restored: 0
        function showFromTray() { restored++ }
    }
    QtObject {
        id: library
        property string installed_json: "[]"
        property string catalog_json: "[]"
        property string active_json: "{}"
        property string status: "Test catalog"
        property bool busy: false
        property int total: 0
        function search(query, page) {}
    }
    Kog.Visualizer { id: visualizer; app: app; settingsFile: "/tmp/kog-visualizer-qml-test.ini" }
    Kog.ClassicPlayer { id: classic; app: app; mainWindow: main }
    Kog.SkinBrowser { id: browser; library: library }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Unable to assign|Binding loop/)
        app.playback_state = "playing"
    }
    function cleanup() { visualizer.hide(); classic.hide(); browser.hide() }
    function test_transport_semantics() {
        classic.show()
        classic.buttonAction(1)
        compare(app.playback_state, "playing")
        classic.buttonAction(2)
        compare(app.playback_state, "paused")
        classic.buttonAction(2)
        compare(app.playback_state, "paused")
        classic.buttonAction(1)
        compare(app.playback_state, "playing")
        classic.buttonAction(3)
        compare(app.playback_state, "stopped")
        const restored = main.restored
        classic.close()
        compare(main.restored, restored + 1)
    }
    function test_visualizer_only_samples_while_visible() {
        visualizer.show()
        const count = app.frameCount
        tryVerify(() => app.frameCount > count)
        const plot = findChild(visualizer, "audioVisualization")
        compare(plot.frame.wave.length, 4)
        findChild(visualizer, "visualizerMode").currentIndex = 1
        compare(plot.waveform, true)
        visualizer.hide()
        const stopped = app.frameCount
        wait(120)
        compare(app.frameCount, stopped)
    }
    function test_empty_gallery_and_resize() {
        browser.show()
        browser.width = 610
        wait(100)
        compare(browser.items.length, 0)
    }
}
