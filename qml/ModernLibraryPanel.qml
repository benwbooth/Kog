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
    padding: 6
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
        spacing: 5
        RowLayout {
            Layout.fillWidth: true
            Label { text: qsTr("Music library"); font.bold: true; Layout.fillWidth: true }
            ToolButton {
                text: qsTr("Folder…")
                Accessible.name: qsTr("Choose music folder")
                onClicked: { root.app.choose_music_folder(); root.refreshRoot() }
            }
        }
        Label {
            text: root.app ? root.app.directory_path : ""
            textFormat: Text.PlainText
            elide: Text.ElideMiddle
            Layout.fillWidth: true
        }
        TextField {
            id: search
            objectName: "modernLibrarySearch"
            Layout.fillWidth: true
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
            reuseItems: false
            opacity: searchLayout.ready ? 1 : 0
            enabled: opacity === 1
            columnWidthProvider: function(column) { return Math.max(0, width - 14) }
            selectionModel: ItemSelectionModel { model: root.libraryModel }
            selectionBehavior: TableView.SelectRows
            selectionMode: TableView.SingleSelection
            delegate: TreeViewDelegate {
                required property string fileName
                required property string filePath
                required property string fileIcon
                implicitWidth: tree.width
                implicitHeight: 28
                text: fileName
                icon.name: fileIcon
                onClicked: root.selectedPath = filePath
                onDoubleClicked: {
                    if (hasChildren) tree.toggleExpanded(row)
                    else root.app.activate_local_path(filePath)
                }
            }
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
            Button {
                text: qsTr("Add to playlist")
                enabled: root.selectedPath.length > 0
                onClicked: root.app.add_local_path(root.selectedPath)
            }
            Button {
                text: qsTr("Play")
                enabled: root.selectedPath.length > 0
                onClicked: root.app.activate_local_path(root.selectedPath)
            }
            Item { Layout.fillWidth: true }
            BusyIndicator {
                running: !!root.libraryModel && (root.libraryModel.searching || searchLayout.busy)
                visible: running
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
            }
        }
    }
}
