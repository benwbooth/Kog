# 5. Tracks and voices

Each track is one channel: whatever the decoder reports as a channel, such as
a pulse wave on the NES, an operator pair on an OPL chip, one of the
PlayStation's 24 sample voices, or a MIDI channel. Most chip channels play one
note at a time, so their tracks are plain melodies.

## Polyphonic channels and chords

Some channels play several notes at once. A MIDI channel can play chords and
let notes ring while others start, and a tracker channel can keep a note
ringing in the background. Such a channel is still one track. Notes that start
together are written as a chord between single quotes, and a note that rings
on past the next one carries its own length inside the chord:

```
#TRACK A "MIDI 1" channel=0 voice=0 kind=tonal l4
A | o4 'ceg'4 'c1e'4 f4 g4 |
```

The first chord is a C major triad, a quarter note. In the second, the low C
holds for a whole note while E, then F and G, play above it. Chapter 6 has
the details.

Only channels that really play several notes become chords; a chip channel
that plays one note at a time never does. Two cases still split a channel
into several tracks, told apart by `voice=`:

- a channel that has drum hits (`x`) as well as notes;
- a decoder that reports separate voices with their own pitch bends.

## Channel commands

Instrument changes, levels, pans, pitch bends and chip parameters belong to
the channel and are written on its track. On a split channel they are written
on voice 0, and bends on the track of the voice that bends.

## Kinds

| Kind | Meaning | Notes are written as |
| --- | --- | --- |
| `tonal` | a voice with a musical pitch | note names |
| `sample` | a sample-playback voice | note names (often relative pitch, chapter 12) |
| `noise` | a noise generator | hits (`x`) |
| `percussion` | drum or rhythm voices | hits (`x`), or note names when the drum has a pitch |
| `mixed` | the decoder only reports a mix of voices | hits (`x`) |

## Track order and labels

Tracks are ordered by channel number, then voice. Labels run `A` to `Z`, then
`AA`, `AB`, and so on. A channel that never sounds a note or changes a setting
has no track.
