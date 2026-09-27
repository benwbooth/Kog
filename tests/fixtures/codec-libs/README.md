# Optional codec regression fixtures

All files contain an original 0.5-second, 440 Hz sine wave (44,100 Hz,
mono signed 16-bit PCM, amplitude 12,000). No game audio or third-party music.
`tone.wav` is the byte-for-byte oracle for archive extraction.

- `tone.msf`: MP3 from FFmpeg/libmp3lame (96 kb/s, no ID3/Xing), wrapped in
  a Sony MSF header; exercises vgmstream's MPEG path inside a game container.
- `tone.logg`: FFmpeg/libvorbis, exercises libvorbisfile through vgmstream.
- `tone.m4a`: FFmpeg AAC, exercises vgmstream's FFmpeg integration.
- `lzma.7z`, `lzma2.7z`: 7-Zip LZMA and LZMA2 archives.
- `bzip2.zip`, `lzma.zip`: Python ZIPs using the indicated compression.
- `tone.wav.xz`, `tone.wav.bz2`: Python standard-library compression.
- `zstd.tar`, `lz4.tar`: compressed tar archives (libarchive and LZ4).

The playback checks require finite, nonzero PCM both before and after seeking.
Archive checks compare every extracted byte, rather than just listing entries.
These small fixtures cover the newly enabled paths; they are not a claim of
compatibility with every game rip.
