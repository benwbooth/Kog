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
    property double lastPixelEventTime: 0
    property real pixelVelocity: 0

    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    function minimumContentY() {
        return view.originY - view.topMargin;
    }

    function maximumContentY() {
        return Math.max(minimumContentY(), view.originY + view.contentHeight - view.height + view.bottomMargin);
    }

    function stop() {
        velocity = 0;
        momentumTimer.stop();
        pixelGestureEndTimer.stop();
        lastFrameTime = 0;
        lastPixelEventTime = 0;
        pixelVelocity = 0;
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

    function moveTo(position) {
        const minimum = minimumContentY();
        const maximum = maximumContentY();
        view.contentY = Math.max(minimum, Math.min(maximum, position));
    }

    function applyPixelDelta(pixelDelta) {
        if (pixelDelta === 0 || maximumContentY() <= minimumContentY())
            return;

        const now = Date.now();
        const contentDelta = -pixelDelta;
        const elapsed = lastPixelEventTime > 0 ? (now - lastPixelEventTime) / 1000 : 0;
        momentumTimer.stop();
        velocity = 0;
        moveTo(view.contentY + contentDelta);

        if (elapsed >= 0.004 && elapsed <= 0.08) {
            const instantaneousVelocity = contentDelta / elapsed;
            pixelVelocity = pixelVelocity === 0 ? instantaneousVelocity : pixelVelocity * 0.65 + instantaneousVelocity * 0.35;
        } else {
            pixelVelocity = contentDelta * 60;
        }
        pixelVelocity = Math.max(-maximumVelocity, Math.min(maximumVelocity, pixelVelocity));
        lastPixelEventTime = now;
        pixelGestureEndTimer.restart();
    }

    function finishPixelGesture() {
        lastPixelEventTime = 0;
        velocity = pixelVelocity;
        pixelVelocity = 0;
        if (Math.abs(velocity) < 40) {
            stop();
            return;
        }
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
        moveTo(next);

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

        if (event.device.type === PointerDevice.TouchPad && event.pixelDelta.y !== 0) {
            applyPixelDelta(event.pixelDelta.y);
            event.accepted = true;
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

    property Timer pixelGestureEndTimer: Timer {
        interval: 45
        repeat: false
        onTriggered: kineticWheel.finishPixelGesture()
    }
}
