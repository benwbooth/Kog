#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
platform=${1:-device}
case "$platform" in
  device) sdk_name=iphoneos; target=aarch64-apple-ios; triple=arm64-apple-ios17.0; supported_platform=iPhoneOS ;;
  simulator) sdk_name=iphonesimulator; target=aarch64-apple-ios-sim; triple=arm64-apple-ios17.0-simulator; supported_platform=iPhoneSimulator ;;
  *) echo "Usage: $0 [device|simulator]" >&2; exit 2 ;;
esac
"$repo/ios/native/build.sh" "$platform"
sdk=$(xcrun --sdk "$sdk_name" --show-sdk-path)
build="$repo/ios/.native-build/$platform"
output="$repo/ios/.native-build/package-$platform"
app="$output/Kog.app"
mkdir -p "$output" "$build/link"
cp "$repo/target/$target/release/libkog_ios_audio.a" "$build/link/"
cp "$build/prefix/lib/"*.a "$build/link/"
rm -rf "$app"
mkdir -p "$app"
# This fallback packaging path does not invoke Xcode's asset compiler, which
# requires Xcode's root-installed system resources. Loose PNGs are recognized
# by UIImage/SwiftUI Image and CFBundleIcons on device.
cp "$repo/ios/Kog/Info.plist" "$app/Info.plist"
cp "$repo/LICENSE" "$repo/THIRD_PARTY_NOTICES.md" "$app/"
cp -R "$repo/LICENSES" "$app/"
/usr/libexec/PlistBuddy -c "Set :CFBundleExecutable Kog" "$app/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier org.kog.player" "$app/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName Kog" "$app/Info.plist"
for resource in "$repo"/ios/Kog/Assets.xcassets/*.imageset/*.png; do cp "$resource" "$app/"; done
magick "$repo/ios/Kog/Assets.xcassets/AppIcon.appiconset/Kog-1024.png" \
  -resize 180x180 "$app/AppIcon-60@3x.png"
magick "$repo/ios/Kog/Assets.xcassets/AppIcon.appiconset/Kog-1024.png" \
  -resize 167x167 "$app/AppIcon-83.5@2x.png"
/usr/libexec/PlistBuddy -c 'Add :MinimumOSVersion string 17.0' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :UIDeviceFamily array' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :UIDeviceFamily:0 integer 1' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :UIDeviceFamily:1 integer 2' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleSupportedPlatforms array' "$app/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms:0 string $supported_platform" "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons dict' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon dict' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles array' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon-60@3x' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~ipad dict' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~ipad:CFBundlePrimaryIcon dict' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~ipad:CFBundlePrimaryIcon:CFBundleIconFiles array' "$app/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :CFBundleIcons~ipad:CFBundlePrimaryIcon:CFBundleIconFiles:0 string AppIcon-83.5@2x' "$app/Info.plist"
cd "$repo/ios"
xcrun --sdk "$sdk_name" swiftc -O -D KOG_NATIVE_AUDIO -module-name Kog \
  -target "$triple" -sdk "$sdk" -L "$build/link" \
  -lkog_ios_audio -lkog_syntrax_embedded -lkog_syntrax_core -lkog_psf2_embedded -lkog_psf2_play_core -lPlayCore -lFramework_Http -lapp_shared -lCodeGen -lFramework -llibzstd_zlibwrapper_static -lxxhash -lchdr-static -lzstd -llzma -lbz2 -larchive -lavformat -lavcodec -lavutil -lswresample \
  -lc++ -lz -liconv \
  -framework AudioToolbox -framework CoreAudio -framework CoreFoundation \
  -framework Security -framework VideoToolbox Kog/*.swift -o "$app/Kog"
plutil -lint "$app/Info.plist"
# An ad-hoc signature verifies bundle integrity; installing on a physical
# device still requires an Apple development signing identity and profile.
codesign --force --sign - "$app"
codesign --verify --deep --strict --verbose=2 "$app"
file "$app/Kog"
echo "Built ad-hoc signed Kog bundle: $app (development signing is required for a physical iPhone)"
