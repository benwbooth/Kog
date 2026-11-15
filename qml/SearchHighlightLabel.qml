import QtQuick
import QtQuick.Controls

Label {
    id: label
    required property string sourceText
    required property string query
    required property var searchModel
    property bool wholeQuery: false
    // Paths and filenames run to the edge and are cut there, never "…".
    property bool clipText: false
    // Wraps onto as many lines as the text needs, so none is cut off.
    property bool wrap: false
    readonly property bool highlighting: query.trim().length > 0
    // Only highlighted search results need pre-elision for rich text. Plain
    // browsing lets Text render and elide the filename in one pass.
    readonly property bool elided: wrap ? false : clipText
        ? metrics.advanceWidth > width - leftPadding - rightPadding + 0.5
        : highlighting ? metrics.elidedText !== sourceText : truncated

    text: highlighting ? searchModel.highlightedName(sourceText, query,
                             clipText || wrap ? sourceText : metrics.elidedText, wholeQuery)
                       : sourceText
    textFormat: highlighting ? Text.RichText : Text.PlainText
    elide: highlighting || clipText || wrap ? Text.ElideNone : Text.ElideRight
    clip: clipText
    wrapMode: wrap ? Text.WrapAtWordBoundaryOrAnywhere : Text.NoWrap
    // No clip: plain text elides itself and highlighted text is elided above
    // before it is set. A clip per label made each of a list's hundreds of
    // cells its own draw call, which slowed every frame while scrolling.
    Accessible.name: sourceText

    TextMetrics {
        id: metrics
        text: label.highlighting || label.clipText ? label.sourceText : ""
        font: label.font
        elide: Text.ElideRight
        elideWidth: label.highlighting && !label.clipText
            ? Math.max(0, label.width - label.leftPadding - label.rightPadding)
            : 0
    }
}
