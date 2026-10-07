"""Check transport glyph click targets at the width where Radio used to overlap volume."""

import fcntl
import os
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
import wave
from pathlib import Path

import pyte


with tempfile.TemporaryDirectory(prefix="kog-tui-transport-") as base:
    paths = [Path(base, name) for name in ("a.wav", "b.wav")]
    for path in paths:
        with wave.open(str(path), "wb") as output:
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
    binary = Path(
        os.environ.get(
            "KOG_TUI_TEST_BINARY",
            Path(__file__).resolve().parents[1] / "target/debug/kog-tui",
        )
    )
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 72, 0, 0))
    process = subprocess.Popen(
        [str(binary)] if binary.name == "kog-tui" else [str(binary), "--tui"],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        env=env,
        close_fds=True,
    )
    os.close(slave)
    screen = pyte.Screen(72, 24)
    stream = pyte.Stream(screen)

    def drain(seconds=0.2):
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

    def send(data, seconds=0.2):
        os.write(master, data if isinstance(data, bytes) else data.encode())
        drain(seconds)

    def wait_for(value, timeout=8):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if value in "\n".join(screen.display):
                return
            drain(0.1)
        raise AssertionError((value, screen.display[-5:]))

    def wait_playing(title, timeout=8):
        # The status line says only "Playing"; the footer names the track.
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            footer = screen.display[20].split()
            if "Playing" in screen.display[21] and title in footer[:3]:
                return
            drain(0.1)
        raise AssertionError((title, screen.display[-5:]))

    def glyph_x(glyph, row=20):
        for column in range(screen.columns):
            if screen.buffer[row][column].data == glyph:
                return column
        raise AssertionError((glyph, screen.display[row]))

    def click_glyph(glyph):
        column = glyph_x(glyph)
        send(f"\x1b[<0;{column + 1};21M\x1b[<0;{column + 1};21m")

    try:
        wait_for("Search playlist")
        assert "⚄" in screen.display[20]
        assert "♪" in screen.display[22]
        click_glyph("⚄")
        wait_for("Random Radio: on")
        click_glyph("⚄")
        wait_for("Random Radio: off")

        for count, path in enumerate(paths, 1):
            send(b"\x1ba")  # Main-menu Add File accelerator.
            wait_for("Add file or folder")
            send(os.fsencode(path) + b"\r")
            # Track preparation replaces the "Added" status, so count rows.
            wait_for(f"{count} tracks")
        click_glyph("▶")
        wait_playing("a")
        click_glyph("⏭")
        wait_playing("b")
        click_glyph("⏮")
        wait_playing("a")
        send("q", 0.5)
        if "Confirm exit" in "\n".join(screen.display):
            y = next(y for y, line in enumerate(screen.display) if "[Exit]" in line)
            x = screen.display[y].find("[Exit]") + 2
            send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")
        process.wait(timeout=5)
        assert process.returncode == 0, process.returncode
        print("72-column TUI transport glyph clicks and Radio/volume separation: PASS")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)
