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
| Partial | File tree | Real Qt `TreeView` over a lazy, filesystem-watching `QFileSystemModel`, with expandable directories, themed icons, parent/root selection, tooltips, and add-on-file-activate | Smart folders, expansion persistence, context actions, selection actions, and Cog drag behavior |
| Partial | Playlist table | Metadata rows, alternating backgrounds, selection, double-click playback, live play marker, filtering, removal, drop-add, and decoder-provided subsong expansion | Every Cog column, sorting, column chooser/order/width persistence, multiselect, reordering, queue, ratings, context menu, inline editing, and playlist persistence |
| Partial | Info Inspector | Real detached window follows the playing track and displays common/technical metadata including length and filename | Album artist, composer, full date semantics, true codec identification, extracted artwork, selection-vs-playing policy, and saved placement |
| Partial | Mini player | Detached compact transport with title, artist, and seek | Cog's mini/dual mode switching, sizing/chrome, volume and remaining controls, placement persistence, and capture corpus |
| Partial | Preferences | Cog-derived General, Playback, Synthesis, and Updating sections use themed Qt controls. Synthesis persists RustySynth/SF2, OPL3Windows, Nuked SC-55, or Munt MT-32/CM-32L and uses native pickers for SF2 and user ROM locations. | Remaining synthesis/effect controls, appearance, output-device detail, notifications, shortcuts, Last.fm, remote-control, time-stretch, and platform path panes |
| Not started | Equalizer | None | Enable/tracking controls, presets, preamp, Cog's complete band surface, editing gestures, persistence, and DSP binding |
| Partial | Lyrics | Cog-sized resizable read-only scrolling window follows the playing track, with ID3v2 USLT and generic tag extraction in Cog's fallback order; the hamburger View submenu exposes it with `Ctrl+Shift+L` | Decoder-native lyrics outside Lofty's tag coverage, selection-vs-playing policy, synchronized lyrics, placement persistence, and deterministic populated/empty captures |
| Not started | Spectrum and SC-55 visualization | None | Spectrum modes/settings and the Nuked SC-55 visualization window |
| Not started | Spotlight/library search | None | Search location, result columns/sorting, add-to-playlist, reveal action, and keyboard flow |
| Not started | Tag editor | None | Multi-track edit states, artwork, save/reload/error behavior, and decoder-safe file updates |
| Not started | Open URL and path suggester | None | URL entry/history, validation, local sandbox/path suggestion equivalents, and error states |
| Not started | About, credits, and feedback | None | Cross-platform equivalents using Kog identity and third-party notices |

## Interaction and integration surface

| State | Behavior |
| --- | --- |
| Partial | Qt platform-native file/folder dialogs (with Qt's platform fallback), local drag-and-drop, search, core transport shortcuts, seek, volume, end-of-track advance |
| Partial | Kog deliberately uses one toolbar hamburger menu instead of Cog's macOS menu bar. Implemented file, playlist-removal, View-window, playback, preferences, and quit commands retain shortcuts and state-based enablement; playlist save/load, Open Recent, URL playback, queue/stop-after-selection, previous/next album, repeat modes, and shuffle modes remain |
| Not started | Media keys, desktop notifications, dock/tray behavior, Now Playing/MPRIS/SMTC/MediaRemote integration, Last.fm, remote control, accessibility pass, localization |

## Visual verification states

Each finished surface needs fixed-size captures for empty, populated, selected,
playing, paused, stopped, filtered, loading, and error states where applicable.
The test harness must use deterministic metadata and fonts, compare structure
and pixels with an explicit tolerance, and retain Cog and Kog images together
so visual similarity cannot be asserted from memory.
