import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.platform as Platform

ApplicationWindow {
    id: root
    required property var app
    title: qsTr("Kog — %1").arg(mode === 3 ? qsTr("MML Score") : qsTr("Channel Inspector"))
    width: Math.min(1800, Screen.desktopAvailableWidth)
    height: 740
    minimumWidth: 640
    minimumHeight: 400
    color: "#10191f"
    palette.window: "#10191f"
    palette.windowText: "#d7e6ed"
    palette.text: "#d7e6ed"
    palette.button: "#253944"
    palette.buttonText: "#d7e6ed"
    palette.base: "#192832"
    palette.highlight: "#50c8ef"
    property var frame: ({ channels: [], rows: [], description: {}, global: [] })
    readonly property var channels: frame.channels || []
    property var rows: []
    property bool follow: true
    property int mode: 2
    readonly property var modeNames: [qsTr("Keyboards"), qsTr("Tracker"), qsTr("Keyboards + tracker"), qsTr("MML score")]
    // Open (or bring forward) the window showing one view.
    function showMode(next) {
        mode = next
        show()
        raise()
        requestActivate()
    }
    // MML score: the playing bar arrives as highlighted rich text, the other
    // bars are fetched once per score revision by their delegates.
    // Each piece of the MML state is its own property so a change in one (the
    // recording progress message ticks every second) does not rebuild the list.
    property string mmlMessage: ""
    property string mmlHeader: ""
    property int mmlRevision: 0
    readonly property var guide: mmlGuide
    // Only these change while a bar plays, so other bars are not re-laid out.
    property int mmlCurrent: -1
    property string mmlCurrentHtml: ""
    function refresh() {
        if (!visible || visibility === Window.Minimized) return
        if (mode === 3) {
            try {
                const next = JSON.parse(app.mml_state())
                if (next.message !== mmlMessage) mmlMessage = next.message
                if ((next.header || "") !== mmlHeader) mmlHeader = next.header || ""
                // Grow or shrink the list in place: replacing the model would
                // reset the view to the top.
                while (mmlModel.count < next.bars) mmlModel.append({})
                if (mmlModel.count > next.bars) mmlModel.remove(next.bars, mmlModel.count - next.bars)
                if (next.revision !== mmlRevision) mmlRevision = next.revision
                if (next.current !== mmlCurrent) mmlCurrent = next.current
                if (next.currentHtml !== mmlCurrentHtml) mmlCurrentHtml = next.currentHtml
            } catch (_) { }
            return
        }
        try {
            const next = JSON.parse(app.channel_snapshot(mode !== 0))
            const nextRows = mode === 0 ? [] : (next.rows || [])
            if (!sameRows(rows, nextRows)) rows = nextRows
            frame = next
        } catch (_) { }
    }
    function sameFields(a, b) {
        if (a.length !== b.length) return false
        for (let i = 0; i < a.length; ++i)
            if (a[i].name !== b[i].name || a[i].value !== b[i].value) return false
        return true
    }
    function sameRows(a, b) {
        if (a.length !== b.length) return false
        for (let i = 0; i < a.length; ++i) {
            const x = a[i], y = b[i]
            if (x.time !== y.time || x.label !== y.label || x.cells.length !== y.cells.length || !sameFields(x.global || [], y.global || [])) return false
            for (let j = 0; j < x.cells.length; ++j) {
                const p = x.cells[j], q = y.cells[j]
                if (p.channel !== q.channel || p.notes !== q.notes || p.instrument !== q.instrument || p.volume !== q.volume || !sameFields(p.effects || [], q.effects || [])) return false
            }
        }
        return true
    }
    function fields(items) { return (items || []).map(f => f.name + " " + f.value).join(" · ") }
    function relativePitch(channel) {
        return (channel.fields || []).some(f => f.name === "Pitch basis" && f.value.indexOf("Relative") === 0)
    }
    function noteText(notes, showOffsets = true) {
        return (notes || []).map(n => {
            const k = Math.round(n.key)
            const names = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"]
            return names[((k % 12) + 12) % 12] + (Math.floor(k / 12) - 1) + (showOffsets && Math.abs(n.key - k) > 0.02 ? " " + Math.round((n.key - k) * 100) + "¢" : "")
        }).join("  ")
    }
    function cellText(row, channel) {
        return (row.cells || []).filter(c => c.channel === channel).map(c =>
            [c.notes, c.instrument, c.volume, fields(c.effects)].filter(v => v).join(" ")).join(" | ")
    }
    // Tracker cells as classic columns: note, instrument, volume, effects.
    // Effects named FX are the tracker's own effect columns; their values
    // stand alone.
    function cellParts(row, channel) {
        const cells = (row.cells || []).filter(c => c.channel === channel)
        const first = key => (cells.find(c => c[key]) || {})[key] || ""
        return [
            cells.map(c => c.notes).filter(v => v).join(" "),
            first("instrument"),
            first("volume"),
            cells.reduce((all, c) => all.concat(c.effects || []), []).map(f => /^FX\d*$/.test(f.name) ? f.value : f.name + " " + f.value).join(" ")
        ]
    }
    // Column widths in characters, per channel: they grow to fit what has
    // been shown (within limits) so columns stay put from row to row.
    readonly property var trackerLimits: [[3, 7], [2, 8], [2, 4], [3, 18]]
    property var trackerWidths: ({})
    property string trackerSource: ""
    function fitTracker() {
        const source = (frame.description && frame.description.backend || "") + "/" + channels.length
        const widths = source === trackerSource ? Object.assign({}, trackerWidths) : {}
        let changed = source !== trackerSource
        for (const row of rows) {
            for (const channel of channels) {
                const parts = cellParts(row, channel.id)
                const current = widths[channel.id] || trackerLimits.map(l => l[0])
                const next = current.map((w, i) => Math.max(w, Math.min(trackerLimits[i][1], parts[i].length)))
                if (next.some((w, i) => w !== current[i]) || !widths[channel.id]) { widths[channel.id] = next; changed = true }
            }
        }
        trackerSource = source
        if (changed) trackerWidths = widths
    }
    function columnChars(channel) { return trackerWidths[channel] || trackerLimits.map(l => l[0]) }
    // One character of the tracker font, and the width of a channel's column.
    readonly property int trackerChar: 6
    function channelWidth(channel) {
        const chars = columnChars(channel)
        return (chars[0] + chars[1] + chars[2] + chars[3] + 3) * trackerChar + 12
    }
    onRowsChanged: fitTracker()
    FontLoader { source: Qt.resolvedUrl("fonts/spleen-6x12.otf") }
    onChannelsChanged: fitTracker()
    onVisibleChanged: if (visible) refresh()
    Timer { interval: 33; repeat: true; running: root.visible && root.visibility !== Window.Minimized; onTriggered: root.refresh() }
    Shortcut { sequence: "Escape"; onActivated: root.hide() }
    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            Label { text: qsTr("Channels"); font.bold: true }
            // Every view is one click away instead of hidden in a drop-down.
            TabBar {
                objectName: "channelInspectorMode"
                currentIndex: root.mode
                Repeater {
                    model: root.modeNames
                    TabButton {
                        required property string modelData
                        required property int index
                        text: modelData
                        width: implicitWidth
                        onClicked: root.mode = index
                    }
                }
            }
            Label { visible: root.mode === 3; text: qsTr("Bars per line") }
            SpinBox {
                objectName: "mmlBarsPerLine"
                visible: root.mode === 3
                from: 1; to: 16; value: 4
                onValueModified: root.app.set_mml_bars_per_line(value)
            }
            Button {
                objectName: "mmlCopyButton"
                visible: root.mode === 3
                text: qsTr("Copy")
                enabled: root.mmlRevision > 0
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Copy the whole MML score")
                onClicked: {
                    mmlClipboard.text = root.app.mml_text()
                    mmlClipboard.selectAll()
                    mmlClipboard.copy()
                    root.mmlNotice = root.mmlMessage.length ? qsTr("Copied the score recorded so far") : qsTr("Copied the MML score")
                }
            }
            Button {
                objectName: "mmlExportButton"
                visible: root.mode === 3
                text: qsTr("Export…")
                enabled: root.mmlRevision > 0
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Save the MML score as a .mml file")
                onClicked: mmlExportDialog.open()
            }
            Button {
                objectName: "mmlGuideButton"
                visible: root.mode === 3
                text: qsTr("Guide")
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Read the Kog MML guide")
                onClicked: { mmlGuide.show(); mmlGuide.raise(); mmlGuide.requestActivate() }
            }
            Item { Layout.fillWidth: true }
            Label { text: root.frame.seeking ? qsTr("Seeking…") : root.frame.playing ? qsTr("Playing") : qsTr("Paused / stopped") }
            Button { text: root.frame.playing ? qsTr("Pause") : qsTr("Play"); onClicked: root.app.play_pause() }
        }
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8
        Label {
            text: root.app.now_title || qsTr("Channel Inspector")
            color: "#edf4f7"; font.pixelSize: 19; font.bold: true
            textFormat: Text.PlainText; elide: Text.ElideRight; Layout.fillWidth: true
        }
        Label {
            objectName: "channelCoverage"
            text: (root.frame.description.backend || "") + " · " + (root.frame.description.detail || qsTr("Play a track to inspect its channels."))
            color: "#a8bdc9"; wrapMode: Text.Wrap; textFormat: Text.PlainText; Layout.fillWidth: true
        }
        Label { text: root.fields(root.frame.global); color: "#83d4bb"; textFormat: Text.PlainText; elide: Text.ElideRight; Layout.fillWidth: true }
        Label {
            objectName: "mmlNotice"
            visible: root.mode === 3 && root.mmlNotice.length > 0
            text: root.mmlNotice
            color: "#83d4bb"; textFormat: Text.PlainText; Layout.fillWidth: true
        }
        Label {
            visible: root.mode === 3 && root.mmlMessage.length > 0
            text: root.mmlMessage
            color: "#a8bdc9"; textFormat: Text.PlainText; Layout.fillWidth: true
        }
        ListView {
            id: mmlBars
            objectName: "mmlScore"
            visible: root.mode === 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 0
            model: ListModel { id: mmlModel }
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { }
            KineticWheelHandler { view: mmlBars }
            readonly property int playing: root.mmlCurrent
            reuseItems: true
            cacheBuffer: height
            onPlayingChanged: if (root.follow && playing >= 0) positionViewAtIndex(playing, ListView.Beginning)
            header: Text {
                width: mmlBars.width
                text: root.mmlHeader
                textFormat: Text.StyledText; wrapMode: Text.Wrap
                color: "#6f8794"; font.family: "monospace"; font.pixelSize: 12
                bottomPadding: 8
            }
            delegate: Rectangle {
                id: mmlBar
                required property int index
                readonly property bool current: index === root.mmlCurrent
                // Re-fetch when a newer partial or final score arrives.
                readonly property string html: root.mmlRevision >= 0 ? root.app.mml_bar(index) : ""
                width: mmlBars.width
                height: barText.implicitHeight + 12
                radius: 0
                color: current ? "#13303b" : index % 2 ? "#111b21" : "#0f171c"
                Rectangle { width: 3; height: parent.height; color: "#50c8ef"; visible: mmlBar.current }
                Text {
                    id: barText
                    objectName: "mmlBarText"
                    x: 8; y: 6
                    width: parent.width - 16
                    text: mmlBar.current && root.mmlCurrentHtml ? root.mmlCurrentHtml : mmlBar.html
                    // StyledText lays out much faster; only the playing bar
                    // needs RichText for its highlight backgrounds.
                    textFormat: mmlBar.current ? Text.RichText : Text.StyledText
                    wrapMode: Text.Wrap
                    color: "#dce3e8"; font.family: "monospace"; font.pixelSize: 12
                }
            }
        }
        SplitView {
            visible: root.mode !== 3
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Vertical
            ListView {
                id: keyboards
                objectName: "channelKeyboards"
                visible: root.mode !== 1
                clip: true
                spacing: 8
                SplitView.fillHeight: root.mode === 0
                SplitView.preferredHeight: root.height * 0.43
                SplitView.minimumHeight: 100
                model: root.mode === 1 ? 0 : root.channels.length
                reuseItems: true
                cacheBuffer: height
                boundsBehavior: Flickable.StopAtBounds
                maximumFlickVelocity: 12000
                flickDeceleration: 2200
                KineticWheelHandler { view: keyboards }
                KineticWheelHandler { view: keyboards; orientation: Qt.Horizontal }
                // Keep the full keyboard readable in narrow windows. All
                // channels share the horizontal scroll position.
                contentWidth: Math.max(width, 1742)
                flickableDirection: Flickable.AutoFlickIfNeeded
                ScrollBar.vertical: ScrollBar { }
                ScrollBar.horizontal: ScrollBar { }
                delegate: Rectangle {
                    required property int index
                    readonly property var channel: root.channels[index] || ({})
                    width: keyboards.contentWidth - 14
                    height: 108
                    radius: 5
                    color: "#192832"
                    RowLayout {
                        anchors.fill: parent; anchors.margins: 8; spacing: 12
                        ColumnLayout {
                            Layout.preferredWidth: 180
                            Layout.minimumWidth: 180
                            Layout.maximumWidth: 180
                            Label { text: channel.name || ""; textFormat: Text.PlainText; color: "#eff5f7"; font.bold: true; elide: Text.ElideRight; Layout.fillWidth: true }
                            Label { text: channel.instrument || channel.kind || ""; textFormat: Text.PlainText; color: "#a8bdc9"; font.pixelSize: 10; elide: Text.ElideRight; Layout.fillWidth: true }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 5
                                Label { text: qsTr("Level"); color: "#a8bdc9"; font.pixelSize: 10 }
                                ProgressBar {
                                    id: channelLevel
                                    objectName: "channelLevelMeter"
                                    from: 0; to: 1
                                    value: Number.isFinite(channel.level) ? Math.max(0, Math.min(1, channel.level)) : 0
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 8
                                    padding: 0
                                    Accessible.name: qsTr("%1 level").arg(channel.name || "")
                                    background: Rectangle { color: "#33434d"; radius: 3 }
                                    contentItem: Item {
                                        Rectangle {
                                            width: parent.width * channelLevel.position
                                            height: parent.height
                                            color: "#50c8ef"
                                            radius: 3
                                        }
                                    }
                                }
                                Label {
                                    text: Math.round(channelLevel.value * 100) + "%"
                                    color: "#a8bdc9"; font.pixelSize: 10
                                    Layout.minimumWidth: 31
                                    horizontalAlignment: Text.AlignRight
                                }
                            }
                            Label { text: root.noteText(channel.notes, !root.relativePitch(channel)) || (channel.active ? (channel.kind || qsTr("Active")).toUpperCase() : "—"); textFormat: Text.PlainText; color: "#50c8ef"; font.pixelSize: 10; elide: Text.ElideRight; Layout.fillWidth: true }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            ChannelKeyboard {
                                objectName: "channelKeyboard"
                                notes: channel.notes || []
                                showPitchOffsets: !root.relativePitch(channel)
                                Layout.minimumWidth: implicitWidth
                                Layout.preferredWidth: implicitWidth
                                Layout.maximumWidth: implicitWidth
                                Layout.preferredHeight: implicitHeight
                                Layout.alignment: Qt.AlignHCenter
                            }
                            Label { text: root.fields(channel.fields); textFormat: Text.PlainText; color: "#91aab8"; font.pixelSize: 10; elide: Text.ElideRight; Layout.fillWidth: true }
                        }
                    }
                }
            }
            ColumnLayout {
                visible: root.mode !== 0
                SplitView.fillHeight: true
                SplitView.minimumHeight: 120
                spacing: 4
                RowLayout {
                    Label { text: qsTr("Tracker · note / instrument / volume / effects"); color: "#b6cbd5"; Layout.fillWidth: true }
                    CheckBox { text: qsTr("Follow playback"); checked: root.follow; onToggled: root.follow = checked }
                }
                // A classic tracker: a small pixel font, one column per
                // channel, and note, instrument, volume and effects each in
                // their own colour.
                Rectangle {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    color: "#070b0e"
                    border.color: "#1d2a32"
                    Flickable {
                        id: trackerHorizontal
                        anchors.fill: parent
                        anchors.margins: 1
                        clip: true
                        readonly property int labelWidth: 13 * root.trackerChar
                        contentWidth: Math.max(width, labelWidth + root.channels.reduce((sum, c) => sum + root.channelWidth(c.id), 0))
                        contentHeight: height
                        flickableDirection: Flickable.HorizontalFlick
                        ScrollBar.horizontal: ScrollBar { }
                        Column {
                            width: trackerHorizontal.contentWidth
                            height: trackerHorizontal.height - 14
                            Rectangle {
                                width: parent.width; height: 20
                                color: "#101a21"
                                Row {
                                    anchors.verticalCenter: parent.verticalCenter
                                    TrackerText { width: trackerHorizontal.labelWidth; height: 20; leftPadding: 6; text: qsTr("ROW"); color: "#6c8494" }
                                    Repeater {
                                        model: root.mode === 0 ? 0 : root.channels.length
                                        Item {
                                            required property int index
                                            readonly property var channel: root.channels[index]
                                            width: root.channelWidth(channel.id); height: 20
                                            Rectangle { width: 1; height: parent.height; color: "#2a3b46" }
                                            TrackerText {
                                                anchors.verticalCenter: parent.verticalCenter
                                                x: 6; width: parent.width - 8
                                                text: String(index + 1).padStart(2, "0") + " " + channel.name
                                                color: "#a9c4d2"
                                            }
                                        }
                                    }
                                }
                            }
                            ListView {
                                id: tracker
                                objectName: "channelTracker"
                                width: parent.width; height: parent.height - 20
                                clip: true
                                model: root.mode === 0 ? 0 : root.rows.length
                                currentIndex: root.frame.current_row === null || root.frame.current_row === undefined ? -1 : root.frame.current_row
                                function followRow() { if (root.follow && currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Center) }
                                onCurrentIndexChanged: followRow()
                                onCountChanged: followRow()
                                Connections { target: root; function onRowsChanged() { tracker.followRow() } }
                                ScrollBar.vertical: ScrollBar { }
                                delegate: Rectangle {
                                    id: trackerRow
                                    required property int index
                                    readonly property var row: root.rows[index] || ({})
                                    readonly property bool current: index === tracker.currentIndex
                                    width: tracker.width
                                    height: row.global && row.global.length ? 28 : 15
                                    color: current ? "#1d4a5e" : index % 4 === 0 ? "#0f171d" : "#070b0e"
                                    Row {
                                        TrackerText {
                                            width: trackerHorizontal.labelWidth; height: 15; leftPadding: 6
                                            text: trackerRow.row.label || ""
                                            color: trackerRow.current ? "#ffffff" : "#d8b85a"
                                        }
                                        Repeater {
                                            model: root.channels.length
                                            Item {
                                                id: cell
                                                required property int index
                                                readonly property int channelId: root.channels[index].id
                                                readonly property var parts: root.cellParts(trackerRow.row, channelId)
                                                readonly property var chars: root.columnChars(channelId)
                                                width: root.channelWidth(channelId); height: 15
                                                Rectangle { width: 1; height: trackerRow.height; color: "#2a3b46" }
                                                Row {
                                                    x: 6
                                                    spacing: root.trackerChar
                                                    Repeater {
                                                        model: 4
                                                        TrackerText {
                                                            required property int index
                                                            width: cell.chars[index] * root.trackerChar
                                                            height: 15
                                                            readonly property bool empty: !cell.parts[index]
                                                            // Empty fields show as dots, like a tracker.
                                                            text: empty ? (index === 0 ? "---" : "..") : cell.parts[index]
                                                            color: empty ? "#33454f" : ["#eef4f7", "#58c8f0", "#8fdc7c", "#e39cf2"][index]
                                                        }
                                                    }
                                                }
                                                HoverHandler { id: cellHover }
                                                ToolTip.visible: cellHover.hovered && cell.parts.some(p => p.length > 0)
                                                ToolTip.text: root.cellText(trackerRow.row, cell.channelId)
                                            }
                                        }
                                    }
                                    TrackerText {
                                        anchors.left: parent.left; anchors.leftMargin: trackerHorizontal.labelWidth + 6
                                        anchors.bottom: parent.bottom; anchors.bottomMargin: 1
                                        text: root.fields(trackerRow.row.global)
                                        color: "#83d4bb"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Label {
            visible: root.mode !== 3 && root.channels.length === 0
            text: root.frame.seeking ? qsTr("Waiting for the decoder to finish seeking…") : qsTr("No channel data at this position.")
            color: "#a8bdc9"; Layout.fillWidth: true
        }
        Label {
            visible: (root.frame.dropped_frames || 0) > 0
            text: qsTr("%1 older snapshots skipped while the view was idle.").arg(root.frame.dropped_frames || 0)
            color: "#dfba78"; font.pixelSize: 10
        }
    }
    MmlGuide { id: mmlGuide; app: root.app }
    // Copies the score to the system clipboard.
    TextEdit { id: mmlClipboard; visible: false }
    property string mmlNotice: ""
    Timer { interval: 4000; running: root.mmlNotice.length > 0; onTriggered: root.mmlNotice = "" }
    Platform.FileDialog {
        id: mmlExportDialog
        title: qsTr("Export MML score")
        fileMode: Platform.FileDialog.SaveFile
        defaultSuffix: "mml"
        nameFilters: [qsTr("Kog MML (*.mml)"), qsTr("All files (*)")]
        currentFile: Platform.StandardPaths.writableLocation(Platform.StandardPaths.DocumentsLocation)
            + "/" + ((root.app.now_title || "score").replace(/[\\/:*?"<>|]/g, "_")) + ".mml"
        onAccepted: {
            const error = root.app.export_mml(file.toString())
            root.mmlNotice = error.length ? error : qsTr("Exported %1").arg(decodeURIComponent(file.toString().replace(/^file:\/\//, "")))
        }
    }
}
