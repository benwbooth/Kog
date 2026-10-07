# 9. Commands and parameters

Commands change a setting at the moment they appear on the track line. They
take no time.

## Level, pan and bend

| Command | Range | Meaning |
| --- | --- | --- |
| `v750` | 0 to 1000 | the channel's level, in thousandths of full scale |
| `p-250` | -1000 to 1000 | the channel's pan, from full left to full right |
| `P+12` | cents | the sounding note's pitch offset from its semitone (chapter 6) |
| `V100` | 0 to 127 | the velocity of following key-ons (chapter 8) |

The level is what the decoder reports for the channel: a programmed volume, a
hardware envelope, or a measured output level, depending on the chip.

## Instruments

`@` sets the channel's instrument:

```
@"Pulse duty 1"
@"ADPCM 065010"
@Piano
```

Instrument names come from the decoder: a MIDI program, a tracker instrument
number, a sample's address in sound memory, or a waveform name. Quotes are
needed only when the name has spaces or MML symbols.

## Chip parameters

Everything else the chip exposes is a parameter, written `name=value`:

```
Period=253 Sweep=0 Envelope=15 ADSR="255 24517" "Volume L/R"=750/288
```

Names are the ones shown in the Channel Inspector's channel details. Each
parameter is written when its value changes. Values are written as the decoder
reports them, with one change: register values reported in hexadecimal are
written in decimal (`Period=0FD` becomes `Period=253`). Values wider than 16
bits, such as memory addresses or several registers packed together, stay in
hexadecimal, which is easier to read for those: `Loop=066120`.

A parameter that changes on nearly every frame is a measurement rather than a
setting, for example the sample position a voice is currently playing. Kog
leaves those out; the Channel Inspector still shows them live (chapter 13).

## Quoting

A name or value can be written bare if it is one word of printable ASCII
without `"`, `=`, `|`, `;` or `\`. Otherwise it is quoted:

| Escape | Meaning |
| --- | --- |
| `\"` | a double quote |
| `\\` | a backslash |
| `\u{b7}` | any other character, by its Unicode number in hex |

So a channel named "SPU 1 · Voice 3" is written `"SPU 1 \u{b7} Voice 3"`.
When reading, Kog also accepts UTF-8 text inside quotes.

## Order within a tick

Commands written before a note at the same moment apply before it sounds.
Kog writes each moment's commands first, then its note:

```
@"Pulse duty 2" v800 Envelope=12 V96 e8
```

Commands written in the middle of a note split it with `^` (chapter 7).
