#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="${1:-$root_dir/dist/macos}"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root_dir/Cargo.toml" | head -n1)"
architecture="$(uname -m)"
app="$output_dir/Kog.app"
contents="$app/Contents"

rm -rf "$output_dir"
mkdir -p "$contents/MacOS" "$contents/Resources" "$contents/Frameworks"
install -m755 "$root_dir/target/release/kog" "$contents/MacOS/kog"
install -m644 "$root_dir/packaging/macos/Info.plist" "$contents/Info.plist"
install -m644 "$root_dir/LICENSE" "$contents/Resources/LICENSE"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$contents/Info.plist"

helpers=(
  kog-sfm-helper kog-psf-helper kog-psf2-helper kog-2sf-helper
  kog-snsf-helper kog-syntrax-helper kog-sc55-helper
)
extra_executables=()
for helper in "${helpers[@]}"; do
  helper_path="$(find "$root_dir/target/release/build" -type f \
    -path "*/bin/$helper" -print -quit)"
  if [[ -z "$helper_path" ]]; then
    echo "missing release helper: $helper" >&2
    exit 1
  fi
  install -m755 "$helper_path" "$contents/MacOS/$helper"
  extra_executables+=("-executable=$contents/MacOS/$helper")
done

iconset="$output_dir/Kog.iconset"
mkdir -p "$iconset"
rsvg-convert -w 1024 -h 1024 "$root_dir/qml/icons/kog.svg" -o "$output_dir/Kog-1024.png"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$output_dir/Kog-1024.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  doubled=$((size * 2))
  sips -z "$doubled" "$doubled" "$output_dir/Kog-1024.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$contents/Resources/Kog.icns"
rm -rf "$iconset" "$output_dir/Kog-1024.png"

qt_lib="${QT_ROOT_DIR:?QT_ROOT_DIR is set by install-qt-action}/lib"
brew_search=(
  -libpath="$(brew --prefix ffmpeg)/lib"
  -libpath="$(brew --prefix libarchive)/lib"
)
macdeployqt "$app" -qmldir="$root_dir/qml" -always-overwrite -verbose=2 \
  "${extra_executables[@]}" "${brew_search[@]}"

dylib_search=(
  -s "$qt_lib"
  -s "$(brew --prefix ffmpeg)/lib"
  -s "$(brew --prefix libarchive)/lib"
)
for executable in "$contents/MacOS/"*; do
  dylibbundler -od -b -ns -x "$executable" -d "$contents/Frameworks" \
    -p '@executable_path/../Frameworks/' "${dylib_search[@]}"
done

if find "$contents" -type f -perm -100 -print0 | xargs -0 -n1 otool -L \
    | grep -E '/(opt/homebrew|usr/local)/(opt|Cellar)/'; then
  echo "the app bundle still references a Homebrew library" >&2
  exit 1
fi

codesign --force --deep --sign - "$app"
codesign --verify --deep --strict --verbose=2 "$app"

dmg="$output_dir/Kog-$version-macos-$architecture.dmg"
hdiutil create -volname "Kog $version" -srcfolder "$app" -ov -format UDZO "$dmg"
