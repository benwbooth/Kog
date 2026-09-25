# Kog for Android

The Android client is a native Kotlin/Jetpack Compose app. It uses Kog's Rust
server for library browsing, metadata, search, playlists, favorites, random
radio, cover art, and transcoded streams. Its queue and playback state stay on
the Android device. Playback runs in a Media3 session service so it continues
in the background and receives system and Bluetooth media controls.

## Build and install

Install JDK 17, Rust, CMake, Ninja, pkg-config, and the Android SDK (platform
36, build tools 35, and NDK 28.2.13676358). Initialize the Git submodules and
build the native decoders before Gradle:

```sh
git submodule update --init --recursive
android/native/build.sh arm64-v8a
cd android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Set `ANDROID_HOME` and, if the NDK is elsewhere, `ANDROID_NDK_HOME`. The native
build downloads and statically links FFmpeg 8.1.2 and libarchive 3.8.8, builds
Kog's other vendored decoders and helper programs, and packages the C++ runtime.
The APK produced by GitHub Actions includes arm64; run
`android/native/build.sh x86_64` as well for an x86_64 emulator. Android 9
(API 28) or newer is required because libvgm uses the platform's iconv API.

The app accepts a Kog server URL and access token or Basic credentials in
Settings. On an Android emulator, `http://10.0.2.2:8420` reaches a server on
the development computer. On a phone, use an address the phone can reach.
Settings also lets you select a device folder or individual files through
Android's document picker. Access to those files persists across app starts.

## Current playback coverage

Server tracks use Kog's Rust decoders and the server's AAC, Opus, or FLAC
stream. Common device formats, including MP3 and FLAC, use Media3's platform
decoders. Other device files use the packaged `kog-audio` Rust decoder through
a JNI bridge and are delivered to the Media3 session as 48 kHz stereo PCM.
The bridge shares Kog's decoder registry and its format libraries with the
desktop and server. The Android Media3 session handles background playback,
lock screen controls, and Bluetooth controls for both paths.

The device picker stores access to selected files and folders. Native playback
copies a selected file into the app cache because Kog's decoders require a
filesystem path. Companion-file formats currently need more work: only the
selected file is copied, so external sample banks or related miniPSF files may
not be available. Opening a local archive plays its first expanded track; the
Android library does not yet show individual archive members or subsongs.
