# 17. Questions and limits

## Why does a note have a long tie like `2^6`?

The note's length is not a single note value at the score's tempo. Either
the music really holds it that long (a note held for two thirds of a bar is
`2^6` in 4/4), or the beat Kog found does not match the music in that section,
for example because the tempo changes. Ties never change what is played; they
only show a length that has no single note value.

## Why does a MIDI song have fewer tracks than voices?

Each MIDI channel is one track. Chords and notes that ring on while others
play are written as chords (chapter 6), so a piano part stays on one line
instead of one line per finger.

## Why is the tempo double or half what I expect?

Without a tempo in the file, Kog chooses how many beat steps make a quarter
note. Music written in eighths at 75 BPM and in sixteenths at 150 BPM look the
same in a recording. Kog prefers the choice that writes the most note gaps as
plain values, then a tempo near 125 BPM.

## Why do the bar lines not match the sheet music?

Kog cannot see the time signature of a chip song, and it places bar 1 where
the biggest chords fall. Pick-up notes, tempo changes and songs in 3/4 or 6/8
can therefore have bar lines in different places from the original score. The
notes and their timing are unaffected.

## Why are the notes of a PlayStation song in the wrong key?

Sample voices only report playback rates. See chapter 12.

## Why are some register values hexadecimal?

Values wider than 16 bits are memory addresses or packed registers and stay
hexadecimal (chapter 9). Everything else is decimal.

## Where did `Current=` and other values go?

Values that change on nearly every frame are measurements, not settings, and
are left out of scores. The Channel Inspector shows them live.

## Can I edit the text?

Yes. The text is complete: Kog's reader rebuilds the score from it exactly.
Changing notes, lengths or parameters changes the score; a mistake such as a
bar that is too long is reported with its line number.

## Can Kog play MML?

Not yet. The MML view shows a recording of what Kog played.

## What is lost compared with the song file?

- Events shorter than one frame (a few milliseconds).
- Exact timing: notes are placed on the beat, half, third or quarter beat.
- Measurements such as sample read positions.
- For relative-pitch sample chips, the absolute key.
- Anything the decoder does not report, such as a game's own software mixing.

## Is the score the same on every device?

Yes, given the same decoder and synth settings. Phones and the desktop use the
same recorder; the web player asks the server, which records with the
settings of its stream.
