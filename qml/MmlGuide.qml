import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The Kog MML guide: chapters on the left, the chapter's text on the right.
ApplicationWindow {
    id: root
    required property var app
    title: qsTr("Kog MML Guide")
    width: 1100
    height: 760
    minimumWidth: 560
    minimumHeight: 360
    color: "#10191f"
    palette.window: "#10191f"
    palette.windowText: "#d7e6ed"
    palette.text: "#d7e6ed"
    palette.base: "#192832"
    palette.highlight: "#50c8ef"
    property var chapters: []
    property int chapter: 0
    onVisibleChanged: if (visible && chapters.length === 0) {
        try { chapters = JSON.parse(app.mml_guide()) } catch (_) { chapters = [] }
    }
    Shortcut { sequence: "Escape"; onActivated: root.hide() }
    Shortcut { sequences: ["Ctrl+PgDown", "Alt+Right"]; onActivated: root.chapter = Math.min(root.chapters.length - 1, root.chapter + 1) }
    Shortcut { sequences: ["Ctrl+PgUp", "Alt+Left"]; onActivated: root.chapter = Math.max(0, root.chapter - 1) }
    RowLayout {
        anchors.fill: parent
        spacing: 0
        ListView {
            id: contents
            objectName: "mmlGuideChapters"
            Layout.preferredWidth: 260
            Layout.fillHeight: true
            clip: true
            model: root.chapters.length
            currentIndex: root.chapter
            header: Label {
                width: contents.width
                text: qsTr("Kog MML")
                font.pixelSize: 20; font.bold: true
                color: "#edf4f7"
                padding: 16
            }
            delegate: ItemDelegate {
                required property int index
                width: contents.width
                text: root.chapters[index].title
                highlighted: index === root.chapter
                onClicked: root.chapter = index
            }
            ScrollBar.vertical: ScrollBar { }
        }
        Rectangle { Layout.fillHeight: true; width: 1; color: "#253944" }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            ScrollView {
                id: page
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                TextEdit {
                    id: body
                    objectName: "mmlGuideText"
                    width: page.availableWidth
                    padding: 28
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    textFormat: TextEdit.MarkdownText
                    color: "#dce3e8"
                    selectionColor: "#2e6f86"
                    font.pixelSize: 15
                    text: root.chapters.length ? root.chapters[root.chapter].markdown : ""
                    onTextChanged: page.ScrollBar.vertical.position = 0
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 10
                Button {
                    text: qsTr("‹ Previous")
                    enabled: root.chapter > 0
                    onClicked: root.chapter -= 1
                }
                Item { Layout.fillWidth: true }
                Label {
                    text: root.chapters.length ? qsTr("%1 of %2").arg(root.chapter + 1).arg(root.chapters.length) : ""
                    color: "#a8bdc9"
                }
                Item { Layout.fillWidth: true }
                Button {
                    text: qsTr("Next ›")
                    enabled: root.chapter < root.chapters.length - 1
                    onClicked: root.chapter += 1
                }
            }
        }
    }
}
