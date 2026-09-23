import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml.Models

// Trusted native UI. Neither this model nor its paths are exposed to WebChannel.
Pane {
    id: root
    required property var app
    required property var libraryModel
    property string selectedPath: ""
    required property var skinStyle
    padding: 2
    font.pixelSize: 12
    palette.text: skinStyle.text
    palette.windowText: skinStyle.text
    palette.buttonText: skinStyle.headerText
    palette.base: skinStyle.background
    palette.window: skinStyle.background
    palette.highlight: skinStyle.selection
    palette.highlightedText: skinStyle.selectionText
    background: Rectangle { color: root.skinStyle.background; border.color: root.skinStyle.frame }
    objectName: "modernLibraryPanel"
    clip: true

    function refreshRoot() {
        selectedPath = ""
        if (libraryModel && app) libraryModel.setRootPath(app.directory_path)
    }
    Component.onCompleted: refreshRoot()
    onLibraryModelChanged: refreshRoot()
    Connections {
        target: root.app
        function onDirectory_pathChanged() { root.refreshRoot() }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 2
        RowLayout {
            Layout.fillWidth: true
            Label { text: qsTr("Media Library — Local Files"); color: root.skinStyle.text; font.bold: true; Layout.fillWidth: true }
            LibraryButton {
                text: qsTr("Folder…")
                Accessible.name: qsTr("Choose music folder")
                onClicked: { root.app.choose_music_folder(); root.refreshRoot() }
            }
        }
        TextField {
            id: search
            objectName: "modernLibrarySearch"
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            color: root.skinStyle.text
            placeholderTextColor: root.skinStyle.text
            leftPadding: 4
            rightPadding: 4
            topPadding: 2
            bottomPadding: 2
            background: Rectangle { color: root.skinStyle.background; border.color: root.skinStyle.frame }
            placeholderText: qsTr("Search files, folders and archives…")
            maximumLength: 200
            selectByMouse: true
            onTextChanged: { root.selectedPath = ""; searchDebounce.restart() }
        }
        Timer {
            id: searchDebounce
            interval: 180
            onTriggered: if (root.libraryModel) root.libraryModel.searchText = search.text
        }
        TreeSearchLayout { id: searchLayout; view: tree; model: root.libraryModel }
        TreeView {
            id: tree
            objectName: "modernLibraryTree"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: root.libraryModel
            rootIndex: root.libraryModel ? root.libraryModel.viewRootIndex : undefined
            reuseItems: true
            rowHeightProvider: function(row) { return 20 }
            contentHeight: rows * 20
            opacity: searchLayout.ready ? 1 : 0
            enabled: opacity === 1
            columnWidthProvider: function(column) { return Math.max(0, width - 14) }
            selectionModel: ItemSelectionModel { model: root.libraryModel }
            selectionBehavior: TableView.SelectRows
            selectionMode: TableView.SingleSelection
            pointerNavigationEnabled: false
            delegate: TreeViewDelegate {
                id: treeDelegate
                required property string fileName
                required property string filePath
                required property string fileIcon
                readonly property bool customIcon: fileIcon.startsWith("kog-format-")
                readonly property color iconTextColor: root.selectedPath === filePath
                    ? root.skinStyle.selectionText : root.skinStyle.text
                readonly property bool useLightIcon:
                    0.2126 * iconTextColor.r + 0.7152 * iconTextColor.g
                        + 0.0722 * iconTextColor.b > 0.5
                implicitWidth: tree.width
                implicitHeight: 20
                text: fileName
                icon.name: customIcon ? "" : fileIcon
                icon.source: customIcon
                    ? Qt.resolvedUrl("icons/" + fileIcon
                        + (useLightIcon ? "-light" : "") + ".svg") : ""
                palette.text: root.selectedPath === filePath ? root.skinStyle.selectionText : root.skinStyle.text
                background: Rectangle { color: root.selectedPath === filePath ? root.skinStyle.selection : root.skinStyle.background }
                onClicked: {
                    root.selectedPath = filePath
                    if (hasChildren) tree.toggleExpanded(row)
                }
                onDoubleClicked: {
                    if (!hasChildren) root.app.activate_local_path(filePath)
                }
            }
            KineticWheelHandler { view: tree }
            ScrollBar.vertical: ScrollBar {}
        }
        Label {
            Layout.fillWidth: true
            visible: text.length > 0
            text: root.libraryModel ? root.libraryModel.searchStatus : ""
            textFormat: Text.PlainText
            elide: Text.ElideRight
        }
        RowLayout {
            Layout.fillWidth: true
            LibraryButton {
                text: qsTr("Add to playlist")
                enabled: root.selectedPath.length > 0
                onClicked: root.app.add_local_path(root.selectedPath)
            }
            LibraryButton {
                text: qsTr("Play")
                enabled: root.selectedPath.length > 0
                onClicked: root.app.activate_local_path(root.selectedPath)
            }
            Label {
                text: root.app ? root.app.directory_path : ""
                color: root.skinStyle.text
                textFormat: Text.PlainText
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            BusyIndicator {
                running: !!root.libraryModel && (root.libraryModel.searching || searchLayout.busy)
                visible: running
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
            }
        }
    }
    component LibraryButton: Button {
        implicitHeight: 22
        leftPadding: 6
        rightPadding: 6
        topPadding: 2
        bottomPadding: 2
        background: Rectangle { color: root.skinStyle.header; border.color: root.skinStyle.frame }
        contentItem: Text {
            text: parent.text
            font: parent.font
            color: root.skinStyle.headerText
            opacity: parent.enabled ? 1 : 0.5
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
}
