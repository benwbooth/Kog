# 4. Header reference

Headers describe the whole song. Each takes one line and starts with `#`
followed by its name. Values that may contain spaces are quoted (chapter 9).

## `#KOG-MML 1`

Required, and conventionally first. It names the language and its version.
Kog reads version 1.

## `#TITLE "text"`

The song's title, quoted.

## `#SOURCE "text"`

The decoder that played the song, for example `"Game Music Emu 0.6.5"`,
`"Psf"`, or `"HivelyTracker 1.9"`.

## `#TEMPO bpm[ inferred]`

Quarter notes per minute, with exactly three decimal places: `#TEMPO 120.000`.
When the song states its tempo (MIDI files do) Kog uses it. Otherwise Kog
works the tempo out from where the notes fall and adds `inferred`. When
`#TIMING` is present it gives the real times, and `#TEMPO` is the song's
average.

## `#TICKS n`

How many ticks make a quarter note. Recorded scores use 96, which makes every
note value from a whole note down to a 1/128 note, and every triplet down to
1/96, a whole number of ticks. All times in the file are counted in ticks from
the start of the song.

## `#BAR n`

Quarter notes per bar: `#BAR 4` is 4/4 time, `#BAR 3` is 3/4. Kog uses the
song's time signature when it has one (MIDI), otherwise 4.

## `#LENGTH n`

The song's length in ticks. Every track must add up to exactly this length.

## `#PICKUP n`

Extra ticks at the start of the first bar, so that later bar lines fall where
the strongest beats are. With `#PICKUP 32`, bar 1 is 32 ticks longer than the
others. Omitted when zero.

## `#TIMING tick:seconds …`

Times for songs whose tempo drifts. Each entry pins a tick to a time in
seconds with six decimals: `#TIMING 0:0.025057 48:0.200455 768:3.202261`.
Between two entries time runs evenly; after the last, at `#TEMPO`. Long maps
continue on further `#TIMING` lines. Chapter 13 explains where they come from.
Timing only affects when notes sound, not how they are written: a quarter note
is 96 ticks wherever it is.

## `#TRANSPOSE instrument ±semitones`

An octave shift for one instrument whose pitch is only relative (chapter 12):
`#TRANSPOSE "ADPCM 065010" +24`.

## `#TRACK label name key=value … l<length>`

Declares a track. The label is one or more capital letters and is used at the
start of the track's lines. The name is the voice's name from the Channel
Inspector, quoted if it has spaces.

| Property | Meaning |
| --- | --- |
| `channel=n` | the decoder's channel number |
| `voice=n` | which voice of that channel (0 for most; see chapter 5) |
| `kind=k` | `tonal`, `noise`, `percussion`, `sample` or `mixed` |
| `l<length>` | the default length for notes and rests written without one (required) |

Example: `#TRACK C "SPU 1 · Voice 3" channel=2 voice=0 kind=sample l4.`

## `#TUNE label key:cents …`

The usual detune of each key on a track (chapter 11).

## `#PITCH label name= key(cents):value …`

A register that follows the pitch, listed once per pitch (chapter 11).

## `#MACRO n target offset:value …`

A sequence of parameter changes that starts with a note (chapter 10). Macros
are numbered from 0 in order.
