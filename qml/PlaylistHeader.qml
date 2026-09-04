pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

Rectangle {
    id: root

    required property var theme
    required property var app
    property string savedLayout: ""
    property string sortColumn: "index"
    property bool sortAscending: true
    property real availableWidth: width
    property var columns: []
    property bool layoutReady: false
    property bool adjustingColumns: false
    property string menuColumn: "index"

    readonly property var visibleColumns: columns.filter(column => column.visible)
    readonly property real totalWidth: {
        let total = 0
        for (const column of visibleColumns)
            total += column.width
        return total
    }

    signal sortRequested(string column)
    signal columnLayoutChanged(string layout)

    implicitHeight: 30
    implicitWidth: totalWidth
    color: theme.window
    border.color: theme.mid
    clip: true

    FontMetrics {
        id: columnFontMetrics
        font.pixelSize: 11
    }

    function makeColumn(identifier, label, menuLabel, width, minimumWidth,
            maximumWidth, visible, alignment, flexible) {
        return {
            "id": identifier,
            "label": label,
            "menuLabel": menuLabel,
            "width": width,
            "minimumWidth": minimumWidth,
            "maximumWidth": maximumWidth,
            "visible": visible,
            "alignment": alignment,
            "flexible": flexible
        }
    }

    function defaultColumns() {
        return [
            makeColumn("index", "#", qsTr("Index"), 54, 28, 64, true,
                Text.AlignRight, false),
            makeColumn("status", "", qsTr("Status"), 38, 38, 38, true,
                Text.AlignHCenter, false),
            makeColumn("rating", qsTr("Rating"), qsTr("Rating"), 78, 48, 128, true,
                Text.AlignLeft, false),
            makeColumn("title", qsTr("Title"), qsTr("Title"), 220, 96, 1024, true,
                Text.AlignLeft, true),
            makeColumn("albumartist", qsTr("Album Artist"), qsTr("Album Artist"),
                150, 96, 1024, false, Text.AlignLeft, true),
            makeColumn("artist", qsTr("Artist"), qsTr("Artist"), 190, 96, 1024, true,
                Text.AlignLeft, true),
            makeColumn("composer", qsTr("Composer"), qsTr("Composer"), 151, 96,
                1024, false, Text.AlignLeft, true),
            makeColumn("album", qsTr("Album"), qsTr("Album"), 220, 96, 1024, true,
                Text.AlignLeft, true),
            makeColumn("length", qsTr("Length"), qsTr("Length"), 70, 44, 160, true,
                Text.AlignRight, false),
            makeColumn("date", qsTr("Year"), qsTr("Year"), 58, 42, 160, true,
                Text.AlignRight, false),
            makeColumn("genre", qsTr("Genre"), qsTr("Genre"), 120, 48, 512, true,
                Text.AlignLeft, true),
            makeColumn("track", "№", qsTr("Track"), 54, 32, 96, true,
                Text.AlignRight, false),
            makeColumn("playcount", qsTr("Plays"), qsTr("Play Count"), 71, 42, 120,
                false, Text.AlignRight, false),
            makeColumn("path", qsTr("Path"), qsTr("Path"), 180, 64, 2048, false,
                Text.AlignLeft, true),
            makeColumn("filename", qsTr("Filename"), qsTr("Filename"), 180, 64,
                1024, false, Text.AlignLeft, true),
            makeColumn("codec", qsTr("Codec"), qsTr("Codec"), 80, 48, 1024, false,
                Text.AlignLeft, false),
            makeColumn("samplerate", qsTr("Sample Rate"), qsTr("Sample Rate"), 92,
                64, 1024, false, Text.AlignRight, false),
            makeColumn("bitspersample", qsTr("Bits"), qsTr("Bits Per Sample"), 64,
                48, 1024, false, Text.AlignRight, false),
            makeColumn("bitrate", qsTr("Bitrate"), qsTr("Bitrate"), 84, 56, 1024,
                false, Text.AlignRight, false)
        ]
    }

    function copyColumn(column) {
        return makeColumn(column.id, column.label, column.menuLabel, column.width,
            column.minimumWidth, column.maximumWidth, column.visible,
            column.alignment, column.flexible)
    }

    function columnIndex(identifier) {
        for (let index = 0; index < columns.length; ++index) {
            if (columns[index].id === identifier)
                return index
        }
        return -1
    }

    function columnVisible(identifier) {
        const index = columnIndex(identifier)
        return index >= 0 && columns[index].visible
    }

    function setColumnWidth(identifier, width) {
        const index = columnIndex(identifier)
        if (index < 0)
            return
        const updated = columns.map(copyColumn)
        updated[index].width = Math.max(updated[index].minimumWidth,
            Math.min(updated[index].maximumWidth, width))
        columns = updated
    }

    function fitAvailableWidth() {
        if (adjustingColumns || availableWidth <= 0 || columns.length === 0)
            return
        let currentWidth = 0
        for (const column of columns) {
            if (column.visible)
                currentWidth += column.width
        }
        let remaining = availableWidth - currentWidth
        if (remaining <= 0.5)
            return

        const updated = columns.map(copyColumn)
        adjustingColumns = true
        for (let pass = 0; pass < 8 && remaining > 0.5; ++pass) {
            const candidates = updated.filter(column => column.visible
                && column.flexible && column.width < column.maximumWidth - 0.5)
            if (candidates.length === 0)
                break
            const share = remaining / candidates.length
            let consumed = 0
            for (const column of candidates) {
                const added = Math.min(share, column.maximumWidth - column.width)
                column.width += added
                consumed += added
            }
            remaining -= consumed
            if (consumed <= 0.5)
                break
        }
        columns = updated
        adjustingColumns = false
    }

    function restoreLayout() {
        const defaults = defaultColumns()
        const value = savedLayout.trim()
        if (value.length === 0) {
            columns = defaults
            return
        }

        if (value.indexOf(";") >= 0) {
            const entries = value.split(";")
            const restored = []
            const seen = []
            let valid = entries.length === defaults.length
            let visibleCount = 0
            for (let entryIndex = 0; valid && entryIndex < entries.length; ++entryIndex) {
                const fields = entries[entryIndex].split(",")
                const defaultIndex = fields.length === 3
                    ? defaults.findIndex(column => column.id === fields[0].trim())
                    : -1
                const width = fields.length === 3 ? Number(fields[1]) : Number.NaN
                const visibility = fields.length === 3 ? fields[2].trim() : ""
                valid = defaultIndex >= 0 && seen.indexOf(defaultIndex) < 0
                    && Number.isFinite(width) && (visibility === "0" || visibility === "1")
                if (!valid)
                    break
                const column = copyColumn(defaults[defaultIndex])
                column.width = Math.max(column.minimumWidth,
                    Math.min(column.maximumWidth, width))
                column.visible = visibility === "1"
                visibleCount += column.visible ? 1 : 0
                seen.push(defaultIndex)
                restored.push(column)
            }
            if (valid && visibleCount > 0) {
                columns = restored
                return
            }
        } else {
            const widths = value.split(",").map(width => Number(width.trim()))
            const legacyIdentifiers = ["index", "rating", "title", "artist", "album",
                "length", "date", "genre", "track"]
            let valid = widths.length === legacyIdentifiers.length
            for (let index = 0; valid && index < widths.length; ++index)
                valid = Number.isFinite(widths[index])
            if (valid) {
                for (let index = 0; index < legacyIdentifiers.length; ++index) {
                    const target = defaults.find(column =>
                        column.id === legacyIdentifiers[index])
                    target.width = Math.max(target.minimumWidth,
                        Math.min(target.maximumWidth, widths[index]))
                }
                columns = defaults
                return
            }
        }
        columns = defaults
    }

    function encodedLayout() {
        return columns.map(column => column.id + "," + column.width.toFixed(2)
            + "," + (column.visible ? "1" : "0")).join(";")
    }

    function persistLayout() {
        columnLayoutChanged(encodedLayout())
    }

    function toggleColumn(identifier) {
        const index = columnIndex(identifier)
        if (index < 0)
            return
        if (columns[index].visible && visibleColumns.length === 1)
            return
        const updated = columns.map(copyColumn)
        updated[index].visible = !updated[index].visible
        columns = updated
        fitAvailableWidth()
        persistLayout()
    }

    function visiblePosition(identifier) {
        for (let index = 0; index < visibleColumns.length; ++index) {
            if (visibleColumns[index].id === identifier)
                return index
        }
        return -1
    }

    function canMoveColumn(identifier, direction) {
        const position = visiblePosition(identifier)
        return position >= 0 && position + direction >= 0
            && position + direction < visibleColumns.length
    }

    function moveColumn(identifier, direction) {
        if (!canMoveColumn(identifier, direction))
            return
        const position = visiblePosition(identifier)
        const neighborIdentifier = visibleColumns[position + direction].id
        const sourceIndex = columnIndex(identifier)
        const neighborIndex = columnIndex(neighborIdentifier)
        const updated = columns.map(copyColumn)
        const temporary = updated[sourceIndex]
        updated[sourceIndex] = updated[neighborIndex]
        updated[neighborIndex] = temporary
        columns = updated
        persistLayout()
    }

    function autoFitColumn(identifier) {
        const index = columnIndex(identifier)
        if (index < 0)
            return
        const column = columns[index]
        let contentWidth = columnFontMetrics.advanceWidth(
            column.label.length > 0 ? column.label + "  ▼" : "▶")
        for (let row = 0; row < root.app.playlist_count; ++row) {
            contentWidth = Math.max(contentWidth, columnFontMetrics.advanceWidth(
                root.app.track_value_at(row, identifier)))
        }
        setColumnWidth(identifier, contentWidth + 18)
        persistLayout()
    }

    function resetLayout() {
        columns = defaultColumns()
        fitAvailableWidth()
        persistLayout()
    }

    Component.onCompleted: {
        restoreLayout()
        layoutReady = true
        fitAvailableWidth()
    }
    onSavedLayoutChanged: if (layoutReady && savedLayout !== encodedLayout()) {
        restoreLayout()
        fitAvailableWidth()
    }
    onAvailableWidthChanged: if (layoutReady) fitAvailableWidth()

    component HeaderCell: Rectangle {
        id: cell

        required property var column

        width: column.width
        height: root.height
        color: headerHover.hovered ? root.theme.button : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("Sort by %1").arg(column.menuLabel)

        Rectangle {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: parent.height * 0.56
            color: root.theme.mid
        }

        Text {
            anchors.fill: parent
            anchors.leftMargin: 7
            anchors.rightMargin: 7
            text: cell.column.label + (root.sortColumn === cell.column.id
                ? (root.sortAscending ? "  ▲" : "  ▼")
                : "")
            color: root.theme.buttonText
            font.pixelSize: 11
            font.bold: root.sortColumn === cell.column.id
            horizontalAlignment: cell.column.alignment
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        HoverHandler { id: headerHover }

        MouseArea {
            anchors.fill: parent
            anchors.rightMargin: 5
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton) {
                    root.menuColumn = cell.column.id
                    columnMenu.popup()
                } else {
                    root.sortRequested(cell.column.id)
                }
            }
        }

        MouseArea {
            id: separatorDrag

            property real previousX: 0

            z: 2
            width: 9
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.horizontalCenter: parent.right
            acceptedButtons: Qt.LeftButton
            cursorShape: Qt.SplitHCursor
            preventStealing: true
            onPressed: mouse => {
                previousX = mapToItem(root, mouse.x, mouse.y).x
                mouse.accepted = true
            }
            onPositionChanged: mouse => {
                if (!pressed)
                    return
                const currentX = mapToItem(root, mouse.x, mouse.y).x
                root.setColumnWidth(cell.column.id,
                    cell.column.width + currentX - previousX)
                previousX = currentX
            }
            onReleased: root.persistLayout()
            onDoubleClicked: mouse => {
                root.autoFitColumn(cell.column.id)
                mouse.accepted = true
            }
        }
    }

    Row {
        anchors.fill: parent

        Repeater {
            model: root.visibleColumns

            HeaderCell {
                required property var modelData
                column: modelData
            }
        }
    }

    Menu {
        id: columnMenu

        MenuItem {
            text: qsTr("Move Column Left")
            icon.name: "go-previous"
            enabled: root.canMoveColumn(root.menuColumn, -1)
            onTriggered: root.moveColumn(root.menuColumn, -1)
        }
        MenuItem {
            text: qsTr("Move Column Right")
            icon.name: "go-next"
            enabled: root.canMoveColumn(root.menuColumn, 1)
            onTriggered: root.moveColumn(root.menuColumn, 1)
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Album")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("album")
            onTriggered: root.toggleColumn("album")
        }
        MenuItem {
            text: qsTr("Album Artist")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("albumartist")
            onTriggered: root.toggleColumn("albumartist")
        }
        MenuItem {
            text: qsTr("Artist")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("artist")
            onTriggered: root.toggleColumn("artist")
        }
        MenuItem {
            text: qsTr("Bitrate")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("bitrate")
            onTriggered: root.toggleColumn("bitrate")
        }
        MenuItem {
            text: qsTr("Bits Per Sample")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("bitspersample")
            onTriggered: root.toggleColumn("bitspersample")
        }
        MenuItem {
            text: qsTr("Codec")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("codec")
            onTriggered: root.toggleColumn("codec")
        }
        MenuItem {
            text: qsTr("Composer")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("composer")
            onTriggered: root.toggleColumn("composer")
        }
        MenuItem {
            text: qsTr("Filename")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("filename")
            onTriggered: root.toggleColumn("filename")
        }
        MenuItem {
            text: qsTr("Genre")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("genre")
            onTriggered: root.toggleColumn("genre")
        }
        MenuItem {
            text: qsTr("Index")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("index")
            onTriggered: root.toggleColumn("index")
        }
        MenuItem {
            text: qsTr("Length")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("length")
            onTriggered: root.toggleColumn("length")
        }
        MenuItem {
            text: qsTr("Path")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("path")
            onTriggered: root.toggleColumn("path")
        }
        MenuItem {
            text: qsTr("Play Count")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("playcount")
            onTriggered: root.toggleColumn("playcount")
        }
        MenuItem {
            text: qsTr("Rating")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("rating")
            onTriggered: root.toggleColumn("rating")
        }
        MenuItem {
            text: qsTr("Sample Rate")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("samplerate")
            onTriggered: root.toggleColumn("samplerate")
        }
        MenuItem {
            text: qsTr("Status")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("status")
            onTriggered: root.toggleColumn("status")
        }
        MenuItem {
            text: qsTr("Title")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("title")
            onTriggered: root.toggleColumn("title")
        }
        MenuItem {
            text: qsTr("Track")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("track")
            onTriggered: root.toggleColumn("track")
        }
        MenuItem {
            text: qsTr("Year")
            icon.name: checked ? "view-visible" : "view-hidden"
            checkable: true
            checked: root.columnVisible("date")
            onTriggered: root.toggleColumn("date")
        }
        MenuSeparator {}
        MenuItem {
            text: qsTr("Reset Columns")
            icon.name: "edit-undo"
            onTriggered: root.resetLayout()
        }
    }
}
