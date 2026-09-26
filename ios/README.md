# Kog for iPhone

The iOS client is a native SwiftUI frontend with its own queue. It uses the Kog
server's library, search, playlist, stars, radio, artwork, and transcoded stream
APIs. Imported files stay in Kog's application storage and play locally. The
Rust static library connects local formats to the same `kog-audio` decoder
registry used by the other Kog frontends.

## Build on Apple silicon

Install Xcode, accept its license, and finish its first-launch components. The
Mac needs the macOS version required by its Xcode release. Then:

```sh
brew install cmake ninja pkg-config xcodegen imagemagick
rustup update stable
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
git submodule update --init --recursive
cd ios
xcodegen generate
xcodebuild -project Kog.xcodeproj -scheme Kog -configuration Release \
  -destination 'generic/platform=iOS' CODE_SIGNING_ALLOWED=NO build
```

The Xcode build phase runs `ios/native/build.sh` for the selected platform. It
builds static FFmpeg, libarchive, Syntrax, Play! PSF2, and melonDS 2SF libraries,
then links `kog-ios-audio`. The first build takes several minutes. Its downloads
and intermediate output live under
`ios/.native-build` and `target`, which Git ignores.

For a toolchain whose Xcode first-launch setup is still pending, the standalone
packager can link the app without `xcodebuild` or `actool`:

```sh
./ios/native/package-unsigned.sh device
```

It creates `ios/.native-build/package-device/Kog.app` with an ad-hoc signature
for bundle verification. That signature does not authorize installation on a
physical iPhone. Xcode development signing and provisioning are still required.

To run on an iPhone, connect and trust the phone, select your Apple development
team in Xcode, change the bundle identifier if your team requires it, and run
the `Kog` scheme on that device. Automatic signing is enabled in the project.
The app does not need a server to play imported local files.

## Current format limits

The local decoder uses the same `kog-audio` registry as desktop Kog. Syntrax,
PSF2, and 2SF use static renderer libraries through the shared Rust backend. SFM,
PSF1, SNSF, and Nuked SC-55 remain separate executables on desktop because
their upstream licenses cannot be linked into Kog's GPL-3.0-or-later binary.
iOS cannot run those four helper executables, so their local playback remains unavailable.
Imported archives can be browsed by folder, including nested archives, and
subsong selection uses the same backend as the other frontends.

Server playback uses Kog's stream URL with no authentication or a bearer token.
For Basic authentication the player supplies credentials through AVFoundation's
resource loader challenge delegate. Playback with that mode still needs an
on-device test.
