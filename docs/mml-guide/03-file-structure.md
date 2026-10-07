# 3. The shape of a file

A Kog MML file has two parts: a header, then the music in blocks of bars.

```
#KOG-MML 1
#TITLE "Stage 1"
#SOURCE "Game Music Emu 0.6.5"
#TEMPO 150.223 inferred
#TICKS 96 ; per quarter note
#BAR 4 ; quarter notes
#LENGTH 9600 ; ticks
#TRACK A "2A03 Pulse 1" channel=0 voice=0 kind=tonal l8
#TRACK B "2A03 Pulse 2" channel=1 voice=0 kind=tonal l4
#TUNE A 69:+2 73:+7
#PITCH A Period= 69(+2):253 73(+7):200
#MACRO 0 v 0:1000 4:933 14:867
; bars 1-4 0:00.025
A | @"Pulse duty 1" p0 o4 a > c+ f+ c+ | < g+ > c+ < a > c+ | … | … |
B | @"Pulse duty 1" p0 o3 f+ > f+ f < f+ | a > c+ f+ d | … | … |
; bars 5-8 0:06.792
A | …
```

## Lines

The file is plain text, one statement per line. Kog writes ASCII only: any
other character in a name is written as an escape (chapter 9).

- A line starting with `#` is a header.
- A line starting with `;` is a comment. Kog writes one before each block of
  bars, giving the bar numbers and the time the block starts.
- A line starting with a track label (`A`, `B`, … `Z`, `AA`, …) holds music
  for that track.
- Blank lines are ignored.

A `;` anywhere else starts a comment that runs to the end of the line.

## Header first

All headers come before the first track line. A header after the music has
started is an error. Chapter 4 describes each header.

## Blocks of bars

The music is written in blocks. Each block starts with a comment such as
`; bars 5-8 0:06.792`, followed by one line for every track. Each line holds
the same bars for its track, separated by bar lines (`|`). With the default
setting a block covers four bars; chapter 2 shows how to change it.

Reading a track means reading its lines in order, block after block. The
octave, default length and velocity carry over from one line to the next.

Bar lines are checked: the first `|` on a line must fall on a bar boundary, and
each later `|` must close a bar of exactly the length set by `#BAR` (the first
bar may be longer; see `#PICKUP`). A note that lasts past a bar line is
continued with `^` in the next bar (chapter 7).
