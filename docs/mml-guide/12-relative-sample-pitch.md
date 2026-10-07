# 12. Relative sample pitch

Sample-playback chips, such as the PlayStation SPU, do not know which note a
sample plays. They only know how fast to play it: at normal speed, twice as
fast (an octave up), and so on. Kog shows such voices with relative keys:
middle C (`o4 c`) is the sample at its normal rate.

That rate says nothing about the note the sample was recorded at. A bass
sample recorded high and played slowly can come out many octaves below middle
C, and a melody played with it would read `o-1 b o-1 a+` even though it is
plainly a bass line in the second octave.

## Transposition

For each instrument on a relative-pitch voice, Kog looks at the notes it
plays and moves them by whole octaves until their middle note is near middle
C. The shift is recorded so that the score still reads back exactly:

```
#TRANSPOSE "ADPCM 065010" +24
#TRANSPOSE "ADPCM 0680E0" +24
```

While a track's instrument (`@`) is one with a `#TRANSPOSE` entry, its notes
are written that many semitones higher than their relative key, and read back
by subtracting it again. Shifts are always whole octaves, so note names stay
the same; only the octave changes.

## What the keys mean

Even after transposition, keys on these voices are relative. Intervals within
one instrument are right, and a melody's shape is right, but the absolute key
is a guess. Two instruments can be shifted by different amounts. When the
music sounds in a different key from the score, this is why.
