# Using Kog

## Choose an interface

Launch Kog from your desktop to open Qt. On Linux and macOS, launching `kog`
in an interactive terminal opens the terminal player. Override this with
`--gui`, `--tui`, or `--server`. Windows currently supports Qt and server mode.
Separate `kog-tui` and `kog-server` packages are available for Linux and Apple
Silicon in successful [build artifacts](https://github.com/benwbooth/Kog/actions/workflows/packages.yml);
keep each extracted bundle together with its libraries.

## Add your music

Choose a music folder from the folder button or the main menu. Browse the tree,
search files and folders, or add a whole folder to the queue. On desktop,
double-click or drag items into the playlist. On phones, use the **+** buttons
in Library and tap a queued song to play it.

ZIP, 7z, and RAR archives browse like folders. Kog expands supported subsongs
and cue sheets and keeps companion files together. Search works through
subfolders and archives; progress is shown while indexes are built.

Save named playlists to return to an album or mix, and star favorite tracks.
Random Radio keeps choosing music from your selected library root. The current
queue is saved between launches; each frontend keeps its own playback session.

## Terminal controls

Press **?** for the complete keyboard guide. **Tab** changes panes, arrows move
through items, **Enter** opens or plays, **Space** pauses, and **Delete** removes
selected queue entries. **/** searches files, **o** chooses a music folder,
**m** opens the menu, and **Esc** asks to quit. Menu mnemonics work with **Alt**.
Use **x** to mark items and **v**, then arrows or a click, for a range when your terminal reserves
Shift-click. Left/Right scroll playlist columns; Left at the beginning returns
to the file tree, and Right from the tree focuses the playlist.

Mouse clicks, double-clicks, context menus, dragging dividers, and scrolling
work in terminals with SGR mouse support. Resizing the terminal reflows the UI.
The terminal player includes radio, playlists, settings, equalizer, artwork,
and audio visualization. Font and image support depend on your terminal.

## Listen on another device

In Qt, open **Preferences → Server**, set your music folder and credentials,
and start the server. Open its address in a browser, or connect the Android or
iPhone app with the same address and credentials. Each device has its own
queue. The server decodes formats that browsers cannot play themselves.

`kog --server` or `kog-server` starts the same service without a local UI,
using the saved settings. It listens on loopback by default; set credentials
before allowing network connections. See the [server guide](SERVER.md).

The mobile web player can be added to your home screen. Native
[Android](../android/README.md) and [iOS](../ios/README.md) apps can also import
files and play them locally without a server. Their guides list current limits.

## Desktop extras

Qt follows your desktop theme and supports a system tray, mini-player,
now-playing popups, tag editing, lyrics, a 31-band equalizer, and audio
visualizers. Change tray behavior in **Preferences → General**. **F1** opens
About Kog. Classic Winamp skins and experimental modern skins are available;
see [skins and visualizers](SKINS_AND_VISUALIZERS.md) for compatibility details.

## Exporting audio

Right-click playlist rows in Qt and choose **Export…**, or use **Tracks →
Export…** in the terminal player, to render tracks to audio files: FLAC,
WAV (16 or 24-bit), Apple Lossless, MP3, AAC (M4A), Ogg Vorbis or Opus.
Tracks are decoded exactly as they play, with the same synth and emulator
settings, and can include the equalizer and effects. Files are named after
the track number and title, never overwrite existing files, and carry title,
artist, album and track tags. Several tracks can also get an M3U playlist of
the exported files. In the terminal, **Preferences → Cycle Export Format**
and **Export With Effects On/Off** choose how. Saved playlists export as M3U
from their right-click menu in Qt and the web player, and from **Saved →
Export as M3U…** in the terminal.

## Effects

**Preferences → Effects** in Qt, and **Preferences → Effects…** in the
terminal player, add an effects chain after the equalizer: reverb, stereo
widener, echo, chorus, warmth (bass boost and high cut), bitcrusher,
distortion, and compressor/limiter. Effects run top to bottom; each can be
switched off, moved, removed, or added more than once, and every setting has
a slider. Presets (Room, Hall, Wide chip, Lo-fi handheld, Arcade cabinet,
Echo chamber, Overdrive) are a starting point. Changes apply at once and are
saved for both players. The web player and the phone apps do not have
effects yet.

**Custom effects** are built from blocks: filters (low-pass, high-pass,
band-pass, notch, peak, shelves, all-pass), delay lines, combs, all-passes,
gain, waveshapers, bit and rate crushers, stereo width, pan, compressors and
ring modulators. Blocks run in series, in parallel paths that are summed at
their own levels, or in feedback loops, and any setting can be moved by an
LFO or an envelope follower. Start from an empty effect or from a template
(the built-in effects rebuilt from blocks, an auto-wah and a tremolo). In Qt,
**New custom effect** opens the editor; in the terminal, **N** makes one and
**E** edits it, with **A** adding a block, **P** parallel paths, **F** a
feedback loop, **L**/**V** an LFO or envelope follower, and **M** choosing
what moves the selected setting. Saved custom effects can be added to the
chain like any other effect.

## MIDI, SoundFonts, and Roland emulation

Use **View → Channel Keyboards + Tracker** (or the separate keyboards, tracker
and **MML Score** entries) to watch a keyboard for each channel and a
tracker with notes, effects, and song data. Qt and web also use **Ctrl+Shift+I**;
the native phone apps expose it in Now Playing. See the
[channel inspector guide](CHANNEL_INSPECTOR.md) for controls and decoder coverage.

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
