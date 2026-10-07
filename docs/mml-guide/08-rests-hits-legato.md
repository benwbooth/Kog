# 8. Rests, hits and legato

## Rests

`r` is silence for a length: `r4`, `r8.`, `r1^4`. A rest with no length uses
the default length. Two rests in a row simply add up; Kog writes a new `r`
where a command or bar line splits a silence.

The end of a note is where its sound was released. A note that is let go a
little before the next one leaves a short rest, so staccato phrases read as
`c8 r16 d8 r16`.

## Hits

`x` is an unpitched sound: a noise channel, a drum, or a sample with no
known pitch. It takes a length like a note:

```
#TRACK D "2A03 Noise" channel=3 voice=0 kind=noise l16
D | V100 x x V60 x x V100 x8 x8 |
```

Hits take their loudness from `V` and their sound from the track's
instrument and parameters.

## Velocity

`V` sets the key-on velocity, 0 to 127, for the notes and hits that follow on
the track:

```
V127 c4 V64 d4 e4          ; c loud, d and e at half velocity
```

Kog writes `V` only when it changes. Velocity is the strength of the key-on;
for chips without velocity Kog uses the voice's level at that moment.

## Legato

`&` before a note means its pitch changed without a new key-on: the same
sound slides or steps to the next note.

```
c8 &d8 &e8                 ; one sound that steps through three pitches
```

Kog marks legato only for chips that report their key-on events (the
PlayStation SPU, for example). For other chips a pitch change and a new
key-on look the same, so notes are written normally.

## Same-pitch retriggers

When a chip starts the same pitch again, it is a new note: `c8 c8`. When the
pitch merely continues it is one note, possibly continued with `^`.
