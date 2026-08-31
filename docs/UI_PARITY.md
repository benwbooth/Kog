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
| Partial | Main window and toolbar | Cog-shaped toolbar, transport, position, volume, search, split content, and duration footer render and operate | Platform-specific chrome, toolbar customization, full shuffle/repeat policy, speed/pitch controls, and reference-image diffs |
| Partial | File tree | Browsable local directory list with parent navigation and add-on-activate | Hierarchical lazy tree, watched folders, smart folders, expansion persistence, icons, context actions, and Cog drag behavior |
| Partial | Playlist table | Metadata rows, alternating backgrounds, selection, double-click playback, live play marker, filtering, removal, and drop-add | Every Cog column, sorting, column chooser/order/width persistence, multiselect, reordering, queue, ratings, context menu, inline editing, and playlist persistence |
| Partial | Info Inspector | Real detached window follows the playing track and displays common/technical metadata including length and filename | Album artist, composer, full date semantics, true codec identification, extracted artwork, selection-vs-playing policy, and saved placement |
| Partial | Mini player | Detached compact transport with title, artist, and seek | Cog's mini/dual mode switching, sizing/chrome, volume and remaining controls, placement persistence, and capture corpus |
| Partial | Preferences | Real MIDI pane selects and persists RustySynth or OPL3Windows, and selects, validates, clears, and persists an SF2 SoundFont | General, appearance, playlist, output, remaining MIDI engines/options, notifications, shortcuts, Last.fm, remote-control, time-stretch, and platform path panes |
| Not started | Equalizer | None | Enable/tracking controls, presets, preamp, Cog's complete band surface, editing gestures, persistence, and DSP binding |
| Not started | Lyrics | None | Lyrics display, update behavior, empty/error states, selection and window persistence |
| Not started | Spectrum and SC-55 visualization | None | Spectrum modes/settings and the Nuked SC-55 visualization window |
| Not started | Spotlight/library search | None | Search location, result columns/sorting, add-to-playlist, reveal action, and keyboard flow |
| Not started | Tag editor | None | Multi-track edit states, artwork, save/reload/error behavior, and decoder-safe file updates |
| Not started | Open URL and path suggester | None | URL entry/history, validation, local sandbox/path suggestion equivalents, and error states |
| Not started | About, credits, and feedback | None | Cross-platform equivalents using Kog identity and third-party notices |

## Interaction and integration surface

| State | Behavior |
| --- | --- |
| Partial | File/folder dialogs, local drag-and-drop, search, core transport shortcuts, seek, volume, end-of-track advance |
| Not started | Full Cog menu hierarchy and enablement rules, playlist save/load, Open Recent, URL playback, queue/stop-after-selection, previous/next album, repeat modes, shuffle modes |
| Not started | Media keys, desktop notifications, dock/tray behavior, Now Playing/MPRIS/SMTC/MediaRemote integration, Last.fm, remote control, accessibility pass, localization |

## Visual verification states

Each finished surface needs fixed-size captures for empty, populated, selected,
playing, paused, stopped, filtered, loading, and error states where applicable.
The test harness must use deterministic metadata and fonts, compare structure
and pixels with an explicit tolerance, and retain Cog and Kog images together
so visual similarity cannot be asserted from memory.
