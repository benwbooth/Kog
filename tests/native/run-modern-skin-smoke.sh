#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_dir="$(mktemp -d -t kog-modern-skin.XXXXXX)"
skin_archive="${KOG_MODERN_SKIN_ARCHIVE:-$repo_dir/native/webamp/packages/webamp-modern/assets/skins/MMD3.wal}"
screenshot_path="${KOG_MODERN_SMOKE_SCREENSHOT:-$test_dir/modern-skin-smoke.png}"
qt_qmake="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
qt_moc="$(command -v moc || true)"
qt_rcc="$(command -v rcc || true)"
if [[ -z "$qt_moc" ]]; then qt_moc="$("$qt_qmake" -query QT_INSTALL_LIBEXECS)/moc"; fi
if [[ -z "$qt_rcc" ]]; then qt_rcc="$("$qt_qmake" -query QT_INSTALL_LIBEXECS)/rcc"; fi
webengine_rpath_link=()
if [[ -n "${KOG_QTWEBENGINE_RPATH_LINK:-}" ]]; then
  webengine_rpath_link=("-Wl,-rpath-link,${KOG_QTWEBENGINE_RPATH_LINK}")
fi
"$qt_moc" "$repo_dir/native/kog_modern_skin.h" -o "$test_dir/moc_kog_modern_skin.cpp"
"$qt_moc" "$repo_dir/tests/native/modern_skin_smoke.cpp" -o "$test_dir/modern_skin_smoke.moc"
(cd "$repo_dir" && "$qt_rcc" --name modern_runtime web/modern/runtime.qrc -o "$test_dir/qrc_modern_runtime.cpp")
# Qt's pkg-config flags intentionally expand into individual compiler arguments.
# shellcheck disable=SC2046
c++ -std=c++17 -fPIC -pthread -I"$repo_dir/native" -I"$test_dir" \
  $(pkg-config --cflags Qt6Widgets Qt6Quick Qt6Qml Qt6WebChannel Qt6WebEngineQuick Qt6WebEngineCore) \
  "$repo_dir/tests/native/modern_skin_smoke.cpp" "$repo_dir/native/kog_modern_skin.cpp" \
  "$test_dir/moc_kog_modern_skin.cpp" "$test_dir/qrc_modern_runtime.cpp" \
  $(pkg-config --libs Qt6Widgets Qt6Quick Qt6Qml Qt6WebChannel Qt6WebEngineQuick Qt6WebEngineCore) \
  "${webengine_rpath_link[@]}" \
  -o "$test_dir/modern-skin-smoke"
mkdir -p "$test_dir/cache" "$test_dir/config" "$test_dir/data" "$test_dir/runtime"
chmod 700 "$test_dir/runtime"
echo "Running real QtWebEngine MMD3 smoke; screenshot will be: $screenshot_path"
run_smoke=(env XDG_CACHE_HOME="$test_dir/cache" XDG_CONFIG_HOME="$test_dir/config" XDG_DATA_HOME="$test_dir/data" XDG_RUNTIME_DIR="$test_dir/runtime" "$test_dir/modern-skin-smoke" "$repo_dir" "$skin_archive" "$screenshot_path")
if command -v xvfb-run >/dev/null; then
  xvfb-run -a --server-args="-screen 0 1280x1024x24" "${run_smoke[@]}"
else
  QT_QPA_PLATFORM=offscreen "${run_smoke[@]}"
fi
test -s "$screenshot_path"
echo "Modern skin screenshot saved at $screenshot_path"
