import QtQuick

Canvas {
    id: root
    required property var app
    property bool active: visible
    property bool waveform: false
    property var frame: ({ wave: [], spectrum: [] })
    property var displayed: []
    property color backgroundColor: "#10191f"
    property color lowColor: "#42dfa3"
    property color highColor: "#45baff"

    function updateFrame() {
        frame = JSON.parse(app.visualizer_frame())
        const next = []
        for (let i = 0; i < frame.spectrum.length; ++i)
            next.push(Math.max(frame.spectrum[i], (displayed[i] || 0) * 0.86))
        displayed = next
        requestPaint()
    }
    onWaveformChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    Timer { interval: 33; repeat: true; running: root.active; onTriggered: root.updateFrame() }
    onPaint: {
        const ctx = getContext("2d")
        ctx.reset()
        ctx.fillStyle = backgroundColor
        ctx.fillRect(0, 0, width, height)
        const padding = Math.min(24, height * 0.08)
        const w = width - padding * 2
        const h = height - padding * 2
        ctx.strokeStyle = "#24323b"
        ctx.lineWidth = 1
        for (let i = 1; i < 4; ++i) {
            const y = padding + h * i / 4
            ctx.beginPath(); ctx.moveTo(padding, y); ctx.lineTo(width - padding, y); ctx.stroke()
        }
        const gradient = ctx.createLinearGradient(0, height, width, 0)
        gradient.addColorStop(0, lowColor)
        gradient.addColorStop(1, highColor)
        if (waveform) {
            const wave = frame.wave
            if (wave.length < 2) return
            ctx.strokeStyle = gradient
            ctx.lineWidth = 2
            ctx.beginPath()
            for (let i = 0; i < wave.length; ++i) {
                const x = padding + w * i / (wave.length - 1)
                const y = height / 2 - wave[i] * h * 0.48
                if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y)
            }
            ctx.stroke()
        } else {
            ctx.fillStyle = gradient
            const stride = w / Math.max(1, displayed.length)
            for (let i = 0; i < displayed.length; ++i) {
                const bar = displayed[i] * h
                ctx.fillRect(padding + i * stride, height - padding - bar,
                             Math.max(1, stride - Math.min(3, stride * 0.22)), bar)
            }
        }
    }
}
