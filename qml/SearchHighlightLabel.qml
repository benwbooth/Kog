import QtQuick
import QtQuick.Controls

Label {
    id: label
    required property string sourceText
    required property string query
    required property var searchModel
    property bool wholeQuery: false
    readonly property bool highlighting: query.trim().length > 0
    // Only highlighted search results need pre-elision for rich text. Plain
    // browsing lets Text render and elide the filename in one pass.
    readonly property bool elided: highlighting
        ? metrics.elidedText !== sourceText : truncated

    text: highlighting ? searchModel.highlightedName(sourceText, query, metrics.elidedText, wholeQuery)
                       : sourceText
    textFormat: highlighting ? Text.RichText : Text.PlainText
    elide: highlighting ? Text.ElideNone : Text.ElideRight
    wrapMode: Text.NoWrap
    // No clip: plain text elides itself and highlighted text is elided above
    // before it is set. A clip per label made each of a list's hundreds of
    // cells its own draw call, which slowed every frame while scrolling.
    Accessible.name: sourceText

    TextMetrics {
        id: metrics
        text: label.highlighting ? label.sourceText : ""
        font: label.font
        elide: Text.ElideRight
        elideWidth: label.highlighting
            ? Math.max(0, label.width - label.leftPadding - label.rightPadding)
            : 0
    }
}
