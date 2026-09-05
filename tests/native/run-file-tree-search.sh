#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-file-tree-search.XXXXXX)"
qt_moc="$(command -v moc || true)"
if [[ -z "$qt_moc" ]]; then
  qt_qmake="${QMAKE:-qmake6}"
  qt_moc="$("$qt_qmake" -query QT_INSTALL_LIBEXECS)/moc"
fi
"$qt_moc" "$repo_dir/native/kog_file_tree_search.h" -o "$test_dir/moc_search.cpp"
# Qt pkg-config flags intentionally expand into separate compiler arguments.
# shellcheck disable=SC2046
c++ -std=c++17 -fPIC -pthread -I"$repo_dir/native" \
  $(pkg-config --cflags Qt6Widgets Qt6Quick Qt6Concurrent libarchive) \
  "$repo_dir/tests/native/file_tree_search.cpp" "$repo_dir/native/kog_file_tree_search.cpp" \
  "$repo_dir/native/kog_tree_archive.cpp" \
  "$test_dir/moc_search.cpp" \
  $(pkg-config --libs Qt6Widgets Qt6Quick Qt6Concurrent libarchive) -o "$test_dir/file-tree-search"
QT_QPA_PLATFORM=offscreen "$test_dir/file-tree-search"
