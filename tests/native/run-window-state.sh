#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-window-state.XXXXXX)"
qt_moc="$(command -v moc || true)"
if [[ -z "$qt_moc" ]]; then
  qt_qmake="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
  qt_moc="$("$qt_qmake" -query QT_INSTALL_LIBEXECS)/moc"
fi
"$qt_moc" "$repo_dir/native/kog_modern_skin.h" -o "$test_dir/moc_kog_modern_skin.cpp"
webengine_rpath_link=()
if [[ -n "${KOG_QTWEBENGINE_RPATH_LINK:-}" ]]; then
  webengine_rpath_link=("-Wl,-rpath-link,${KOG_QTWEBENGINE_RPATH_LINK}")
fi
qt_version="$(pkg-config --modversion Qt6Core)"
IFS=. read -r qt_major qt_minor qt_patch <<< "$qt_version"
session_flags=()
# Match build.rs: the private interface's session method first exists in 6.11.
if (( qt_major > 6 || (qt_major == 6 && qt_minor >= 11) )); then
  qt_qmake="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
  qt_headers="$("$qt_qmake" -query QT_INSTALL_HEADERS)"
  if [[ -f "$qt_headers/QtGui/$qt_version/QtGui/qpa/qplatformwindow_p.h" ]]; then
    session_flags=(-DKOG_WAYLAND_SESSION_RESTORE
      -I"$qt_headers/QtGui/$qt_version/QtGui" -I"$qt_headers/QtGui/$qt_version"
      -I"$qt_headers/QtCore/$qt_version/QtCore" -I"$qt_headers/QtCore/$qt_version")
  fi
fi
echo "Testing window restoration with Qt $qt_version (${#session_flags[@]} private API compiler flags)"
# Qt flags are intentionally word-split into individual compiler arguments.
# shellcheck disable=SC2046
c++ -std=c++17 -fPIC -pthread -I"$repo_dir/native" \
  "${session_flags[@]}" \
  $(pkg-config --cflags Qt6Widgets Qt6Test Qt6WebChannel Qt6WebEngineQuick Qt6WebEngineCore) \
  "$repo_dir/tests/native/window_state.cpp" \
  "$repo_dir/native/kog_desktop_integration.cpp" \
  "$repo_dir/native/kog_modern_skin.cpp" "$test_dir/moc_kog_modern_skin.cpp" \
  "$repo_dir/native/kog_window_state.cpp" \
  $(pkg-config --libs Qt6Widgets Qt6Test Qt6WebChannel Qt6WebEngineQuick Qt6WebEngineCore) \
  "${webengine_rpath_link[@]}" -o "$test_dir/window-state"
XDG_CONFIG_HOME="$test_dir/config" QT_QPA_PLATFORM=offscreen "$test_dir/window-state"
