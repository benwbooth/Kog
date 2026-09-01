pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 790
    height: 690
    minimumWidth: 650
    minimumHeight: 560
    title: qsTr("Edit Tags — Kog")
    color: palette.window

    property string rowIndices: ""
    property var snapshot: ({})
    property bool dirty: false
    property string artworkAction: "keep"
    property string artworkPath: ""
    property string artworkUri: ""
    property string artworkDescription: ""
    property string errorText: ""

    component EditorField: RowLayout {
        id: editorField

        required property string label
        required property string fieldKey
        property bool numeric: false
        property bool mixed: false
        property bool changed: false
        property alias text: input.text

        Layout.fillWidth: true
        spacing: 10

        function load(value) {
            changed = false
            mixed = value ? value.mixed : false
            input.text = value ? value.value : ""
        }

        function addEdit(target) {
            if (changed)
                target[fieldKey] = input.text
        }

        Label {
            Layout.preferredWidth: editorField.label.length > 0 ? 104 : 0
            visible: editorField.label.length > 0
            horizontalAlignment: Text.AlignRight
            text: editorField.label
            color: root.palette.windowText
        }

        TextField {
            id: input

            Layout.fillWidth: true
            selectByMouse: true
            placeholderText: editorField.mixed ? qsTr("Multiple values") : ""
            inputMethodHints: editorField.numeric
                ? Qt.ImhDigitsOnly
                : Qt.ImhNoPredictiveText
            validator: RegularExpressionValidator {
                regularExpression: editorField.numeric ? /^$|^[1-9][0-9]{0,9}$/ : /^.*$/
            }
            onTextEdited: {
                editorField.changed = true
                editorField.mixed = false
                root.dirty = true
                root.errorText = ""
            }
            Accessible.name: editorField.label
        }
    }

    component EditorArea: ColumnLayout {
        id: editorArea

        required property string label
        required property string fieldKey
        property bool mixed: false
        property bool changed: false
        property alias text: input.text

        Layout.fillWidth: true
        spacing: 5

        function load(value) {
            changed = false
            mixed = value ? value.mixed : false
            input.text = value ? value.value : ""
        }

        function addEdit(target) {
            if (changed)
                target[fieldKey] = input.text
        }

        Label {
            text: editorArea.label
            color: root.palette.windowText
            font.bold: true
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.preferredHeight: 112
            clip: true

            TextArea {
                id: input

                wrapMode: TextEdit.Wrap
                selectByMouse: true
                placeholderText: editorArea.mixed ? qsTr("Multiple values") : ""
                onTextChanged: {
                    if (activeFocus) {
                        editorArea.changed = true
                        editorArea.mixed = false
                        root.dirty = true
                        root.errorText = ""
                    }
                }
                Accessible.name: editorArea.label
            }
        }
    }

    function errorResult(message) {
        return { ok: false, error: message }
    }

    function parseResult(value) {
        try {
            return JSON.parse(value)
        } catch (error) {
            return errorResult(qsTr("Kog returned an invalid tag response."))
        }
    }

    function openForRows(rows) {
        rowIndices = rows.join(",")
        errorText = ""
        const result = parseResult(app.tag_editor_data(rowIndices))
        if (!result.ok) {
            errorText = result.error || qsTr("These tracks cannot be edited.")
            snapshot = ({
                ok: false,
                summary: qsTr("Tag editing unavailable"),
                location: "",
                artwork: ({ state: "none" })
            })
            dirty = false
            artworkAction = "keep"
            artworkPath = ""
            artworkUri = ""
            artworkDescription = ""
            show()
            requestActivate()
            return
        }
        loadSnapshot(result)
        show()
        raise()
        requestActivate()
    }

    function loadSnapshot(result) {
        snapshot = result
        titleField.load(result.fields.title)
        artistField.load(result.fields.artist)
        albumArtistField.load(result.fields.albumArtist)
        albumField.load(result.fields.album)
        composerField.load(result.fields.composer)
        genreField.load(result.fields.genre)
        dateField.load(result.fields.year)
        trackField.load(result.fields.trackNumber)
        trackTotalField.load(result.fields.trackTotal)
        discField.load(result.fields.discNumber)
        discTotalField.load(result.fields.discTotal)
        commentField.load(result.fields.comment)
        lyricsField.load(result.fields.lyrics)
        artworkAction = "keep"
        artworkPath = ""
        artworkUri = result.artwork.uri || ""
        artworkDescription = result.artwork.description || ""
        dirty = false
        errorText = ""
    }

    function chooseArtwork() {
        const result = parseResult(app.choose_tag_artwork())
        if (result.cancelled)
            return
        if (!result.ok) {
            errorText = result.error || qsTr("The artwork could not be loaded.")
            return
        }
        artworkAction = "replace"
        artworkPath = result.path
        artworkUri = result.uri
        artworkDescription = result.description
        errorText = ""
        dirty = true
    }

    function removeArtwork() {
        artworkAction = "remove"
        artworkPath = ""
        artworkUri = ""
        artworkDescription = qsTr("Cover artwork will be removed")
        errorText = ""
        dirty = true
    }

    function save() {
        const fields = ({})
        titleField.addEdit(fields)
        artistField.addEdit(fields)
        albumArtistField.addEdit(fields)
        albumField.addEdit(fields)
        composerField.addEdit(fields)
        genreField.addEdit(fields)
        dateField.addEdit(fields)
        trackField.addEdit(fields)
        trackTotalField.addEdit(fields)
        discField.addEdit(fields)
        discTotalField.addEdit(fields)
        commentField.addEdit(fields)
        lyricsField.addEdit(fields)
        const request = { fields: fields }
        if (artworkAction !== "keep")
            request.artwork = { action: artworkAction, path: artworkPath }
        const result = parseResult(app.save_tags(rowIndices, JSON.stringify(request)))
        if (!result.ok) {
            errorText = result.error || qsTr("The tags could not be saved.")
            const refreshed = parseResult(app.tag_editor_data(rowIndices))
            if (refreshed.ok)
                loadSnapshot(refreshed)
            errorText = result.error || qsTr("The tags could not be saved.")
            return
        }
        dirty = false
        close()
    }

    Shortcut {
        sequence: StandardKey.Save
        enabled: root.visible && root.snapshot.ok === true && root.dirty
        onActivated: root.save()
    }
    Shortcut {
        sequence: "Escape"
        enabled: root.visible
        onActivated: root.close()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Pane {
            Layout.fillWidth: true
            padding: 18

            RowLayout {
                anchors.fill: parent
                spacing: 14

                Rectangle {
                    Layout.preferredWidth: 46
                    Layout.preferredHeight: 46
                    radius: 8
                    color: root.palette.alternateBase
                    border.color: root.palette.mid

                    Label {
                        anchors.centerIn: parent
                        text: "♫"
                        font.pixelSize: 25
                        color: root.palette.highlight
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Label {
                        Layout.fillWidth: true
                        text: root.snapshot.summary || qsTr("Edit Tags")
                        font.pixelSize: 20
                        font.bold: true
                        elide: Text.ElideRight
                    }
                    Label {
                        Layout.fillWidth: true
                        text: root.snapshot.location || ""
                        color: root.palette.placeholderText
                        elide: Text.ElideMiddle
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: root.palette.mid
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: availableWidth

            ColumnLayout {
                x: 22
                width: parent.width - 44
                spacing: 18

                Label {
                    Layout.fillWidth: true
                    visible: root.errorText.length > 0
                    text: root.errorText
                    wrapMode: Text.Wrap
                    color: root.palette.brightText
                    background: Rectangle {
                        color: Qt.tint(root.palette.highlight, "#35ff3b30")
                        radius: 6
                        border.color: root.palette.highlight
                    }
                    leftPadding: 12
                    rightPadding: 12
                    topPadding: 9
                    bottomPadding: 9
                }

                GridLayout {
                    Layout.fillWidth: true
                    visible: root.snapshot.ok === true
                    columns: root.width >= 760 ? 2 : 1
                    columnSpacing: 18
                    rowSpacing: 9

                    GroupBox {
                        title: qsTr("Track")
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8
                            EditorField { id: titleField; label: qsTr("Title"); fieldKey: "title" }
                            EditorField { id: artistField; label: qsTr("Artist"); fieldKey: "artist" }
                            EditorField { id: composerField; label: qsTr("Composer"); fieldKey: "composer" }
                            EditorField { id: genreField; label: qsTr("Genre"); fieldKey: "genre" }
                            EditorField { id: dateField; label: qsTr("Date"); fieldKey: "year" }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Label {
                                    Layout.preferredWidth: 104
                                    horizontalAlignment: Text.AlignRight
                                    text: qsTr("Track")
                                }
                                EditorField {
                                    id: trackField
                                    Layout.fillWidth: true
                                    label: ""
                                    fieldKey: "trackNumber"
                                    numeric: true
                                }
                                Label { text: qsTr("of") }
                                EditorField {
                                    id: trackTotalField
                                    Layout.fillWidth: true
                                    label: ""
                                    fieldKey: "trackTotal"
                                    numeric: true
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Label {
                                    Layout.preferredWidth: 104
                                    horizontalAlignment: Text.AlignRight
                                    text: qsTr("Disc")
                                }
                                EditorField {
                                    id: discField
                                    Layout.fillWidth: true
                                    label: ""
                                    fieldKey: "discNumber"
                                    numeric: true
                                }
                                Label { text: qsTr("of") }
                                EditorField {
                                    id: discTotalField
                                    Layout.fillWidth: true
                                    label: ""
                                    fieldKey: "discTotal"
                                    numeric: true
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignTop
                        spacing: 14

                        GroupBox {
                            title: qsTr("Album")
                            Layout.fillWidth: true

                            ColumnLayout {
                                anchors.fill: parent
                                spacing: 8
                                EditorField { id: albumField; label: qsTr("Album"); fieldKey: "album" }
                                EditorField { id: albumArtistField; label: qsTr("Album Artist"); fieldKey: "albumArtist" }
                            }
                        }

                        GroupBox {
                            title: qsTr("Cover Artwork")
                            Layout.fillWidth: true

                            ColumnLayout {
                                anchors.fill: parent
                                spacing: 9

                                Rectangle {
                                    Layout.alignment: Qt.AlignHCenter
                                    Layout.preferredWidth: 170
                                    Layout.preferredHeight: 170
                                    radius: 8
                                    color: root.palette.alternateBase
                                    border.color: root.palette.mid
                                    clip: true

                                    Image {
                                        anchors.fill: parent
                                        anchors.margins: 1
                                        source: root.artworkUri
                                        fillMode: Image.PreserveAspectFit
                                        asynchronous: true
                                        visible: root.artworkUri.length > 0
                                    }
                                    Label {
                                        anchors.centerIn: parent
                                        width: parent.width - 24
                                        visible: root.artworkUri.length === 0
                                        text: root.snapshot.artwork
                                            && root.snapshot.artwork.state === "mixed"
                                            ? qsTr("Different covers")
                                            : qsTr("No cover")
                                        horizontalAlignment: Text.AlignHCenter
                                        color: root.palette.placeholderText
                                        font.pixelSize: 15
                                    }
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: root.artworkDescription
                                    color: root.palette.placeholderText
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                }
                                RowLayout {
                                    Layout.alignment: Qt.AlignHCenter
                                    Button {
                                        text: qsTr("Choose…")
                                        icon.name: "image-x-generic"
                                        onClicked: root.chooseArtwork()
                                    }
                                    Button {
                                        text: qsTr("Remove")
                                        icon.name: "edit-delete"
                                        enabled: root.artworkUri.length > 0
                                            || (root.snapshot.artwork !== undefined
                                                && root.snapshot.artwork !== null
                                                && root.snapshot.artwork.state !== "none")
                                        onClicked: root.removeArtwork()
                                    }
                                }
                            }
                        }
                    }
                }

                EditorArea {
                    id: commentField
                    visible: root.snapshot.ok === true
                    label: qsTr("Comment")
                    fieldKey: "comment"
                }
                EditorArea {
                    id: lyricsField
                    visible: root.snapshot.ok === true
                    label: qsTr("Lyrics")
                    fieldKey: "lyrics"
                }
                Item { Layout.preferredHeight: 4 }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: root.palette.mid
        }

        Pane {
            Layout.fillWidth: true
            padding: 12

            RowLayout {
                anchors.fill: parent
                Label {
                    Layout.fillWidth: true
                    text: root.dirty
                        ? qsTr("Unsaved changes")
                        : qsTr("Only fields you change are written")
                    color: root.palette.placeholderText
                }
                Button {
                    text: qsTr("Cancel")
                    onClicked: root.close()
                }
                Button {
                    text: qsTr("Save")
                    icon.name: "document-save"
                    enabled: root.snapshot.ok === true && root.dirty
                        && root.errorText.length === 0
                    highlighted: true
                    onClicked: root.save()
                }
            }
        }
    }
}
