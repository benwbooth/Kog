import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    id: testCase
    name: "PlaylistAutoFit"
    width: 900
    height: 120
    when: windowShown

    readonly property string longTitle: "W".repeat(220)

    QtObject {
        id: app
        property int playlist_count: 2
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
    }
    QtObject {
        id: searchModel
        function highlightedName(source) { return source }
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
            ? header.columns[header.columnIndex("title")].width : 0
        height: 24
        leftPadding: 26
        rightPadding: 6
        sourceText: testCase.longTitle
        query: ""
        searchModel: searchModel
        font.pixelSize: 11
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
}
