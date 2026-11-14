pragma ComponentBehavior: Bound

import QtQml
import QtQuick
import QtQuick.Controls

// Stands in for the column header when songs show as rows or cover tiles:
// a "Sort by" menu over the same columns, and the direction beside it.
// Choosing the sorted column again flips its direction, as a header click
// does.
Item {
    id: root

    required property var theme
    property string sortColumn: "index"
    property bool sortAscending: true
    property string orderLabel: qsTr("Queue Order")

    // The rows view's lines: which fields each of the three shows, in the
    // order they were checked. Only the rows view offers the picker.
    property bool showLines: false
    property var rowLines: []
    property var defaultRowLines: []

    signal sortRequested(string column)
    signal rowLinesRequested(var lines)

    readonly property var sortColumns: [
        { id: "index", label: root.orderLabel },
        { id: "title", label: qsTr("Title") },
        { id: "artist", label: qsTr("Artist") },
        { id: "album", label: qsTr("Album") },
        { id: "albumartist", label: qsTr("Album Artist") },
        { id: "length", label: qsTr("Length") },
        { id: "date", label: qsTr("Year") },
        { id: "track", label: qsTr("Track") },
        { id: "genre", label: qsTr("Genre") },
        { id: "playcount", label: qsTr("Play Count") },
        { id: "filename", label: qsTr("Filename") },
        { id: "path", label: qsTr("Path") }
    ]
    readonly property var rowFields: sortColumns.filter(column => column.id !== "index")
        .concat([{ id: "format", label: qsTr("Format") }])
    function lineFields(line) {
        const fields = rowLines && rowLines[line]
        return Array.isArray(fields) ? fields : []
    }
    function toggleField(line, field) {
        const next = [0, 1, 2].map(index => lineFields(index).slice())
        const at = next[line].indexOf(field)
        if (at >= 0)
            next[line].splice(at, 1)
        else
            next[line].push(field)
        rowLinesRequested(next)
    }
    readonly property string sortLabel: {
        const found = sortColumns.find(column => column.id === sortColumn)
        return found ? found.label : sortColumn
    }
    readonly property bool darkTheme: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5

    implicitHeight: 34

    component SortButton: AbstractButton {
        id: button
        property string label: ""
        implicitHeight: 24
        implicitWidth: buttonLabel.implicitWidth + 16
        hoverEnabled: true
        background: Rectangle {
            radius: 6
            color: Qt.rgba(root.theme.buttonText.r, root.theme.buttonText.g,
                root.theme.buttonText.b,
                button.down ? 0.14 : (button.hovered ? 0.08 : 0))
            Behavior on color { ColorAnimation { duration: 120 } }
        }
        contentItem: Label {
            id: buttonLabel
            text: button.label
            font.pixelSize: 11
            font.bold: true
            color: root.theme.buttonText
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Label {
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 4
            text: qsTr("Sort by")
            font.pixelSize: 11
            color: root.theme.buttonText
            opacity: 0.65
        }

        SortButton {
            id: columnButton
            anchors.verticalCenter: parent.verticalCenter
            label: root.sortLabel + "  ▾"
            Accessible.name: qsTr("Sort by %1").arg(root.sortLabel)
            onClicked: sortMenu.popup(columnButton, 0, columnButton.height)
        }
        SortButton {
            anchors.verticalCenter: parent.verticalCenter
            label: root.sortAscending ? "▲" : "▼"
            Accessible.name: root.sortAscending ? qsTr("Ascending") : qsTr("Descending")
            onClicked: root.sortRequested(root.sortColumn)
        }
        Item {
            visible: root.showLines
            width: 10
            height: 1
        }
        SortButton {
            id: linesButton
            visible: root.showLines
            anchors.verticalCenter: parent.verticalCenter
            label: qsTr("Lines") + "  ▾"
            Accessible.name: qsTr("Choose what each line shows")
            onClicked: linesPopup.opened ? linesPopup.close() : linesPopup.open()
        }
    }

    // One checkable field list per line. A field joins the end of its line
    // when checked, so the order checked is the order shown.
    Popup {
        id: linesPopup
        parent: linesButton
        y: linesButton.height + 2
        padding: 10
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent

        contentItem: Column {
            spacing: 8

            Row {
                spacing: 14

                Repeater {
                    model: 3

                    delegate: Column {
                        id: lineColumn
                        required property int index
                        readonly property var fields: root.lineFields(index)
                        spacing: 0
                        width: 132

                        Label {
                            text: qsTr("Line %1").arg(lineColumn.index + 1)
                            font.pixelSize: 11
                            font.bold: true
                            color: root.theme.text
                            bottomPadding: 4
                        }
                        Repeater {
                            model: root.rowFields

                            delegate: AbstractButton {
                                id: fieldButton
                                required property var modelData
                                readonly property int order: lineColumn.fields.indexOf(modelData.id)
                                width: lineColumn.width
                                implicitHeight: 22
                                hoverEnabled: true
                                checkable: true
                                checked: order >= 0
                                Accessible.role: Accessible.CheckBox
                                Accessible.name: qsTr("Line %1: %2")
                                    .arg(lineColumn.index + 1).arg(modelData.label)
                                onClicked: root.toggleField(lineColumn.index, modelData.id)
                                background: Rectangle {
                                    radius: 4
                                    color: Qt.rgba(root.theme.text.r, root.theme.text.g,
                                        root.theme.text.b, fieldButton.hovered ? 0.08 : 0)
                                }
                                contentItem: Item {
                                    Rectangle {
                                        id: box
                                        x: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 16
                                        height: 16
                                        radius: 4
                                        color: fieldButton.checked ? root.theme.accent : "transparent"
                                        border.width: fieldButton.checked ? 0 : 1
                                        border.color: root.theme.mid
                                        // The field's place on its line.
                                        Label {
                                            anchors.centerIn: parent
                                            visible: fieldButton.checked
                                            text: String(fieldButton.order + 1)
                                            font.pixelSize: 10
                                            font.bold: true
                                            color: root.theme.highlightedText
                                        }
                                    }
                                    Label {
                                        anchors.left: box.right
                                        anchors.leftMargin: 8
                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: fieldButton.modelData.label
                                        font.pixelSize: 12
                                        elide: Text.ElideRight
                                        color: root.theme.text
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SortButton {
                label: qsTr("Reset to Default")
                onClicked: root.rowLinesRequested(root.defaultRowLines)
            }
        }
    }

    Menu {
        id: sortMenu
        Instantiator {
            model: root.sortColumns
            delegate: MenuItem {
                required property var modelData
                text: modelData.label
                checkable: true
                checked: root.sortColumn === modelData.id
                // Choosing the sorted column from the menu leaves it as it
                // is; the arrow beside the menu flips it.
                onTriggered: {
                    if (root.sortColumn !== modelData.id)
                        root.sortRequested(modelData.id)
                    checked = Qt.binding(() => root.sortColumn === modelData.id)
                }
            }
            onObjectAdded: (index, object) => sortMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => sortMenu.removeItem(object)
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        height: 1
        color: root.theme.mid
        opacity: 0.6
    }
}
