# Generated libsidplayfp sources

`sidtune/sidplayer1.bin` and `sidtune/sidplayer2.bin` are the C byte-array
outputs of libsidplayfp's upstream `xa` build rule for `sidplayer1.a65` and
`sidplayer2.a65`. Cog carries these generated files for its pinned
libsidplayfp build; Kog retains those exact outputs because the Git submodule
contains the assembly inputs but not the generated includes.

They are executable parts of libsidplayfp's real MUS player, not test data or
placeholder implementations. They retain libsidplayfp's GPL-2.0-or-later
terms.
