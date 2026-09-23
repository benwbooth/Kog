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
bundle="Kog-$version-$platform-$architecture-cli"
mkdir -p "$root/dist/cli/$bundle"
cp "$root/target/release/kog-tui" "$root/target/release/kog-server" "$root/dist/cli/$bundle/"
cat > "$root/dist/cli/$bundle/README.txt" <<'EOF'
Kog terminal and headless server binaries

Run ./kog-tui in a terminal to browse and play locally.
Run ./kog-server to serve the web UI and API using Kog's saved server settings.
The server binds the saved address and port even when the desktop server toggle is off.
Configure credentials and TLS in Kog before exposing the server beyond loopback.

These binaries use the system's native audio and codec libraries. Install Kog's
usual runtime dependencies for your platform, including FFmpeg for transcoding.
EOF
tar -C "$root/dist/cli" -czf "$root/dist/cli/$bundle.tar.gz" "$bundle"
