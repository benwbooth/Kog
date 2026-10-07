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
    property var state: ({version:1, description:{backend:"Test MIDI",kind:"events",detail:"Sequenced keys and commands"}, playing:true, seeking:false, current_row:0,
        channels:[{id:0,name:"MIDI 1",kind:"tonal",instrument:"Piano",level:0.8,pan:0,active:true,notes:[{key:60,velocity:0.8,held:true},{key:64.3,velocity:0.6,held:false}],fields:[{name:"Sustain",value:"On"}]}],
        rows:[{time:0,label:"0000.000",cells:[{channel:0,notes:"C-4 E-4",instrument:"01",volume:"64",effects:[{name:"CC64 Sustain",value:"127"}]}],global:[{name:"Tempo",value:"120 BPM"}]}],global:[]})
    QtObject {
        id: backend
        property string now_title: "Inspection fixture"
        property int requests: 0
        function channel_snapshot() { requests++; return JSON.stringify(test.state) }
        function play_pause() { test.state = Object.assign({}, test.state, {playing:!test.state.playing}) }
    }
    Kog.ChannelInspector { id: inspector; app: backend }
    function init() { inspector.show(); inspector.mode = 2; inspector.refresh() }
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

    function test_hidden_view_stops_polling() {
        inspector.hide()
        const before = backend.requests
        wait(120)
        compare(backend.requests, before)
    }
}
