// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.settings

// Browse another Kog instance's library and queue its streams.
//
// The desktop app already plays HTTP audio through its ffmpeg backend, so
// remote playback needs no new decoder: a track is queued as the server's
// /api/stream URL. This window is only the library view and the connection
// settings that go with it.
Window {
    id: root

    required property var app
    signal openPlayer()
    property alias settingsFile: connection.fileName
    readonly property bool connected: serverUrl.length > 0
    readonly property string scheme: tlsMode === "off" ? "http" : "https"
    // Fragments of the API path are encoded, so %2F and spaces survive.
    readonly property string baseUrl: serverUrl.replace(/\/+$/, "")

    property string serverUrl: connection.serverUrl
    property string token: connection.token
    property string codec: connection.codec
    property string tlsMode: connection.tlsMode
    property string authMode: connection.authMode
    property string username: connection.username
    property string password: connection.password
    property string currentPath: ""
    /// Breadcrumb of visited paths, so Up is exact instead of string surgery.
    property var pathStack: []
    property var directories: []
    property var files: []
    property string statusText: qsTr("Enter your server address and connect.")
    property bool busy: false

    function base64(value) {
        return Qt.btoa(value)
    }

    function request(path, onSuccess) {
        const xhr = new XMLHttpRequest()
        const url = root.baseUrl + path
        xhr.open("GET", url)
        if (root.authMode === "basic" && root.username.length > 0)
            xhr.setRequestHeader("Authorization", "Basic " + base64(root.username + ":" + root.password))
        else if (root.token.length > 0)
            xhr.setRequestHeader("Authorization", "Bearer " + root.token)
        root.busy = true
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return
            root.busy = false
            if (xhr.status === 401) {
                root.statusText = qsTr("Authentication failed — check the token or password.")
                return
            }
            if (xhr.status !== 200) {
                root.statusText = qsTr("Request failed (%1)").arg(xhr.status)
                return
            }
            try {
                onSuccess(JSON.parse(xhr.responseText))
            } catch (error) {
                root.statusText = qsTr("Unexpected response from the server.")
            }
        }
        xhr.send()
    }

    function connect() {
        if (!connected) {
            statusText = qsTr("Enter your server address first.")
            return
        }
        connection.serverUrl = serverUrl
        connection.token = token
        connection.codec = codec
        connection.tlsMode = tlsMode
        // A cheap authenticated call proves the address and credentials.
        request("/api/version", function(payload) {
            root.statusText = qsTr("Connected to Kog %1").arg(payload.version)
            root.pathStack = []
            root.browse("")
        })
    }

    function browse(path, fromStack) {
        const query = path.length > 0 ? "?path=" + encodeURIComponent(path) : ""
        request("/api/library" + query, function(payload) {
            if (fromStack !== true && root.currentPath.length > 0
                    && root.currentPath !== payload.path)
                root.pathStack = root.pathStack.concat([root.currentPath])
            root.currentPath = payload.path
            root.directories = payload.directories || []
            root.files = payload.files || []
        })
    }

    function goUp() {
        if (root.pathStack.length === 0)
            return
        const stack = root.pathStack.slice()
        const previous = stack.pop()
        root.pathStack = stack
        root.browse(previous, true)
    }

    function streamUrlFor(file) {
        const codec = root.codec.length > 0 ? root.codec : "aac"
        const fragment = file.fragment ? "&fragment=" + encodeURIComponent(file.fragment) : ""
        return root.baseUrl + "/api/stream?kind=local&path="
            + encodeURIComponent(file.path) + "&codec=" + codec + fragment
    }

    function queue(file) {
        root.app.enqueue_url(streamUrlFor(file))
        root.statusText = qsTr("Queued %1").arg(file.name)
    }

    function queueAll() {
        for (const file of root.files)
            root.app.enqueue_url(streamUrlFor(file))
        root.statusText = qsTr("Queued %1 tracks").arg(root.files.length)
    }

    objectName: "kogRemoteBrowser"
    title: qsTr("Kog — Remote Library")
    width: 520
    height: 620
    visible: false
    color: palette.window

    Settings {
        id: connection
        category: "RemoteServer"
        property string serverUrl: ""
        property string token: ""
        property string username: ""
        property string password: ""
        property string authMode: "token"
        property string codec: "aac"
        property string tlsMode: "off"
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        GridLayout {
            Layout.fillWidth: true
            columns: 4
            columnSpacing: 8
            rowSpacing: 6

            Label { text: qsTr("Server") }
            TextField {
                id: urlField
                Layout.fillWidth: true
                Layout.columnSpan: 3
                placeholderText: qsTr("https://my-desktop:8420")
                text: root.serverUrl
                selectByMouse: true
                onTextChanged: root.serverUrl = text.trim()
                onAccepted: root.connect()
            }

            Label { text: qsTr("Auth") }
            ComboBox {
                Layout.preferredWidth: 110
                model: [qsTr("Token"), qsTr("Password"), qsTr("None")]
                currentIndex: root.authMode === "basic" ? 1 : (root.authMode === "none" ? 2 : 0)
                onActivated: {
                    connection.authMode = ["token", "basic", "none"][currentIndex]
                    root.authMode = connection.authMode
                }
            }
            TextField {
                Layout.fillWidth: true
                Layout.columnSpan: 2
                visible: root.authMode !== "basic"
                placeholderText: qsTr("API token")
                text: root.token
                echoMode: TextInput.Password
                selectByMouse: true
                onTextChanged: root.token = text.trim()
            }
            TextField {
                Layout.fillWidth: true
                visible: root.authMode === "basic"
                placeholderText: qsTr("Username")
                text: root.username
                selectByMouse: true
                onTextChanged: root.username = text.trim()
            }
            TextField {
                Layout.fillWidth: true
                visible: root.authMode === "basic"
                placeholderText: qsTr("Password")
                text: root.password
                echoMode: TextInput.Password
                selectByMouse: true
                onTextChanged: root.password = text
            }

            Label { text: qsTr("Stream") }
            ComboBox {
                id: codecBox
                model: ["aac", "opus", "flac"]
                currentIndex: Math.max(0, model.indexOf(root.codec))
                onActivated: root.codec = model[currentIndex]
            }
            Button {
                text: qsTr("Connect")
                enabled: !root.busy
                onClicked: root.connect()
            }
            Button {
                text: qsTr("Queue All")
                enabled: root.files.length > 0
                onClicked: root.queueAll()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Button {
                text: qsTr("Up")
                enabled: root.pathStack.length > 0
                onClicked: root.goUp()
            }
            Label {
                Layout.fillWidth: true
                text: root.currentPath.length > 0 ? root.currentPath : qsTr("Not connected")
                elide: Text.ElideMiddle
                color: root.palette.placeholderText
            }
            BusyIndicator {
                running: root.busy
                Layout.preferredWidth: 20
                Layout.preferredHeight: 20
            }
        }

        ListView {
            id: entries
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: {
                const rows = []
                for (const directory of root.directories)
                    rows.push({ name: directory.name, path: directory.path, isDir: true })
                for (const file of root.files)
                    rows.push({ name: file.name, path: file.path, isDir: false, fragment: file.fragment })
                return rows
            }
            delegate: ItemDelegate {
                required property var modelData
                width: entries.width
                text: (modelData.isDir ? "▸ " : "") + modelData.name
                onClicked: {
                    if (modelData.isDir)
                        root.browse(modelData.path)
                    else
                        root.queue(modelData)
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: root.statusText
            wrapMode: Text.WordWrap
            color: root.palette.placeholderText
        }

        RowLayout {
            Layout.alignment: Qt.AlignRight
            Button {
                text: qsTr("Show Player")
                onClicked: root.openPlayer()
            }
            Button {
                text: qsTr("Close")
                onClicked: root.hide()
            }
        }
    }
}
