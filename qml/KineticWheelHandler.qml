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
    // Browser wheel deltas describe distance, not added kinetic energy.
    // Move on the input event, then ease the remainder over a short glide.
    // Keeping a destination also preserves small high-resolution wheel steps
    // instead of losing them to deceleration between input events.
    property real wheelStep: 120
    property real wheelDuration: 0.12
    property bool wheelAnimating: false
    property real wheelTarget: 0
    property real wheelStart: 0
    property real wheelElapsed: 0
    property real deceleration: 2500
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
        wheelAnimating = false;
        momentumAnimation.stop();
        pixelGestureEndTimer.stop();
        lastPixelEventTime = 0;
        pixelVelocity = 0;
        // A Shift+wheel run borrows the horizontal axis; ending the run hands
        // the engine back to the handler's own orientation.
        drivingHorizontally = horizontal;
    }

    function start(steps, sideways) {
        const axis = sideways !== undefined ? sideways : horizontal;
        if (steps === 0)
            return false;
        if (axis !== drivingHorizontally)
            stop();
        drivingHorizontally = axis;
        const delta = -steps * wheelStep;
        const current = currentContent();
        const pending = wheelAnimating ? wheelTarget - current : 0;
        // A reversal starts where the content is now, without first finishing
        // travel requested in the old direction.
        const origin = pending * delta > 0 ? wheelTarget : current;
        wheelTarget = Math.max(minimumContent(), Math.min(maximumContent(), origin + delta));
        pixelGestureEndTimer.stop();
        lastPixelEventTime = 0;
        pixelVelocity = 0;
        if (wheelTarget === current) {
            stop();
            return false;
        }
        const immediate = Math.sign(delta) * Math.min(Math.abs(delta) * 0.5, Math.abs(wheelTarget - current));
        moveTo(current + immediate);
        wheelStart = currentContent();
        wheelElapsed = 0;
        wheelAnimating = true;
        velocity = (wheelTarget - wheelStart) * 3 / wheelDuration;
        momentumAnimation.start();
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

    function applyPixelDelta(pixelDelta, sideways, synthesizeMomentum) {
        const axis = sideways !== undefined ? sideways : horizontal;
        if (axis !== drivingHorizontally)
            stop();
        drivingHorizontally = axis;
        if (pixelDelta === 0 || maximumContent() <= minimumContent())
            return;

        const now = Date.now();
        const contentDelta = -pixelDelta;
        const elapsed = lastPixelEventTime > 0 ? (now - lastPixelEventTime) / 1000 : 0;
        momentumAnimation.stop();
        wheelAnimating = false;
        velocity = 0;
        moveTo(currentContent() + contentDelta);

        // Pixel wheels already supply their distance, and OS-generated
        // momentum already contains its own deceleration. Neither needs an
        // extra fling after the event stream ends.
        if (synthesizeMomentum === false) {
            pixelGestureEndTimer.stop();
            lastPixelEventTime = 0;
            pixelVelocity = 0;
            return;
        }

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
        momentumAnimation.start();
    }

    function advance(frameTime) {
        if (wheelAnimating) {
            wheelElapsed += Math.max(0, frameTime);
            const progress = Math.min(1, wheelElapsed / wheelDuration);
            const remaining = 1 - progress;
            moveTo(wheelStart + (wheelTarget - wheelStart) * (1 - remaining * remaining * remaining));
            velocity = (wheelTarget - wheelStart) * 3 * remaining * remaining / wheelDuration;
            if (progress === 1 || currentContent() === minimumContent() || currentContent() === maximumContent())
                stop();
            return;
        }
        const elapsed = Math.min(0.05, frameTime);
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

    function needsPixelMomentum(event) {
        return event.device.type === PointerDevice.TouchPad && event.phase !== Qt.ScrollMomentum;
    }

    function handleVerticalWheel(event) {
        // Shift turns the wheel sideways. The event stays on the y axis, so
        // this vertical handler is the only one Qt offers it to; drive the
        // horizontal engine from here.
        if (event.modifiers & Qt.ShiftModifier) {
            if (event.pixelDelta.y !== 0) {
                applyPixelDelta(event.pixelDelta.y, true, needsPixelMomentum(event));
                event.accepted = true;
                return;
            }
            let sidesteps = event.angleDelta.y / 120;
            event.accepted = sidesteps !== 0 && start(sidesteps, true);
            return;
        }

        // Precision mice can provide pixel deltas too. Prefer that distance
        // over angleDelta regardless of the device's reported type.
        if (event.pixelDelta.y !== 0) {
            applyPixelDelta(event.pixelDelta.y, false, needsPixelMomentum(event));
            event.accepted = true;
            return;
        }

        let steps = event.angleDelta.y / 120;
        if (steps === 0) {
            event.accepted = false;
            return;
        }

        event.accepted = start(steps);
    }

    function handleHorizontalWheel(event) {
        // Native horizontal wheels and trackpad swipes report on the x axis,
        // which is the only axis Qt delivers to this handler.
        if (event.pixelDelta.x !== 0) {
            applyPixelDelta(event.pixelDelta.x, true, needsPixelMomentum(event));
            event.accepted = true;
            return;
        }

        let steps = event.angleDelta.x / 120;
        if (steps === 0) {
            event.accepted = false;
            return;
        }

        event.accepted = start(steps);
    }

    property FrameAnimation momentumAnimation: FrameAnimation {
        onTriggered: kineticWheel.advance(frameTime)
    }

    property Timer pixelGestureEndTimer: Timer {
        interval: 45
        repeat: false
        onTriggered: kineticWheel.finishPixelGesture()
    }
}
