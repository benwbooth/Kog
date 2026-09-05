import QtQuick

Canvas {
    id: root
    required property var app
    property bool active: visible
    property bool waveform: false
    // Keep waveform for the classic player's small scope and existing callers.
    property string mode: waveform ? "waveform" : "spectrum"
    property var frame: ({ wave: [], spectrum: [] })
    property var displayed: []
    property var spectrumHistory: []
    property var waveHistory: []
    readonly property int historyLimit: 96
    readonly property var heatColors: {
        const colors = []
        for (let i = 0; i < 64; ++i) {
            const t = i / 63
            colors.push(Qt.rgba(0.03 + 0.22 * t, 0.09 + 0.78 * t,
                               0.14 + 0.68 * Math.sin(t * Math.PI / 2), 1))
        }
        return colors
    }
    property color backgroundColor: "#10191f"
    property color lowColor: "#42dfa3"
    property color highColor: "#45baff"

    function updateFrame() {
        frame = JSON.parse(app.visualizer_frame())
        const next = []
        for (let i = 0; i < frame.spectrum.length; ++i)
            next.push(Math.max(frame.spectrum[i], (displayed[i] || 0) * 0.86))
        displayed = next
        if (mode === "spectrogram") {
            spectrumHistory.push(frame.spectrum.slice())
            if (spectrumHistory.length > historyLimit) spectrumHistory.shift()
        } else if (mode === "trails") {
            waveHistory.push(frame.wave.slice())
            if (waveHistory.length > 10) waveHistory.shift()
        }
        requestPaint()
    }
    function clearHistory() {
        spectrumHistory = []
        waveHistory = []
    }
    onModeChanged: { clearHistory(); requestPaint() }
    onActiveChanged: if (!active) clearHistory()
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
        if (mode === "spectrogram") {
            const columns = spectrumHistory.length
            const bands = frame.spectrum.length
            const dx = w / historyLimit
            const dy = h / Math.max(1, bands)
            for (let column = 0; column < columns; ++column) {
                const values = spectrumHistory[column]
                for (let band = 0; band < values.length; ++band) {
                    ctx.fillStyle = heatColors[Math.round(Math.max(0, Math.min(1, values[band])) * 63)]
                    ctx.fillRect(padding + (historyLimit - columns + column) * dx,
                                 height - padding - (band + 1) * dy, Math.ceil(dx), Math.ceil(dy))
                }
            }
            return
        }
        if (mode === "radial") {
            const cx = width / 2
            const cy = height / 2
            const radius = Math.min(w, h) * 0.21
            const extension = Math.min(w, h) * 0.25
            ctx.lineCap = "round"
            ctx.lineWidth = Math.max(1, Math.min(6, radius * 0.07))
            const count = displayed.length
            for (let i = 0; i < count; ++i) {
                const angle = i * Math.PI * 2 / count - Math.PI / 2
                const strength = displayed[i]
                if (strength < 0.004) continue
                const t = i / Math.max(1, count - 1)
                ctx.strokeStyle = Qt.rgba(lowColor.r * (1-t) + highColor.r * t,
                    lowColor.g * (1-t) + highColor.g * t, lowColor.b * (1-t) + highColor.b * t, 1)
                ctx.beginPath()
                ctx.moveTo(cx + Math.cos(angle) * radius, cy + Math.sin(angle) * radius)
                ctx.lineTo(cx + Math.cos(angle) * (radius + extension * strength),
                           cy + Math.sin(angle) * (radius + extension * strength))
                ctx.stroke()
            }
            ctx.strokeStyle = "#24323b"
            ctx.lineWidth = 1
            ctx.beginPath(); ctx.arc(cx, cy, radius * 0.82, 0, Math.PI * 2); ctx.stroke()
            return
        }
        ctx.strokeStyle = "#24323b"
        ctx.lineWidth = 1
        for (let i = 1; i < 4; ++i) {
            const y = padding + h * i / 4
            ctx.beginPath(); ctx.moveTo(padding, y); ctx.lineTo(width - padding, y); ctx.stroke()
        }
        const gradient = ctx.createLinearGradient(0, height, width, 0)
        gradient.addColorStop(0, lowColor)
        gradient.addColorStop(1, highColor)
        if (mode === "waveform" || mode === "trails") {
            const wave = frame.wave
            if (wave.length < 2) return
            ctx.strokeStyle = gradient
            ctx.lineWidth = 2
            const traces = mode === "trails" ? waveHistory : [wave]
            for (let age = 0; age < traces.length; ++age) {
                const trace = traces[age]
                ctx.globalAlpha = Math.pow((age + 1) / traces.length, 2) * (age === traces.length - 1 ? 1 : 0.4)
                ctx.beginPath()
                for (let i = 0; i < trace.length; ++i) {
                    const x = padding + w * i / (trace.length - 1)
                    const y = height / 2 - trace[i] * h * 0.48
                    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y)
                }
                ctx.stroke()
            }
            ctx.globalAlpha = 1
        } else {
            ctx.fillStyle = gradient
            const stride = w / Math.max(1, displayed.length)
            for (let i = 0; i < displayed.length; ++i) {
                const bar = displayed[i] * h
                const barWidth = Math.max(1, stride - Math.min(3, stride * 0.22))
                if (mode === "mirrored") {
                    ctx.fillRect(padding + i * stride, height / 2 - bar / 2, barWidth, bar / 2)
                    ctx.globalAlpha = 0.3
                    ctx.fillRect(padding + i * stride, height / 2, barWidth, bar / 2)
                    ctx.globalAlpha = 1
                } else {
                    ctx.fillRect(padding + i * stride, height - padding - bar, barWidth, bar)
                }
            }
        }
    }
}
