#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-window-state.XXXXXX)"
# Qt flags are intentionally word-split into individual compiler arguments.
# shellcheck disable=SC2046
c++ -std=c++17 -fPIC -I"$repo_dir/native" \
  $(pkg-config --cflags Qt6Widgets Qt6Test) \
  "$repo_dir/tests/native/window_state.cpp" \
  "$repo_dir/native/kog_desktop_integration.cpp" \
  "$repo_dir/native/kog_window_state.cpp" \
  $(pkg-config --libs Qt6Widgets Qt6Test) -o "$test_dir/window-state"
XDG_CONFIG_HOME="$test_dir/config" QT_QPA_PLATFORM=offscreen "$test_dir/window-state"
