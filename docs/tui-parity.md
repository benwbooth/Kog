# Terminal UI parity checklist

This is the feature inventory for the terminal frontend, checked against `qml/Main.qml` and `crates/kog-web/src/lib.rs`. A checked item has been exercised in the isolated PTY test at the terminal sizes where the control is visible. Unchecked items still need implementation, verification, or both; the TUI should not be described as fully equivalent while they remain.

Run `nix develop --command cargo build -p kog --bin kog`, then `uv run --with pyte --with mutagen scripts/tui_pty_check.py` and `nix develop --command uv run --with pyte scripts/tui_server_interop_check.py`. The tests create their own music trees, archives, settings directories, and SQLite databases. They do not write to the user's library or playlists.

## Window and input

- [x] Terminal resize redraws without losing the active pane.
- [x] Top playlist search edits in its field, filters as typed, clears, and handles cursor keys and paste.
- [x] File search edits in its field, searches as typed, clears, and shows progress.
- [x] Mouse drag resizes the file tree divider and keeps the chosen width on resize.
- [x] Files and Playlists section headers expand and collapse.
- [x] Hamburger opens a structured, keyboard and mouse operable menu with working commands.
- [x] Right click opens context menus for tree items, playlist rows, saved playlists, and columns.
- [x] Mouse wheel and Home/End navigation keep long tree and playlist selections visible, including at 48 columns.
- [x] Multi selection supports Ctrl, Shift, and deletion of all selected rows.
- [x] Path, URL, name, and preference prompts edit in a centered dialog with a visible caret and Ctrl+A replacement.

## Library and playlists

- [x] Files are displayed in a lazily expandable tree, including nested folders.
- [x] A token-protected remote Kog library can be browsed, searched, queued by folder, and streamed; URL, auth mode, and codec controls are in the Remote Server menu. PTY fixtures check the mock API and a real headless Kog server, including nested archive playback.
- [x] A folder click expands or collapses it; a folder double click queues its contents without starting playback.
- [x] A file double click adds and plays; single click selects.
- [x] Ctrl and Shift mouse selection, Shift arrow range selection, and Ctrl+A work in the tree; Add applies to selected files and folders. Play Now on a folder starts its first loaded track.
- [x] Tree context actions can change and reset the visible root, blacklist selected songs, and move several local files to trash after one confirmation. Delete opens the same confirmation.
- [x] Nested archive folders can be explored and added. Direct adds and folder scans expand multi-song files; a three-song NSF is checked in unit and PTY fixtures.
- [x] Saved playlist single click selects the source; double click adds its tracks to the playing pane.
- [x] Favorites, create, rename, delete, save current pane, and add to a saved playlist work.
- [x] Queue remove, clear, reorder, sort, star, and search work with the visible selection.
- [x] Playlist rows show decoded title, artist, and album without probing on the input thread.
- [x] Default #, Title, Artist, and Album columns have header sorting, mouse resizing, visibility toggles, and auto fit from loaded metadata.
- [x] All 20 Qt columns can be shown, hidden, sorted, resized, reordered, and scrolled horizontally. The terminal saves its character-width layout and can import an existing Qt column order.
- [x] Add File, Add URL, Save Current Playlist, and Add to Saved Playlist write the expected queue and database entries.
- [x] Save Selection As and saved playlist Duplicate write the expected database entries.
- [x] Saved playlist Add, Play, and Replace are available from its context menu.
- [x] Playlist row Show in File Tree focuses the matching local file.
- [x] Saved playlist export writes a portable M3U; prune missing deletes only absent local/archive entries.
- [x] Ctrl and Shift select saved playlists; Add and Play use the selected set, and confirmed Delete removes the selected saved lists.
- [x] Tree and playlist song/folder blacklist actions persist to the shared database; the menu can view and remove entries.
- [x] Confirmed Move to Trash runs off the input thread, removes local tree items and their current playlist rows, and is exercised with disposable files.
- [x] Track tag editing stages the Qt fields and artwork actions, saves through the shared writer off the input thread, and updates multiple local files. The PTY test checks album and artwork tags and continued playback after saving.

## Playback and settings

- [x] Play, pause, stop, and seek respond to mouse input and update the transport in a PTY with a ten-second WAV.
- [x] Previous, next, and automatic track completion respond correctly in PTY playback.
- [x] Repeat and shuffle use Qt's shared playback-order engine. Its focused unit tests cover repeat one, album, all, and album shuffle; the PTY test checks mode cycling, unique all-track shuffle, and previous.
- [x] Volume is a visible draggable slider with click, drag, mute, keyboard, and percentage feedback.
- [x] Random Radio can start from an empty queue, stage a round off the input thread, and play one track.
- [x] Radio advances at the end of its visible queue and stages another song.
- [x] Radio excludes blacklisted tracks after a root change; the PTY test verifies that only the allowed WAV plays.
- [x] Per-row queue and stop-after markers change playback order in PTY playback.
- [x] Queue positions and stop-after markers stay with their tracks across sort, move, and removal.
- [x] Late album metadata regroups only the unplayed part of album shuffle; a focused playback-order test verifies the played prefix and remaining album sequence.
- [ ] Album metadata arriving during playback has a dedicated terminal regression fixture for shuffle and repeat order.
- [x] Current track, elapsed time, duration, and play/pause state update during PTY playback.
- [x] Music root, volume, equalizer preset, preamp, and output device controls are reachable from menus.
- [x] The Qt opening-files preference is saved and applied to activated files; the PTY fixture checks replace-and-play and enqueue without interrupting playback.
- [x] MIDI backend, SoundFont, ROM directory, and MT-32 mapping controls update the live shared decoder settings; the PTY fixture checks menu access, validation, clearing, and persistence.
- [ ] Remaining Qt/Web playback preferences are reachable.
- [x] Track info, lyrics, and equalizer have terminal modal views. The info view includes Qt's technical fields and source path.
- [x] Supported Formats opens the shared decoder catalog in a scrollable terminal view; PTY checks its first format group.
- [x] The spectrum visualizer reads live audio data, and individual equalizer bands can be edited from Preferences.

## Verification

- [x] Tests exercise text entry, clicks, drags, double clicks, menus, resize, and the playlist database on a PTY.
- [x] Decoder diagnostics are written to an owner-only log instead of drawing over the TUI; the PTY fixture checks the log permissions.
- [x] A populated 120×40 terminal frame was reviewed against the running Qt and web layouts for toolbar, sidebar, striped playlist, search fields, and transport placement.
- [ ] Cover art and the full Qt/Web information density have suitable terminal representations.

## Cross-frontend feature inventory

The Qt hamburger, playlist and file context menus, playlist header, and the web player were used as references. The items below remain open even if a related basic action above is checked.

| Area | Qt/Web behavior still missing or unverified in TUI |
| --- | --- |
| File tree | Remote multi-song files need `/api/expand` when queued; local three-song NSF and remote nested archives are verified. |
| Saved playlists | Remote track round trips through save, load, and export need a fixture. |
| Playback | Terminal timing fixture for late album tags, playback error recovery, and output device test on real hardware. |
| Library settings | Read CUE and M3U/PLS folder preferences need menu controls and behavior tests. |
| Synthesis | SC-55 and MT-32 archive import needs terminal controls and isolated ROM fixtures. |
| Server settings | Address, port, authentication, TLS, codec/cache, device management, and start/stop controls are not in the TUI; `--server` runs the persisted Qt configuration. |
| Media | Automatic cover downloading and cover art display need terminal representations and tests. |
| Views | Compact mini player, Winamp skin presentation, and live inspector refresh need terminal equivalents. |
