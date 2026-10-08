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

## Chords

Notes that start at the same moment on one track are a chord: the notes
between single quotes, then one length.

```
'ceg'4                     ; C, E and G together, a quarter note
'ceg'                      ; the same, at the default length
'c>c'2                     ; C and the C an octave above, a half note
```

Inside the quotes, `<`, `>` and `o` change the octave and `V` changes the
velocity for the notes after them, exactly as outside, and the change
carries on after the chord. Kog writes a chord's notes from low to high.
Detunes `(±cents)` and legato `&` belong to each note: `'&ce(+5)g'`.

The length after the closing quote is how far the track moves on. A note
whose sound is longer or shorter than that carries its own length inside the
quotes:

```
'c1eg'4 f4 g4 a4           ; C rings for a whole note under E G, F, G, A
'c8eg'4                    ; C is let go after an eighth; E and G last a quarter
```

A tie after a chord, `'ceg'4 ^8`, lengthens the notes written without a
length of their own. Notes with their own length are complete.

A single note written in quotes, such as `'c1'4`, is a note that keeps
ringing while the track moves on after a quarter note.

While a chord plays, it is highlighted until its longest note ends.

