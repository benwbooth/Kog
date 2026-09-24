import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    id: testCase
    name: "PlaylistAutoFit"
    width: 900
    height: 120
    visible: true
    when: windowShown

    readonly property string longTitle: "W".repeat(220)

    QtObject {
        id: app
        property int playlist_count: 2
        property int playlist_revision: 0
        property int current_index: -1
        property string playback_state: "stopped"
        function track_status_message_at() { return "" }
        function track_missing_at() { return false }
        function track_number_at(row) { return String(row + 1) }
        function track_value_at(row, column) {
            if (column === "title")
                return row === 0 ? testCase.longTitle : "Short title"
            return column === "artist" ? "Artist" : ""
        }
    }
    QtObject {
        id: theme
        property color window: "#ffffff"
        property color mid: "#cccccc"
        property color button: "#eeeeee"
        property color buttonText: "#111111"
        property color base: "#ffffff"
        property color alternateBase: "#f8f8f8"
        property color highlight: "#438cf5"
        property color highlightedText: "#ffffff"
        property color placeholderText: "#999999"
        property color text: "#111111"
    }
    QtObject {
        id: searchModel
        function highlightedName(source) { return source }
        function icon_name() { return "kog-format-audio" }
    }

    Kog.PlaylistHeader {
        id: header
        width: 900
        theme: theme
        app: app
    }
    Kog.SearchHighlightLabel {
        id: titleLabel
        y: 36
        width: header.columnIndex("title") >= 0
            ? header.effectiveColumnWidth(
                header.columns[header.columnIndex("title")]) : 0
        height: 24
        leftPadding: 26
        rightPadding: 6
        sourceText: testCase.longTitle
        query: ""
        searchModel: searchModel
        font.pixelSize: 11
    }
    Kog.PlaylistRow {
        id: playlistRow
        y: 70
        width: 900
        app: app
        rowIndex: 0
        columns: header
        theme: theme
        searchModel: searchModel
    }
    SignalSpy { id: columnsSpy; target: header; signalName: "columnsChanged" }
    SignalSpy { id: layoutSpy; target: header; signalName: "columnLayoutChanged" }

    function test_auto_fit_all_updates_once_and_shows_long_title() {
        columnsSpy.clear()
        layoutSpy.clear()
        header.autoFitAllColumns()
        compare(columnsSpy.count, 1)
        compare(layoutSpy.count, 1)

        const titleWidth = header.columns[header.columnIndex("title")].width
        verify(titleWidth > 1024, "Long titles must fit beyond the old 1024 px cap")
        compare(titleLabel.width, titleWidth)
        compare(titleLabel.elided, false)

        const savedWidth = titleWidth
        header.savedLayout = header.encodedLayout()
        header.restoreLayout()
        compare(header.columns[header.columnIndex("title")].width, savedWidth)
        compare(titleLabel.elided, false)
    }

    function test_manual_resize_updates_cells_without_rebuilding_columns() {
        header.resetLayout()
        wait(0)
        const title = header.columns[header.columnIndex("title")]
        const titleCell = findChild(playlistRow, "playlistCell_title")
        verify(titleCell !== null)
        let edge = 0
        for (const column of header.visibleColumns) {
            edge += column.width
            if (column.id === "title")
                break
        }
        const originalTotalWidth = header.totalWidth

        columnsSpy.clear()
        layoutSpy.clear()
        mousePress(header, edge - 1, 15)
        mouseMove(header, edge + 39, 15)
        compare(header.resizingWidth, title.width + 40)
        compare(titleLabel.width, title.width + 40)
        compare(titleCell.width, title.width + 40)
        compare(findChild(playlistRow, "playlistCell_title"), titleCell)
        compare(playlistRow.columnAt(edge + 20).id, "title")
        compare(header.totalWidth, originalTotalWidth + 40)
        compare(header.columns[header.columnIndex("title")].width, title.width)
        compare(columnsSpy.count, 0, "Dragging must not recreate the column model")
        compare(layoutSpy.count, 0, "Dragging must not persist each pointer move")

        mouseMove(header, edge + 59, 15)
        compare(titleLabel.width, title.width + 60)
        compare(header.totalWidth, originalTotalWidth + 60)
        compare(columnsSpy.count, 0)

        mouseRelease(header, edge + 59, 15)
        compare(columnsSpy.count, 1)
        compare(layoutSpy.count, 1)
        compare(header.columns[header.columnIndex("title")].width, title.width + 60)
        compare(header.resizingColumn, "")
        compare(titleCell.width, title.width + 60)
    }

    function test_saved_layout_gains_size_columns() {
        header.resetLayout()
        const oldLayout = header.columns
            .filter(column => column.id !== "filesize" && column.id !== "filesizebytes")
            .map(column => column.id + "," + column.width + ","
                + (column.visible ? "1" : "0"))
            .join(";")
        header.savedLayout = oldLayout
        header.restoreLayout()
        compare(header.columns.length, 22)
        compare(header.columns[header.columnIndex("title")].width, 220)
        compare(header.columnVisible("filesize"), true)
        compare(header.columnVisible("filesizebytes"), false)
        header.resetLayout()
    }
}
