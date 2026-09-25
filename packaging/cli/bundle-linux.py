#!/usr/bin/env python3
"""Bundle ELF dependencies beside a Kog command-line executable."""

import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


SYSTEM_LIBRARIES = {
    "libc.so.6", "libm.so.6", "libpthread.so.0", "librt.so.1",
    "libdl.so.2", "libresolv.so.2", "libutil.so.1", "libanl.so.1",
    "ld-linux-x86-64.so.2", "linux-vdso.so.1",
}
DEPENDENCY = re.compile(r"^\s*(\S+)\s+=>\s+(\S+)")


def linked_libraries(binary: Path) -> dict[str, Path]:
    env = os.environ.copy()
    env.pop("LD_LIBRARY_PATH", None)
    result = subprocess.run(["ldd", str(binary)], capture_output=True, text=True, env=env)
    output = result.stdout + result.stderr
    if "not a dynamic executable" in output or "statically linked" in output:
        return {}
    if result.returncode:
        raise RuntimeError(f"ldd failed for {binary}: {output}")
    libraries = {}
    for line in output.splitlines():
        match = DEPENDENCY.match(line)
        if not match:
            continue
        name, location = match.groups()
        name = Path(name).name
        if location == "not":
            raise RuntimeError(f"unresolved dependency of {binary}: {name}")
        if name not in SYSTEM_LIBRARIES:
            if not location.startswith("/"):
                raise RuntimeError(f"unresolved dependency of {binary}: {line}")
            libraries[name] = Path(location)
    return libraries


def main(directory: Path) -> None:
    if not shutil.which("patchelf"):
        raise RuntimeError("patchelf is required to make the Linux archive relocatable")
    executables = sorted(
        path for path in directory.iterdir() if path.is_file() and os.access(path, os.X_OK)
    )
    library_dir = directory / "lib"
    library_dir.mkdir(exist_ok=True)
    sources = {}
    for executable in executables:
        for name, source in linked_libraries(executable).items():
            previous = sources.get(name)
            if previous and previous.resolve() != source.resolve():
                if previous.read_bytes() != source.read_bytes():
                    raise RuntimeError(f"conflicting copies of {name}: {previous} and {source}")
            sources[name] = source
    for name, source in sources.items():
        shutil.copy2(source, library_dir / name, follow_symlinks=True)

    for binary in executables:
        if linked_libraries(binary):
            subprocess.run(["patchelf", "--set-rpath", "$ORIGIN/lib", str(binary)], check=True)
    for library in library_dir.iterdir():
        library.chmod(library.stat().st_mode | 0o200)
        subprocess.run(["patchelf", "--set-rpath", "$ORIGIN", str(library)], check=True)

    for binary in [*executables, *library_dir.iterdir()]:
        for name, location in linked_libraries(binary).items():
            if location.resolve().parent != library_dir.resolve():
                raise RuntimeError(f"{binary} still needs host library {name}: {location}")
    print(f"Bundled {len(sources)} Linux libraries in {directory.name}")


if __name__ == "__main__":
    try:
        main(Path(sys.argv[1]).resolve())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(f"Linux CLI packaging: {error}")
