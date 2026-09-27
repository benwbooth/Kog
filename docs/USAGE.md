# Using Kog

## Choose an interface

Launch Kog from your desktop to open Qt. On Linux and macOS, launching `kog`
in an interactive terminal opens the terminal player. Override this with
`--gui`, `--tui`, or `--server`. Windows currently supports Qt and server mode.
Separate `kog-tui` and `kog-server` packages are available for Linux and Apple
Silicon; keep each extracted bundle together with its libraries.

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

