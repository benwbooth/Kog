#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
  echo "usage: $0 linux|macos x86_64|arm64" >&2
  exit 2
fi
platform="$1"
architecture="$2"
case "$platform:$architecture" in
  linux:x86_64|macos:arm64) ;;
  *) echo "unsupported CLI target: $platform:$architecture" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n1)"
dist="$root/dist/cli"
mkdir -p "$dist"
stage="$(mktemp -d "$dist/.stage.XXXXXX")"
trap 'rm -rf "$stage"' EXIT

helpers=(
  kog-sfm-helper kog-psf-helper kog-psf2-helper kog-2sf-helper
  kog-snsf-helper kog-syntrax-helper kog-sc55-helper
)
ffmpeg="$(command -v ffmpeg || true)"
if [[ -z "$ffmpeg" ]]; then
  echo "ffmpeg is required for the self-contained server and TUI packages" >&2
  exit 1
fi

for target in tui server; do
  bundle="Kog-$version-$platform-$architecture-$target"
  directory="$stage/$bundle"
  mkdir -p "$directory"
  install -m755 "$root/target/release/kog-$target" "$directory/kog-$target"
  install -m755 "$ffmpeg" "$directory/ffmpeg"
  for helper in "${helpers[@]}"; do
    helper_path="$(python3 - "$root/target/release/build" "$helper" <<'PY'
from pathlib import Path
import sys

build = Path(sys.argv[1])
name = sys.argv[2]
matches = [path for path in build.glob(f"kog-audio-*/out/**/bin/{name}") if path.is_file()]
if matches:
    print(max(matches, key=lambda path: path.stat().st_mtime))
PY
)"
    if [[ -z "$helper_path" ]]; then
      echo "missing release helper: $helper" >&2
      exit 1
    fi
    install -m755 "$helper_path" "$directory/$helper"
  done

  if [[ "$platform" == linux ]]; then
    python3 "$root/packaging/cli/bundle-linux.py" "$directory"
  else
    python3 "$root/packaging/cli/bundle-macos.py" "$directory"
  fi

  cat > "$directory/README.txt" <<EOF
Kog $target for $platform $architecture

Run ./kog-$target from this directory. The ffmpeg encoder and Kog decoder
helpers are included beside it; non-system shared libraries are in ./lib.
Keep the whole directory together when moving it to another machine.

Linux still needs a compatible glibc, the kernel, and an audio device for TUI
playback. macOS uses its built-in system libraries and frameworks. No Qt or
Homebrew installation is needed for these command-line packages.
EOF
  install -m644 "$root/LICENSE" "$directory/LICENSE"
  tar -C "$stage" -czf "$dist/$bundle.tar.gz" "$bundle"
done
