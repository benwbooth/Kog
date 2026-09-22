#!/usr/bin/env python3
"""Generate matching dark and light 24 px format pictograms for Qt and web.

Keep one geometry per family. Qt loads the two explicit ink colors; web inlines
the dark SVG and tints its strokes with CSS to match the current row text.
"""

from pathlib import Path


ICONS = {
    "gameboy": '''
        <rect x="5" y="2" width="14" height="20" rx="3"/>
        <rect x="7.5" y="4.5" width="9" height="7" rx=".7"/>
        <path d="M8 16h4M10 14v4"/>
        <circle cx="15" cy="15" r=".8" fill="{ink}" stroke="none"/>
        <circle cx="17" cy="17" r=".8" fill="{ink}" stroke="none"/>
    ''',
    "nes": '''
        <rect x="2" y="7.5" width="20" height="9" rx="1.7"/>
        <path d="M5 12h4M7 10v4M10.5 14h3"/>
        <circle cx="16" cy="12" r="1"/>
        <circle cx="19" cy="12" r="1"/>
    ''',
    "snes": '''
        <path d="M6.5 7h11a4.5 4.5 0 0 1 4.3 5.8l-1 3.2a2 2 0 0 1-3.2 1L15 15H9l-2.6 2a2 2 0 0 1-3.2-1l-1-3.2A4.5 4.5 0 0 1 6.5 7Z"/>
        <path d="M5.5 11.5h4M7.5 9.5v4M10.5 14h3"/>
        <circle cx="16" cy="10.5" r=".65" fill="{ink}" stroke="none"/>
        <circle cx="18" cy="12" r=".65" fill="{ink}" stroke="none"/>
        <circle cx="15" cy="13" r=".65" fill="{ink}" stroke="none"/>
    ''',
    "gba": '''
        <path d="M6 6.5h12a4 4 0 0 1 3.8 5.3l-1.5 4.4a2 2 0 0 1-2.9 1.1L15 15.5H9l-2.4 1.8a2 2 0 0 1-2.9-1.1l-1.5-4.4A4 4 0 0 1 6 6.5Z"/>
        <rect x="9" y="8.5" width="6" height="5" rx=".5"/>
        <path d="M4.5 11.5h3M6 10v3"/>
        <circle cx="18" cy="11" r=".8" fill="{ink}" stroke="none"/>
    ''',
    "ds": '''
        <rect x="5" y="2" width="14" height="20" rx="2"/>
        <rect x="7.5" y="4.3" width="9" height="6" rx=".5"/>
        <path d="M5 12h14"/>
        <rect x="8" y="14" width="8" height="5" rx=".5"/>
        <circle cx="17.5" cy="18.5" r=".55" fill="{ink}" stroke="none"/>
    ''',
    "psx": '''
        <circle cx="12" cy="12" r="9"/>
        <circle cx="12" cy="12" r="3"/>
        <circle cx="12" cy="12" r=".7" fill="{ink}" stroke="none"/>
        <path d="M6.4 7.8a7 7 0 0 1 3-2M17.6 16.2a7 7 0 0 1-3 2"/>
    ''',
    "ps2": '''
        <path d="M4 8.5 19 6l2 3-15 2.5-2-3ZM6 11.5v6.3L21 15V9M4 8.5v6.3l2 3"/>
        <path d="m7.5 14.3 10.5-1.8"/>
        <circle cx="18.8" cy="16.6" r=".6" fill="{ink}" stroke="none"/>
    ''',
    "saturn": '''
        <rect x="2.5" y="6" width="19" height="12" rx="2.5"/>
        <ellipse cx="12" cy="11" rx="5.5" ry="3.3"/>
        <path d="M5 16h4M16 16h2"/>
    ''',
    "n64": '''
        <path d="M5 7h14l2 8.5a2 2 0 0 1-3.5 1.7L15.5 15h-7l-2 2.2A2 2 0 0 1 3 15.5L5 7Z"/>
        <path d="M7 11h4M9 9v4M12 14v4"/>
        <circle cx="16" cy="10" r=".7" fill="{ink}" stroke="none"/>
        <circle cx="18" cy="12" r=".7" fill="{ink}" stroke="none"/>
    ''',
    "arcade": '''
        <path d="M7 2h10l2 5v14H5V7l2-5Z"/>
        <path d="M7 7h10M8 9h8v5H8zM7 17h10"/>
        <circle cx="10" cy="16" r=".6" fill="{ink}" stroke="none"/>
        <circle cx="14" cy="16" r=".6" fill="{ink}" stroke="none"/>
    ''',
    "msx": '''
        <path d="M7 3h10v3h2v15H5V6h2V3Z"/>
        <path d="M8 6h8M7.5 10h9M7.5 13h9M8 21v-4h8v4"/>
    ''',
    "pcengine": '''
        <rect x="2" y="7" width="20" height="10" rx="1.5"/>
        <path d="M5 10h14M5 13h8M16 13h3"/>
        <circle cx="5.2" cy="15" r=".5" fill="{ink}" stroke="none"/>
    ''',
    "spectrum": '''
        <path d="M3 8h18l1 8H2l1-8Z"/>
        <path d="M5 10h14M5 12.5h14M6 15h12M9 10v2.5M14 10v2.5"/>
    ''',
    "atari": '''
        <path d="M12 4v9M9 16h6M7 16l-2 4h14l-2-4H7Z"/>
        <circle cx="12" cy="3.5" r="1.5"/>
        <circle cx="17" cy="17.8" r=".7" fill="{ink}" stroke="none"/>
    ''',
    "c64": '''
        <rect x="2" y="7" width="20" height="11" rx="1.7"/>
        <path d="M4.5 10h15M4.5 12.5h15M4.5 15h15M7 10v2.5M12 10v2.5M17 10v2.5"/>
    ''',
    "amiga": '''
        <path d="M5 2.5h14v19H5V2.5Z"/>
        <path d="M8 2.5v6h8v-6M8 13h8v6H8zM10 5h4"/>
    ''',
    "chip": '''
        <rect x="7" y="7" width="10" height="10" rx="1"/>
        <path d="M10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4"/>
        <path d="M10 10h4v4h-4z"/>
    ''',
    "tracker": '''
        <rect x="3" y="3" width="18" height="18" rx="1.5"/>
        <path d="M3 7h18M8 7v14M12 10h2M16 10h2M12 14h5M12 18h2M5 10h1M5 14h1M5 18h1"/>
    ''',
    "midi": '''
        <rect x="2" y="5" width="20" height="14" rx="1.5"/>
        <path d="M2 14h20M6 14v5M10 14v5M14 14v5M18 14v5M6 5v6M10 5v6M16 5v6"/>
    ''',
    "audio": '''
        <path d="M2 12h2l2-4 3 8 3-11 3 14 3-7h4"/>
    ''',
    "archive": '''
        <rect x="3" y="5" width="18" height="15" rx="1.5"/>
        <path d="M3 9h18M9 13h6M10 16h4"/>
    ''',
    "playlist": '''
        <path d="M7 3h11a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z"/>
        <path d="M9 8h7M9 11h7M9 14h4M14 17v-3l3-.7v3"/>
        <circle cx="13" cy="17.3" r=".8"/>
        <circle cx="16" cy="16.7" r=".8"/>
    ''',
    "cue": '''
        <path d="M5 2h10l4 4v16H5V2ZM15 2v4h4"/>
        <circle cx="12" cy="14" r="4"/>
        <circle cx="12" cy="14" r=".7" fill="{ink}" stroke="none"/>
    ''',
    "paper": '''
        <path d="M3 2h12l6 6v14H3V2ZM15 2v6h6"/>
    ''',
}


def main() -> None:
    output_dir = Path(__file__).resolve().parents[1] / "qml" / "icons"
    for name, body in ICONS.items():
        for suffix, ink in (("", "#111111"), ("-light", "#ffffff")):
            svg = (
                '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" '
                f'fill="none" stroke="{ink}" stroke-width="1.8" '
                'stroke-linecap="round" stroke-linejoin="round">\n'
                + "\n".join(line.strip() for line in body.strip().splitlines())
                .replace("{ink}", ink)
                + "\n</svg>\n"
            )
            (output_dir / f"kog-format-{name}{suffix}.svg").write_text(svg)


if __name__ == "__main__":
    main()
