# 10. Macros

Many chips shape each note over time: a volume envelope fades it, a duty
cycle sequence changes its tone, vibrato wobbles its pitch. Written out, every
note would be followed by a stream of small changes. Macros collect them.

## Defining a macro

```
#MACRO 1 v 0:1000 4:933 14:867 22:800 32:733 41:667
#MACRO 2 Envelope= 0:15 4:14 14:13 22:12 32:11 41:10
#MACRO 5 P 0:0 12:+15 24:0 36:-15 48:0
```

A macro has a number, a target, and steps. Each step is
`ticks after the macro starts:value`.

| Target | Sets |
| --- | --- |
| `v` | the level |
| `p` | the pan |
| `P` | the pitch bend in cents |
| `Name=` | the chip parameter `Name` |

Values for `v`, `p` and `P` are whole numbers; parameter values are words or
quoted strings like any parameter value.

Macros are numbered from 0, in order, with no gaps.

## Starting a macro

`~n` starts macro n at that point on the track. It is usually written just
before a note:

```
A | ~1 ~2 c+4 ~1 ~2 f+4 |
```

Here both notes get the same fade, carried out by two macros: one for the
level and one for the chip's envelope register.

## How Kog makes macros

While recording, any value that changes more than once during a single note
becomes a macro. The value set as the note starts becomes the macro's step at
0. Notes with exactly the same sequence share one macro, so a song with a
handful of envelopes has a handful of macros, however many notes it has.

Changes that happen once per note, or between notes, stay as ordinary
commands on the track.

## Expanding macros

A macro is shorthand: starting macro 1 at tick T means the same as writing
each of its steps at T plus its offset. Programs that read Kog MML can call
`Score::expanded_events` to replace macros with the commands they stand for.
