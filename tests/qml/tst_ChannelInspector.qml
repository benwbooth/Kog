import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml" as Kog

TestCase {
    id: test
    name: "ChannelInspector"
    when: windowShown
    visible: true
    width: 1120
    height: 740
    property string initialState: ""
    property var state: ({version:1, description:{backend:"Test MIDI",kind:"events",detail:"Sequenced keys and commands"}, playing:true, seeking:false, current_row:0,
        channels:[{id:0,name:"MIDI 1",kind:"tonal",instrument:"Piano",level:0.8,pan:0,active:true,notes:[{key:60,velocity:0.8,held:true},{key:64.3,velocity:0.6,held:false}],fields:[{name:"Sustain",value:"On"}]}],
        rows:[{time:0,label:"0000.000",cells:[{channel:0,notes:"C-4 E-4",instrument:"01",volume:"64",effects:[{name:"CC64 Sustain",value:"127"}]}],global:[{name:"Tempo",value:"120 BPM"}]}],global:[]})
    QtObject {
        id: backend
        property string now_title: "Inspection fixture"
        property int requests: 0
        function channel_snapshot() { requests++; return JSON.stringify(test.state) }
        function play_pause() { test.state = Object.assign({}, test.state, {playing:!test.state.playing}) }
        property int barRequests: 0
        property int mmlCurrent: 1
        property int mmlRevision: 2
        property int mmlBars: 3
        function mml_state() {
            return JSON.stringify({revision: mmlRevision, message: mmlMessage, bars: mmlBars, current: mmlCurrent, header: "#KOG-MML 1",
                currentHtml: "<p>A | c4 <span style=\"background-color:#50c8ef\">e4</span> |</p>"})
        }
        property int barsPerLine: 4
        property string exported: ""
        function mml_text() { return "#KOG-MML 1\n" }
        function export_mml(file) { exported = file; return "" }
        function mml_guide() {
            return JSON.stringify([{title: "1. Introduction", markdown: "# 1. Introduction\n\nKog MML is a notation."},
                                   {title: "2. Notes", markdown: "# 2. Notes\n\nA note is `c`."}])
        }
        property string mmlMessage: ""
        function set_mml_bars_per_line(bars) { barsPerLine = bars }
        function mml_bar(index) { barRequests++; return "<p>; bar " + (index + 1) + "<br/>A | c4 e4 |</p>" }
    }
    Kog.ChannelInspector { id: inspector; app: backend }
    SignalSpy { id: backgroundPaints; signalName: "painted" }
    function initTestCase() { initialState = JSON.stringify(state) }
    function init() { backend.mmlBars = 3; backend.mmlRevision = 2; backend.mmlCurrent = 1; backend.mmlMessage = ""; state = JSON.parse(initialState); inspector.width = 1800; inspector.show(); inspector.mode = 2; inspector.refresh() }
    function cleanup() { inspector.hide() }

    function test_modes_and_polyphonic_keyboard() {
        compare(inspector.channels.length, 1)
        compare(inspector.channels[0].notes.length, 2)
        const keyboards = findChild(inspector.contentItem, "channelKeyboards")
        const tracker = findChild(inspector.contentItem, "channelTracker")
        verify(keyboards !== null)
        verify(tracker !== null)
        tryCompare(keyboards, "count", 1)
        compare(inspector.cellText(inspector.rows[0], 0), "C-4 E-4 01 64 CC64 Sustain 127")
        compare(tracker.currentIndex, 0)
        inspector.mode = 1
        compare(keyboards.visible, false)
        inspector.mode = 0
        compare(keyboards.visible, true)
        inspector.mode = 2
        wait(80)
        verify(keyboards.itemAtIndex(0).visible)
        const picture = grabImage(inspector.contentItem)
        verify(picture.width > 600)
        picture.save("/tmp/kog-channel-inspector-qt.png")
    }

    function test_mml_score_highlights_the_playing_bar() {
        inspector.mode = 3
        inspector.refresh()
        const score = findChild(inspector.contentItem, "mmlScore")
        verify(score.visible)
        tryCompare(score, "count", 3)
        compare(findChild(inspector.contentItem, "channelKeyboards").visible, false)
        tryVerify(() => score.itemAtIndex(1) !== null)
        const playing = score.itemAtIndex(1)
        verify(playing.current)
        verify(findChild(playing, "mmlBarText").text.indexOf("background-color") >= 0)
        score.positionViewAtBeginning()
        tryVerify(() => score.itemAtIndex(0) !== null)
        verify(!score.itemAtIndex(0).current)
        // Other bars are fetched once per score revision, not on every refresh.
        const requests = backend.barRequests
        inspector.refresh()
        inspector.refresh()
        compare(backend.barRequests, requests)
        backend.mmlCurrent = 2
        inspector.refresh()
        tryVerify(() => score.itemAtIndex(2) !== null && score.itemAtIndex(2).current)
        grabImage(inspector.contentItem).save("/tmp/kog-channel-inspector-mml-qt.png")
    }

    function test_mml_copy_and_export() {
        inspector.mode = 3
        inspector.refresh()
        const copy = findChild(inspector, "mmlCopyButton")
        verify(copy.visible && copy.enabled)
        copy.clicked()
        compare(inspector.mmlNotice, "Copied the MML score")
        const exportButton = findChild(inspector, "mmlExportButton")
        verify(exportButton.visible && exportButton.enabled)
    }

    function test_mml_guide_opens_with_chapters() {
        inspector.mode = 3
        const button = findChild(inspector, "mmlGuideButton")
        verify(button !== null && button.visible)
        button.clicked()
        const window = inspector.guide
        tryCompare(window, "visible", true)
        compare(window.chapters.length, 2)
        const text = findChild(window.contentItem, "mmlGuideText")
        verify(text.text.indexOf("Kog MML is a notation") >= 0)
        window.chapter = 1
        verify(text.text.indexOf("A note is") >= 0)
        window.hide()
    }

    function test_mml_bars_per_line_control() {
        inspector.mode = 3
        const spin = findChild(inspector.contentItem.parent.parent, "mmlBarsPerLine") || findChild(inspector, "mmlBarsPerLine")
        verify(spin !== null)
        verify(spin.visible)
        spin.value = 2
        spin.valueModified()
        compare(backend.barsPerLine, 2)
        inspector.mode = 2
        verify(!spin.visible)
    }

    function test_mml_follow_never_jumps_back_to_the_top() {
        backend.mmlBars = 200
        backend.mmlRevision = 20
        backend.mmlCurrent = 0
        inspector.mode = 3
        inspector.refresh()
        const score = findChild(inspector.contentItem, "mmlScore")
        tryCompare(score, "count", 200)
        let previous = score.contentY
        for (let block = 1; block < 40; ++block) {
            // Recording progress changes the message on every refresh.
            backend.mmlMessage = "Still recording... " + block
            inspector.refresh()
            wait(5)
            if (block % 3 === 0) {
                backend.mmlCurrent = block / 3
                inspector.refresh()
                wait(20)
            }
            verify(score.contentY >= previous, "scrolled back from " + previous + " to " + score.contentY + " at " + block)
            previous = score.contentY
        }
        verify(previous > 200)
        backend.mmlMessage = ""
        backend.mmlBars = 3
        backend.mmlRevision = 2
        backend.mmlCurrent = 1
    }

    function test_mml_update_keeps_scroll_position() {
        backend.mmlBars = 200
        backend.mmlRevision = 10
        backend.mmlCurrent = -1
        inspector.mode = 3
        inspector.refresh()
        const score = findChild(inspector.contentItem, "mmlScore")
        tryCompare(score, "count", 200)
        score.contentY = 1500
        // A longer partial score arrives while recording continues.
        backend.mmlBars = 220
        backend.mmlRevision = 11
        inspector.refresh()
        // Not even momentarily back at the top.
        compare(score.contentY, 1500)
        tryCompare(score, "count", 220)
        wait(50)
        compare(score.contentY, 1500)
        backend.mmlBars = 3
        backend.mmlRevision = 2
        backend.mmlCurrent = 1
    }

    function test_hidden_view_stops_polling() {
        inspector.hide()
        const before = backend.requests
        wait(120)
        compare(backend.requests, before)
    }

    function test_narrow_window_scrolls_full_width_keyboard() {
        const keyboards = findChild(inspector.contentItem, "channelKeyboards")
        tryCompare(keyboards, "count", 1)
        const piano = findChild(keyboards.itemAtIndex(0), "channelKeyboard")
        verify(piano !== null)
        tryCompare(piano, "width", 1520)
        tryCompare(piano, "height", 64)
        inspector.width = 800
        tryVerify(() => keyboards.contentWidth > keyboards.width)
        compare(piano.width, 1520)
        compare(piano.height, 64)
        keyboards.contentX = keyboards.contentWidth - keyboards.width
        verify(keyboards.contentX > 0)
        wait(80)
        grabImage(inspector.contentItem).save("/tmp/kog-channel-inspector-qt-narrow.png")
        keyboards.contentX = 0
    }

    function test_relative_sample_pitch_has_no_cents_or_bend_marker() {
        state.channels[0].fields.push({name:"Pitch basis", value:"Relative (C4 = normal sample rate)"})
        inspector.refresh()
        const keyboards = findChild(inspector.contentItem, "channelKeyboards")
        tryCompare(keyboards, "count", 1)
        const piano = findChild(keyboards.itemAtIndex(0), "channelKeyboard")
        compare(piano.showPitchOffsets, false)
        compare(inspector.noteText(state.channels[0].notes, false), "C4  E4")
        const highlights = findChild(piano, "channelKeyHighlights")
        tryCompare(highlights, "count", 2)
        const marker = findChild(highlights.itemAt(1), "channelPitchOffset")
        compare(marker.visible, false)
        state.channels[0].fields.pop()
        inspector.refresh()
        compare(piano.showPitchOffsets, true)
        compare(marker.visible, true, "a known MIDI bend remains visible")
    }

    function test_playback_reuses_keyboard_background_and_tracker_rows() {
        const keyboards = findChild(inspector.contentItem, "channelKeyboards")
        tryCompare(keyboards, "count", 1)
        const piano = findChild(keyboards.itemAtIndex(0), "channelKeyboard")
        backgroundPaints.target = findChild(piano, "channelKeyboardBackground")
        wait(80)
        backgroundPaints.clear()
        const rows = inspector.rows
        const highlights = findChild(piano, "channelKeyHighlights")
        const highlight = highlights.itemAt(0)
        for (let i = 0; i < 8; ++i) {
            state.channels[0].notes[0].velocity = 0.2 + i * 0.05
            state.channels[0].notes[0].key = 60 + i % 3
            inspector.refresh()
            wait(20)
        }
        compare(backgroundPaints.count, 0)
        verify(highlights.itemAt(0) === highlight)
        verify(inspector.rows === rows)
        state.rows[0].cells[0].notes = "D-4"
        inspector.refresh()
        compare(inspector.rows[0].cells[0].notes, "D-4")
        backgroundPaints.target = null
    }

    function test_wheel_moves_twenty_four_channels_immediately() {
        const template = state.channels[0]
        state.channels = []
        for (let i = 0; i < 24; ++i) state.channels.push(Object.assign({}, template, {id:i,name:"SPU voice " + i}))
        inspector.mode = 0
        inspector.refresh()
        const keyboards = findChild(inspector.contentItem, "channelKeyboards")
        tryCompare(keyboards, "count", 24)
        keyboards.contentY = 0
        wait(40)
        mouseWheel(keyboards, 400, 100, 0, -120, Qt.NoButton, Qt.NoModifier)
        verify(keyboards.contentY >= 40, "scroll input must move on the input event")
        tryVerify(() => keyboards.contentY >= 119, 250)
        keyboards.contentY = 0
    }
}
