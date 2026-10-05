import QtQuick
import QtCore

// The legacy Settings object has no persisted properties and is read-only.
// Each changed property is committed through the shared SQLite backend.
QtObject {
    id: root
    required property var app
    property string category: ""
    property string fileName: ""
    property url location: fileName.length ? "file://" + fileName : ""
    property var values: ({})
    property bool ready: false
    property var saved: ({})
    property Settings legacy: Settings { category: root.category; location: root.location }

    function key(name) { return "qml/" + fileName + "/" + category + "/" + name }
    function setValue(name, value) {
        if (!ready) return
        const encoded = JSON.stringify(value)
        if (saved[name] === encoded) return
        if (app.save_ui_setting(key(name), encoded)) saved[name] = encoded
    }
    function saveChanges() {
        if (!ready) return
        const current = values
        for (const name of Object.keys(current)) setValue(name, current[name])
    }
    onValuesChanged: saveChanges()
    Component.onCompleted: {
        for (const name of Object.keys(values)) {
            const fallback = JSON.stringify(legacy.value(name, values[name]))
            const encoded = app.load_ui_setting(key(name), fallback)
            root[name] = JSON.parse(encoded)
            saved[name] = encoded
        }
        ready = true
    }
    Component.onDestruction: saveChanges()
}
