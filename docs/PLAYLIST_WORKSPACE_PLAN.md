# Playlist workspace plan

## Interaction model

The pinned **Play Queue** tab is the only list consumed by transport, radio,
shuffle, and repeat. Saved playlists open in separate editor tabs on a single
click (or Enter in the terminal). Reopening a playlist focuses its existing tab.
Opening a playlist and selecting an editor row never starts playback.

Editor actions use consistent labels in every frontend:

| Action | Result |
| --- | --- |
| Play Now | Append the selection to Play Queue and start its first track |
| Play Next | Append the selection and schedule it ahead of normal traversal |
| Add to Queue | Append the selection without changing current playback |
| Add Queue Selection | Copy selected queue rows into the editor |
| Remove / Move Up / Move Down | Edit the saved-playlist draft only |
| Save | Persist the draft; clear the marker only after storage confirms success |
| Undo / Redo | Undo or redo draft edits without affecting playback |
| Close | Close a clean tab; offer Save / Discard / Cancel for a dirty draft |

With no editor selection, playback actions apply to the whole playlist. With a
selection, they apply to those rows in playlist order. Queue additions are
copies: later edits cannot change music already queued. Favorites is a read-only
playlist tab, with favorites still managed using the existing star controls.

## Shared implementation

1. Add a portable Rust workspace state machine beside playback policy. It owns
   tab identity, loading generations, drafts, dirty state, history, save/close
   decisions, selection normalization, and queue action plans.
2. Expose the same workspace through the Swift/Kotlin JSON bridge. Qt, TUI, and
   Web call the Rust type directly. Storage and audio remain existing adapters.
3. Add a tab strip and editor actions to Qt, Web, iOS, Android, and terminal.
   Preserve the existing queue views and transport while editors are active.
4. Connect explicit saves to existing shared playlist replacement APIs. Failed
   or stale saves retain the draft; closing never silently loses edits.
5. Verify the state machine with command-level tests, build every adapter, and
   exercise open/edit/queue/save/close flows on available runtimes.

The implementation must keep playback independent of the active tab. Loading a
large playlist, switching tabs, saving, and editing cannot reset the player or
its current row. Async loads and saves carry generation/revision values so a
late reply cannot overwrite a newer draft or a reopened tab.

## Implemented controls and verification

Qt, terminal, Web, iOS, and Android use `kog-playback-policy::workspace`.
The mobile clients use that same type through the JSON bridge. Each frontend
persists its own open tabs and unsaved drafts. Saved contents remain in the
shared library database; writes check the original contents inside a transaction
and preserve the draft if another client changed the playlist.

The terminal uses Alt+[ / Alt+] to switch tabs, Alt+1 for Play Queue, Ctrl+S to
save, and Ctrl+W to close. In an editor, p/n/a run Play Now / Play Next / Add to
Queue; x selects a row, d removes it, K/J move it, and u/r undo/redo. A copies the
play queue into the draft; C copies its selection. The on-screen strip and help
show these controls. Desktop and mobile editors expose the same commands as
buttons or menu items.

Regression coverage lives in the policy workspace and bridge tests, the database
conflict/rollback test, and `tests/playlist-workspace`. Run the Qt and PTY smoke
checks from `nix develop` after building `kog` and `kog-tui`. Both smoke checks use
private settings and library directories. The shared-backend workflow also
checks WebAssembly, Swift against the iOS SDK, and the Android Kotlin adapter.
