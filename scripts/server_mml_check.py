"""Record an NSF's MML score through the server API used by the web player."""

import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SONG = ROOT / "native/game-music-emu/test.nsf"

with tempfile.TemporaryDirectory(prefix="kog-server-mml-") as base:
    root = Path(base)
    music = root / "Music"
    music.mkdir()
    shutil.copy(SONG, music / "test.nsf")
    env = os.environ.copy()
    for key, folder in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")):
        env[key] = str(root / folder)
    settings = root / "config" / "kog"
    settings.mkdir(parents=True)
    (settings / "music-directory").write_text(str(music))
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        port = available.getsockname()[1]
    token = "MML fixture token"
    (settings / "server.json").write_text(
        json.dumps({"enabled": True, "address": "127.0.0.1", "port": port, "auth": "token", "token": token})
    )
    address = f"http://127.0.0.1:{port}"
    if os.environ.get("KOG_MML_SERVER_INFO"):
        print(address, token, music / "test.nsf", flush=True)
    with (root / "server.log").open("wb") as log:
        server = subprocess.Popen([str(ROOT / "target/debug/kog"), "--server"], env=env, stdout=log, stderr=log)
        try:
            for _ in range(100):
                try:
                    urllib.request.urlopen(address + "/api/health", timeout=0.5).close()
                    break
                except Exception:
                    time.sleep(0.1)
            query = {"kind": "local", "path": str(music / "test.nsf"), "codec": "aac"}

            def score(have=None):
                parameters = dict(query, **({"have": have} if have is not None else {}))
                request = urllib.request.Request(
                    address + "/api/mml?" + urllib.parse.urlencode(parameters),
                    headers={"Authorization": "Bearer " + token},
                )
                with urllib.request.urlopen(request, timeout=20) as response:
                    return json.load(response)

            reply = score()
            assert reply["status"] in ("recording", "ready"), reply
            partial = None
            until = time.monotonic() + 180
            while reply["status"] == "recording" and time.monotonic() < until:
                if reply.get("document") and partial is None:
                    partial = reply["revision"]
                time.sleep(0.5)
                reply = score(reply["revision"] if reply.get("document") else None)
            assert reply["status"] == "ready", reply
            assert partial is not None, "a partial score is served while recording"
            unchanged = score(reply["revision"])
            assert "document" not in unchanged, "a held revision is not resent"
            document = score()["document"]
            text = document["text"]
            assert text.startswith("#KOG-MML 1\n"), text[:80]
            assert document["bars"] and document["tracks"], document.keys()
            notes = [span for span in document["spans"] if span["kind"] == "note"]
            assert notes and all(text[s["from"]:s["to"]][0] in "abcdefg&^<>" for s in notes)
            if os.environ.get("KOG_MML_SERVER_INFO"):
                input("Server running; press Enter to stop\n")
            print(f"server MML score: {len(document['bars'])} bars, {len(notes)} note pieces, partial revision {partial}: PASS")
        finally:
            server.terminate()
            server.wait(timeout=10)
