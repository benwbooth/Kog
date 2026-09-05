<p align="center">
  <img src="qml/icons/kog.svg" width="96" height="96" alt="Kog logo">
</p>

<h1 align="center">Kog</h1>

<p align="center">
  A cross-platform music player for ordinary audio, MIDI, trackers, chiptunes,
  and game music.
</p>

<p align="center">
  <a href="https://github.com/benwbooth/Kog/actions/workflows/packages.yml"><img src="https://github.com/benwbooth/Kog/actions/workflows/packages.yml/badge.svg" alt="Cross-platform package status"></a>
</p>

Kog is a local desktop music player inspired by
[Cog](https://github.com/losnoco/Cog). It combines a Rust playback core with a
Qt Quick interface and aims to make mixed music collections feel like one
library—even when that collection includes SoundFont MIDI, tracker modules,
console soundtracks, and archived albums alongside FLAC and MP3 files.

Kog is under active development. The playback paths described below are real,
but format compatibility and exact Cog parity are still being tested across a
larger file corpus. See the [format parity matrix](docs/FORMAT_PARITY.md) for
the honest, format-by-format status.

## Download

Download the latest packages from [GitHub Releases](https://github.com/benwbooth/Kog/releases/latest).

| Platform | Packages |
| --- | --- |
| Windows | MSI installer or portable ZIP |
| macOS | DMG for Apple Silicon or Intel |
| Linux | AppImage, portable AppDir archive, Flatpak bundle, or Flatpak repository archive |

Release packages include the decoder components they need. You do not need to
install separate player programs or command-line decoders.

The current packages are unsigned development releases. Windows may show a
SmartScreen warning, and macOS packages are ad-hoc signed rather than notarized.

## Getting started

Use the hamburger menu to add individual files, a music folder, a playlist, an
archive, or an HTTP(S) URL. The file browser can be re-rooted to any folder;
single-click a folder to expand it, then double-click or drag files and folders
into the playlist. Shift and Ctrl selection work for adding groups of items.

Kog uses the desktop's Qt theme and native file dialogs. Closing the main window
to the system tray is enabled by default where a tray is available. Tray,
minimize, and close behavior can be changed under **Preferences → General**.
The main window remembers its size and maximized state. Position is restored
on Windows, macOS, X11, and supported modern Wayland desktops.

Optional now-playing popups have playback controls and stay open while hovered.
Drag the popup's header to move it; Kog remembers the position. Right-click its
header to reset it to the bottom-right, above the panel.

### Skins and visualizers

**View → Classic Skins** opens a read-only Internet Archive gallery with search,
previews, and downloads. You can also import a classic Winamp `.wsz` or `.zip`.
Installed skins stay available offline. Choose **Use skin** to open the classic
player, and **Queue** to return to the normal Kog window.

This first version skins the main transport panel, with 1×/2×/3× scaling.
The playlist and equalizer still use Kog's native interface. Modern `.wal`
skins, windowshade/shaped windows, and legacy visualizer plugins are not supported.
Skin downloads retain their source and attribution; their artwork is not
covered by Kog's license. Kog does not run scripts or executables from skins.

**View → Visualizer** (**Ctrl+Shift+V**) offers a blue/green spectrum and an
oscilloscope driven by the actual decoded audio. **F11** toggles full screen;
**Escape** leaves full screen or closes the visualizer. No extra programs or
preset downloads are needed. MilkDrop/projectM effects are not included yet.

## What it can do

- Play local files, playlists, archives, direct HTTP streams, and HLS streams.
- Expand subsongs and companion-file formats without flattening their identity.
- Play MIDI through an SF2 SoundFont, OPL3 emulation, Nuked SC-55, or Munt
  MT-32/CM-32L emulation.
- Read metadata and edit common tags, lyrics, and cover art on supported local
  files.
- Search large playlists, manage an explicit playback queue, and use album/all
  shuffle plus one/album/all repeat modes.
- Use a 31-band equalizer, lyrics and inspector windows, a mini-player, system
  tray controls, optional themed now-playing popups with playback controls, Linux MPRIS
  lock-screen/media-key controls, and audio-reactive playback indicators.
- Decode undeclared legacy metadata with Japanese and other regional encoding
  heuristics, including Shift-JIS, EUC-JP, UTF-16, and UTF-32 inputs.
- Follow the system audio output or remember a selected output device.

## Format support

This is a practical overview, not an exhaustive extension list. Kog also uses
content probing where extensions are ambiguous.

| Family | Implemented support |
| --- | --- |
| Everyday audio | AAC, AIFF, ALAC, CAF, FLAC, MP1/MP2/MP3, MP4/M4A, Ogg Vorbis, Opus, WAV, WebM/Matroska, WMA/ASF, APE, AC-3, DTS, TTA, WavPack, Musepack, Shorten, RealAudio, and DSD containers |
| Trackers | MOD, XM, S3M, IT, MPTM and the wider libopenmpt family; compressed module aliases; AHX/HVL; Organya; Syntrax JXS |
| Chiptune | AY, GBS, HES, KSS, NSF/NSFE, SAP, SPC, SFM, SID/PSID, VGM/VGZ, S98, DRO, GYM, plus AdPlug's OPL family |
| Game audio | NCSF, GSF, QSF, SSF, DSF, USF, PSF, PSF2, SNSF, 2SF and their mini variants, plus the broad vgmstream family |
| MIDI | MID/MIDI, KAR, RMID, MIDS/MDS, LDS, XMF/MXMF, HMI/HMP/HMQ, MUS, and XMI |
| Containers | CUE, APL, M3U/M3U8, PLS, ZIP, RAR, 7Z, RSN, VGM7Z, and GZ |

Several families have file-specific requirements, and support is not yet equally
mature across every platform. In particular, the current Windows package does
not bundle the 2SF helper. The [full format matrix](docs/FORMAT_PARITY.md)
records the backend, tested behavior, and remaining work for each family.

## MIDI, SoundFonts, and Roland emulation

Choose a MIDI engine under **Hamburger menu → Preferences → Synthesis**.

| Engine | What you need |
| --- | --- |
| RustySynth | An SF2 SoundFont selected in Preferences |
| OPL3Windows | Nothing extra; Kog includes the Nuked OPL3 core |
| Nuked SC-55 | A complete supported SC-55-family ROM set from hardware you own |
| Munt MT-32/CM-32L | A compatible control ROM and PCM ROM pair from hardware you own |

Munt playback maps General MIDI program numbers to the closest stock MT-32
patches by default. Disable **Map General MIDI programs to MT-32 patches** for
scores authored specifically for the MT-32 or CM-32L, especially files that
load custom timbres with SysEx.

Kog does not include or download Roland ROMs. You can select a ROM folder or
import a ZIP, 7Z, RAR, TAR, gzip, bzip2, xz, or other libarchive-supported
archive. ROMs are identified by their contents, so they do not need special
filenames.

For scripted setups, Kog also understands:

```text
KOG_SOUNDFONT=/path/to/bank.sf2
KOG_SC55_ROMS=/path/to/sc55-rom-directory
KOG_MT32_ROMS=/path/to/mt32-rom-directory
KOG_MT32_GM_PROGRAM_MAPPING=true|false
KOG_MIDI_ENGINE=rustysynth-sf2|opl3windows|nuked-sc55|munt-mt32
```

Organya playback needs a user-owned `soundbank.wdb`, or a `wavetable.dat` and
`drums.dat` pair. Put the files beside the `.org` file, in Kog's platform data
directory, or set `KOG_ORGANYA_SOUNDBANK`.

## Build from source

The supported development environment is the included Nix flake:

```sh
git clone --recurse-submodules https://github.com/benwbooth/Kog.git
cd Kog
nix develop
cargo run
```

For an existing checkout, initialize the native sources first:

```sh
git submodule update --init --recursive
```

A direct Cargo build requires Rust, C and C++23 compilers, CMake, `pkg-config`,
Qt 6 with Qt Quick and Qt Quick Controls, FFmpeg development libraries, zlib,
and libarchive 3.2 or newer. On Wayland, install KDE's Layer Shell Qt QML module
for corner-anchored, draggable now-playing popups. The Nix shell supplies it,
along with the dependency versions and FFmpeg configuration used by Kog's
regression tests.

Useful project references:

- [Packaging and release artifacts](packaging/README.md)
- [Format parity matrix](docs/FORMAT_PARITY.md)
- [UI parity matrix](docs/UI_PARITY.md)
- [Architecture](docs/ARCHITECTURE.md)
- [License policy](docs/LICENSING.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

Bug reports and representative problem files are welcome in the
[issue tracker](https://github.com/benwbooth/Kog/issues). Please mention the
platform, package type, file format, and what happened during playback or seek.

## License

Kog-authored code is licensed under GPL-3.0-or-later. Third-party libraries,
optional helper components, and test assets retain their own licenses. See the
[license policy](docs/LICENSING.md) and
[third-party notices](THIRD_PARTY_NOTICES.md) for details.
