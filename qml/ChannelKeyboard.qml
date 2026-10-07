import QtQuick

Canvas {
    id: root
    property var notes: []
    property int firstKey: 0
    property int lastKey: 127
    property color accent: "#50c8ef"
    implicitHeight: 48
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Active keys: %1").arg(notes.map(n => Math.round(n.key)).join(", ") || qsTr("none"))
    onNotesChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onFirstKeyChanged: requestPaint()
    onLastKeyChanged: requestPaint()
    function black(key) { return [1, 3, 6, 8, 10].indexOf(((key % 12) + 12) % 12) >= 0 }
    function note(key) { return notes.find(n => Math.round(n.key) === key) }
    onPaint: {
        const ctx = getContext("2d")
        ctx.clearRect(0, 0, width, height)
        let whites = 0
        for (let k = firstKey; k <= lastKey; ++k) if (!black(k)) whites++
        const w = width / Math.max(1, whites)
        let white = 0
        ctx.lineWidth = 1
        for (let k = firstKey; k <= lastKey; ++k) {
            if (black(k)) continue
            const active = note(k)
            ctx.fillStyle = active ? (active.held ? accent : "#83d4bb") : "#dde4e8"
            ctx.fillRect(white * w, 0, w - 0.5, height)
            ctx.strokeStyle = "#33434d"
            ctx.strokeRect(white * w, 0, w, height)
            if (k % 12 === 0 && w >= 7) {
                ctx.fillStyle = "#32424e"
                ctx.font = "8px sans-serif"
                ctx.fillText("C" + (Math.floor(k / 12) - 1), white * w + 1, height - 3)
            }
            white++
        }
        white = 0
        for (let k = firstKey; k <= lastKey; ++k) {
            if (!black(k)) { white++; continue }
            const active = note(k)
            ctx.fillStyle = active ? (active.held ? accent : "#83d4bb") : "#18242d"
            ctx.fillRect(white * w - w * 0.31, 0, w * 0.62, height * 0.62)
            ctx.strokeStyle = "#0c141b"
            ctx.strokeRect(white * w - w * 0.31, 0, w * 0.62, height * 0.62)
        }
        // Fractional pitch stays visible during vibrato and slides.
        for (const n of notes) {
            const key = Math.round(n.key)
            const delta = n.key - key
            if (key < firstKey || key > lastKey || Math.abs(delta) < 0.02) continue
            let x = 0
            for (let k = firstKey; k < key; ++k) if (!black(k)) x++
            ctx.fillStyle = "#e66b33"
            ctx.fillRect((x + (black(key) ? 0 : 0.5) + delta) * w - 1, height * 0.7, 2, height * 0.22)
        }
    }
}
