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
        property int playlist_count: 3
        property int playlist_revision: 0
        property int current_index: 0
        property int activatedRow: -1
        property string removedRows: ""
        function track_number_at(row) { return String(row + 1) }
        function track_value_at(row, column) { return column === "length" ? "3:00" : "Track " + row }
        function activate_playlist_index(row) { activatedRow = row }
        function remove_tracks(rows) { removedRows = rows }
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
    Kog.ClassicPlayer { id: classic; app: app; mainWindow: main; settingsFile: "/tmp/kog-classic-qml-test.ini" }
    Kog.SkinBrowser { id: browser; library: library }
    function init() {
        failOnWarning(/TypeError|ReferenceError|Unable to assign|Binding loop/)
        app.playback_state = "playing"
    }
    function cleanup() { visualizer.hide(); classic.hide(); browser.hide() }
    function test_classic_bitmap_playlist() {
        const assets = {}
        for (const name of ["main", "cbuttons", "titlebar", "numbers", "playpaus", "posbar", "volume", "shufrep", "text", "pledit"])
            assets[name] = Qt.resolvedUrl("../../native/webamp/packages/webamp/assets/skins/base-2.91/" + name.toUpperCase() + ".BMP").toString()
        classic.skin = {title: "Classic fixture", assets: assets}
        classic.toolbarVisible = false
        classic.playlistVisible = true
        classic.show()
        wait(150)
        grabImage(classic.contentItem).save("/tmp/kog-classic-playlist-test.png")
        const list = findChild(classic, "classicPlaylist")
        compare(list.count, 3)
        mouseDoubleClickSequence(list, 35, 19)
        compare(app.activatedRow, 1)
        classic.skin = {assets: {}}
    }
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
    function test_classic_playlist_and_toolbar() {
        classic.toolbarVisible = false
        classic.playlistVisible = true
        classic.show()
        const list = findChild(classic, "classicPlaylist")
        compare(list.count, 3)
        compare(classic.height, (116 + 232) * classic.scaleFactor)
        compare(findChild(classic, "classicToolbar").visible, false)
        findChild(classic, "classicToolbarToggle").triggered()
        compare(classic.toolbarVisible, true)
        compare(classic.height, (116 + 232) * classic.scaleFactor + 44)
        const playlist = list.parent
        playlist.selectRow(0, Qt.NoModifier)
        playlist.selectRow(2, Qt.ShiftModifier)
        compare(playlist.selectedRows.join(","), "0,1,2")
        playlist.removeSelected()
        compare(app.removedRows, "0,1,2")
        classic.playlistVisible = false
        compare(classic.height, 116 * classic.scaleFactor + 44)
        classic.toolbarVisible = false
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
    function test_all_visualizer_modes_and_bounded_history() {
        visualizer.show()
        const plot = findChild(visualizer, "audioVisualization")
        const selector = findChild(visualizer, "visualizerMode")
        compare(selector.count, 6)
        for (let i = 0; i < selector.count; ++i) {
            selector.currentIndex = i
            compare(plot.mode, visualizer.modeIds[i])
            plot.updateFrame()
            wait(50)
            verify(plot.available)
        }
        selector.currentIndex = 2
        for (let i = 0; i < 150; ++i) plot.updateFrame()
        compare(plot.spectrumHistory.length, plot.historyLimit)
        selector.currentIndex = 5
        compare(plot.spectrumHistory.length, 0)
        for (let i = 0; i < 25; ++i) plot.updateFrame()
        compare(plot.waveHistory.length, 10)
        visualizer.hide()
        compare(plot.waveHistory.length, 0)
    }
}
