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

for target in tui server; do
  bundle="Kog-$version-$platform-$architecture-$target"
  directory="$stage/$bundle"
  mkdir -p "$directory"
  install -m755 "$root/target/release/kog-$target" "$directory/kog-$target"

  if [[ "$platform" == linux ]]; then
    python3 "$root/packaging/cli/bundle-linux.py" "$directory"
  else
    python3 "$root/packaging/cli/bundle-macos.py" "$directory"
  fi

  cat > "$directory/README.txt" <<EOF
Kog $target for $platform $architecture

Run ./kog-$target from this directory. Encoding uses linked FFmpeg libraries.
Decoders run in-process. Non-system shared libraries are in ./lib.
Keep the whole directory together when moving it to another machine.

Linux still needs a compatible glibc, the kernel, and an audio device for TUI
playback. macOS uses its built-in system libraries and frameworks. No Qt or
Homebrew installation is needed for these command-line packages.
EOF
  install -m644 "$root/LICENSE" "$directory/LICENSE"
  install -m644 "$root/THIRD_PARTY_NOTICES.md" "$directory/THIRD_PARTY_NOTICES.md"
  cp -R "$root/LICENSES" "$directory/LICENSES"
  tar -C "$stage" -czf "$dist/$bundle.tar.gz" "$bundle"
done
