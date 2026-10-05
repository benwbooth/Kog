#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-playlist-sidebar.XXXXXX)"
trap 'cat "$test_dir/run.log"; rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir"/{config,data,runtime,cache,qml}
chmod 700 "$test_dir/runtime"
touch "$test_dir/run.log"
python3 - "$repo_dir" "$test_dir" <<'PREPARE'
from pathlib import Path
import json,sys
repo,out=map(Path,sys.argv[1:])
for source in (repo/'qml').iterdir():
    if source.name != 'Main.qml': (out/'qml'/source.name).symlink_to(source)
(out/'qml/Checks').symlink_to(repo/'tests/playlist-workspace')
source=(repo/'qml/Main.qml').read_text().replace('import QtQuick\n','import QtQuick\nimport "Checks" as Checks\n',1)
i=source.rfind('}')
fixture=str(repo/'tests/fixtures/codec-libs/tone.wav')
source=source[:i]+'\n    Checks.SidebarSmoke { window: root; app: appController; fixture: '+json.dumps(fixture)+' }\n'+source[i:]
(out/'qml/Main.qml').write_text(source)
PREPARE
XDG_CONFIG_HOME="$test_dir/config" XDG_DATA_HOME="$test_dir/data" XDG_CACHE_HOME="$test_dir/cache" \
XDG_RUNTIME_DIR="$test_dir/runtime" XDG_CURRENT_DESKTOP= QT_QUICK_CONTROLS_STYLE=Basic QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=basic QT_QUICK_BACKEND=software \
KOG_QML_DIR="$test_dir/qml" QTWEBENGINE_DISABLE_SANDBOX=1 unshare --user --map-root-user dbus-run-session -- timeout 30 "$repo_dir/target/debug/kog" --gui > "$test_dir/run.log" 2>&1
python3 - "$test_dir/data/kog/kog.db" <<'CHECK'
import json,sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    failures=db.execute("SELECT name FROM playlists WHERE name LIKE 'SIDEBAR FAIL:%'").fetchall()
    assert not failures, failures
    assert db.execute("SELECT count(*) FROM playlists WHERE name='Sidebar routing complete'").fetchone()[0] == 1, 'Sidebar smoke did not complete'
    state=json.loads(db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='qt:default'").fetchone()[0])
    assert state['queue'] == []
    assert db.execute('SELECT count(*) FROM playlist_entries').fetchone()[0] == 3, 'Appending a draft must not save it automatically'
print('QT SIDEBAR PASS: select/open/reuse/plus/named destination/pane and tab drops/context append/undo/read-only')
CHECK
if rg -q 'TypeError|ReferenceError|Binding loop|Cannot assign' "$test_dir/run.log"; then exit 1; fi
