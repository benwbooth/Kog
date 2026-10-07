# Kog MML

Kog MML is one text notation for every chip and sequencer Kog can inspect:
MIDI, trackers, OPL, NES/Game Boy/SNES/PlayStation sound chips, and the rest
of the families in [Channel Inspector](CHANNEL_INSPECTOR.md). It follows
classic MML (`o4 l8 c d+ e-4. r`) and adds what is needed to describe any of
those sources exactly.

Open **Channel Inspector → MML score** in Qt or the web player, or press **4**
in the terminal inspector. Kog records the song once with the same decoder and
synth settings used for playback. Bars appear while recording continues. The
playing bar follows playback and every piece of each sounding note is
highlighted.

## Exactness

A score is the piano roll recorded from the decoder: per-voice notes on an
integer tick grid, plus the chip parameters that changed while they played.
`kog_inspection::mml::encode` writes it as text, and `parse` reads that text
back into the identical score. Tests check this for random scores and for a
recording from every inspected decoder family.

Recording is subject to the inspector's limits. State is sampled at the
decoder's frame rate (up to 200 Hz), so changes shorter than one frame are not
seen. Values that move on most frames (sample addresses, fine-grained
envelope levels, measured levels) are written at key-on only; their live
values stay in the channel inspector. Songs without a length are recorded for
ten minutes.

## File layout

```
#KOG-MML 1
#TITLE "Stage 1"
#SOURCE "Game Music Emu 0.6.5"
#TIMEBASE 528000 2646      ; one tick is 2646 / 528000 seconds
#METER 80 320 inferred      ; ticks per quarter note, ticks per bar
#LENGTH 601                 ; song length in ticks
#TRACK A 0 0 "tonal" "2A03 Pulse 1" l8
#TRACK B 1 0 "tonal" "2A03 Pulse 2" l4
#MACRO 0 v 3:"933" 10:"867" 16:"800"
#MACRO 1 {"Envelope"} 3:"14" 10:"13" 16:"12"
; bar 1 0:00.000
A | @"Pulse duty 1" v1000 {"Period"="0FD"} ~0 ~1 V127 a(+2)%35 > c+(+7) |
B | r2 < f+(+2)2 |
```

Headers come first. `#TIMEBASE` scales the decoder's frame step to a whole
number of ticks. `#METER` uses the MIDI tempo and time signature when the
source has them; otherwise the beat is estimated from note onsets and marked
`inferred`. Bars only lay out the text: every event has an absolute tick, and
the parser checks that each bar holds exactly `#METER`'s bar length.

`#TRACK label channel voice "kind" "name" l<length>` declares one track per
voice. Polyphonic channels such as MIDI get one track per simultaneous voice
(`voice` 0, 1, …). Channel commands are written on voice 0.

## Commands

| Syntax | Meaning |
| --- | --- |
| `c d e f g a b`, `+` `#` `-` | note and accidentals |
| `o4`, `>`, `<` | octave (MIDI key 60 is `o4 c`) |
| `4`, `8.`, `%n` | length: standard note values against `#METER`'s quarter, or exactly `n` ticks |
| `c(+37)` | note detuned by +37 cents at its onset |
| `&c` | legato: the pitch changes without a new key-on (chips that report key-ons) |
| `^8` | continue the previous note or rest |
| `r` | rest |
| `x` | unpitched hit: noise, drums, untuned samples |
| `V100` | key-on velocity, 0–127 |
| `v750`, `p-250` | channel level and pan in thousandths |
| `P+12` | pitch offset in cents from the sounding note's semitone |
| `@"Duty 25%"` | instrument |
| `{"Name"="value" …}` | chip-specific parameters, named as in the channel inspector |
| `~3` | start macro 3 at this tick |
| `\|` | bar line |
| `;` | comment to the end of the line |

A command takes effect at the tick where it appears. Commands inside a note
split it into `^` pieces, so a note can carry parameter changes while held.

## Macros

A value that steps more than once while a note sounds, such as a volume
envelope, a duty sequence, or vibrato, becomes a macro:

```
#MACRO 2 v 1:"1000" 7:"933" 14:"867" 21:"800"
A | ~2 c+(+7)4 |
```

Each step is `ticks after the macro starts:"value"`. The target is `v`, `p`,
`P`, or a chip parameter `{"Name"}`. Notes with the same automation share one
macro. `Score::expanded_events` replaces macros with the commands they stand
for.
