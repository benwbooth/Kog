# 16. A worked example

This is the opening of an NES song, recorded by Kog, followed by a line by
line reading. Some macros and table entries are shortened with `…`.

```
#KOG-MML 1
#TITLE "Stage 1"
#SOURCE "Game Music Emu 0.6.5"
#TEMPO 150.223 inferred
#TICKS 96 ; per quarter note
#BAR 4 ; quarter notes
#LENGTH 9600 ; ticks
#TIMING 0:0.025057 48:0.200455 768:3.202261 816:3.392693 …
#TRACK A "2A03 Pulse 1" channel=0 voice=0 kind=tonal l8
#TRACK B "2A03 Pulse 2" channel=1 voice=0 kind=tonal l4
#TUNE A 61:+2 64:+2 65:+2 66:+2 68:+2 69:+2 73:+7 …
#PITCH A Period= 61(+2):402 64(+2):338 65(+2):319 69(+2):253 73(+7):200 …
#TUNE B 45:+2 50:+2 54:+2 57:+2 …
#PITCH B Period= 45(+2):1015 54(+2):603 57(+2):507 …
#MACRO 0 v 0:1000 4:933 14:867 22:800 32:733 41:667
#MACRO 1 Envelope= 0:15 4:14 14:13 22:12 32:11 41:10
; bars 1-4 0:00.025
A | @"Pulse duty 1" p0 Sweep=0 ~0 ~1 V127 o4 a > c+ f+ c+ < g+ > c+ < a > c+ | …
B | @"Pulse duty 1" p0 Sweep=0 ~0 ~1 V127 o3 f+ > f+ f < f+ a | > c+ f+ d c+ | …
```

## The header

- `#TEMPO 150.223 inferred`: the NES file has no tempo of its own; Kog found
  the beat from where notes fall, about 150 quarter notes a minute.
- `#TICKS 96`, `#BAR 4`: 96 ticks to a quarter note, four quarter notes to a
  bar. `#LENGTH 9600` is 25 bars.
- `#TIMING`: the song starts 25 milliseconds in, and the beat drifts a little;
  these points keep highlighting in time.
- Two tracks, one for each NES pulse channel. Track A's notes are mostly
  eighth notes (`l8`), track B's quarter notes (`l4`).
- `#TUNE A …69:+2 73:+7`: on the NES, this channel's A plays 2 cents sharp and
  its C sharp 7 cents sharp. Notes on track A are written without brackets
  and carry those detunes.
- `#PITCH A Period= …`: the NES period register for each pitch. Every note on
  track A sets it; it is listed here once instead of before every note.
- `#MACRO 0` and `#MACRO 1`: the NES volume envelope, as a level (`v`) and as
  the chip's 4-bit envelope register, both stepping down from full as the note
  plays.

## Track A, bar 1

| Text | Meaning |
| --- | --- |
| `@"Pulse duty 1"` | instrument: pulse wave with duty setting 1 |
| `p0` | centre pan |
| `Sweep=0` | the pulse channel's sweep register is off |
| `~0 ~1` | start the two envelope macros with the next note |
| `V127` | full velocity |
| `o4 a` | A in octave 4, an eighth note (the default), 2 cents sharp (`#TUNE`) |
| `> c+` | up an octave: C sharp in octave 5, an eighth note |
| `f+ c+` | F sharp, then C sharp, eighth notes |
| `< g+` | down an octave: G sharp in octave 4 |
| `> c+ < a > c+` | the arpeggio continues |
| `\|` | end of bar 1: eight eighth notes, exactly one bar |

Track B plays the bass line in quarter notes underneath.
