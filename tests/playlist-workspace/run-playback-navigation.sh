#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-playback-navigation.XXXXXX)"
trap 'cat "$test_dir/run.log"; rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir"/{config,data,runtime,cache,qml,tracks}
chmod 700 "$test_dir/runtime"
touch "$test_dir/run.log"
python3 - "$repo_dir" "$test_dir" <<'PREPARE'
from pathlib import Path
import json,sys,wave
repo,out=map(Path,sys.argv[1:])
for file in (repo/'qml').iterdir():
    if file.name != 'Main.qml': (out/'qml'/file.name).symlink_to(file)
(out/'qml/Checks').symlink_to(repo/'tests/playlist-workspace')
with wave.open(str(repo/'tests/fixtures/codec-libs/tone.wav')) as wav:
    params=wav.getparams(); audio=wav.readframes(wav.getnframes())
for name in ('outside-1','outside-2','first','middle','last'):
    with wave.open(str(out/'tracks'/f'{name}.wav'),'wb') as wav:
        wav.setparams(params); wav.writeframes(audio*30)
source=(repo/'qml/Main.qml').read_text().replace('import QtQuick\n','import QtQuick\nimport "Checks" as Checks\n',1)
i=source.rfind('}')
source=source[:i]+'\n    Checks.PlaybackNavigationSmoke { window: root; app: appController; editor: playlistEditor; fixtures: '+json.dumps(str(out/'tracks'))+' }\n'+source[i:]
(out/'qml/Main.qml').write_text(source)
PREPARE
XDG_CONFIG_HOME="$test_dir/config" XDG_DATA_HOME="$test_dir/data" XDG_CACHE_HOME="$test_dir/cache" \
XDG_RUNTIME_DIR="$test_dir/runtime" XDG_CURRENT_DESKTOP= QT_QUICK_CONTROLS_STYLE="${QT_QUICK_CONTROLS_STYLE:-Basic}" QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME="${QT_QPA_PLATFORMTHEME:-basic}" QT_QUICK_BACKEND=software \
KOG_QML_DIR="$test_dir/qml" QTWEBENGINE_DISABLE_SANDBOX=1 unshare --user --map-root-user dbus-run-session -- timeout 35 "$repo_dir/target/debug/kog" --gui > "$test_dir/run.log" 2>&1
python3 - "$test_dir/data/kog/kog.db" <<'CHECK'
from pathlib import Path
import json,sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    failures=db.execute("SELECT name FROM playlists WHERE name LIKE 'PLAYBACK FAIL:%'").fetchall()
    assert not failures, failures
    assert db.execute("SELECT count(*) FROM playlists WHERE name='Playlist navigation complete'").fetchone()[0] == 1, 'Playback navigation smoke did not complete'
    state=json.loads(db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='qt:default'").fetchone()[0])
    assert [Path(track['path']).stem for track in state['queue']] == ['outside-1','outside-2','first','middle','last']
    assert state['current'] == 2 and state['workspace']['active'] == 'queue'
    assert db.execute('SELECT count(*) FROM playlist_entries').fetchone()[0] == 3
print('QT PLAYLIST NAVIGATION PASS: real transport and buttons, retained queue, and saved playlist verified')
CHECK
if rg -q 'TypeError|ReferenceError|Binding loop|Cannot assign' "$test_dir/run.log"; then exit 1; fi
