import QtQuick

Canvas {
    id: root
    property var notes: []
    property int firstKey: 0
    property int lastKey: 127
    property color accent: "#50c8ef"
    implicitWidth: 1520
    implicitHeight: 64
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
        // Fit the same 1520:64 keyboard proportions as the web canvas.
        const w = Math.min(width / Math.max(1, whites), height * 1520 / (75 * 64))
        const keyHeight = w * 75 * 64 / 1520
        ctx.save()
        ctx.translate((width - w * whites) / 2, (height - keyHeight) / 2)
        let white = 0
        ctx.lineWidth = 1
        for (let k = firstKey; k <= lastKey; ++k) {
            if (black(k)) continue
            const active = note(k)
            ctx.fillStyle = active ? (active.held ? accent : "#83d4bb") : "#dde4e8"
            ctx.fillRect(white * w, 0, w - 0.5, keyHeight)
            ctx.strokeStyle = "#33434d"
            ctx.strokeRect(white * w, 0, w, keyHeight)
            white++
        }
        white = 0
        for (let k = firstKey; k <= lastKey; ++k) {
            if (!black(k)) { white++; continue }
            const active = note(k)
            ctx.fillStyle = active ? (active.held ? accent : "#83d4bb") : "#18242d"
            ctx.fillRect(white * w - w * 0.31, 0, w * 0.62, keyHeight * 0.62)
            ctx.strokeStyle = "#0c141b"
            ctx.strokeRect(white * w - w * 0.31, 0, w * 0.62, keyHeight * 0.62)
        }
        // Draw labels after the keys so the next key cannot paint over them.
        // Fit even C-1 inside its white key without clipping the octave.
        white = 0
        ctx.font = "12px sans-serif"
        ctx.fillStyle = "#32424e"
        for (let k = firstKey; k <= lastKey; ++k) {
            if (black(k)) continue
            if (k % 12 === 0 && w >= 7) {
                const label = "C" + (Math.floor(k / 12) - 1)
                const labelWidth = ctx.measureText(label).width
                ctx.save()
                ctx.translate(white * w + 1, keyHeight - 3)
                ctx.scale(Math.min(1, (w - 3) / Math.max(1, labelWidth)), 1)
                ctx.fillText(label, 0, 0)
                ctx.restore()
            }
            white++
        }
        // Fractional pitch stays visible during vibrato and slides.
        for (const n of notes) {
            const key = Math.round(n.key)
            const delta = n.key - key
            if (key < firstKey || key > lastKey || Math.abs(delta) < 0.02) continue
            let x = 0
            for (let k = firstKey; k < key; ++k) if (!black(k)) x++
            ctx.fillStyle = "#e66b33"
            ctx.fillRect((x + (black(key) ? 0 : 0.5) + delta) * w - 1, keyHeight * 0.7, 2, keyHeight * 0.22)
        }
        ctx.restore()
    }
}
