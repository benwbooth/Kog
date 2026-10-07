# 6. Notes, octaves and pitch

## Note names

A note is a letter from `a` to `g`, optionally followed by accidentals and a
length:

| Written | Meaning |
| --- | --- |
| `c` | C |
| `c+` or `c#` | C sharp |
| `d-` | D flat (the same key as C sharp) |
| `c++` | C double sharp (the same key as D) |

Kog writes sharps with `+`; it accepts `#` and `-` when reading. Accidentals
can be repeated, and each moves the note one semitone.

## Octaves

Each track has a current octave. A note is played in the current octave.

| Command | Meaning |
| --- | --- |
| `o4` | set the octave to 4 |
| `>` | one octave up |
| `<` | one octave down |

Octaves count like MIDI: `o4 c` is middle C (MIDI key 60), `o4 a` is the A at
440 Hz (key 69), `o-1 c` is MIDI key 0. Octaves may be negative.

Every track starts in octave 4. Kog writes `o` for the first note of a track
and for jumps of more than two octaves, and `<`, `>`, `<<`, `>>` for smaller
moves, so melodies read the way they move.

Octave changes are commands in their own right. They do not take time and can
stand anywhere, including in front of a note: `> c` and `>c` are the same.

## Detune

Notes are written as the nearest semitone, with an optional detune in cents
(hundredths of a semitone) in brackets:

```
a(+12)    ; A, 12 cents sharp
a(-50)    ; A, half a semitone flat
a(+0)     ; A, exactly in tune
```

When a note has no brackets, its detune is the one listed for its key in the
track's `#TUNE` table, or 0 if the key is not listed (chapter 11). Kog writes
brackets only when a note's detune differs from its key's usual one.

Detune is the pitch at the moment the note starts. If the pitch moves while
the note is held, that is a bend.

## Bends

`P` sets the sounding note's pitch offset in cents from its semitone:

```
c4 P+20 ^8 P+40 ^8 P0 ^4   ; a slide up and back
```

Bends apply to the note that is sounding on the track. A bend that keeps
changing during a note, such as vibrato, is usually written as a macro
(chapter 10).

A slide that crosses into the next semitone ends the note and starts the next
one. With chips that report key-ons (chapter 8), the new note is marked
legato with `&`.
