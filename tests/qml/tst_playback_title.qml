import QtQuick
import QtTest
import "../../qml"

TestCase {
    name: "PlaybackTitle"
    PlaybackTitle {
        id: title
        playbackState: "stopped"
        trackTitle: "Not Playing"
    }
    function test_title_data() {
        return [
            { tag: "empty", state: "stopped", track: "Not Playing", expected: "Kog", active: false },
            { tag: "playing", state: "playing", track: "Duck Tales", expected: "Duck Tales", active: true },
            { tag: "paused", state: "paused", track: "Duck Tales", expected: "Duck Tales", active: true },
            { tag: "stopped-retains-metadata", state: "stopped", track: "Duck Tales", expected: "Kog", active: false }
        ]
    }
    function test_title(data) {
        title.playbackState = data.state
        title.trackTitle = data.track
        compare(title.text, data.expected)
        compare(title.active, data.active)
        compare(title.windowTitle, data.active ? data.track + " — Kog" : "Kog")
    }
    function test_stop_and_replay() {
        title.trackTitle = "A long title that was scrolling before Stop"
        title.playbackState = "playing"
        compare(title.text, title.trackTitle)
        title.playbackState = "stopped"
        compare(title.text, "Kog")
        compare(title.active, false)
        title.playbackState = "playing"
        compare(title.text, title.trackTitle)
    }
}
