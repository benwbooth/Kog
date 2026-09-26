# iOS / mobile Web feature parity

The native iPhone app uses SwiftUI for presentation and Kog's Rust libraries for
music and library operations. Server and device queues are independent.
Offline browse, archive expansion, metadata, search, playlists, favorites, and
radio call the same API handlers in process; they do not start a server socket.

## Implementation checklist

| Area | Implemented native behavior |
| --- | --- |
| Server playback | Bearer/Basic authentication, live connection checks and retry, AAC/Opus/FLAC, persistent queue, codec and MIDI-synth reload |
| Library | Folder and nested-archive browsing, file/folder addition, persistent tree-root picker, scoped search/radio, search counts/pause/clear, refresh |
| Queue | Tap play/pause, filter, multi-selection and bulk removal/addition, swipe deletion, drag reorder, metadata sorting and original order, details, reveal |
| Transport | Previous/next, seek, stop, shuffle, repeat one/all, radio with an immediate toggle and ten-track background buffer, one volume slider and mute |
| Saved playlists | Create/rename/delete, add/play/replace queue, save queue, duplicate, prune, export, delete confirmation, entry counts and removal |
| Favorites | Server and offline device favorites, saved through shared Rust database operations |
| Local music | Import files/folders, Open in Kog, archive playback, linked Rust/native decoders, offline search/playlists/radio, import SoundFonts and synth ROM directories |
| Downloads | Save server files to the iPhone; archive tracks retain their outer archive so companion banks remain available |
| Visualization | Actual PCM mini waveform; expanded waveform and spectrum, using the same audio output for server streams and local files |
| Details | Title, artist, album, album artist, composer, year, genre, track number, duration, codec, sample rate, depth, bitrate, channels, exact/human file size, readable path |
| Preferences | Server/auth/codec, server/device MIDI synths, notifications, About/licenses, source-specific library controls |
| Phone integration | Background audio, lock-screen metadata/artwork, media commands including stop, interruptions/headphone disconnect, scrollable expanded player |

## Playback changes

- The server's Nuked SC-55 renderer now installs a valid sample callback while
  booting/resetting and restores it after rendering. Previously, playing the
  Darkstalkers archive MIDI crashed the server, appearing as a connection error.
- Core Audio receives noninterleaved Float32 channel planes. Passing the Rust
  interleaved buffer format directly to AVAudioPlayerNode crashed on the phone.
- Both server and local playback now expose decoded PCM for visualization.
  iOS uses URLSession for the network transport, including system routing and
  authentication, and passes a bounded stream to the shared FFmpeg decoder.
  Closing a decoder cancels the network request and wakes blocked reads.
  FFmpeg's direct TCP transport timed out on this phone even while URLSession
  reached the same server; it is not used by native iOS playback.
- Seeking a progressive server stream requests `start_ms` and seeks the original
  shared decoder before encoding. It works before a full-track cache exists.
  Offset encodes have separate cache identities. Local playback seeks directly.
- Concurrent stream encoders use unique temporary files, preventing one request
  from truncating or renaming away another request's audio.
- Pause/stop takes effect immediately even while the decoder is reading from a
  slow network stream; completed reads cannot restart a paused player.

## Verification evidence

- The previously crashing SC-55 MIDI returned HTTP 200 with 744,164 AAC bytes in
  3.54 seconds; server health still returned 200 afterward. The SC-55 reuse
  regression produced nonzero PCM for the initial job and two warm jobs.
- A live search scoped to the Arcade folder completed successfully. A focused
  regression checks rejection of search roots outside the configured library.
- The in-process library regression passed browse, playlist create/read/append,
  favorites, export, replacing a playlist with zero entries, and deletion.
- Swift source type-checks against the iOS 27 SDK, targeting iOS 17 or later.
- Authenticated network PCM decoding, seeking, and end-of-stream passed; the
  HTTP/HLS decoder regression also passed. The uncached server-seek regression
  verifies that audio starts at the requested position and has only the
  remaining duration. Concurrent-stream cache and incremental/default radio
  endpoint regressions passed.
- After the development server restarted, a request for The Seven Apprentices
  SPC at 270 seconds returned HTTP 200 in 0.64 seconds with 244,754 AAC bytes.
  Its ADTS frame count confirms approximately 10 seconds of remaining audio
  from a 280-second track.
- The first updated physical-device run passed offline browse/metadata, search,
  playlists/favorites, radio, Core Audio PCM delivery, seeking, pause and resume.
  Its direct-FFmpeg network check failed, prompting the URLSession transport fix.
- The final signed build installed and the physical iPhone on iOS 27 passed
  the complete diagnostic at 2026-09-26 23:13:02 UTC: browse/metadata, offline
  playlists/favorites/search/radio, native PCM tap, local seek/pause/resume,
  authenticated server playback and server PCM tap after starting at 7.08
  seconds. The app was then reopened normally without diagnostic arguments.
- `KOG_DEVICE_TESTS` and the explicit `--verify-kog-device` launch argument enable
  an isolated, muted device check. It exercises offline metadata/search/radio,
  playlist/favorite persistence, actual Core Audio playback, PCM tap delivery,
  seek, pause/resume, and a current server stream started at an offset. Results
  are written to `Library/Caches/Kog Verification/result.json`. Normal launches
  do not run these checks or replace the user's queue.

## Remaining verification boundaries

Compilation and the implementation checklist do not establish exhaustive touch
or format parity. Physical-device results are recorded separately from build
success. Not every decoder family or synth-asset combination has been exercised
on this iPhone. Network artwork for on-device files remains limited to embedded
or sibling artwork; server tracks use server artwork lookup. Mixed server/device
tracks cannot be saved together to a source-specific playlist: the UI asks the
user to choose tracks from one source.
