# 15. Grammar

This chapter defines Kog MML precisely. Spaces and tabs separate tokens and
may appear between any two tokens; inside a token they are not allowed. A `;`
outside a quoted string starts a comment to the end of the line.

```
file        = { line } ;
line        = header | comment | music | blank ;
comment     = ";" text ;

header      = "#KOG-MML" "1"
            | "#TITLE" string
            | "#SOURCE" string
            | "#TEMPO" integer "." digit digit digit [ "inferred" ]
            | "#TICKS" integer
            | "#BAR" integer
            | "#LENGTH" integer
            | "#PICKUP" integer
            | "#TIMING" { integer ":" integer "." digit*6 }
            | "#TRANSPOSE" word signed
            | "#TRACK" label word { property } "l" length
            | "#TUNE" label { integer ":" signed }
            | "#PITCH" label word "=" { signed [ cents ] ":" word }
            | "#MACRO" integer target { integer ":" word } ;

property    = ( "channel" | "voice" | "kind" ) "=" word ;
target      = "v" | "p" | "P" | word "=" ;

music       = label { item } ;
item        = "|"
            | note | chord | rest | hit | tie
            | "o" signed | ">" | "<"
            | "V" integer | "v" signed | "p" signed | "P" signed
            | "@" word
            | "~" integer
            | word "=" word ;

note        = [ "&" ] letter { accidental } [ cents ] [ length ] ;
chord       = "'" { chordpart } "'" [ length ] ;
chordpart   = note | "o" signed | ">" | "<" | "V" integer ;
letter      = "a" | "b" | "c" | "d" | "e" | "f" | "g" ;
accidental  = "+" | "#" | "-" ;
cents       = "(" signed ")" ;
rest        = "r" [ length ] ;
hit         = "x" [ length ] ;
tie         = "^" [ length ] ;

length      = value { "^" value } ;
value       = integer "t"
            | integer { "." } ;

word        = bare | string ;
bare        = 1*( printable ASCII except space " = | ; \ ) ;
string      = '"' { character | "\\\"" | "\\\\" | "\\u{" hex "}" } '"' ;
label       = 1*( "A" .. "Z" ) ;
signed      = [ "+" | "-" ] integer ;
```

## Rules beyond the grammar

- `#KOG-MML 1` must be present. All headers come before the first music line.
- `#TICKS` and `#BAR` must come before any `#TRACK`. `#TUNE` and `#PITCH`
  must come after the `#TRACK` they name.
- `#MACRO` numbers start at 0 and increase by one.
- A length `n` must divide the whole note (`4 × #TICKS`) evenly, and each dot
  must add a whole number of ticks.
- A track's notes, chords, hits, rests and ties must add up to `#LENGTH`
  (a chord counts its length after the closing quote).
- A chord holds at least one note and no spaces.
- On each line, the first `|` must fall on a bar boundary and every later `|`
  must close one bar exactly. The first bar is `#PICKUP` ticks longer than
  the rest; the last bar ends at `#LENGTH`.
- `~n` must name a defined macro.
- A macro step's value must be a whole number for `v`, `p` and `P`.
- `V` values are clamped to 0–127.

## State carried along a track

| State | Starts at | Changed by |
| --- | --- | --- |
| octave | 4 | `o`, `<`, `>` |
| velocity | 0 | `V` |
| default length | the `l` of `#TRACK` | (fixed) |
| instrument for `#TRANSPOSE` | none | `@` |

The state carries across bar lines and from one line of a track to the next.

## How the reader builds events

| Text | Event at the current tick | Time advances by |
| --- | --- | --- |
| note | a note: key (minus any `#TRANSPOSE` shift), detune (bracket, else `#TUNE`, else 0), current velocity, legato flag, length | its length |
| chord | a note for each note inside, all at the current tick; each lasts its own length, or the chord's length if it has none | the chord's length |
| `x` | a hit with the current velocity | its length |
| `r` | nothing | its length |
| `^` | adds its length to the preceding note or hit, or to the preceding chord's notes that have no length of their own (or continues a rest) | its length |
| `v`, `p`, `P`, `@`, `name=value`, `~n` | that command | 0 |
| `o`, `<`, `>`, `V` | nothing (changes state only) | 0 |
| `\|` | nothing (checks bar boundaries) | 0 |

## A complete small file

This file is complete and valid: two bars of a melody over a bass line, with
an envelope macro and a noise hit.

```
#KOG-MML 1
#TITLE "Example"
#SOURCE "Hand written"
#TEMPO 120.000
#TICKS 96
#BAR 4
#LENGTH 768
#TRACK A "Lead" channel=0 voice=0 kind=tonal l8
#TRACK B "Bass" channel=1 voice=0 kind=tonal l4
#TRACK C "Noise" channel=2 voice=0 kind=noise l4
#TUNE A 72:+5
#MACRO 0 v 0:1000 48:800 96:600
; bars 1-2 0:00.000
A | @Square V100 ~0 o4 c d e f g4 > c4 | c(+0)12 < b12 a12 g2. |
B | @Triangle V90 o2 c c g g | c1 |
C | V80 r4 x r x | r4 x8 x8 r2 |
```

The `> c4` at the end of bar 1 is key 72, so the `#TUNE` entry makes it 5
cents sharp. In bar 2 the same C is written `c(+0)`, which plays it in tune.
The first C of bar 1 is key 60, which has no entry and so is in tune.
