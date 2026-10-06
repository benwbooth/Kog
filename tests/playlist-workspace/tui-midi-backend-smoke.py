#!/usr/bin/env python3
"""Exercise the MIDI backend picker in the real TUI with private settings.

nix develop -c uv run --with pyte python tests/playlist-workspace/tui-midi-backend-smoke.py
"""
import codecs
from contextlib import closing
import fcntl
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

import pyte

repo = Path(__file__).resolve().parents[2]
choices = {
    "rustysynth-sf2": "RustySynth (SF2)",
    "opl3windows": "OPL3Windows (Nuked OPL3)",
    "nuked-sc55": "Nuked SC-55",
    "munt-mt32": "Munt (MT-32 / CM-32L)",
}
with tempfile.TemporaryDirectory(prefix="kog-midi-picker-") as directory:
    root = Path(directory)
    for name in ("config/kog", "data/kog", "cache", "runtime"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    (root / "config/kog/output-volume").write_text("0")
    (root / "config/kog/midi-engine").write_text("opl3windows")
    database = root / "data/kog/kog.db"

    def launch():
        pid, terminal = pty.fork()
        if pid == 0:
            os.environ.update(XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
                              XDG_CACHE_HOME=str(root / "cache"), XDG_RUNTIME_DIR=str(root / "runtime"), TERM="xterm-256color")
            os.environ.pop("KOG_SESSION_ID", None)
            binary = os.environ.get("KOG_TUI_BINARY", str(repo / "target/debug/kog-tui"))
            os.execv(binary, [binary])
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        screen = pyte.Screen(120, 40)
        return pid, terminal, screen, pyte.Stream(screen), codecs.getincrementaldecoder("utf-8")("replace")

    pid, terminal, screen, stream, decoder = launch()

    def drain(seconds=0.25):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([terminal], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                try:
                    stream.feed(decoder.decode(os.read(terminal, 65536)))
                except OSError:
                    break

    def send(keys):
        os.write(terminal, keys)
        drain()

    def saved():
        with closing(sqlite3.connect(database)) as db:
            row = db.execute("SELECT value FROM app_state WHERE namespace='preferences' AND key='midi-engine'").fetchone()
        return row[0]

    def locate(label):
        y, line = next((y, line) for y, line in enumerate(screen.display) if label in line)
        return line.rindex(label), y

    def click_label(label):
        x, y = locate(label)
        send(f"\x1b[<0;{x+2};{y+1}M\x1b[<0;{x+2};{y+1}m".encode())

    def open_picker():
        before = saved()
        send(b"m\x1bp")  # Main menu -> Alt+P: Preferences.
        click_label("MIDI Synthesis")
        click_label("MIDI Backend")
        assert "╭─ MIDI Backend " in "\n".join(screen.display), "Backend list did not open"
        assert saved() == before, "Opening the list changed the backend"
        for value, label in choices.items():
            x, y = locate(label)
            assert (screen.buffer[y][x - 2].data == "✓") == (value == before), "Incorrect current-backend marker"
        x, y = locate(choices[before])
        other = next(label for value, label in choices.items() if value != before)
        other_x, other_y = locate(other)
        assert screen.buffer[y][x].bg != screen.buffer[other_y][other_x].bg, "Current backend is not initially highlighted"

    try:
        drain(2)
        open_picker()
        send(b"\x1b[H")  # Browse without committing.
        assert saved() == "opl3windows"
        send(b"\x1b")
        assert "╭─ MIDI Backend " not in "\n".join(screen.display), "Escape did not return to synthesis settings"
        assert "╭─ MIDI Synthesis " in "\n".join(screen.display)
        assert saved() == "opl3windows", "Cancel changed the backend"
        send(b"m")

        # Jump straight to arbitrary choices with the mouse, including ROM
        # backends: selecting a preference does not require starting a track.
        for value in ("munt-mt32", "rustysynth-sf2", "nuked-sc55", "opl3windows"):
            open_picker()
            click_label(choices[value])
            assert saved() == value, "Mouse selection did not save the requested backend"
            assert "╭─ MIDI Backend " not in "\n".join(screen.display), "Selection did not close the list"
            assert "MIDI backend: " + choices[value] in "\n".join(screen.display)

        open_picker()
        send(b"\r")  # Re-select the highlighted current backend.
        assert saved() == "opl3windows"
        open_picker()
        send(b"\x1b[B\r")  # Down from OPL3Windows -> SC-55.
        assert saved() == "nuked-sc55", "Arrow/Enter selection failed"
        open_picker()
        send(b"\x1bm")  # Alt+M selects Munt directly.
        assert saved() == "munt-mt32", "Mnemonic selection failed"
        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        os.close(terminal)
        pid, terminal, screen, stream, decoder = launch()
        drain(2)
        open_picker()
        assert saved() == "munt-mt32", "Backend choice did not survive restart"
        send(b"\x1b[H")
        send(b"\x1b[<0;2;39M\x1b[<0;2;39m")
        assert saved() == "munt-mt32", "Clicking outside committed a pending choice"
        assert "╭─ MIDI Backend " not in "\n".join(screen.display)
        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        print("TUI MIDI PICKER PASS: all four mouse choices, current marker/highlight, arrows/Enter, mnemonic, cancellation, unchanged choice, persistence/restart")
    except BaseException:
        print("\n".join(screen.display))
        raise
    finally:
        if pid:
            os.kill(pid, signal.SIGTERM)
            os.waitpid(pid, 0)
        os.close(terminal)
