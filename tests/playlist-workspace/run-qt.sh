#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-playlist-workspace.XXXXXX)"
trap 'rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir"/{config,data,runtime,cache}
chmod 700 "$test_dir/runtime"
XDG_CONFIG_HOME="$test_dir/config" XDG_DATA_HOME="$test_dir/data" XDG_CACHE_HOME="$test_dir/cache" \
XDG_RUNTIME_DIR="$test_dir/runtime" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
KOG_QML_DIR="$repo_dir/tests/playlist-workspace" QTWEBENGINE_DISABLE_SANDBOX=1 unshare --user --map-root-user dbus-run-session -- timeout 25 "$repo_dir/target/debug/kog" --gui > "$test_dir/run.log" 2>&1
cat "$test_dir/run.log"
rg -q 'WORKSPACE PASS' "$test_dir/run.log"
if rg -q 'WORKSPACE FAIL|Binding loop|TypeError|ReferenceError|Cannot assign' "$test_dir/run.log"; then exit 1; fi
