#!/usr/bin/env python3
"""Copy the Breeze icons Kog names into qml/icons/breeze, as Kog's own icon theme.

Kog's menus, window buttons and toolbars name freedesktop icons
(scripts/breeze-icon-names.txt). Kog draws them from Breeze and Breeze Dark
copied into the binary (qml/icons/breeze/icons.qrc), never from the desktop's
icon theme, so they look the same on every desktop.

    scripts/vendor-breeze-icons.py /nix/store/...-breeze-icons-6.x/share/icons

Re-run after adding an icon name to the list. Only the plain size folders are
copied (the icons are SVG, so they stay sharp at any scale).
"""
import configparser
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "qml/icons/breeze"
NAMES = ROOT / "scripts/breeze-icon-names.txt"
THEMES = {"breeze": "", "breeze-dark": "breeze"}


def main() -> None:
    source = Path(sys.argv[1])
    names = [n.strip() for n in NAMES.read_text().split() if n.strip()]
    if OUT.exists():
        shutil.rmtree(OUT)
    files = []
    for theme, inherits in THEMES.items():
        src = source / theme
        index = configparser.ConfigParser(interpolation=None, strict=False)
        index.optionxform = str
        index.read(src / "index.theme")
        used = []
        for directory in index["Icon Theme"]["Directories"].split(","):
            if "@" in directory or directory not in index:
                continue
            copied = False
            for name in names:
                icon = src / directory / f"{name}.svg"
                if icon.exists():
                    dest = OUT / theme / directory / icon.name
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(icon.resolve(), dest)
                    files.append(dest.relative_to(OUT))
                    copied = True
            if copied:
                used.append(directory)
        lines = [
            "[Icon Theme]",
            f"Name={index['Icon Theme']['Name']}",
            "Comment=Breeze icons Kog uses, copied by scripts/vendor-breeze-icons.py",
        ]
        if inherits:
            lines.append(f"Inherits={inherits}")
        lines.append("Directories=" + ",".join(used))
        for directory in used:
            lines.append("")
            lines.append(f"[{directory}]")
            lines += [f"{k}={v}" for k, v in index[directory].items()]
        (OUT / theme / "index.theme").write_text("\n".join(lines) + "\n")
        files.append(Path(theme) / "index.theme")
    qrc = ['<!DOCTYPE RCC>', '<RCC version="1.0">', '  <qresource prefix="/kog/icons">']
    qrc += [f"    <file>{f.as_posix()}</file>" for f in sorted(files)]
    qrc += ["  </qresource>", "</RCC>"]
    (OUT / "icons.qrc").write_text("\n".join(qrc) + "\n")
    print(f"{len(files)} files in {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
