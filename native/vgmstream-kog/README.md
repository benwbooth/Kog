# Shared game-audio codec build

This CMake project builds the pinned vgmstream sources with the same static
MPEG, Vorbis, ATRAC9, CELT 0.6.1/0.11 and Speex dependencies on every native
platform. FFmpeg headers come from `kog-audio`'s existing pkg-config probe;
Rust links the existing FFmpeg libraries. No host tools or codec DLLs are
downloaded during configuration. G.719 is excluded because upstream vgmstream
documents unclear redistribution terms for its reference implementation.

The standalone check uses Kog's actual playback bridge and tests PCM/seek for
the synthetic MSF, Vorbis and AAC fixtures, PCM round trips for both CELT
versions and Speex, and ATRAC9 initialization (not a full ATRAC9 rip test):

```sh
cmake -S native/vgmstream-kog -B /tmp/kog-codecs -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DKOG_CODEC_TESTS=ON \
  -DKOG_FFMPEG_INCLUDE_DIRS="$(pkg-config --variable=includedir libavcodec)"
cmake --build /tmp/kog-codecs --parallel
ctest --test-dir /tmp/kog-codecs --output-on-failure
cargo test --locked -p kog-audio --lib optional_
```

The Rust regressions also check byte-exact archive extraction. To exercise
the mobile archive configuration on a build host, run
`native/build-mobile-archive.sh` with a temporary build directory and prefix,
then compile `tests/native/archive_probe.c` against that prefix's static
libarchive and its `pkg-config --static --libs libarchive` dependencies.
Neither a native build nor these fixtures establishes compatibility with
every game rip; broader coverage is tracked in `docs/FORMAT_PARITY.md`.
