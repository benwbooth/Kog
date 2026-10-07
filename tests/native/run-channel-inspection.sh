#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
lib_dir="${1:-}"
if [[ -z "$lib_dir" ]]; then
  newest=""
  for archive in "$repo_dir"/target/debug/build/kog-audio-*/out/lib/libvgm-player.a; do
    if [[ -f "$archive" && ( -z "$newest" || "$archive" -nt "$newest" ) ]]; then
      newest="$archive"
    fi
  done
  if [[ -z "$newest" ]]; then
    echo "Build kog-audio first, or pass its native library directory." >&2
    exit 1
  fi
  lib_dir="$(dirname "$newest")"
fi
test_dir="$(mktemp -d -t kog-channel-inspection.XXXXXX)"
trap 'rm -rf "$test_dir"' EXIT
c++ -std=c++17 -O1 "$repo_dir/tests/native/channel_inspection.cpp" \
  -L"$lib_dir" -lvgm-player -lvgm-emu -lvgm-utils -lz -lm \
  -o "$test_dir/channel-inspection"
"$test_dir/channel-inspection"
