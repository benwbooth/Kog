# Kog for Android

The Android client is a native Kotlin/Jetpack Compose app. It uses Kog's Rust
server for library browsing, metadata, search, playlists, favorites, random
radio, cover art, and transcoded streams. Its queue and playback state stay on
the Android device. Playback runs in a Media3 session service so it continues
in the background and receives system and Bluetooth media controls.

## Build and install

Install JDK 17 and the Android SDK (platform 36 and build tools 35), then run:

```sh
cd android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The app accepts a Kog server URL and access token or Basic credentials in
Settings. On an Android emulator, `http://10.0.2.2:8420` reaches a server on
the development computer. On a phone, use an address the phone can reach.
Settings also lets you select a device folder or individual files through
Android's document picker. Access to those files persists across app starts.

## Current playback coverage

Server tracks use Kog's existing Rust decoders and the server's AAC, Opus, or
FLAC stream. Device files currently use Android Media3's built-in decoders.
MP3 device playback has been verified on an Android emulator. Kog-specific
local formats such as NSF, VGM, and tracker modules require an Android build
of `kog-audio` and a native decoder bridge; their file icons and queue entries
alone do not make them playable offline. This is the remaining format-parity
work for Android.

`kog-audio` currently builds many C and C++ libraries in `build.rs`, and its
archive and FFmpeg dependencies come from desktop `pkg-config`. An Android
cross build requires target versions of those dependencies before a JNI bridge
can use Kog's shared `PcmReader` decoder path.
