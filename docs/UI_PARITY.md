# Cog UI parity matrix

Reference: `losnoco/Cog` commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722` (2026-08-16), especially
`Base.lproj/MainMenu.xib`, the other `Base.lproj/*.xib` windows, the Swift
preferences panes, and `.github/images/MainWindow.png`.

`Done` requires matching states, interactions, keyboard behavior, resizing,
and deterministic visual captures on all three desktop platforms. `Partial`
means the control is real and usable but does not yet cover Cog's complete
surface.

## Windows and panels

| State | Cog surface | Current Kog coverage | Required parity work |
| --- | --- | --- | --- |
| Partial | Main window and toolbar | Cog-shaped toolbar, themed transport controls, position, volume, search, split content, duration footer, and the user-selected cross-platform hamburger command menu render and operate without overriding the active Qt/KDE palette | Platform-specific chrome, toolbar customization, full shuffle/repeat policy, speed/pitch controls, and reference-image diffs |
| Partial | File tree | Real Qt `TreeView` over a lazy, filesystem-watching `QFileSystemModel`, with expandable directories, themed icons, parent/root selection, tooltips, native file double-click using Cog's opening policy, and standards-based URI drag into the playlist | Smart folders, expansion persistence, context actions, multi-selection actions, and corpus-driven comparison of Cog's exact drag gestures |
| Partial | Playlist table | Metadata rows, alternating backgrounds, Ctrl/Command and Shift multi-selection, keyboard range selection, double-click playback, live play marker, filtering, batch removal, external/tree drop-add, internal multi-row drag reordering with a themed insertion marker, a playlist context menu, decoder-provided subsong expansion, stable type-aware header sorting with original-order restoration, and Cog's complete 19-column surface. Header resizing, separator double-click auto-fit, visibility, and move-left/right order persist as one validated layout with migration from Kog's earlier nine-width setting. Cog-style Save As and Save Selection As write and reopen M3U/M3U8/PLS while preserving local, remote, CUE, subsong, and archived-member identities. | Direct header-drag ordering, queue, ratings, play counts, inline editing, automatic session persistence, and corpus-driven comparison of Cog's exact drag gestures |
| Partial | Info Inspector | Real detached window follows the playing track and displays common/technical metadata including length and filename | Album artist, composer, full date semantics, true codec identification, extracted artwork, selection-vs-playing policy, and saved placement |
| Partial | Mini player | Detached compact transport with title, artist, and seek | Cog's mini/dual mode switching, sizing/chrome, volume and remaining controls, placement persistence, and capture corpus |
| Partial | Preferences | Cog-derived General, Playback, Synthesis, and Updating sections use themed Qt controls. Playlist opening behavior preserves Cog's clear-and-play, enqueue, and enqueue-and-play choices; synthesis persists RustySynth/SF2, OPL3Windows, Nuked SC-55, or Munt MT-32/CM-32L and uses native pickers for SF2 and user ROM locations. | Modifier-specific opening behavior, remaining synthesis/effect controls, appearance, output-device detail, notifications, shortcuts, Last.fm, remote-control, time-stretch, and platform path panes |
| Partial | Equalizer | The hamburger View menu and `Ctrl+E` toggle a themed Cog-sized window with enable and genre-tracking controls, Cog's exact 22-preset library and 10-to-31-band interpolation, preamp, all 31 bands, Flatten EQ, Level Preamp, tooltips, and Cog's right-drag curve editing gesture. Strict settings persist across launches, genre changes select the longest matching preset, and one shared Q=1.4 peaking-biquad DSP path applies live to every decoder with independent channel state and resets on track changes and seeks. | Direct PCM comparison with Cog's vDSP implementation, saved window placement, accessibility audit, and deterministic light/dark captures on Linux, Windows, and macOS |
| Partial | Lyrics | Cog-sized resizable read-only scrolling window follows the playing track, with ID3v2 USLT and generic tag extraction in Cog's fallback order; the hamburger View submenu exposes it with `Ctrl+Shift+L` | Decoder-native lyrics outside Lofty's tag coverage, selection-vs-playing policy, synchronized lyrics, placement persistence, and deterministic populated/empty captures |
| Not started | Spectrum and SC-55 visualization | None | Spectrum modes/settings and the Nuked SC-55 visualization window |
| Not started | Spotlight/library search | None | Search location, result columns/sorting, add-to-playlist, reveal action, and keyboard flow |
| Not started | Tag editor | None | Multi-track edit states, artwork, save/reload/error behavior, and decoder-safe file updates |
| Partial | Open URL and path suggester | The hamburger menu exposes Cog's Add URL command with its `Ctrl+Shift+O` shortcut, themed modal HTTP(S) entry, Rust URL validation, Cog opening-policy integration, and linked-FFmpeg HTTP/HLS playback | URL history/autocomplete, richer inline validation and loading/error states, local sandbox/path suggestion equivalents, and deterministic captures |
| Not started | About, credits, and feedback | None | Cross-platform equivalents using Kog identity and third-party notices |

## Interaction and integration surface

| State | Behavior |
| --- | --- |
| Partial | Platform-native file/folder/save dialogs, local drag-and-drop, search, core transport shortcuts, seek, volume, end-of-track advance |
| Partial | Kog deliberately uses one toolbar hamburger menu instead of Cog's macOS menu bar. Implemented file/URL and playlist opening, Save As, Save Selection As, playlist-removal, View-window (including Equalizer), playback, preferences, and quit commands retain icons, shortcuts, and state-based enablement; Open Recent, queue/stop-after-selection, previous/next album, repeat modes, and shuffle modes remain |
| Not started | Media keys, desktop notifications, dock/tray behavior, Now Playing/MPRIS/SMTC/MediaRemote integration, Last.fm, remote control, accessibility pass, localization |

## Visual verification states

Each finished surface needs fixed-size captures for empty, populated, selected,
playing, paused, stopped, filtered, loading, and error states where applicable.
The test harness must use deterministic metadata and fonts, compare structure
and pixels with an explicit tolerance, and retain Cog and Kog images together
so visual similarity cannot be asserted from memory.
