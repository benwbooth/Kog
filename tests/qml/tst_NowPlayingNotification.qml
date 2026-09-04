import QtQuick
import QtTest
import "../../qml" as Kog

TestCase {
    name: "NowPlayingNotification"
    when: windowShown
    width: 500
    height: 300

    QtObject {
        id: playback
        property string now_title: "A very long Japanese title 日本語 <Live> & music"
        property string now_artist: "Artist"
        property string current_album: "Album"
        property string playback_state: "playing"
        property int current_index: 0
        property int playlist_count: 3
        property real duration_seconds: 180
        property real position_seconds: 45
        property int nextCalls: 0
        property int previousCalls: 0
        function play_pause() {
            playback_state = playback_state === "playing" ? "paused" : "playing"
        }
        function stop() { playback_state = "stopped" }
        function next() { nextCalls++; now_title = "Next song" }
        function previous() { previousCalls++ }
    }

    Kog.NowPlayingNotification {
        id: notification
        app: playback
        displayDuration: 300
        settingsFile: "/tmp/kog-notification-qml-tests.ini"
    }

    function init() {
        failOnWarning(/TypeError|ReferenceError|Unable to assign/)
        notification.dismiss()
        mouseMove(notification.contentItem, -20, -20)
        notification.displayDuration = 300
        playback.current_index = 0
        playback.playback_state = "playing"
        playback.now_title = "日本語 <Live> & music"
        playback.position_seconds = 45
    }

    function cleanup() { notification.dismiss() }

    function test_transport_and_live_state() {
        notification.present()
        const play = findChild(notification, "notificationPlayPause")
        const stop = findChild(notification, "notificationStop")
        compare(play.toolTip, "Pause")
        play.clicked()
        compare(playback.playback_state, "paused")
        compare(play.toolTip, "Play")
        stop.clicked()
        compare(playback.playback_state, "stopped")
        compare(stop.enabled, false)
        play.clicked()
        compare(playback.playback_state, "playing")
        const previousCount = playback.previousCalls
        findChild(notification, "notificationPrevious").clicked()
        compare(playback.previousCalls, previousCount + 1)
        const nextCount = playback.nextCalls
        findChild(notification, "notificationNext").clicked()
        compare(playback.nextCalls, nextCount + 1)
        compare(findChild(notification, "notificationTitle").text, "Next song")
    }

    function test_replacement_restarts_expiry() {
        notification.present()
        wait(180)
        playback.now_title = "Replacement"
        notification.present()
        wait(180)
        verify(notification.visible)
        compare(findChild(notification, "notificationTitle").text, "Replacement")
        tryCompare(notification, "visible", false, 1000)
    }

    function test_dismiss_does_not_stop_playback() {
        notification.present()
        findChild(notification, "dismissNotification").clicked()
        compare(notification.visible, false)
        compare(playback.playback_state, "playing")
    }

    function test_empty_queue_does_not_open() {
        playback.current_index = -1
        notification.present()
        compare(notification.visible, false)
    }

    function test_hover_keeps_controls_available() {
        notification.present()
        mouseMove(notification.contentItem, 200, 90)
        tryCompare(notification, "pointerInside", true)
        wait(notification.displayDuration + 100)
        verify(notification.visible)
        mouseMove(notification.contentItem, -20, -20)
        tryCompare(notification, "pointerInside", false)
        tryCompare(notification, "visible", false, 1000)
    }

    function test_metadata_is_plain_text_and_progress_is_clamped() {
        compare(findChild(notification, "notificationTitle").textFormat, Text.PlainText)
        compare(notification.progress, 0.25)
        playback.position_seconds = 500
        compare(notification.progress, 1)
    }

    function test_icons_are_loaded() {
        for (const name of ["notificationPrevious", "notificationPlayPause", "notificationStop",
                "notificationNext", "dismissNotification"]) {
            const icon = findChild(findChild(notification, name), "notificationButtonIcon")
            verify(icon !== null)
            tryCompare(icon, "status", Image.Ready)
        }
    }

    function test_drag_remembers_position_and_reset() {
        notification.resetPosition()
        notification.present()
        const header = findChild(notification, "notificationHeader")
        waitForRendering(header)
        wait(50)
        mousePress(header, 170, 12)
        mouseMove(header, 140, 12, 30)
        mouseMove(header, 100, 12, 30)
        mouseRelease(header, 100, 12)
        verify(notification.rightMargin > 16)
        const movedRight = notification.rightMargin
        if (notification.layerPlacement)
            compare(notification.layerPlacement.surface.margins.right, Math.round(movedRight))
        const component = Qt.createComponent("../../qml/NowPlayingNotification.qml")
        compare(component.status, Component.Ready)
        const restored = component.createObject(null, {
            app: playback, settingsFile: notification.settingsFile
        })
        verify(restored !== null)
        tryCompare(restored, "rightMargin", movedRight)
        restored.destroy()
        notification.resetPosition()
        compare(notification.rightMargin, 16)
        compare(notification.bottomMargin, 16)
    }
}
