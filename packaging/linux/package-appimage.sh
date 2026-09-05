#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="${1:-$root_dir/dist/linux}"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root_dir/Cargo.toml" | head -n1)"
app_dir="$output_dir/Kog.AppDir"
tool_dir="$output_dir/tools"

rm -rf "$app_dir" "$tool_dir"
mkdir -p "$app_dir/usr/bin" "$tool_dir" "$output_dir"
install -m755 "$root_dir/target/release/kog" "$app_dir/usr/bin/kog"
install -Dm644 "$root_dir/packaging/linux/org.kog.player.metainfo.xml" \
  "$app_dir/usr/share/metainfo/org.kog.player.metainfo.xml"
desktop_icon="$tool_dir/org.kog.player.svg"
install -m644 "$root_dir/qml/icons/kog.svg" "$desktop_icon"

helpers=(
  kog-sfm-helper kog-psf-helper kog-psf2-helper kog-2sf-helper
  kog-snsf-helper kog-syntrax-helper kog-sc55-helper
)
helper_args=()
for helper in "${helpers[@]}"; do
  helper_path="$(find "$root_dir/target/release/build" -type f \
    -path "*/bin/$helper" -print -quit)"
  if [[ -z "$helper_path" ]]; then
    echo "missing release helper: $helper" >&2
    exit 1
  fi
  install -m755 "$helper_path" "$app_dir/usr/bin/$helper"
  helper_args+=(--executable "$app_dir/usr/bin/$helper")
done

linuxdeploy="$tool_dir/linuxdeploy-x86_64.AppImage"
qt_plugin="$tool_dir/linuxdeploy-plugin-qt-x86_64.AppImage"
curl --fail --location --retry 3 \
  https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage \
  --output "$linuxdeploy"
curl --fail --location --retry 3 \
  https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage \
  --output "$qt_plugin"
chmod +x "$linuxdeploy" "$qt_plugin"

export APPIMAGE_EXTRACT_AND_RUN=1
export PATH="$tool_dir:$PATH"
export QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
export QML_SOURCES_PATHS="$root_dir/qml"
export LINUXDEPLOY_OUTPUT_VERSION="$version"
export LDAI_OUTPUT="$output_dir/Kog-$version-linux-x86_64.AppImage"

"$linuxdeploy" \
  --appdir "$app_dir" \
  --executable "$app_dir/usr/bin/kog" \
  "${helper_args[@]}" \
  --desktop-file "$root_dir/packaging/linux/org.kog.player.desktop" \
  --icon-file "$desktop_icon"

# Calling the Qt plugin directly also works when AppImages must self-extract.
# In that mode linuxdeploy runs from a cache directory and cannot discover a
# plugin that is adjacent to the original AppImage.
"$qt_plugin" --appdir "$app_dir"

# Qt WebEngine's Chromium child process, resource packs, and locales live
# outside the shared libraries. linuxdeploy-plugin-qt deploys them when it
# sees QtWebEngineCore; fail the package job rather than publishing an AppImage
# that can only use a system Qt installation.
for required_path in \
  "$app_dir/usr/libexec/QtWebEngineProcess" \
  "$app_dir/usr/resources/qtwebengine_resources.pak" \
  "$app_dir/usr/resources/qtwebengine_devtools_resources.pak" \
  "$app_dir/usr/resources/qtwebengine_resources_100p.pak" \
  "$app_dir/usr/resources/qtwebengine_resources_200p.pak" \
  "$app_dir/usr/translations/qtwebengine_locales/en-US.pak"; do
  if [[ ! -f "$required_path" ]]; then
    echo "missing deployed Qt WebEngine runtime file: $required_path" >&2
    exit 1
  fi
done
"$linuxdeploy" --appdir "$app_dir" --output appimage

while IFS= read -r executable; do
  if ldd "$executable" 2>/dev/null | grep -q 'not found'; then
    echo "unresolved shared library in $executable" >&2
    ldd "$executable" >&2
    exit 1
  fi
done < <(find "$app_dir/usr/bin" -type f -perm -100 -print)

tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
  -C "$output_dir" -czf "$output_dir/Kog-$version-linux-x86_64-portable.tar.gz" \
  Kog.AppDir
