pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Builds a custom effect from blocks. The patch is a list of stages run top
// to bottom; a stage is a block, parallel paths (each path runs on the input
// and the paths are summed), or a feedback loop. LFOs and envelope followers
// can move any block setting. The patch is shown as an indented list.
ApplicationWindow {
    id: root
    // { blocks, modulators, templates } from the effects catalog.
    property var catalog: ({ blocks: [], modulators: [], templates: [] })
    property var draft: ({ name: "", modulators: [], stages: [] })
    property string originalName: ""
    property var takenNames: []
    signal saved(var patch, string originalName)

    title: qsTr("Custom effect")
    width: 860
    height: 760
    minimumWidth: 560
    minimumHeight: 420

    function edit(patch, taken) {
        draft = JSON.parse(JSON.stringify(patch))
        originalName = patch.name
        takenNames = taken.filter(name => name !== patch.name)
        show()
        raise()
        requestActivate()
    }
    function changed() { draft = Object.assign({}, draft) }
    function blockInfo(kind) { return catalog.blocks.find(b => b.kind === kind) || { label: kind, params: [] } }
    function modulatorInfo(kind) { return catalog.modulators.find(m => m.kind === kind) || { label: kind, params: [] } }
    function newBlock(kind) {
        const info = blockInfo(kind)
        const params = {}
        for (const spec of info.params) params[spec.id] = spec.default
        return { block: kind, enabled: true, params: params, modulations: [] }
    }
    function newModulator(kind) {
        const params = {}
        for (const spec of modulatorInfo(kind).params) params[spec.id] = spec.default
        return { kind: kind, params: params }
    }
    // Every stage as a row, with the list it lives in so rows can edit it.
    readonly property var rows: {
        const out = []
        const walk = (list, depth) => {
            for (let i = 0; i < list.length; ++i) {
                const stage = list[i]
                if (stage.block) out.push({ type: "block", stage: stage, list: list, index: i, depth: depth })
                else if (stage.parallel) {
                    out.push({ type: "parallel", stage: stage, list: list, index: i, depth: depth })
                    for (let b = 0; b < stage.parallel.length; ++b) {
                        out.push({ type: "branch", stage: stage, branch: stage.parallel[b], list: stage.parallel, index: b, depth: depth + 1 })
                        walk(stage.parallel[b].stages, depth + 2)
                    }
                } else if (stage.stages) {
                    out.push({ type: "feedback", stage: stage, list: list, index: i, depth: depth })
                    walk(stage.stages, depth + 1)
                }
            }
        }
        walk(draft.stages, 0)
        return out
    }
    readonly property string nameProblem: !draft.name || !draft.name.trim() ? qsTr("Give the effect a name")
        : takenNames.indexOf(draft.name.trim()) >= 0 ? qsTr("Another custom effect has this name") : ""

    // A menu that adds a block, parallel paths or a feedback loop to a list.
    component AddMenu: Menu {
        id: addMenu
        property var target: []
        Repeater {
            model: root.catalog.blocks
            MenuItem {
                required property var modelData
                text: modelData.label
                onTriggered: { addMenu.target.push(root.newBlock(modelData.kind)); root.changed() }
            }
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Parallel paths")
            onTriggered: { addMenu.target.push({ parallel: [{ level: 1, stages: [] }, { level: 1, stages: [] }] }); root.changed() }
        }
        MenuItem {
            text: qsTr("Feedback loop")
            onTriggered: { addMenu.target.push({ feedback: 0.5, stages: [] }); root.changed() }
        }
    }

    // One setting: a slider, or a list for a setting with named choices,
    // plus what moves it.
    component ParamRow: RowLayout {
        id: paramRow
        property var spec: ({})
        property var owner: ({})
        property bool modulatable: false
        readonly property real value: owner.params[spec.id] !== undefined ? owner.params[spec.id] : spec.default
        readonly property var modulation: (owner.modulations || []).find(m => m.param === spec.id)
        spacing: 8
        Label { text: paramRow.spec.label; Layout.preferredWidth: 120; elide: Text.ElideRight }
        ComboBox {
            visible: paramRow.spec.choices && paramRow.spec.choices.length > 0
            Layout.fillWidth: true
            model: paramRow.spec.choices || []
            currentIndex: Math.round(paramRow.value)
            onActivated: index => { paramRow.owner.params[paramRow.spec.id] = index; root.changed() }
        }
        Slider {
            visible: !paramRow.spec.choices || paramRow.spec.choices.length === 0
            Layout.fillWidth: true
            from: paramRow.spec.min; to: paramRow.spec.max; stepSize: paramRow.spec.step
            value: paramRow.value
            onMoved: { paramRow.owner.params[paramRow.spec.id] = value; root.changed() }
        }
        Label {
            visible: !paramRow.spec.choices || paramRow.spec.choices.length === 0
            text: paramRow.value.toFixed(paramRow.spec.step >= 1 ? 0 : paramRow.spec.step >= 0.1 ? 1 : 2) + (paramRow.spec.unit ? " " + paramRow.spec.unit : "")
            Layout.preferredWidth: 80
            horizontalAlignment: Text.AlignRight
        }
        ComboBox {
            visible: paramRow.modulatable && (!paramRow.spec.choices || paramRow.spec.choices.length === 0) && root.draft.modulators.length > 0
            Layout.preferredWidth: 150
            model: [qsTr("Fixed")].concat(root.draft.modulators.map((m, i) => qsTr("%1 %2").arg(m.kind === "lfo" ? qsTr("LFO") : qsTr("Envelope")).arg(i + 1)))
            currentIndex: paramRow.modulation ? paramRow.modulation.source + 1 : 0
            onActivated: index => {
                const list = paramRow.owner.modulations = (paramRow.owner.modulations || []).filter(m => m.param !== paramRow.spec.id)
                if (index > 0) list.push({ param: paramRow.spec.id, source: index - 1, depth: 0.2 })
                root.changed()
            }
        }
        Slider {
            visible: !!paramRow.modulation
            Layout.preferredWidth: 110
            from: -1; to: 1; stepSize: 0.01
            value: paramRow.modulation ? paramRow.modulation.depth : 0
            ToolTip.visible: hovered
            ToolTip.text: qsTr("How far it moves: %1% of the range").arg(Math.round(value * 100))
            onMoved: { paramRow.modulation.depth = value; root.changed() }
        }
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.margins: 6
            Label { text: qsTr("Name") }
            TextField {
                objectName: "patchName"
                Layout.fillWidth: true
                text: root.draft.name
                onTextEdited: { root.draft.name = text; root.changed() }
            }
            Button {
                text: qsTr("Add")
                onClicked: topMenu.open()
                AddMenu { id: topMenu; target: root.draft.stages }
            }
            Button {
                text: qsTr("Add LFO")
                enabled: root.draft.modulators.length < 8
                onClicked: { root.draft.modulators.push(root.newModulator("lfo")); root.changed() }
            }
            Button {
                text: qsTr("Add envelope")
                enabled: root.draft.modulators.length < 8
                onClicked: { root.draft.modulators.push(root.newModulator("envelope")); root.changed() }
            }
        }
    }

    ScrollView {
        anchors.fill: parent
        contentWidth: availableWidth
        ColumnLayout {
            width: parent.width - 24
            x: 12
            spacing: 10

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 10
                wrapMode: Text.Wrap
                opacity: 0.75
                text: qsTr("Blocks run top to bottom. Parallel paths each take the input and are added together at their own levels; an empty path passes the input through. A feedback loop sends its output back to its input. LFOs and envelope followers can move any setting.")
            }

            Repeater {
                model: root.draft.modulators.length
                GroupBox {
                    id: modulatorBox
                    required property int index
                    readonly property var modulator: root.draft.modulators[index]
                    Layout.fillWidth: true
                    title: root.modulatorInfo(modulator.kind).label + " " + (index + 1)
                    ColumnLayout {
                        anchors.fill: parent
                        Repeater {
                            model: root.modulatorInfo(modulatorBox.modulator.kind).params
                            ParamRow { required property var modelData; spec: modelData; owner: modulatorBox.modulator; Layout.fillWidth: true }
                        }
                        Button {
                            text: qsTr("Remove")
                            onClicked: {
                                const gone = modulatorBox.index
                                root.draft.modulators.splice(gone, 1)
                                // Settings it moved are fixed again; later sources move up.
                                const fix = list => { for (const stage of list) {
                                    if (stage.block) stage.modulations = (stage.modulations || []).filter(m => m.source !== gone).map(m => Object.assign(m, { source: m.source > gone ? m.source - 1 : m.source }))
                                    if (stage.parallel) for (const branch of stage.parallel) fix(branch.stages)
                                    if (stage.feedback !== undefined) fix(stage.stages)
                                } }
                                fix(root.draft.stages)
                                root.changed()
                            }
                        }
                    }
                }
            }

            Label {
                visible: root.draft.stages.length === 0
                text: qsTr("Empty. Use Add to put in a block, parallel paths or a feedback loop.")
                opacity: 0.7
            }

            Repeater {
                model: root.rows
                Frame {
                    id: rowFrame
                    required property var modelData
                    readonly property var row: modelData
                    Layout.fillWidth: true
                    Layout.leftMargin: row.depth * 22
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 4
                        RowLayout {
                            CheckBox {
                                visible: rowFrame.row.type === "block"
                                checked: rowFrame.row.stage.enabled !== false
                                onToggled: { rowFrame.row.stage.enabled = checked; root.changed() }
                            }
                            Label {
                                font.bold: true
                                Layout.fillWidth: true
                                text: rowFrame.row.type === "block" ? root.blockInfo(rowFrame.row.stage.block).label
                                    : rowFrame.row.type === "parallel" ? qsTr("Parallel paths")
                                    : rowFrame.row.type === "branch" ? qsTr("Path %1").arg(rowFrame.row.index + 1)
                                    : qsTr("Feedback loop")
                            }
                            Button {
                                visible: rowFrame.row.type !== "block"
                                text: rowFrame.row.type === "parallel" ? qsTr("Add path") : qsTr("Add inside")
                                onClicked: {
                                    if (rowFrame.row.type === "parallel") { rowFrame.row.stage.parallel.push({ level: 1, stages: [] }); root.changed() }
                                    else innerMenu.open()
                                }
                                AddMenu { id: innerMenu; target: rowFrame.row.type === "branch" ? rowFrame.row.branch.stages : (rowFrame.row.stage.stages || []) }
                            }
                            ToolButton {
                                text: "▲"
                                enabled: rowFrame.row.index > 0
                                onClicked: { const list = rowFrame.row.list; list.splice(rowFrame.row.index - 1, 0, list.splice(rowFrame.row.index, 1)[0]); root.changed() }
                            }
                            ToolButton {
                                text: "▼"
                                enabled: rowFrame.row.index < rowFrame.row.list.length - 1
                                onClicked: { const list = rowFrame.row.list; list.splice(rowFrame.row.index + 1, 0, list.splice(rowFrame.row.index, 1)[0]); root.changed() }
                            }
                            ToolButton {
                                text: "✕"
                                onClicked: { rowFrame.row.list.splice(rowFrame.row.index, 1); root.changed() }
                            }
                        }
                        RowLayout {
                            visible: rowFrame.row.type === "branch" || rowFrame.row.type === "feedback"
                            Label { text: rowFrame.row.type === "branch" ? qsTr("Level") : qsTr("Amount"); Layout.preferredWidth: 120 }
                            Slider {
                                Layout.fillWidth: true
                                from: 0; to: rowFrame.row.type === "branch" ? 2 : 0.98; stepSize: 0.01
                                value: rowFrame.row.type === "branch" ? rowFrame.row.branch.level : (rowFrame.row.stage.feedback || 0)
                                onMoved: {
                                    if (rowFrame.row.type === "branch") rowFrame.row.branch.level = value
                                    else rowFrame.row.stage.feedback = value
                                    root.changed()
                                }
                            }
                        }
                        Repeater {
                            model: rowFrame.row.type === "block" ? root.blockInfo(rowFrame.row.stage.block).params : []
                            ParamRow { required property var modelData; spec: modelData; owner: rowFrame.row.stage; modulatable: true; Layout.fillWidth: true }
                        }
                    }
                }
            }
            Item { Layout.preferredHeight: 10 }
        }
    }

    footer: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.margins: 6
            Label { text: root.nameProblem; color: "#d9534f"; Layout.fillWidth: true }
            Button { text: qsTr("Cancel"); onClicked: root.close() }
            Button {
                objectName: "patchSave"
                text: qsTr("Save")
                highlighted: true
                enabled: root.nameProblem.length === 0
                onClicked: {
                    root.draft.name = root.draft.name.trim()
                    root.saved(JSON.parse(JSON.stringify(root.draft)), root.originalName)
                    root.close()
                }
            }
        }
    }
}
