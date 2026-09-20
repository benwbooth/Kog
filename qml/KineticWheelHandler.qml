// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

WheelHandler {
    id: kineticWheel

    required property Flickable view
    // The native WheelHandler.orientation decides which axis Qt delivers at
    // all: a vertical handler never sees x-only wheel events, and a
    // horizontal one never sees y-only events. (Shadowing this property with
    // a QML one used to leave the C++ filter on Vertical, so horizontal
    // momentum silently never ran.) Shift+wheel is a y-axis event that must
    // scroll sideways, so the vertical handler drives the horizontal engine
    // itself via start(steps, true).
    readonly property bool horizontal: orientation === Qt.Horizontal
    // The axis the active momentum run drives; Shift+wheel flips it on a
    // vertical handler.
    property bool drivingHorizontally: horizontal
    property real velocity: 0
    property real maximumVelocity: 9000
    property real impulsePerStep: 1250
    property real deceleration: 2500
    property double lastFrameTime: 0
    property double lastPixelEventTime: 0
    property real pixelVelocity: 0

    target: null
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    function minimumContent() {
        return drivingHorizontally
            ? view.originX - view.leftMargin
            : view.originY - view.topMargin;
    }

    function maximumContent() {
        return drivingHorizontally
            ? Math.max(minimumContent(), view.originX + view.contentWidth - view.width + view.rightMargin)
            : Math.max(minimumContent(), view.originY + view.contentHeight - view.height + view.bottomMargin);
    }

    function currentContent() {
        return drivingHorizontally ? view.contentX : view.contentY;
    }

    function stop() {
        velocity = 0;
        momentumTimer.stop();
        pixelGestureEndTimer.stop();
        lastFrameTime = 0;
        lastPixelEventTime = 0;
        pixelVelocity = 0;
        // A Shift+wheel run borrows the horizontal axis; ending the run hands
        // the engine back to the handler's own orientation.
        drivingHorizontally = horizontal;
    }

    function start(steps, sideways) {
        const axis = sideways !== undefined ? sideways : horizontal;
        if (axis !== drivingHorizontally) {
            // A momentum run left over from the other axis must not carry
            // into this one.
            velocity = 0;
            drivingHorizontally = axis;
        }
        if (steps === 0 || maximumContent() <= minimumContent())
            return false;
        const impulse = -steps * impulsePerStep;
        if (velocity * impulse < 0)
            velocity *= 0.2;
        velocity = Math.max(-maximumVelocity, Math.min(maximumVelocity, velocity + impulse));
        lastFrameTime = Date.now();
        momentumTimer.start();
        return true;
    }

    function moveTo(position) {
        const minimum = minimumContent();
        const maximum = maximumContent();
        const clamped = Math.max(minimum, Math.min(maximum, position));
        if (drivingHorizontally)
            view.contentX = clamped;
        else
            view.contentY = clamped;
    }

    function applyPixelDelta(pixelDelta) {
        if (pixelDelta === 0 || maximumContent() <= minimumContent())
            return;

        const now = Date.now();
        const contentDelta = -pixelDelta;
        const elapsed = lastPixelEventTime > 0 ? (now - lastPixelEventTime) / 1000 : 0;
        momentumTimer.stop();
        velocity = 0;
        moveTo(currentContent() + contentDelta);

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
        const minimum = minimumContent();
        const maximum = maximumContent();
        const next = Math.max(minimum, Math.min(maximum, currentContent() + velocity * elapsed));
        const hitBoundary = next === currentContent() && ((velocity < 0 && next <= minimum) || (velocity > 0 && next >= maximum));
        moveTo(next);

        const nextSpeed = Math.max(0, Math.abs(velocity) - deceleration * elapsed);
        velocity = Math.sign(velocity) * nextSpeed;
        if (hitBoundary || nextSpeed < 1)
            stop();
    }

    onWheel: event => {
        if (horizontal)
            handleHorizontalWheel(event);
        else
            handleVerticalWheel(event);
    }

    function handleVerticalWheel(event) {
        // Shift turns the wheel sideways. The event stays on the y axis, so
        // this vertical handler is the only one Qt offers it to; drive the
        // horizontal engine from here.
        if (event.modifiers & Qt.ShiftModifier) {
            let sidesteps = event.angleDelta.y / 120;
            if (sidesteps === 0)
                sidesteps = event.pixelDelta.y / 40;
            event.accepted = sidesteps !== 0 && start(sidesteps, true);
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

    function handleHorizontalWheel(event) {
        // Native horizontal wheels and trackpad swipes report on the x axis,
        // which is the only axis Qt delivers to this handler.
        if (event.device.type === PointerDevice.TouchPad && event.pixelDelta.x !== 0) {
            applyPixelDelta(event.pixelDelta.x);
            event.accepted = true;
            return;
        }

        let steps = event.angleDelta.x / 120;
        if (steps === 0)
            steps = event.pixelDelta.x / 40;
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
