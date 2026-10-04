# Shared frontend backend contract

The five frontends are adapters over the same Rust rules. Equal queue metadata,
mode settings, random seed, and command history must produce equal decisions.
Sessions are independent: they do not synchronize their queues, random seeds,
or current track across devices.

## Ownership

| Behavior | Backend owner | Frontend responsibility |
| --- | --- | --- |
| Next, Previous, end of stream, failed open | `kog-playback-policy::PlaybackOrder` | Execute `Play`, `Radio`, or `Stop`; report successful starts and failed opens |
| Shuffle, repeat, queue overrides, stop after | `PlaybackOrder` | Send commands; display returned state; remap row identities on edits |
| Natural text, numeric, album/disc/track and star sorting | `kog-playback-policy::sort` | Supply raw metadata; apply returned order |
| Selection, range anchor, activation | `kog-playback-policy::selection` | Translate gestures; retain returned indices and anchor |
| Saved playlist tabs, edits, undo, dirty close, enabled actions | `kog-playback-policy::workspace` | Render snapshots; execute load/save/queue effects |
| Playlist text filtering | `sort::matches_query` | Supply searchable metadata and display matching rows |
| Radio picks, subsongs, blacklists, persisted rounds | `kog-server::radio::Radio` | Supply root and scope |
| Ready radio buffer, waiting, stale replies | `kog-playback-policy::radio::RadioBuffer` | Schedule service requests; honor generation checks |
| Browse, search, expansion, tags, saved playlists, stars | `kog-server::api` and `kog-core` | HTTP or in-process calls; present results |
| Decoder choice, archives, subsongs, companion resolution | `kog-audio` | Supply accessible locators and configured decoder assets |
| Speakers, browser audio, AVAudioEngine, Media3, permissions | Platform adapters | Output and lifecycle callbacks; no independent queue traversal |

Swift and Kotlin's `SharedPlaybackPolicy` classes only serialize commands and
retain opaque Rust state. Android's `PolicyPlayer` lives in `PlaybackService`;
ExoPlayer's automatic shuffle/repeat decisions are disabled. Queue navigation
from the app, notification, headset, and end-of-stream callbacks reaches Rust.

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

### UI contract checks

`tests/ui-contract/playlist.json` contains expected states and effects for 30
steps, covering activation, queue selection remapping, editor selection, action
availability, edits, history, saving, dirty close, and Favorites. The same
transcript runs through native Rust plus its JSON wire, the production Swift/C
bridge, and the packaged Android JNI bridge and UI snapshot parser. These tests
check the actual language boundaries; they are not screenshots or device audio
tests.

- Rust: `cargo test -p kog-playback-policy`.
- Swift/C: `bash tests/ui-contract/run-swift.sh` (Swift and Rust required).
- Android: build `assembleDebug` and `assembleDebugAndroidTest`, install both
  APKs, then run `adb shell am instrument -w
  org.kog.player.test/org.kog.player.UiContractInstrumentation`.
- Qt: after a native build, `nix develop --command bash
  tests/playlist-workspace/run-qt.sh` exercises the real controller and both
  editor components, including range shrinking and enabled actions.
- Terminal: `nix develop --command python3
  tests/playlist-workspace/tui-smoke.py` drives the actual app in a private PTY
  and checks stored draft and database outcomes.
- Browser: build WebAssembly and exercise tabs, action availability, filtered
  selection, save/close, and current-row activation against a private server.

The shared-backend workflow runs policy/Web checks, the Swift bridge transcript
and iOS SDK typecheck, and Android UI/instrumentation compilation. Android JNI
instrumentation and Qt/terminal/browser runtime checks run separately. This
verifies the shared interaction contract; hardware output, operating-system
interruptions, and every native gesture still require platform-specific tests.
