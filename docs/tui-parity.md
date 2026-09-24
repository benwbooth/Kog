# Terminal UI parity checklist

This is the feature inventory for the terminal frontend, checked against `qml/Main.qml` and `crates/kog-web/src/lib.rs`. A checked item has been exercised in the isolated PTY test at the terminal sizes where the control is visible. Unchecked items still need implementation, verification, or both; the TUI should not be described as fully equivalent while they remain.

Run `nix develop --command cargo build -p kog --bin kog`, then `uv run --python 3.13 --with pyte --with mutagen scripts/tui_pty_check.py`, `uv run --python 3.13 --with pyte scripts/tui_keyboard_check.py`, `uv run --python 3.13 --with pyte scripts/tui_size_columns_check.py`, `uv run --python 3.13 --with pyte scripts/tui_transport_check.py`, `nix develop --command uv run --with pyte scripts/tui_server_interop_check.py`, and `uv run --with pyte scripts/tui_server_controls_check.py`. Set `KOG_TUI_TEST_BINARY` to an absolute `kog-tui` path to test the standalone executable instead. The tests create their own music trees, archives, settings directories, and SQLite databases. They do not write to the user's library or playlists.

The provider smoke test is opt-in because it contacts public services: `nix develop --command cargo test -p kog-terminal --lib live_super_mario_galaxy_cover -- --ignored`.

For range selection in any pane, click the first row, press `v`, then click the last row. Esc cancels the pending range. Alt+click also extends a range when the terminal forwards that modifier; Ctrl+click toggles individual rows.

## Keyboard-only controls

Press `?` inside the TUI for the scrollable shortcut guide. Every TUI action exposed by a mouse control is also available through a key or menu:

| Action | Keyboard route |
| --- | --- |
| Change pane and navigate | `Tab` / `Shift+Tab`; arrows, Page Up/Down, Home/End |
| Open menus and item actions | `m` or F10 for the application menu; `M`, Shift+F10, or Menu key for the focused item; every menu item shows an `Alt+letter` shortcut, usable while that menu is open. Main menu shortcuts also work directly from the TUI. |
| Select several rows | Shift+Up/Down, or `v` then Up/Down, selects a range; `J`/`K` moves the cursor without changing selection, then `x` toggles its row; Ctrl+A selects all |
| Show or hide the sidebar and its sections | `t` for sidebar; `z` for the focused Files or Playlists section; Ctrl+Left/Right or `{`/`}` for sidebar width |
| Search and refresh | `/` searches files; `F` searches the playlist; Ctrl+R or `u` refreshes the focused Files or Playlists pane |
| Edit playlist columns | `H` focuses the header; Left/Right chooses a column; Enter sorts; `+`/`-` changes width; Ctrl+Left/Right or `[`/`]` reorders; `a` auto fits; `v` toggles visibility; `M` opens all column actions; Esc leaves the header |
| Playback controls | Space play/pause, `s` stop, `<`/`>` previous/next, `G` seek to an exact time, `h`/`l` seek by ten seconds with Tracks focused, `+`/`-` volume, `R`/`S` repeat/shuffle |
| Other transport and views | Playback menu for mute and radio; View menu for artwork, info, lyrics, equalizer, and spectrum; `C` enters or leaves compact view |
| Edit a prompt | Ctrl+A select all, Ctrl+W delete word, Ctrl+U clear, Ctrl+Left/Right move by word or path component |

The printable alternatives for range selection, context actions, resizing, reordering, and refreshing work when a terminal does not forward modified keys.

## Window and input

- [x] Terminal resize redraws without losing the active pane.
- [x] Top playlist search edits in its field, filters as typed, clears, and handles cursor keys and paste.
- [x] File search edits in its field, searches as typed, clears, and shows progress.
- [x] Mouse drag resizes the file tree divider and keeps the chosen width on resize.
- [x] Files and Playlists section headers expand and collapse.
- [x] Hamburger opens a structured, keyboard and mouse operable menu with working commands.
- [x] Right click opens context menus for tree items, playlist rows, saved playlists, and columns.
- [x] The same context actions open from the keyboard, including at narrow terminal sizes; `?` shows the shortcut guide.
- [x] Every main, submenu, and context-menu action has a unique displayed Alt shortcut within its menu; PTY and unit tests check dispatch and the shortcut labels.
- [x] Mouse wheel and Home/End navigation keep long tree and playlist selections visible, including at 48 columns.
- [x] Multi selection supports Ctrl+click toggling, Alt+click ranges, `v` then click ranges, Shift+arrow ranges, and deletion of all selected rows. Shift+click also works when the terminal forwards it.
- [x] Keyboard-only noncontiguous selection, section collapse, sidebar resize, column sorting/resizing/reordering, and exact seek are exercised in an isolated PTY without mouse events.
- [x] Path, URL, name, and preference prompts edit in a centered dialog with a visible caret and Ctrl+A replacement.
- [x] Music Folder opens a directory chooser with mouse and keyboard browsing, hidden folders, an editable location, and separate Choose and Cancel actions. Arrow keys select, Enter opens, Backspace or Alt+Up goes to the parent, Ctrl+L edits the location, Ctrl+W deletes the previous word, Alt+Backspace or Ctrl+Backspace removes the previous path component, and Ctrl+O chooses the displayed folder.

## Library and playlists

- [x] Files are displayed in a lazily expandable tree, including nested folders.
- [x] A token-protected remote Kog library can be browsed, searched, queued by folder, and streamed; URL, auth mode, and codec controls are in the Remote Server menu. PTY fixtures check the mock API and a real headless Kog server, including nested archive playback.
- [x] A folder click expands or collapses it; a folder double click queues its contents without starting playback.
- [x] A file double click adds and plays; single click selects.
- [x] Ctrl mouse selection, `v` then click or Alt mouse range selection, Shift arrow range selection, and Ctrl+A work in the tree; Add applies to selected files and folders. Play Now on a folder starts its first loaded track.
- [x] Tree context actions can change and reset the visible root, blacklist selected songs, and move several local files to trash after one confirmation. Delete opens the same confirmation.
- [x] Nested archive folders can be explored and added. Direct adds and folder scans expand multi-song files; a three-song NSF is checked in unit and PTY fixtures.
- [x] Saved playlist single click selects the source; double click adds its tracks to the playing pane.
- [x] Favorites, create, rename, delete, save current pane, and add to a saved playlist work.
- [x] Saved playlists show right-aligned song counts, including Favorites; counts refresh after adding, duplicating, and pruning tracks.
- [x] Queue remove, clear, reorder, sort, star, and search work with the visible selection.
- [x] The current playlist and selected row survive a TUI quit and relaunch in a separate atomic session file; the PTY fixture checks the full queue before and after restart.
- [x] Playlist rows show decoded title, artist, and album without probing on the input thread.
- [x] Default #, Title, Artist, and Album columns have header sorting, mouse resizing, visibility toggles, and auto fit from loaded metadata.
- [x] All 22 Qt columns can be shown, hidden, sorted, resized, reordered, and scrolled horizontally with Left/Right in the playlist, a visible draggable scrollbar, native horizontal wheel, Shift/Ctrl+wheel, or bracket keys. The terminal saves its character-width layout and can import an existing Qt column order.
- [x] File Size uses binary units and File Size (Bytes) shows the exact count; a PTY checks both values against a known WAV without touching the user's library.
- [x] Right click opens row, file, saved playlist, column, and empty playlist menus. An open menu consumes right clicks inside it instead of passing them through to the underlying pane.
- [x] Submenus open beside their selected parent item while parent menus remain visible; mouse selection and Left/Esc navigate the menu stack, including after terminal resize.
- [x] Add File, Add URL, Save Current Playlist, and Add to Saved Playlist write the expected queue and database entries.
- [x] Save Selection As and saved playlist Duplicate write the expected database entries.
- [x] Saved playlist Add, Play, and Replace are available from its context menu.
- [x] Playlist row Show in File Tree focuses the matching local file.
- [x] Saved playlist export writes a portable M3U; prune missing deletes only absent local/archive entries.
- [x] Ctrl toggle and `v` then click, Alt+click, or Shift arrow ranges select saved playlists; Add and Play use the selected set, and confirmed Delete removes the selected saved lists.
- [x] Tree and playlist song/folder blacklist actions persist to the shared database; the menu can view and remove entries.
- [x] Confirmed Move to Trash runs off the input thread, removes local tree items and their current playlist rows, and is exercised with disposable files.
- [x] Track tag editing stages the Qt fields and artwork actions, saves through the shared writer off the input thread, and updates multiple local files. The PTY test checks album and artwork tags and continued playback after saving.

## Playback and settings

- [x] Play, pause, stop, and seek respond to mouse input and update the transport in a PTY with a ten-second WAV.
- [x] Footer and compact-player controls share their painted glyph positions with mouse hit testing; previous/next use the single-character ⏮/⏭ track-skip symbols. A PTY clicks the displayed glyphs, and layout tests cover widths where Radio and volume used to overlap.
- [x] Previous, next, and automatic track completion respond correctly in PTY playback.
- [x] Automatic advance skips a missing or undecodable entry, tries the next track at most once, and stops if none can play. A PTY fixture removes the middle WAV during playback and checks that the third WAV starts.
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
- [x] Read CUE and M3U/PLS folder preferences are saved and applied by folder scans; a focused unit test checks the resulting entries.
- [x] MIDI backend, SoundFont, ROM directory, and MT-32 mapping controls update the live shared decoder settings; the PTY fixture checks menu access, validation, clearing, and persistence.
- [x] SC-55 and MT-32 compressed ROM import uses the shared Qt importer and validator on a worker; shared unit tests check extraction and the PTY fixture rejects incomplete sets without retaining files.
- [x] The API Server menu edits the shared bind address, port, authentication, credentials, HTTPS mode/certificate, codec, and cache settings; it starts and stops the embedded server and manages connected devices. A focused PTY test checks real HTTP and HTTPS requests with token and basic authentication, self-signed and imported PEM certificates, device blocking, and private settings storage.
- [x] Player-relevant Qt preferences, including automatic cover downloads, are reachable. Qt window and tray controls do not apply to a terminal process.
- [x] Track info, lyrics, and equalizer have terminal modal views. The info view includes Qt's technical fields, source and artwork paths, and a playback position that refreshes while open in the PTY fixture.
- [x] Supported Formats opens the shared decoder catalog in a scrollable terminal view; PTY checks its first format group.
- [x] The spectrum visualizer reads live audio data, and individual equalizer bands can be edited from Preferences.

## Verification

- [x] Tests exercise text entry, clicks, drags, double clicks, menus, resize, and the playlist database on a PTY.
- [x] Both stdout and stderr diagnostics are redirected to an owner-only log while a separate terminal descriptor paints the TUI; the PTY fixture checks the live descriptors and log permissions.
- [x] A populated 120×40 terminal frame was reviewed against the running Qt and web layouts for toolbar, sidebar, striped playlist, search fields, and transport placement.
- [x] The footer shows a true-color Unicode cover preview from cached, embedded, nearby, or downloaded art; the track info view shows the artwork path. The PTY fixture checks a cover added through the tag editor, and an opt-in live provider test resolves Super Mario Galaxy.
- [x] Clicking the footer cover or using View → Show Album Cover opens a larger, closable artwork dialog. A PTY fixture checks the enlarged pixels and mouse close action.
- [x] Compact Player hides the library and playlist and groups a larger Unicode cover, metadata, seek, transport, and volume in a mini-player card. A PTY check pauses, resumes, seeks, changes volume, and returns to the playlist.
- [ ] The full Qt/Web information density has suitable terminal representations at every terminal size.

## Cross-frontend feature inventory

The Qt hamburger, playlist and file context menus, playlist header, and the web player were used as references. The items below remain open even if a related basic action above is checked.

| Area | Qt/Web behavior still missing or unverified in TUI |
| --- | --- |
| File tree | Remote multi-song files expand through `/api/expand`; the real server fixture checks a three-song NSF and remote nested archive playback. |
| Saved playlists | Remote track save, reload, and M3U export are checked against the real server. |
| Playback | Terminal timing fixture for late album tags and output device test on real hardware. Missing-track recovery is checked in the PTY. |
| Library settings | Read CUE and M3U/PLS menu controls and folder behavior are checked. |
| Synthesis | SC-55 and MT-32 archive controls reject incomplete fixtures; validating a complete proprietary ROM set needs user supplied files. |
| Server settings | Shared configuration, embedded start/stop, device blocking, token/basic authentication, self-signed HTTPS, and PEM import are checked through real requests. |
| Media | Automatic cover downloads use the Qt provider order and shared match rules; footer, compact-player, and enlarged cover views are checked in a PTY, and an opt-in live test resolves Super Mario Galaxy. |
| Views | Compact Player covers the mini player workflow in the same terminal. Native Winamp skin windows have no terminal equivalent; the track inspector refreshes live. |
