#!/usr/bin/env python3
"""Exercise the actual terminal UI in a private PTY and settings directory."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import sqlite3
import struct
import tempfile
import termios
import time

repo = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="kog-playlist-tui-") as directory:
    root = Path(directory)
    for name in ("config/kog", "data/kog", "cache", "runtime"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    (root / "config/kog/output-volume").write_text("0")
    database = root / "data/kog/kog.db"
    with sqlite3.connect(database) as db:
        db.executescript("""
            CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL);
            CREATE TABLE playlist_entries(id INTEGER PRIMARY KEY AUTOINCREMENT, playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, kind TEXT NOT NULL, path TEXT NOT NULL, entry TEXT NOT NULL DEFAULT '', fragment TEXT);
            INSERT INTO playlists(name,position) VALUES('Workspace smoke',0);
        """)
        for position in range(2):
            db.execute("INSERT INTO playlist_entries(playlist_id,position,kind,path) VALUES(1,?,'local',?)", (position, str(repo / "tests/fixtures/codec-libs/tone.wav")))
    def launch():
        pid, terminal = pty.fork()
        if pid == 0:
            os.environ.update(XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"), XDG_RUNTIME_DIR=str(root / "runtime"), TERM="xterm-256color")
            os.execv(str(repo / "target/debug/kog-tui"), ["kog-tui"])
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 34, 120, 0, 0))
        return pid, terminal
    pid, terminal = launch()
    output = bytearray()
    def drain(seconds=0.3):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([terminal], [], [], min(0.05, max(0, deadline-time.monotonic())))[0]:
                try: output.extend(os.read(terminal, 65536))
                except OSError: break
    def send(keys):
        os.write(terminal, keys)
        drain()
    def checkpoint():
        with sqlite3.connect(database) as db:
            row = db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='tui:default'").fetchone()
        return json.loads(row[0]) if row else None
    def state():
        value=checkpoint()
        return value["workspace"] if value else None
    def wait(predicate, message):
        for _ in range(60):
            drain(0.1)
            value=state()
            if value is not None and predicate(value): return value
        raise AssertionError(message)
    def count():
        with sqlite3.connect(database) as db: return db.execute("SELECT count(*) FROM playlist_entries WHERE playlist_id=1").fetchone()[0]
    try:
        drain(2)
        send(b"\x1b[Z")  # Library -> Playlists
        send(b"\x1b[B")  # Favorites -> saved playlist
        send(b"\r")
        wait(lambda value:value["active"]=="local:1" and len(value["tabs"][0]["draft"]["rows"])==2,"Enter should open a draft tab")
        send(b"\x13")  # disabled Save on clean draft must be a no-op
        assert count()==2,"Clean Save changed stored contents"
        send(b"\x1b[H")  # Home: replace selection at first row
        send(b"\x1b[1;2B")  # Shift+Down: range from original anchor
        wait(lambda value:len(value["tabs"][0]["draft"]["selected"])==2,"Range selects both rows")
        send(b"\x1b[1;2A")  # Shift+Up: shrink range
        wait(lambda value:len(value["tabs"][0]["draft"]["selected"])==1,"Range shrinks to anchor")
        send(b"d")
        wait(lambda value:len(value["tabs"][0]["draft"]["rows"])==1,"Delete draft row")
        assert count()==2,"Draft edit wrote through before Save"
        send(b"\x13")  # Ctrl+S
        assert count()==1,"Save did not reach the shared database"
        send(b"u")
        send(b"\x17")  # Ctrl+W
        wait(lambda value:value["pending_close"]=="local:1","Dirty close must ask")
        send(b"c")
        wait(lambda value:value["pending_close"] is None and value["active"]=="local:1","Cancel keeps draft")
        send(b"\x01")  # Ctrl+A: both draft rows
        send(b"p")  # Play Now via backend expansion
        drain(2)
        saved=checkpoint()
        assert len(saved["queue"])==2 and saved["current"]==1, "Native EOS should advance through both duplicate rows"
        assert saved["volume"]==0 and b"Playing" in output, "Muted native output did not start"
        send(b"\x17"); send(b"d")
        wait(lambda value:value["active"]=="queue" and not value["tabs"],"Discard closes editor and returns to queue")
        assert count()==1,"Discard changed saved contents"
        assert b"Play Queue" in output and b"Play Next" in output,"Tab controls were not rendered"
        send(b"\x03")
        os.waitpid(pid,0)
        pid=0
        os.close(terminal)
        output.clear()
        pid, terminal = launch()
        drain(2)
        assert len(checkpoint()["queue"])==2 and checkpoint()["current"]==1, "Restored session lost its queue"
        assert b"Ready to play" in output, "Restored output must stay stopped"
        send(b"\x03")
        os.waitpid(pid,0)
        pid=0
        print("TUI WORKSPACE PASS: editor/selection/save/undo/close/cancel/discard/native audio/EOS/stopped restore")
    finally:
        if pid:
            os.kill(pid,signal.SIGTERM)
            os.waitpid(pid,0)
        os.close(terminal)
