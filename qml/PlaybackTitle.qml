import QtQml

// Playback retains track metadata after Stop so Play can resume that track.
// Window titles must reflect playback state, not the presence of metadata.
QtObject {
    required property string playbackState
    required property string trackTitle
    readonly property bool active: playbackState !== "stopped"
        && trackTitle.length > 0 && trackTitle !== "Not Playing"
    readonly property string text: active ? trackTitle : qsTr("Kog")
    readonly property string windowTitle: active ? trackTitle + " — Kog" : qsTr("Kog")
}
