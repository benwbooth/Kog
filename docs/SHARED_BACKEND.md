# Shared frontend backend contract

All five frontends use `kog-playback-policy::session::Session` as their
application backend. A session owns the queue and row identities, current row,
transport state, modes, selection, sort/filter, playlist drafts, radio buffer,
and pending I/O. Frontends send commands, render snapshots, execute I/O effects,
and return tokened results. They do not maintain a second authoritative queue
or decide which track plays next.

Sessions are independent instances of that same implementation. Sharing the
backend does not synchronize playback across devices.

## Ownership

| Behavior | Backend owner | Frontend responsibility |
| --- | --- | --- |
| Queue edits, duplicate row identity, current row, transport | `session::Session` | Send commands; render read-only projections |
| Next, Previous, EOS, failed opens, shuffle/repeat, queue priority, stop after | `Session` with `PlaybackOrder` | Execute output effects and return tokened events |
| Sorting, filtering, selection and range anchors | `Session` with `sort` and `selection` | Supply metadata and gestures; render visible rows |
| Playlist tabs, drafts, undo, dirty close, load/save acknowledgements | `Session` with `workspace` | Render snapshots; perform storage requests |
| Expansion order, cancellation and deferred Play Now | `Session` | Expand accessible sources and return results, in any completion order |
| Radio buffer, waiting, refill requests and stale replies | `Session` with `radio::RadioBuffer` | Execute radio service requests |
| Radio picks, subsongs, blacklists and persisted rounds | `kog-server::radio::Radio` | Supply root, session ID and request token |
| Browse, search, expansion, tags, saved playlists, stars | `kog-server::api` and `kog-core` | HTTP or in-process calls; present results |
| Decoder choice, archives, subsongs, companion resolution | `kog-audio` | Supply accessible locators and configured decoder assets |
| Speakers, browser audio, AVAudioEngine, Media3, permissions | Platform adapters | Manage output resources and report lifecycle events |

Qt and TUI own typed Rust sessions and project their snapshots into QML or
terminal views. Web compiles the same session to WebAssembly. Swift and Kotlin
`SharedBackendSession` retain opaque Rust state and call `kog_session_json` or
its JNI wrapper. Android's `PolicyPlayer` owns the session in `PlaybackService`,
so notification, headset, and background playback use the same commands.
Media3's timeline is a projection keyed by backend row IDs, and its automatic
shuffle/repeat traversal is disabled.

## Session identity and persistence

- Qt defaults to `qt:default`; TUI defaults to `tui:default`. Set
  `KOG_SESSION_ID` before launching either to select another checkpoint.
- Web defaults to a persistent browser device ID. `?session=NAME` selects an
  independent named session (`web:NAME`) without changing the browser default.
- iOS and Android use persistent per-installation UUIDs. Their session owners
  also accept an explicit session ID for embedded use and integration tests.
- Sessions, radio rounds, UI state, and preferences share the library SQLite
  database: `kog.db` on desktop/server and `library.sqlite` in each mobile
  app's private storage. Checkpoint JSON is a versioned value inside SQLite,
  not another authoritative file. iOS credentials remain in the Keychain.
- Web reads and writes session/UI records through authenticated
  `GET/PUT /api/state/{namespace}/{id}`. Only the server address, login bootstrap
  and device/session identifiers stay in browser storage. Host preferences and
  credentials cannot be read through this API.
- The common versioned checkpoint includes queue locators, row IDs, order,
  selection, filter/sort, volume and playlist drafts. Restore always stops
  output and discards pending I/O; an old callback cannot restart playback.
- The default session imports the previous frontend queue and draft format
  once. Existing JSON/text files, browser storage, SharedPreferences, and
  UserDefaults are migration inputs and are left untouched for recovery. New
  explicitly named sessions start with an empty queue. A corrupt checkpoint
  disables its writer so starting the app cannot replace it with an empty one.

A session ID is a persistence and service namespace, not a live synchronization
protocol. Use distinct IDs for independent players. Two concurrent players
using the same ID do not merge their queues or checkpoints. Saves carry the
revision that was restored. A stale writer gets an explicit conflict (HTTP 409)
and preserves the newer saved record; its current edits remain in memory.
Reloading uses the saved record. Distinct IDs can save independently.

SQLite uses WAL, foreign keys, and a five-second busy timeout. Compound playlist
operations use immediate transactions, including create-with-entries, append,
duplicate, reorder, and checked replacement. Preference changes update only
their own keys. No application-wide reader/writer lock is needed across
processes: SQLite handles writer serialization and readers see committed
snapshots. Browser writes are serialized and coalesced; a lost response retries
the same revision/value before newer edits. Unsaved state blocks automatic
reload and server switching, and warns when leaving the page.

Named HTTP search and radio requests have independent server state. Pausing or
cancelling one search does not affect another session. Radio rounds use separate
SQLite records and do not write the legacy global enabled preference. Requests without a
session ID retain the legacy API behavior. Radio request serials prevent a late
request from reversing a newer root or enabled-state change.

## Asynchronous work

Every I/O request carries a session ID, incarnation and serial. The backend
rejects foreign and stale replies, preserves enqueue order when expansion jobs
finish out of order, and rejects results from a disconnected library scope.
Clear invalidates pending additions; Stop cancels their deferred autoplay while
allowing requested additions to finish. Save acknowledges the captured draft
revision, so edits made during a save remain dirty.

Output tokens belong to a queue row identity, not an index. Moving a playing
row preserves its output, including duplicate locators. Removing it stops
output. Browser elements and mobile callbacks capture the output token; old
end/failure/progress callbacks cannot advance a replacement track. Output
reload effects preserve position and pause state.

## Transport semantics

- Manual Next consumes the explicit next queue before ordinary traversal.
  End of stream honors Repeat One first. Stop After takes precedence at end of
  stream; leaving that row clears its marker.
- Repeat modes cycle Off → One → Album → All. Shuffle modes cycle Off → Albums
  → All. Selecting a non-Off mode disables radio; enabling radio clears both.
- Previous selects the preceding row using the same order everywhere. There is
  no frontend-specific three-second restart rule.
- Failed automatic opens feed `Failed` into the ongoing traversal. Each row is
  attempted at most once in that traversal, including under repeat/shuffle.
- At the end of a non-repeating queue, stop output and retain the current row.
  Removing the current row stops output and clears the current selection.
- Moves/removals provide an explicit old-to-new map, including duplicate rows.
  A sorted view supplies a complete playback sequence; filtering hides rows
  without deleting them from the playback queue.
- Enabling, reshuffling, or changing radio scope does not autoplay. Next or end
  of stream can wait for a refill. Stop, direct activation, or cancellation
  clears that pending start. Responses from older generations are ignored.
- Each radio pick contributes one song, including a selected subsong from a
  multi-song file. The ready buffer is separate from visible playback history.

## Verification

`crates/kog-playback-policy/tests/contract.rs` asserts expected transport,
failure, sorted traversal, duplicate remapping, mobile-wire, radio, sorting,
and filtering outcomes. Module tests cover album grouping and radio lifecycle.
`kog-server::radio_client` tests the real scheduling adapter and NSF decoding;
its opt-in `KOG_RADIO_TEST_FILE` test accepts a real multi-song fixture.
`tests/session.rs` checks session ownership, asynchronous cancellation, output
identity and stopped checkpoint restore. Server tests cover independent search
and radio sessions and delayed radio requests.
`kog-server::local_api` exercises the mobile in-process library routes and
database storage.

Run the shared policy suite, native frontend/server tests, the WebAssembly
check, Android Kotlin plus native builds, and the iOS SDK typecheck/build when
changing this boundary. A successful host Rust check does not verify JNI
linking, Swift compilation, or device output. Runtime checks are still needed
for platform callback ordering, interruptions, background controls, permissions,
and audio hardware; platform build success alone is not evidence of complete
runtime parity.

New UI controls must call these backend commands. If a rule is missing, add it
to Rust and its contract tests before wiring a frontend-specific handler.

## Equivalent playlist interactions

Qt (including Classic), terminal, Web, SwiftUI, and Compose dispatch the same
workspace commands. Opening a saved playlist focuses an editor tab and leaves
playback unchanged. Explicit Play Now, Play Next, and Add to Queue copy the
selection, or the whole draft when nothing is selected. Add Play Queue copies
all queue rows; Add Queue Selection copies only selected rows. Favorites uses
the same read-only snapshot. Saving acknowledges a specific revision, and
checked storage writes reject conflicts with another editor.

The backend supplies enabled actions, including loading/saving, pending-close,
read-only, selection, history, and move boundaries. Disabled commands also do
nothing when sent through shortcuts. Selection replaces, toggles, or extends a
range with a stable anchor. Queue filters affect Select All Results and range
order without changing the queue. Activating the current row toggles playback;
activating another row starts it. Touch, keyboard, and mouse gesture recognition
remain native to each frontend.

### UI and application contract checks

`tests/ui-contract/playlist.json` contains expected states and effects for 30
steps, covering activation, queue selection remapping, editor selection, action
availability, edits, history, saving, dirty close, and Favorites. The same
transcript runs through native Rust plus its JSON wire, the production Swift/C
bridge, and the packaged Android JNI bridge and UI snapshot parser. These tests
check the actual language boundaries. `tests/ui-contract/session.json` adds
33 expected application steps across two independent sessions, including reversed
I/O completion, stale callbacks, duplicate rows, edits during saves and stopped
restore. Android instrumentation additionally exercises two real Media3 players
and native decoding through the production output factory.

- Rust: `cargo test -p kog-playback-policy`.
- Swift/C: `bash tests/ui-contract/run-swift.sh` (Swift and Rust required).
- Android: build `assembleDebug` and `assembleDebugAndroidTest`, install both
  APKs, then run `adb shell am instrument -w
  org.kog.player.test/org.kog.player.UiContractInstrumentation`.
- Qt: after a native build, `nix develop --command bash
  tests/playlist-workspace/run-qt.sh` exercises the real controller and both
  editor components, including range shrinking and enabled actions, plus native
  playback, pause, seek, EOS and stopped restore.
- Terminal: `nix develop --command python3
  tests/playlist-workspace/tui-smoke.py` drives the actual app in a private PTY
  and checks stored draft/database outcomes, native playback, EOS and stopped
  restore.
- Browser: build WebAssembly and exercise tabs, action availability, filtered
  selection, save/close, and current-row activation against a private server.

The shared-backend workflow runs policy/Web checks, the Swift bridge transcript
and iOS SDK typecheck, and Android UI/instrumentation compilation. Android JNI
instrumentation and Qt/terminal/browser runtime checks run separately. This
verifies the shared interaction contract; hardware output, operating-system
interruptions, and every native gesture still require platform-specific tests.
