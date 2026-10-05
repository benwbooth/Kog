#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --locked -p kog-playback-policy
swiftc ios/Kog/Models.swift ios/Kog/SharedPlaybackPolicy.swift tests/ui-contract/SwiftContract.swift -L target/debug -lkog_playback_policy -o /tmp/kog-swift-ui-contract
/tmp/kog-swift-ui-contract tests/ui-contract/playlist.json tests/ui-contract/session.json
