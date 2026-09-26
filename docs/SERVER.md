# Streaming server and remote clients

Kog can serve its library to other devices. The machine holding the music runs
the server; every client is its own player. That matters: each listener gets
their own independent stream, so two people can play different tracks at the
same time, and neither disturbs the desktop session's own playback.

## Turning it on

**Edit → Preferences → Server**:

- **Enable the API server** — the server stays off, and bound to loopback, until
  you ask for it.
- **Address / Port** — the default is `127.0.0.1:8420`. Use `0.0.0.0` to accept
  connections from other devices; Kog refuses a non-loopback bind without
  authentication, so a misconfiguration cannot expose your library.
- **Authentication** — an API token, a username and password, or nothing
  (accepted only on loopback). Tokens are generated in the preferences pane and
  can be copied to the clipboard. Passwords are stored as salted, iterated
  hashes, never in clear text, and every comparison is constant-time.
- **HTTPS** — off, a self-signed certificate (Kog reuses the same identity, so
  a trusted fingerprint keeps working), or your own PEM certificate and key.
  Imported pairs are validated and copied into Kog's TLS directory with `0600`
  permissions.
- **Stream format** — AAC, Opus, or FLAC. AAC plays everywhere; Opus is smaller
  but not supported by every iOS browser; FLAC is lossless and best on a home
  network. The server reports what it supports, so clients do not hardcode it.
- **Stream cache (MB)** — encoded audio is cached on disk. A track that has been
  streamed once is served from the cache with HTTP range requests, so seeking
  and replaying are immediate.

Press **Start Server**, then **Copy Address** to get a URL a phone or another
machine can open.

## The web player

Open the server's address in any browser. The web player is built into the
server binary, so it can never version-skew from the API it talks to.

It mirrors the desktop layout where that makes sense — library browsing with
breadcrumbs, playlists, favorites, and a transport bar — and collapses to a
bottom tab bar and a fixed transport on phones. Sign in with the same token or
username and password; the server address and token are remembered in the
browser's local storage.

## The desktop app as a client

**☰ → Connect to Server…** opens a remote browser: enter the server address and
credentials, pick a stream format, then browse and click tracks to queue them.
Remote tracks are queued without interrupting what is playing, and playback uses
the same HTTP path the app already uses for network streams.

## API

All endpoints are JSON over HTTP(S) and require the configured authentication
when one is set.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/health` | Liveness; never requires auth |
| `GET` | `/api/version` | Server version; used to verify a connection |
| `GET` | `/api/codecs` | Advertised stream formats, bitrates, and cache limits |
| `GET` | `/api/library?path=` | Directories and files below a path |
| `GET` | `/api/library/search?q=&limit=` | Search the library by file name |
| `GET` | `/api/playlists` | Playlists and their entry counts |
| `GET` | `/api/playlists/{id}` | A playlist's entries |
| `POST` | `/api/playlists` | Create a playlist |
| `POST` | `/api/playlists/{id}/entries` | Append entries |
| `DELETE` | `/api/playlists/{id}` | Delete a playlist |
| `GET` | `/api/stars` | Starred entries |
| `POST` | `/api/stars` | Star or unstar an entry |
| `GET` | `/api/stream?kind=&path=&entry=&fragment=&codec=` | One track, transcoded |

`/api/stream` supports `Range` requests and returns `206` for cache hits, so a
player can seek. On a cache miss the encode starts immediately and is teed into
the cache; a client disconnect does not abort it, so the next listener gets the
cached copy.

Entries are addressed with Kog's own locator scheme, so stars, playlists and
streams all agree on identity:

- `path` — a local file or archive
- `outer::member` — a member inside an archive
- `#fragment` — a cue track or a multi-song subsong

## How it is built

- `kog-server` (Rust, axum) — API, authentication, TLS, the encoded-stream
  cache, and the encoder plumbing.
- `kog-audio::streaming` — pulls decoded audio out of the engine through
  rodio's mixer as uniform 48 kHz stereo f32, rather than refactoring ~25
  decoders into a pull API.
- Linked FFmpeg libraries — encode AAC, Ogg Opus, and FLAC streams in-process.
  The packages bundle the required shared libraries without an `ffmpeg`
  executable.
- `crates/kog-web` (Leptos) — the browser client, built with
  `crates/kog-web/build.sh` and embedded into the server.
