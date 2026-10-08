# 7. Lengths, dots, ties and triplets

## Note values

A length follows a note, rest or hit. It is the fraction of a whole note:

| Length | Value | Ticks at `#TICKS 96` |
| --- | --- | --- |
| `1` | whole note | 384 |
| `2` | half note | 192 |
| `4` | quarter note | 96 |
| `8` | eighth note | 48 |
| `16` | sixteenth note | 24 |
| `32` | 1/32 note | 12 |
| `64` | 1/64 note | 6 |
| `128` | 1/128 note | 3 |

## Triplets

Triplet values divide a whole note by multiples of three:

| Length | Value | Ticks |
| --- | --- | --- |
| `3` | half-note triplet | 128 |
| `6` | quarter-note triplet | 64 |
| `12` | eighth-note triplet | 32 |
| `24` | sixteenth-note triplet | 16 |
| `48` | 1/32-note triplet | 8 |
| `96` | 1/64-note triplet | 4 |

Three `12`s fill one quarter note: `c12 d12 e12`.

Any divisor of the whole note is accepted when reading (`5`, `10`, and so on,
if the result is a whole number of ticks); Kog writes only the values above.

## Dots

A dot adds half of the value; a second dot adds a quarter more:

| Written | Value |
| --- | --- |
| `4.` | quarter plus eighth (144 ticks) |
| `4..` | quarter plus eighth plus sixteenth (168 ticks) |
| `8.` | eighth plus sixteenth (72 ticks) |

## Ties

`^` joins values into one length:

```
c4^16      ; a quarter note plus a sixteenth, one note
c1^1^2     ; two and a half whole notes
```

Kog writes a length as a single value (with dots) when it can, and as a
fraction when it cannot (see below). You can still write ties yourself.

## Fractions

`n/d` is n d-ths of a whole note:

```
c5/16      ; five sixteenths: a quarter plus a sixteenth
c3/8       ; three eighths, the same as c4.
c9/4       ; two and a quarter whole notes
c5/96      ; five sixty-fourth triplets
```

The fraction must come to a whole number of ticks. Kog writes a fraction,
reduced to lowest terms, for any length that is not a single plain, dotted or
triplet value, so a note held for five sixteenths reads `c5/16` rather than
`c4^16`. Ties appear only where a note continues across a bar line or past a
command.

## Exact ticks

`Nt` is a length of exactly N ticks: `c7t`. Kog reads it but writes
fractions instead.

## Default length

A note, rest or hit with no length uses its track's default length, set by
`l` on the `#TRACK` line. Kog picks the length the track uses most:

```
#TRACK A "Pulse 1" channel=0 voice=0 kind=tonal l8
A | c d e f g4 > c4 |      ; c d e f are eighth notes
```

## Continuing across bar lines and commands

A note that lasts past a bar line is split there. The part in the next bar is
written as a continuation: `^` followed by its length.

```
A | … c2 | ^4 r2. |        ; the c lasts a half plus a quarter
```

A continuation also appears where a command changes something in the middle
of a note:

```
c4 v600 ^4                 ; a half note whose level drops halfway
```

A continuation has no note name, does not restart the sound, and belongs to
the note before it. A `^` after a rest continues the rest.
