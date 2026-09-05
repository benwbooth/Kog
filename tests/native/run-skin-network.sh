#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-skin-network.XXXXXX)"
# Qt's pkg-config flags intentionally expand into separate compiler arguments.
# shellcheck disable=SC2046
c++ -std=c++17 -fPIC -pthread -I"$repo_dir/native" \
  $(pkg-config --cflags Qt6Gui Qt6Network) \
  "$repo_dir/tests/native/skin_network.cpp" "$repo_dir/native/kog_skin_network.cpp" \
  $(pkg-config --libs Qt6Gui Qt6Network) -o "$test_dir/skin-network"
"$test_dir/skin-network" "$@"
