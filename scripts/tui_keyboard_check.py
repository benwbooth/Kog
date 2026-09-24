"""Exercise the TUI's main workflows with keyboard input only."""

import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import wave
from pathlib import Path

import pyte


with tempfile.TemporaryDirectory(prefix="kog-tui-keyboard-") as base:
    music = Path(base, "Music")
    album = music / "Album"
    album.mkdir(parents=True)
    for name in ("a.wav", "b.wav", "c.wav"):
        with wave.open(str(album / name), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(8000)
            output.writeframes(b"\0\0" * 80_000)

    env = os.environ.copy()
    for key, subdir in (
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ):
        env[key] = str(Path(base, subdir))
    env["TERM"] = "xterm-256color"

    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    binary = Path(
        os.environ.get(
            "KOG_TUI_TEST_BINARY",
            Path(__file__).resolve().parents[1] / "target/debug/kog-tui",
        )
    )
    process = subprocess.Popen(
        [str(binary)] if binary.name == "kog-tui" else [str(binary), "--tui"],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        env=env,
        close_fds=True,
    )
    os.close(slave)
    screen = pyte.Screen(120, 40)
    stream = pyte.Stream(screen)

    def drain(seconds=0.3):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([master], [], [], min(0.05, until - time.monotonic()))[0]:
                try:
                    data = os.read(master, 65_536)
                except OSError:
                    break
                if not data:
                    break
                stream.feed(data.decode("utf8", "replace"))

    def send(data, seconds=0.3):
        os.write(master, data if isinstance(data, bytes) else data.encode())
        drain(seconds)

    def wait_for(text, timeout=6):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if text in "\n".join(screen.display):
                return
            drain(0.1)
        raise AssertionError((text, screen.display[:18], screen.display[-4:]))

    try:
        wait_for("Search playlist")
        send("?")
        wait_for("Keyboard controls")
        send(b"\x1b", 0.4)
        send(b"\x1bv")  # Alt+V opens View directly from the main menu.
        wait_for("╭─ View")
        wait_for("Alt+F")
        send(b"\x1bf")  # Alt+F activates Supported Formats within View.
        wait_for(".m3u")
        send(b"\x1b", 0.4)
        send(b"\x1bp")  # Alt+P opens Preferences directly.
        wait_for("╭─ Preferences")
        send(b"\x1bv")  # Alt+V activates Volume within Preferences.
        wait_for("Volume 0-100")
        send(b"\x1b", 0.4)

        send("o")
        wait_for("Select Music Folder")
        send(b"\x0c\x01" + os.fsencode(music) + b"\r", 0.5)
        wait_for("Album")
        send(b"\x0f", 0.5)
        wait_for("Album")

        send("z")
        wait_for("▸ Files")
        send("z")
        wait_for("▾ Files")
        send(b"\x12")  # Ctrl+R refreshes Files.
        wait_for("Album")
        send("u")  # Printable fallback for terminals that do not forward Ctrl+R.
        wait_for("Album")

        send(b"\r")  # Expand Album.
        wait_for("a.wav")
        send(b"\x1b[B")  # Focus a.wav.
        send("x")  # Toggle off a.wav.
        send("J")  # Cursor-only move to b.wav.
        send("x")
        send("K")
        send("x")
        wait_for("Selected 2 tree items")
        send("v")  # Printable range-selection mode.
        send(b"\x1b[B")
        wait_for("Selected 2 tree items")
        send("v")
        send(b"\x1b[21;2~")  # Shift+F10 opens the selected file's context menu.
        wait_for("File")
        send(b"\x1ba", 0.5)  # Alt+A adds selected a.wav and b.wav.
        wait_for("Added 2 track(s)")

        send(b"\t")  # Playlist pane.
        send("H")  # Focus playlist header.
        wait_for("Title column")
        send("+")
        layouts = list(Path(env["XDG_CONFIG_HOME"]).rglob("tui-column-layout"))
        assert len(layouts) == 1 and "title,40,1" in layouts[0].read_text(), layouts
        send(b"\x1b[C")  # Artist column.
        wait_for("Artist column")
        send(b"\x1b[1;5D")  # Move Artist left.
        layout = layouts[0].read_text()
        assert layout.index("artist,") < layout.index("title,"), layout
        send(b"\r")  # Sort by focused column.
        send("M")
        wait_for("Columns")
        send(b"\x1b", 0.4)
        send("]")  # Printable fallback for column reordering.
        layout = layouts[0].read_text()
        assert layout.index("title,") < layout.index("artist,"), layout
        send("[")
        layout = layouts[0].read_text()
        assert layout.index("artist,") < layout.index("title,"), layout
        send("v")  # Hide Artist without a header right click.
        assert any(entry.startswith("artist,") and entry.endswith(",0") for entry in layouts[0].read_text().split(";"))
        send("a")  # Auto fit columns without double clicking a boundary.
        assert "title,40,1" not in layouts[0].read_text()
        send("H")  # Leave column focus.

        send(b"\x1b[1;5C")  # Resize sidebar without dragging.
        wait_for("Sidebar width:")
        send("}")
        wait_for("Sidebar width:")
        send(b"\x1b[21~")  # F10 opens the application menu.
        wait_for("╭─ Kog")
        send(b"\x1b", 0.4)
        send("M")
        wait_for("Playlist Row")
        send(b"\x1b", 0.4)

        send(b"\r")  # Play selected track.
        wait_for("Playing", 10)
        send("G")  # Exact seek, including hour-long tracks.
        wait_for("Seek to")
        send(b"\x01" + b"0:01\r")
        wait_for("Seeked to 0:01")
        send("C")
        wait_for("Compact Player")
        send("C")
        wait_for("Playlist view")

        send(b"\t")  # Saved playlists pane.
        send("z")
        wait_for("▸ Playlists")
        send("z")
        wait_for("▾ Playlists")

        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 20, 48, 0, 0))
        screen.resize(lines=20, columns=48)
        process.send_signal(signal.SIGWINCH)
        drain(0.5)
        send(b"\t")  # Files remain accessible at a narrow terminal width.
        send("M")
        wait_for("File")
        send(b"\x1b", 0.4)
        send("H")
        wait_for("Title column")
        send("M")
        wait_for("Columns")
        assert "Alt+" in "\n".join(screen.display)
        send(b"\x1bc")  # Open the column visibility submenu by its accelerator.
        wait_for("Visible Columns")
        send(b"\x1bu")  # Toggle Bitrate, below the visible page of the narrow menu.
        assert any(entry.startswith("bitrate,") and entry.endswith(",1") for entry in layouts[0].read_text().split(";"))
        send("H")
        send("?")
        wait_for("Keyboard controls")
        send(b"\x1b[6~")  # Page Down scrolls the help modal.
        send(b"\x1b", 0.4)
        send("m")
        wait_for("╭─ Kog")
        send(b"\r")  # Add File from the application menu.
        wait_for("Add file or folder")
        send(os.fsencode(album / "c.wav") + b"\r")
        wait_for("Added c.wav")
        send("q")
        process.wait(timeout=5)
        assert process.returncode == 0, process.returncode
        print("keyboard-only TUI navigation, selection, context menus, columns, seek, compact view: PASS")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)
