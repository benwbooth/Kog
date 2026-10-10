import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    name: "PatchEditor"
    when: windowShown

    Kog.PatchEditor {
        id: editor
        catalog: ({
            blocks: [
                { kind: "filter", label: "Filter", params: [
                    { id: "type", label: "Type", min: 0, max: 1, default: 0, step: 1, unit: "", choices: ["Low-pass", "High-pass"] },
                    { id: "frequency", label: "Frequency", min: 20, max: 20000, default: 1000, step: 1, unit: "Hz", choices: [] }] },
                { kind: "gain", label: "Gain", params: [{ id: "gain", label: "Gain", min: -60, max: 24, default: 0, step: 0.1, unit: "dB", choices: [] }] }
            ],
            modulators: [{ kind: "lfo", label: "LFO", params: [{ id: "rate", label: "Rate", min: 0.01, max: 20, default: 0.5, step: 0.01, unit: "Hz", choices: [] }] }],
            templates: []
        })
    }
    SignalSpy { id: savedSpy; target: editor; signalName: "saved" }

    function test_edits_nested_stages_and_saves() {
        editor.edit({ name: "Mine", modulators: [], stages: [] }, ["Mine", "Other"])
        compare(editor.nameProblem, "")
        editor.draft.stages.push(editor.newBlock("filter"))
        editor.draft.stages.push({ parallel: [{ level: 1, stages: [editor.newBlock("gain")] }, { level: 0.5, stages: [] }] })
        editor.draft.modulators.push(editor.newModulator("lfo"))
        editor.changed()
        // filter, parallel, path 1, gain, path 2
        compare(editor.rows.length, 5)
        compare(editor.rows[3].type, "block")
        compare(editor.rows[3].depth, 2)
        editor.draft.name = "Other"
        editor.changed()
        verify(editor.nameProblem.length > 0)
        editor.draft.name = "Renamed"
        editor.changed()
        const save = findChild(editor.footer, "patchSave")
        verify(save.enabled)
        save.clicked()
        compare(savedSpy.count, 1)
        const patch = savedSpy.signalArguments[0][0]
        compare(patch.name, "Renamed")
        compare(savedSpy.signalArguments[0][1], "Mine")
        compare(patch.stages[1].parallel[1].level, 0.5)
        compare(patch.modulators.length, 1)
    }
}
