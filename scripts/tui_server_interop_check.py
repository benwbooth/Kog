"""Exercise the terminal remote browser against Kog's real headless server."""

import fcntl
import json
import os
import pty
import select
import signal
import socket
import struct
import subprocess
import tempfile
import termios
import time
import urllib.parse
import urllib.error
import urllib.request
import wave
import zipfile
from pathlib import Path

import pyte


with tempfile.TemporaryDirectory(prefix="kog-tui-interop-") as root:
    root = Path(root)
    music = root / "Music"
    album = music / "album"
    album.mkdir(parents=True)
    wav = root / "song.wav"
    with wave.open(str(wav), "wb") as sound:
        sound.setnchannels(1)
        sound.setsampwidth(2)
        sound.setframerate(8000)
        sound.writeframes(b"\0\0" * 80000)
    with zipfile.ZipFile(album / "pack.zip", "w") as archive:
        archive.write(wav, "inner/sound.wav")

    env = os.environ.copy()
    for key, folder in (
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ):
        env[key] = str(root / folder)
    env["TERM"] = "xterm-256color"
    settings = root / "config" / "kog"
    settings.mkdir(parents=True)
    (settings / "music-directory").write_text(str(music))
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        port = available.getsockname()[1]
    token = "TUI real server fixture token"
    (settings / "server.json").write_text(
        json.dumps({"enabled": True, "address": "127.0.0.1", "port": port, "auth": "token", "token": token})
    )
    executable = str(Path(__file__).resolve().parents[1] / "target/debug/kog")
    with (root / "server.log").open("wb") as server_log:
        server = subprocess.Popen([executable, "--server"], env=env, stdout=server_log, stderr=server_log)
        master = None
        tui = None
        try:
            address = f"http://127.0.0.1:{port}"
            for _ in range(100):
                if server.poll() is not None:
                    raise AssertionError(("server exited", (root / "server.log").read_text()))
                try:
                    urllib.request.urlopen(address + "/api/health", timeout=0.5).close()
                    break
                except Exception:
                    time.sleep(0.1)
            else:
                raise AssertionError(("server did not start", (root / "server.log").read_text()))

            def api(path):
                request = urllib.request.Request(address + path, headers={"Authorization": "Bearer " + token})
                with urllib.request.urlopen(request, timeout=10) as response:
                    return json.load(response)

            archive_path = str(album / "pack.zip")
            root_listing = api("/api/library")
            assert any(item["name"] == "album" for item in root_listing["directories"])
            archive_listing = api("/api/library?path=" + urllib.parse.quote(archive_path))
            assert any(item["name"] == "inner" for item in archive_listing["directories"])
            inner = api("/api/library?path=" + urllib.parse.quote(archive_path + "/inner"))
            assert any(item["entry"] == "inner/sound.wav" for item in inner["files"]), inner
            stream_query = urllib.parse.urlencode({
                "kind": "archive", "path": archive_path, "entry": "inner/sound.wav", "codec": "aac"
            })
            stream_request = urllib.request.Request(
                address + "/api/stream?" + stream_query,
                headers={"Authorization": "Bearer " + token},
            )
            try:
                with urllib.request.urlopen(stream_request, timeout=20) as response:
                    assert response.status == 200 and response.read(64)
            except urllib.error.HTTPError as error:
                raise AssertionError((error.code, error.read().decode(), (root / "server.log").read_text())) from error
            probe_query = urllib.parse.urlencode({
                "kind": "archive", "path": archive_path, "entry": "inner/sound.wav", "codec": "aac", "token": token
            })
            probe = subprocess.run(
                ["ffprobe", "-v", "error", "-show_format", address + "/api/stream?" + probe_query],
                capture_output=True, text=True, timeout=20,
            )
            assert probe.returncode == 0, (probe.stderr, (root / "server.log").read_text())

            master, slave = pty.openpty()
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 240, 0, 0))
            tui = subprocess.Popen([executable, "--tui"], stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True)
            os.close(slave)
            screen = pyte.Screen(240, 40)
            stream = pyte.Stream(screen)

            def drain(seconds=0.2):
                end = time.time() + seconds
                while time.time() < end:
                    ready, _, _ = select.select([master], [], [], min(0.05, max(0, end - time.time())))
                    if ready:
                        try:
                            data = os.read(master, 65536)
                        except OSError:
                            return
                        stream.feed(data.decode("utf8", "replace"))

            def send(value, seconds=0.2):
                os.write(master, value if isinstance(value, bytes) else value.encode())
                drain(seconds)

            def click(x, y):
                send(f"\x1b[<0;{x+1};{y+1}M\x1b[<0;{x+1};{y+1}m")

            def wait_for(value, seconds=10):
                end = time.time() + seconds
                while time.time() < end:
                    if value in "\n".join(screen.display):
                        return
                    drain(0.1)
                diagnostics = root / "config" / "kog" / "tui-diagnostics.log"
                raise AssertionError((value, screen.display[:18], screen.display[-3:], (root / "server.log").read_text(), diagnostics.read_text() if diagnostics.exists() else ""))

            def row(value):
                return next(index for index, line in enumerate(screen.display) if value in line[:50])

            wait_for("Search playlist")
            click(5, 0)
            click(10, 15)
            click(10, 2)
            send(address + "\r")
            click(5, 0)
            click(10, 15)
            click(10, 3)
            send(token + "\r")
            wait_for("Connected to " + address)
            click(10, row("album"))
            wait_for("pack.zip")
            click(10, row("pack.zip"))
            wait_for("inner")
            click(10, row("inner"))
            wait_for("sound.wav")
            click(10, row("sound.wav"))
            click(10, row("sound.wav"))
            wait_for("Playing sound.wav", 20)
            drain(0.8)
            playlist_title = screen.display[2].split("│", 1)[-1]
            assert "sound" in playlist_title and "stream" not in playlist_title, playlist_title
            click(10, 3)
            send("sound", 0.6)
            wait_for("remote matches for sound", 20)
            print("real headless server remote archive browse, stream, and search: PASS")
        finally:
            if tui is not None and tui.poll() is None:
                tui.terminate()
                tui.wait(timeout=5)
            if master is not None:
                os.close(master)
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.terminate()
                    server.wait(timeout=5)
