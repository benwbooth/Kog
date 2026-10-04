#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-playlist-workspace.XXXXXX)"
trap 'cat "$test_dir/run.log"; rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir"/{config,data,runtime,cache}
chmod 700 "$test_dir/runtime"
XDG_CONFIG_HOME="$test_dir/config" XDG_DATA_HOME="$test_dir/data" XDG_CACHE_HOME="$test_dir/cache" \
XDG_RUNTIME_DIR="$test_dir/runtime" XDG_CURRENT_DESKTOP= QT_QUICK_CONTROLS_STYLE=Basic QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=basic QT_QUICK_BACKEND=software \
KOG_QML_DIR="$repo_dir/tests/playlist-workspace" QTWEBENGINE_DISABLE_SANDBOX=1 unshare --user --map-root-user dbus-run-session -- timeout 25 "$repo_dir/target/debug/kog" --gui > "$test_dir/run.log" 2>&1
python3 - "$test_dir/data/kog/kog.db" <<'PYTEST'
import sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    assert db.execute("SELECT count(*) FROM playlists WHERE name='Workspace smoke complete'").fetchone()[0] == 1, "QML smoke did not complete"
    assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0] == 1
print("QT WORKSPACE PASS: open/edit/save/queue/close/undo/rename/focus")
PYTEST
if rg -q 'WORKSPACE FAIL|Binding loop|TypeError|ReferenceError|Cannot assign' "$test_dir/run.log"; then exit 1; fi
