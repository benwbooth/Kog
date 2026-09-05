import QtQuick
import QtQuick.Controls

Label {
    id: label
    required property string sourceText
    required property string query
    required property var searchModel
    property bool wholeQuery: false
    readonly property bool highlighting: query.trim().length > 0

    // Normal browsing keeps the lightweight plain-text rendering path.
    text: highlighting ? searchModel.highlightedName(sourceText, query, metrics.elidedText, wholeQuery)
                       : metrics.elidedText
    textFormat: highlighting ? Text.RichText : Text.PlainText
    wrapMode: Text.NoWrap
    clip: true
    Accessible.name: sourceText

    TextMetrics {
        id: metrics
        text: label.sourceText
        font: label.font
        elide: Text.ElideRight
        elideWidth: Math.max(0, label.width - label.leftPadding - label.rightPadding)
    }
}
