// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

WheelHandler {
    id: kineticWheel

    required property Flickable view
    property real velocity: 0
    property real maximumVelocity: 9000
    property real impulsePerStep: 1250
    property real deceleration: 2500
    property double lastFrameTime: 0

    target: null
    acceptedDevices: PointerDevice.Mouse

    function minimumContentY() {
        return view.originY - view.topMargin;
    }

    function maximumContentY() {
        return Math.max(minimumContentY(), view.originY + view.contentHeight - view.height + view.bottomMargin);
    }

    function stop() {
        velocity = 0;
        momentumTimer.stop();
        lastFrameTime = 0;
    }

    function start(steps) {
        if (steps === 0 || maximumContentY() <= minimumContentY())
            return;
        const impulse = -steps * impulsePerStep;
        if (velocity * impulse < 0)
            velocity *= 0.2;
        velocity = Math.max(-maximumVelocity, Math.min(maximumVelocity, velocity + impulse));
        lastFrameTime = Date.now();
        momentumTimer.start();
    }

    function advance() {
        const now = Date.now();
        const elapsed = lastFrameTime > 0 ? Math.min(0.05, (now - lastFrameTime) / 1000) : 0;
        lastFrameTime = now;
        if (elapsed <= 0)
            return;
        const minimum = minimumContentY();
        const maximum = maximumContentY();
        const next = Math.max(minimum, Math.min(maximum, view.contentY + velocity * elapsed));
        const hitBoundary = next === view.contentY && ((velocity < 0 && next <= minimum) || (velocity > 0 && next >= maximum));
        view.contentY = next;

        const nextSpeed = Math.max(0, Math.abs(velocity) - deceleration * elapsed);
        velocity = Math.sign(velocity) * nextSpeed;
        if (hitBoundary || nextSpeed < 1)
            stop();
    }

    onWheel: event => {
        if (event.modifiers & Qt.ShiftModifier) {
            event.accepted = false;
            return;
        }

        let steps = event.angleDelta.y / 120;
        if (steps === 0)
            steps = event.pixelDelta.y / 40;
        if (steps === 0) {
            event.accepted = false;
            return;
        }

        start(steps);
        event.accepted = true;
    }

    property Timer momentumTimer: Timer {
        interval: 16
        repeat: true
        onTriggered: kineticWheel.advance()
    }
}
