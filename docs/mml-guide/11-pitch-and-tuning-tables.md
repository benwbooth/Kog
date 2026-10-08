# 11. Pitch and tuning tables

Two kinds of information repeat with the pitch rather than with time. Kog
lists them once per track instead of at every note.

## Tuning tables: `#TUNE`

Many chips cannot play exactly in tune. The NES makes its notes from whole
number period registers, so its A is 2 cents sharp and its C sharp 7 cents
sharp, every time. Writing `a(+2) c+(+7)` at every note would bury the melody,
so each track lists its keys' usual detune:

```
#TUNE A 61:+2 64:+2 69:+2 73:+7 75:+7
```

Each entry is `MIDI key:cents`. On that track:

- `a` (key 69 in octave 4) means A, 2 cents sharp.
- `a(+0)` means A exactly in tune.
- `a(-12)` means A, 12 cents flat.
- A key that is not listed has a detune of 0.

Kog lists, for each key, the detune that key has most often. Notes that differ
from it are written with brackets.

## Pitch tables: `#PITCH`

Some registers simply follow the pitch, such as the NES period register or a
frequency number. Kog moves them into a table:

```
#PITCH A Period= 61(+2):402 69(+2):253 73(+7):200 78(+2):150
```

Each entry is `MIDI key(cents):value`, where the cents part is left out when
zero. On that track, every note sets the register to the value listed for its
pitch, and so does every bend (`P`) while a note sounds: a bend to `P+20` on
a `c` sets the value listed for C plus 20 cents. A note whose pitch is not
listed leaves the register alone.

A setting written at the same moment as a note or bend overrides the table
for that note: `F-number=644 c` sets 644 even if the table lists another
value for C. Kog writes such settings only where the recording differs from
the table, so the table and the exceptions together give every register
value exactly.

Kog makes a table when a register follows the pitch: at least four in five
notes and bends of each pitch have the same value, and the register either
takes about as many values as there are pitches (a frequency) or rises or
falls with the pitch (an octave block, a period). A setting that changes
together with other instrument settings is an instrument setting that only
happens to line up with the notes, and gets no table.
