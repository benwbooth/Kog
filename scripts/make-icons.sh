#!/usr/bin/env bash
# Regenerate the web icon PNGs from icons/kog.svg.
#
# Every PNG is the gear logo on the app's dark background: the plain icons
# fill most of the tile, the maskable one keeps the logo inside the safe
# zone, and the apple-touch icon pre-rounds the corners (iOS re-masks them,
# but the rounded source matches the other tiles). Requires inkscape.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
icons="$here/crates/kog-web/icons"
bg="#1b1e20"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The gear artwork: everything inside kog.svg except the <svg> wrapper.
inner="$(sed -n '/<defs>/,$p' "$icons/kog.svg")"

wrapper() { # scale output-name extra-background
    local scale="$1" name="$2" bg_rect="${3:-}"
    local inset
    inset=$(python3 -c "print(f'{(1 - $scale) * 32:.2f}')")
    cat > "$work/$name.svg" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  $bg_rect
  <g transform="translate($inset $inset) scale($scale)">
$inner
  </g>
</svg>
EOF
}

render() { # svg-file size output
    inkscape "$1" --export-type=png --export-filename="$3" -w "$2" -h "$2" >/dev/null 2>&1
}

# Plain tiles: favicon set at 192 and 512, gear at 86% of the tile.
wrapper 0.86 plain "<rect width=\"64\" height=\"64\" fill=\"$bg\"/>"
render "$work/plain.svg" 192 "$icons/icon-192.png"
render "$work/plain.svg" 512 "$icons/icon-512.png"

# Maskable: the logo stays well inside the safe zone (~57%).
wrapper 0.57 maskable "<rect width=\"64\" height=\"64\" fill=\"$bg\"/>"
render "$work/maskable.svg" 512 "$icons/icon-maskable-512.png"

# Apple touch icon: rounded corners like the other tiles; iOS re-masks.
apple_rx="$(python3 -c 'print(f"{24 * 64 / 180:.2f}")')"
wrapper 0.88 apple "<rect width=\"64\" height=\"64\" rx=\"$apple_rx\" fill=\"$bg\"/>"
render "$work/apple.svg" 180 "$icons/apple-touch-icon.png"

ls -la "$icons"/*.png
