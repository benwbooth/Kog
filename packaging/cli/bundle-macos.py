#!/usr/bin/env python3
"""Copy and relink third-party macOS dylibs into a relocatable CLI archive."""

from collections import deque
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


SYSTEM_PREFIXES = ("/usr/lib/", "/System/Library/")


def output(*command: str) -> str:
    return subprocess.check_output(command, text=True)


def dependencies(binary: Path, is_library: bool = False) -> list[str]:
    lines = output("otool", "-L", str(binary)).splitlines()[1:]
    entries = [line.strip().split(" (", 1)[0] for line in lines]
    return entries[1:] if is_library else entries


def runpaths(binary: Path) -> list[str]:
    lines = output("otool", "-l", str(binary)).splitlines()
    paths = []
    for index, line in enumerate(lines):
        if line.strip() == "cmd LC_RPATH":
            for candidate in lines[index + 1:index + 5]:
                match = re.match(r"\s*path (.+) \(offset \d+\)", candidate)
                if match:
                    paths.append(match.group(1))
                    break
    return paths


def resolve(name: str, binary: Path, directory: Path, brew_prefix: Path) -> Path:
    if name.startswith("/"):
        candidates = [Path(name)]
    elif name.startswith("@loader_path/"):
        candidates = [binary.parent / name.removeprefix("@loader_path/")]
    elif name.startswith("@executable_path/"):
        candidates = [directory / name.removeprefix("@executable_path/")]
    elif name.startswith("@rpath/"):
        suffix = name.removeprefix("@rpath/")
        candidates = []
        for path in runpaths(binary):
            path = path.replace("@loader_path", str(binary.parent))
            path = path.replace("@executable_path", str(directory))
            candidates.append(Path(path) / suffix)
        candidates.append(brew_prefix / "lib" / suffix)
        candidates.extend((brew_prefix / "opt").glob(f"*/lib/{suffix}"))
    else:
        raise RuntimeError(f"unsupported dependency name in {binary}: {name}")
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise RuntimeError(f"cannot resolve dependency of {binary}: {name}")


def main(directory: Path) -> None:
    brew_prefix = Path(output("brew", "--prefix").strip())
    library_dir = directory / "lib"
    library_dir.mkdir()
    queue = deque(
        (path, path) for path in sorted(directory.iterdir())
        if path.is_file() and os.access(path, os.X_OK)
    )
    changes = {}
    seen = set()
    while queue:
        binary, original = queue.popleft()
        if binary in seen:
            continue
        seen.add(binary)
        changes[binary] = []
        for name in dependencies(original, binary.parent == library_dir):
            if name.startswith(SYSTEM_PREFIXES):
                continue
            source = resolve(name, original, directory, brew_prefix)
            if str(source).startswith(SYSTEM_PREFIXES):
                continue
            if ".framework/" in name:
                raise RuntimeError(f"unsupported non-system framework: {name}")
            destination = library_dir / Path(name).name
            if not destination.exists():
                shutil.copy2(source, destination, follow_symlinks=True)
                queue.append((destination, source))
            elif destination.resolve() != source and destination.read_bytes() != source.read_bytes():
                raise RuntimeError(f"conflicting copies of {destination.name}: {source}")
            target = f"@loader_path/{'lib/' if binary.parent == directory else ''}{destination.name}"
            changes[binary].append((name, target))

    for binary, replacements in changes.items():
        binary.chmod(binary.stat().st_mode | 0o200)
        if binary.parent == library_dir:
            subprocess.run(
                ["install_name_tool", "-id", f"@loader_path/{binary.name}", str(binary)],
                check=True,
            )
        for original, replacement in replacements:
            subprocess.run(
                ["install_name_tool", "-change", original, replacement, str(binary)],
                check=True,
            )
        subprocess.run(["codesign", "--force", "--sign", "-", str(binary)], check=True)
        subprocess.run(["codesign", "--verify", "--strict", str(binary)], check=True)

    for binary in changes:
        for name in dependencies(binary, binary.parent == library_dir):
            if name.startswith(SYSTEM_PREFIXES):
                continue
            if not name.startswith("@loader_path/"):
                raise RuntimeError(f"{binary} still refers to an external library: {name}")
            if not (binary.parent / name.removeprefix("@loader_path/")).is_file():
                raise RuntimeError(f"missing bundled dependency of {binary}: {name}")
    print(f"Bundled {len(list(library_dir.iterdir()))} macOS libraries in {directory.name}")


if __name__ == "__main__":
    try:
        main(Path(sys.argv[1]).resolve())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(f"macOS CLI packaging: {error}")
