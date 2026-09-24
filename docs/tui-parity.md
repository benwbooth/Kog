# Terminal UI parity checklist

This is the feature inventory for the terminal frontend, checked against `qml/Main.qml` and `crates/kog-web/src/lib.rs`. A checked item has been exercised in the isolated PTY test at the terminal sizes where the control is visible. Unchecked items still need implementation, verification, or both; the TUI should not be described as fully equivalent while they remain.

Run `nix develop --command cargo build -p kog --bin kog`, then `uv run --with pyte scripts/tui_pty_check.py`. The test creates its own music tree, archive, settings directory, and SQLite database. It does not write to the user's library or playlists.

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
- [x] Path, URL, name, and preference prompts edit in a centered dialog with a visible caret.

## Library and playlists

- [x] Files are displayed in a lazily expandable tree, including nested folders.
- [x] A folder click expands or collapses it; a folder double click queues its contents without starting playback.
- [x] A file double click adds and plays; single click selects.
- [x] Nested archive folders can be explored and added. Subsong expansion still needs a dedicated PTY fixture.
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
- [ ] Track tag editing, blacklist, and tree file deletion are available.

## Playback and settings

- [x] Play, pause, stop, and seek respond to mouse input and update the transport in a PTY with a ten-second WAV.
- [x] Previous, next, and automatic track completion respond correctly in PTY playback.
- [x] Repeat and shuffle use Qt's shared playback-order engine. Its focused unit tests cover repeat one, album, all, and album shuffle; the PTY test checks mode cycling, unique all-track shuffle, and previous.
- [x] Volume is a visible draggable slider with click, drag, mute, keyboard, and percentage feedback.
- [x] Random Radio can start from an empty queue, stage a round off the input thread, and play one track.
- [x] Radio advances at the end of its visible queue and stages another song.
- [x] Per-row queue and stop-after markers change playback order in PTY playback.
- [x] Queue positions and stop-after markers stay with their tracks across sort, move, and removal.
- [ ] Album metadata arriving during playback has a dedicated terminal regression fixture for shuffle and repeat order.
- [x] Current track, elapsed time, duration, and play/pause state update during PTY playback.
- [x] Music root, volume, equalizer preset, preamp, and output device controls are reachable from menus.
- [ ] Remote server connection, decoder choices, and all Qt/Web playback preferences are reachable.
- [x] Track info, lyrics, and equalizer have terminal modal views.
- [x] The spectrum visualizer reads live audio data, and individual equalizer bands can be edited from Preferences.

## Verification

- [x] Tests exercise text entry, clicks, drags, double clicks, menus, resize, and the playlist database on a PTY.
- [x] A populated 120×40 terminal frame was reviewed against the running Qt and web layouts for toolbar, sidebar, striped playlist, search fields, and transport placement.
- [ ] Cover art and the full Qt/Web information density have suitable terminal representations.

## Cross-frontend feature inventory

The Qt hamburger, playlist and file context menus, playlist header, and the web player were used as references. The items below remain open even if a related basic action above is checked.

| Area | Qt/Web behavior still missing or unverified in TUI |
| --- | --- |
| File tree | Remote server browser, file deletion, blacklist, and complete subsong fixtures. |
| Playlist | Tag editor. |
| Playback | Album metadata changes during shuffle, playback error recovery, and output device test on real hardware. |
| Settings | Server connection, decoder and synthesizer selection, media downloading, and remaining advanced preferences. |
| Views | Cover art, mini player, skins, and richer info inspector interaction. |
