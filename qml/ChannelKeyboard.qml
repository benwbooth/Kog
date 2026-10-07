import QtQuick

Item {
    id: root
    property var notes: []
    property int firstKey: 0
    property int lastKey: 127
    property color accent: "#50c8ef"
    property bool showPitchOffsets: true
    implicitWidth: 1520
    implicitHeight: 64
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Active keys: %1").arg(notes.map(n => Math.round(n.key)).join(", ") || qsTr("none"))
    function black(key) { return [1, 3, 6, 8, 10].indexOf(((key % 12) + 12) % 12) >= 0 }
    function whiteIndex(key) {
        let count = 0
        for (let k = firstKey; k < key; ++k) if (!black(k)) count++
        return count
    }
    readonly property int whiteCount: whiteIndex(lastKey + 1)
    readonly property real keyWidth: Math.min(width / Math.max(1, whiteCount), height * 1520 / (75 * 64))
    readonly property real keyHeight: keyWidth * 75 * 64 / 1520
    readonly property real leftInset: (width - keyWidth * whiteCount) / 2
    readonly property real topInset: (height - keyHeight) / 2

    // Keys and octave labels are static. Playback only updates a few scene
    // graph rectangles; it never repaints or uploads this canvas each tick.
    Canvas {
        id: background
        objectName: "channelKeyboardBackground"
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        Connections {
            target: root
            function onFirstKeyChanged() { background.requestPaint() }
            function onLastKeyChanged() { background.requestPaint() }
        }
        onPaint: {
            const ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            const w = root.keyWidth
            const h = root.keyHeight
            ctx.save()
            ctx.translate(root.leftInset, root.topInset)
            ctx.lineWidth = 1
            let white = 0
            for (let k = root.firstKey; k <= root.lastKey; ++k) {
                if (root.black(k)) continue
                ctx.fillStyle = "#dde4e8"
                ctx.fillRect(white * w, 0, w - 0.5, h)
                ctx.strokeStyle = "#33434d"
                ctx.strokeRect(white * w, 0, w, h)
                white++
            }
            white = 0
            for (let k = root.firstKey; k <= root.lastKey; ++k) {
                if (!root.black(k)) { white++; continue }
                ctx.fillStyle = "#18242d"
                ctx.fillRect(white * w - w * 0.31, 0, w * 0.62, h * 0.62)
                ctx.strokeStyle = "#0c141b"
                ctx.strokeRect(white * w - w * 0.31, 0, w * 0.62, h * 0.62)
            }
            ctx.restore()
        }
    }
    Repeater {
        objectName: "channelKeyHighlights"
        model: root.notes.length
        Item {
            id: highlight
            objectName: "channelKeyHighlight"
            required property int index
            readonly property var note: root.notes[index] || ({key: -1, held: false})
            readonly property int key: Math.round(note.key)
            readonly property bool blackKey: root.black(key)
            readonly property color fill: note.held ? root.accent : "#83d4bb"
            visible: key >= root.firstKey && key <= root.lastKey
            x: root.leftInset + (root.whiteIndex(key) - (blackKey ? 0.31 : 0)) * root.keyWidth
            y: root.topInset
            width: root.keyWidth * (blackKey ? 0.62 : 1)
            height: root.keyHeight * (blackKey ? 0.62 : 1)
            // The top of a white key fits between its black neighbors.
            Rectangle {
                x: !highlight.blackKey && root.black(highlight.key - 1) ? root.keyWidth * 0.31 + 1 : 1
                y: 1
                width: highlight.blackKey ? parent.width - 2 : root.keyWidth - x - (root.black(highlight.key + 1) ? root.keyWidth * 0.31 + 1 : 1)
                height: highlight.blackKey ? parent.height - 2 : root.keyHeight * 0.62
                color: highlight.fill
            }
            Rectangle {
                visible: !highlight.blackKey
                x: 1; y: root.keyHeight * 0.62 + 1
                width: parent.width - 2; height: parent.height - y - 1
                color: highlight.fill
            }
            Rectangle {
                objectName: "channelPitchOffset"
                visible: root.showPitchOffsets && Math.abs(highlight.note.key - highlight.key) > 0.02
                color: "#e66b33"
                x: parent.width * (0.5 + highlight.note.key - highlight.key) - 1
                y: parent.height * 0.7
                width: 2; height: parent.height * 0.22
            }
        }
    }
    // Labels sit above the highlights and are never redrawn during playback.
    Repeater {
        model: Math.max(0, Math.floor(root.lastKey / 12) - Math.ceil(root.firstKey / 12) + 1)
        Text {
            required property int index
            readonly property int key: (Math.ceil(root.firstKey / 12) + index) * 12
            x: root.leftInset + root.whiteIndex(key) * root.keyWidth + 1
            y: root.topInset + root.keyHeight - height - 1
            width: root.keyWidth - 3
            text: "C" + (Math.floor(key / 12) - 1)
            color: "#32424e"
            font.pixelSize: 12
            minimumPixelSize: 6
            fontSizeMode: Text.HorizontalFit
        }
    }
}
