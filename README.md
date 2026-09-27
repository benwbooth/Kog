<p align="center">
  <img src="qml/icons/kog.svg" width="88" alt="Kog">
</p>
<h1 align="center">Kog</h1>
<p align="center">Your music, from FLAC to chiptunes. On your desktop, in your terminal, or on your phone.</p>
<p align="center">
  <a href="https://github.com/benwbooth/Kog/releases/latest">Download</a> ·
  <a href="#install">Install</a> ·
  <a href="docs/USAGE.md">User guide</a> ·
  <a href="https://github.com/benwbooth/Kog/issues">Report a bug</a>
</p>

[![Builds](https://github.com/benwbooth/Kog/actions/workflows/packages.yml/badge.svg)](https://github.com/benwbooth/Kog/actions/workflows/packages.yml)

Kog is a free, open-source music player inspired by [Cog](https://cog.losno.co/).
Play your albums, audiobooks, MIDI, tracker modules, and game soundtracks from
one library—even when the music lives inside an archive.

- **Browse your files.** Search folders and archives, build playlists, and save favorites.
- **Rediscover your collection.** Random Radio keeps music coming from the folder you choose.
- **Hear unusual formats.** MP3, FLAC, Opus, MIDI, MOD/XM, SID, SPC, VGM, PSF, and many more. [Format details](docs/FORMAT_PARITY.md)
- **Listen around the house.** Run a Kog server and stream your library to a browser or phone, with an independent queue on each device.

## Pick your player

| Frontend | What you get |
| --- | --- |
| **Qt desktop** · Windows, Linux, Apple Silicon Mac | Full library and playlist panes, tag editing, equalizer, visualizers, mini-player, and Winamp skins. |
| **Terminal** · Linux, macOS | A familiar two-pane player with keyboard and mouse controls, resizable panes, artwork, and visualization. No Qt required. |
| **Web** · Desktop and mobile browsers | Your server's library in a browser, with a compact touch layout and home-screen support on phones. |
| **iPhone** · SwiftUI | A native app for server streaming and imported local files. [Build and install](ios/README.md) |
| **Android** · Kotlin/Compose | A native app with server streaming, local playback, and background media controls. [Build and install](android/README.md) |
| **Headless server** · Windows, Linux, macOS | Serve your library and the web player without opening a window. [Setup](docs/SERVER.md) |

The players share Rust audio and library code; each keeps its own playback
queue. Native mobile apps are still in development; their guides describe
installation and remaining limitations.

## See it in action

**Qt desktop**

![Kog desktop with the file tree, soundtrack playlist, and playback controls](docs/screenshots/qt-desktop.png)

<details>
<summary><strong>Terminal and desktop web</strong></summary>

**Terminal** — keyboard and mouse, straight from your shell.

![Kog terminal player with a browsable library and soundtrack queue](docs/screenshots/terminal.png)

**Web** — the same collection in your browser.

![Kog web player with a soundtrack playing](docs/screenshots/web-desktop.jpg)

</details>

<p>
  <img src="docs/screenshots/web-mobile.jpg" width="260" alt="Mobile web player showing a compact queue and bottom navigation">
  &nbsp;
  <img src="docs/screenshots/android.png" width="260" alt="Native Android app browsing an album in the server library">
</p>
<p><em>Mobile web (left) and native Android (right).</em></p>

## Install

[Download the latest release](https://github.com/benwbooth/Kog/releases/latest),
then open the instructions for your platform. Commands below assume one
downloaded version in the current folder.

<details>
<summary><strong>Windows (x64): installer or portable ZIP</strong></summary>

- **MSI:** open `Kog-…-windows-x86_64.msi`, finish the installer, then launch
  **Kog** from the Start menu.
- **Portable:** extract `Kog-…-windows-x86_64-portable.zip` into a folder and
  run `Kog.exe`. Keep the DLLs and subfolders beside it.
- **Headless server:** configure your library and server in Kog's preferences,
  close Kog, then run `.\Kog.exe --server` in PowerShell from the portable folder.

Windows packages are currently unsigned, so Windows may show a publisher warning.

</details>

<details>
<summary><strong>macOS (Apple Silicon): DMG or Homebrew</strong></summary>

**DMG:** open `Kog-…-macos-arm64.dmg`, drag **Kog** into **Applications**, then
open it. If macOS blocks this unnotarized build, try opening it once, then use
**System Settings → Privacy & Security → Open Anyway** for Kog.
[Apple's instructions](https://support.apple.com/en-us/102445)

**Homebrew:** install [Homebrew](https://brew.sh), download `kog.rb` from the
release, and put it in a local tap:

```sh
brew tap-new local/kog
mkdir -p "$(brew --repository local/kog)/Casks"
cp kog.rb "$(brew --repository local/kog)/Casks/kog.rb"
brew install --cask local/kog/kog
```

Create the tap only once. To update, replace its `kog.rb` with the new release's
file and run `brew upgrade --cask local/kog/kog`. These packages require an
Apple Silicon Mac; Intel Macs are unsupported. TUI/server instructions are below.

</details>

<details>
<summary><strong>Linux (x86_64): AppImage, portable bundle, or Flatpak</strong></summary>

**AppImage:** download the `.AppImage`, then:

```sh
chmod +x Kog-*-linux-x86_64.AppImage
./Kog-*-linux-x86_64.AppImage --gui
```

If FUSE is unavailable, use
`APPIMAGE_EXTRACT_AND_RUN=1 ./Kog-*-linux-x86_64.AppImage --gui`.
[AppImage help](https://docs.appimage.org/user-guide/troubleshooting/fuse.html)

**Portable bundle:** extract the `portable.tar.gz` and run its launcher:

```sh
tar -xzf Kog-*-linux-x86_64-portable.tar.gz
./Kog.AppDir/AppRun --gui
```

Keep the whole `Kog.AppDir` directory together.

**Flatpak bundle:** [install Flatpak](https://flatpak.org/setup/) for your
distribution, download the `.flatpak`, then:

```sh
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./Kog-*-linux-x86_64.flatpak
flatpak run org.kog.player --gui
```

**Flatpak repository archive:** for a local repository, download the
`flatpak-repo.tar.zst` instead. With Flatpak, Flathub, and `zstd` installed:

```sh
tar --zstd -xf Kog-*-linux-x86_64-flatpak-repo.tar.zst
flatpak remote-add --user --no-gpg-verify kog-local "$(pwd)/repo"
flatpak install --user kog-local org.kog.player
flatpak run org.kog.player --gui
```

Keep the extracted `repo` directory: it is the source for this local remote.

</details>

<details>
<summary><strong>Terminal player and headless server: Linux or macOS</strong></summary>

1. Download a `tui.tar.gz` or `server.tar.gz` for **linux-x86_64** or
   **macos-arm64**.
2. Extract the archive and open a terminal in its extracted folder. Keep the
   executable and `lib` directory together; Qt is not needed.
3. Run `./kog-tui` for the terminal player, or `./kog-server` for the server.

In the TUI, press **o** to choose your music folder and **?** for keyboard help.
For a server, configure the music folder and server settings once in Qt or the
TUI's preferences, then close that player and launch `kog-server` as the same
user. The default local address is `http://127.0.0.1:8420`.
[Network access and authentication](docs/SERVER.md)

Linux bundles need a compatible glibc and host audio system. macOS bundles
need Apple Silicon and may require the first-launch approval described above.

</details>

<details>
<summary><strong>Android: APK</strong></summary>

1. On an **arm64 Android 9+** device, download `Kog-…-android-arm64-debug.apk`.
2. Open the download. If prompted, allow your browser or file manager to
   **install unknown apps**, then tap **Install**.
3. Open Kog and choose local files, or enter your Kog server address in Settings.

This is a development APK, not a Google Play release.
[Android installation help](https://support.google.com/pixelphone/answer/7391672)
· [Local playback and build details](android/README.md)

</details>

<details>
<summary><strong>iPhone / iPad: unsigned IPA</strong></summary>

1. Download `Kog-…-ios-arm64-unsigned.ipa` to a Mac or Windows computer.
   The app requires **iOS 17+**.
2. Install a signing tool such as [Sideloadly](https://sideloadly.io/), including
   the Apple components it requests on Windows. Connect your unlocked device
   by USB and accept **Trust This Computer**.
3. Load the IPA into the tool, select your device, sign in with your Apple
   account, and start installation. The tool signs Kog for your device.
4. If iOS requests it, enable **Developer Mode** under **Settings → Privacy &
   Security**, then trust your developer profile under **General → VPN &
   Device Management** before opening Kog.

Free-account installs expire after seven days. Enable your signing tool's
automatic refresh, or re-sign before expiry; the computer must be able to
reach your device. This IPA is not an App Store or TestFlight distribution.
[Developer Mode](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device)
· [Build and sign with Xcode](ios/README.md)

</details>

<details>
<summary><strong>Web player: any desktop or phone browser</strong></summary>

1. Start Kog's server using **Preferences → Server** in Qt/TUI, or the headless
   server package above. Configure credentials and a network listening address
   when connecting from another device.
2. Open the server's address in your browser and sign in. Use the server
   computer's LAN address on a phone; the local default is `http://127.0.0.1:8420`.
3. To add a phone launcher:

   - **iPhone/iPad:** Safari → **Share → Add to Home Screen**, enable
     **Open as Web App** if shown, then **Add**.
   - **Android:** Chrome's menu → **Install and create shortcut → Install**
     (older versions say **Install app** or **Add to Home screen**).

The web player needs the Kog server running.
[Server setup](docs/SERVER.md) ·
[Safari help](https://support.apple.com/guide/iphone/open-as-web-app-iphea86e5236/ios) ·
[Chrome help](https://support.google.com/chrome/answer/9658361?co=GENIE.Platform%3DAndroid)

</details>

<details>
<summary><strong>Nix / NixOS (Linux), or build from source</strong></summary>

With Nix and flakes enabled, install the tagged package into your user profile:

```sh
nix profile add 'git+https://github.com/benwbooth/Kog?ref=refs/tags/v0.9.42&submodules=1#default'
kog --gui
```

For a declarative NixOS installation, use the
[flake configuration example](packaging/README.md#nixos). Nix builds Kog from
source, so the first installation takes longer than downloading a binary.

To build from source, clone the submodules too:

```sh
git clone --recurse-submodules https://github.com/benwbooth/Kog.git
cd Kog
```

Then follow the [desktop build guide](docs/BUILDING.md),
[Android build guide](android/README.md), or [iOS build guide](ios/README.md).
GitHub's automatic **Source code** archives omit the submodule sources and
are not ready-to-run installers.

</details>

For the newest features, download artifacts from a successful
[package build](https://github.com/benwbooth/Kog/actions/workflows/packages.yml).

## Start listening

Open Kog, choose your music folder, and add songs to the playlist. In a terminal:

```sh
kog --gui       # Desktop player
kog --tui       # Terminal player (Linux/macOS); press ? for help
kog --server    # Web player and API, using your saved server settings
```

Running `kog` in an interactive Unix terminal selects the TUI automatically.
For phone/browser access, enable the server in **Preferences → Server**, then
open its address. [Setup and authentication](docs/SERVER.md)

Packages include their decoder libraries. MIDI SoundFonts and Roland ROMs are
user supplied; [MIDI setup](docs/USAGE.md#midi-soundfonts-and-roland-emulation)
explains the options.

## More

[User guide](docs/USAGE.md) · [Build from source](docs/BUILDING.md) ·
[Architecture](docs/ARCHITECTURE.md) · [Skins and visualizers](docs/SKINS_AND_VISUALIZERS.md)

Kog-authored code is **GPL-3.0-or-later**. Third-party components and artwork
retain their own licenses. [License policy](docs/LICENSING.md) ·
[Third-party notices](THIRD_PARTY_NOTICES.md)
