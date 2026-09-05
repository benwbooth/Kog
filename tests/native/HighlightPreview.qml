import QtQuick
import QtQuick.Controls
import "../../qml"

Window {
    width: 600
    height: 380
    visible: true
    color: "#242629"
    QtObject {
        id: playlistFixture
        property int playlist_revision: 0
        property int current_index: -1
        property string playback_state: "stopped"
        function track_number_at(row) { return row + 1 }
        function track_status_message_at(row) { return "" }
        function track_value_at(row, column) {
            return { title: "Duck Tales Theme", artist: "Theme Band", album: "Theme Collection" }[column]
        }
    }
    Column {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 8
        Repeater {
            model: [
                { name: "Duck Tales — Theme / THEME.mid", query: "theme", bg: "#242629", fg: "#eff0f1", width: 560 },
                { name: "Duck Tales — Theme / THEME.mid", query: "theme", bg: "#3daee9", fg: "#ffffff", width: 560 },
                { name: "日本語 テーマ 日本語.mid", query: "日本語", bg: "#242629", fg: "#eff0f1", width: 560 },
                { name: "Ｔｈｅｍｅ ｶﾞ Café 🎵.mid", query: "theme ガ café 🎵", bg: "#242629", fg: "#eff0f1", width: 560 },
                { name: "The Theme.mid", query: "theme", bg: "#242629", fg: "#eff0f1", width: 78 },
                { name: "Theme <& Theme>.mid", query: "theme", bg: "#fafafa", fg: "#232629", width: 560 },
                { name: "Theme <& Theme>.mid", query: "", bg: "#242629", fg: "#eff0f1", width: 560 }
            ]
            Rectangle {
                required property var modelData
                width: 568
                height: 30
                color: modelData.bg
                SearchHighlightLabel {
                    x: 4
                    anchors.verticalCenter: parent.verticalCenter
                    width: modelData.width
                    font.pixelSize: 14
                    sourceText: modelData.name
                    query: modelData.query
                    searchModel: testModel
                    color: modelData.fg
                }
            }
        }
        Repeater {
            model: 2
            PlaylistRow {
                required property int index
                width: 568
                rowIndex: index
                app: playlistFixture
                searchModel: testModel
                searchQuery: "theme"
                selected: index === 1
                theme: ({ highlight: "#3daee9", highlightedText: "#ffffff", text: "#eff0f1",
                          button: "#35383b", alternateBase: "#2c3034" })
                columns: ({ visibleColumns: [
                    { id: "title", width: 200, alignment: Text.AlignLeft },
                    { id: "artist", width: 168, alignment: Text.AlignLeft },
                    { id: "album", width: 200, alignment: Text.AlignLeft }
                ] })
            }
        }
    }
}
