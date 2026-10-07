# Kog MML

The full guide, with a chapter for every part of the language, is in
[docs/mml-guide](mml-guide/01-introduction.md) and opens from the **Guide**
button in every MML view. This page is a summary.

Kog MML is one text notation for every chip and sequencer Kog can inspect:
MIDI, trackers, OPL, NES/Game Boy/SNES/PlayStation sound chips, and the rest
of the families in [Channel Inspector](CHANNEL_INSPECTOR.md). It reads like
classic MML (`o4 l8 c d+ > e-4. r ^16`) and adds what is needed to describe
any of those sources.

Open **Channel Inspector → MML score** in Qt, the web player, Android, or iOS,
or press **4** in the terminal inspector. Kog records the song once with the
same decoder and synth settings used for playback. Bars appear while recording
continues. The playing bar follows playback and every piece of each sounding
note is highlighted.

## Exactness

A score is the piano roll recorded from the decoder: per-voice notes, plus the
chip parameters that changed while they played. `kog_inspection::mml::encode`
writes it as text, and `parse` reads that text back into the identical score.
Tests check this for random scores and for a recording from every inspected
decoder family.

Recording is subject to the inspector's limits. State is sampled at the
decoder's frame rate (up to 200 Hz), and key-ons from some decoders arrive tens
of milliseconds late. Notes are placed on the beat as described below, so the
score keeps the music's rhythm rather than every millisecond of the recording. Values that move on most
frames (sample addresses, fine-grained envelope levels, measured levels) are
written at key-on only; their live values stay in the channel inspector. Songs
without a length are recorded for ten minutes.

## File layout

```
#KOG-MML 1
#TITLE "Stage 1"
#SOURCE "Game Music Emu 0.6.5"
#TEMPO 149.660 inferred     ; quarter notes per minute
#TICKS 96                   ; ticks per quarter note
#BAR 4                      ; quarter notes per bar
#LENGTH 241                 ; song length in ticks
#PICKUP 1                   ; extra ticks at the start of bar 1
#TRACK A "2A03 Pulse 1" channel=0 voice=0 kind=tonal l8
#TRACK B "2A03 Pulse 2" channel=1 voice=0 kind=tonal l4
#TUNE A 69:+2 73:+7 78:+2
#PITCH A Period= 69(+2):0FD 73(+7):0C8 78(+2):096
#MACRO 1 v 0:1000 1:933 4:867 6:800
#MACRO 2 Envelope= 0:15 1:14 4:13 6:12
; bars 1-4 0:00.025
A | @"Pulse duty 1" p0 Sweep=0 ~1 ~2 V127 o4 a > c+ f+ c+ < g+ > c+ < a > c+ | < f+ > c+ < f … | … | … |
B | @"Pulse duty 1" p0 Sweep=0 ~1 ~2 V127 o3 f+ > f+ f < f+ a | > c+ f+ d c+ | … | … |
```

Headers come first. `#TEMPO` uses the MIDI tempo when the source has one.
Otherwise Kog finds the rhythmic step that note onsets fall on and follows it
through the song as the tempo drifts, placing each note on the nearest beat,
half, third or quarter of one, and marks the tempo `inferred`. `#TIMING
tick:seconds …` records where the beat drifted from a steady tempo, for
highlighting. `#PICKUP` lengthens the first bar so bar lines fall where the
biggest chords land. Bars only lay out the text: each
line holds four bars of one track, separated by `|`, and the parser checks
that each bar holds exactly `#BAR` quarter notes.

`#TRACK label name key=value … l<length>` declares one track per voice.
Polyphonic channels such as MIDI get one track per simultaneous voice
(`voice=0`, `1`, …). Channel commands are written on voice 0.

## Notes and lengths

| Syntax | Meaning |
| --- | --- |
| `c d e f g a b`, `+` `#` `-` | note and accidentals |
| `o4`, `>`, `<` | octave (MIDI key 60 is `o4 c`); `>` and `<` move one octave |
| `1 2 4 8 16 32 64 128` | whole, half, quarter … 1/128 note |
| `3 6 12 24 48 96` | triplets: `12` is a triplet eighth |
| `4.`, `8..` | dotted and double-dotted lengths |
| `4^16` | tie: a quarter note plus a sixteenth |
| `l8` | default length (on `#TRACK`), used when a note has none |
| `c(+37)` | note detuned by +37 cents (only written when it differs from `#TUNE`) |
| `&c` | legato: the pitch changes without a new key-on (chips that report key-ons) |
| `^8` | continue the previous note across a bar line or command |
| `r` | rest |
| `x` | unpitched hit: noise, drums, untuned samples |

## Commands

| Syntax | Meaning |
| --- | --- |
| `V100` | key-on velocity, 0–127 |
| `v750`, `p-250` | channel level and pan in thousandths |
| `P+12` | pitch offset in cents from the sounding note's semitone |
| `@"Duty 25%"`, `@Square` | instrument |
| `Name=value`, `"Volume L/R"=750/288` | chip parameter, named as in the channel inspector; quotes only when needed. Register values are decimal; addresses and packed registers wider than 16 bits stay hex |
| `~3` | start macro 3 at this tick |
| `\|` | bar line |
| `;` | comment to the end of the line |

A command takes effect at the tick where it appears. Commands inside a note
split it into `^` pieces, so a note can carry parameter changes while held.

## Tuning tables

Chips often play each pitch a few cents off equal temperament, because their
period registers only hold whole numbers. Each track lists every key's usual
detune once:

```
#TUNE A 69:+2 73:+7
```

Each entry is `MIDI key:cents`. A note uses its key's detune unless it is
written explicitly, as in `a(+0)` or `a(-12)`.

## Pitch tables

Registers that only follow the pitch, such as a period or frequency register,
are listed once per track instead of before every note:

```
#PITCH A Period= 69(+2):0FD 73(+7):0C8
```

Each entry is `MIDI key(cents):value`. A note sets the register to its entry.

## Macros

A value that steps more than once while a note sounds, such as a volume
envelope, a duty sequence, or vibrato, becomes a macro:

```
#MACRO 1 v 0:1000 1:933 4:867 6:800
A | ~1 c+(+7)4 |
```

Each step is `ticks after the macro starts:value`. The target is `v`, `p`,
`P`, or a chip parameter `Name=`. Notes with the same automation share one
macro. `Score::expanded_events` replaces macros with the commands they stand
for.

## Relative sample pitch

PlayStation and similar sample chips report playback rates, not musical keys,
so a sample recorded at a low rate can sit several octaves below the music.
Such instruments are moved by whole octaves until their middle note is near
middle C, and the shift is recorded so the score still reads back exactly:

```
#TRANSPOSE "ADPCM 065010" +48
```
