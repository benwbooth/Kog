# 5. Tracks and voices

Each track is one voice that plays one note at a time. A voice is whatever the
decoder reports as a channel: a pulse wave on the NES, an operator pair on an
OPL chip, one of the PlayStation's 24 sample voices, or a MIDI channel.

## Polyphonic channels

Some channels play several notes at once. A MIDI channel can hold a chord, and
a tracker channel can keep a note ringing in the background. Kog splits such a
channel into as many tracks as it needs simultaneous notes. They share a
`channel=` number and are told apart by `voice=`:

```
#TRACK A "MIDI 1" channel=0 voice=0 kind=tonal l4
#TRACK B "MIDI 1" channel=0 voice=1 kind=tonal l4
#TRACK C "MIDI 1" channel=0 voice=2 kind=tonal l2
```

When a new note starts, it goes to the lowest-numbered voice that is silent.
A C major chord therefore puts C on voice 0, E on voice 1 and G on voice 2.

## Channel commands live on voice 0

Instrument changes, levels, pans and chip parameters belong to the channel,
not to one of its notes. They are written on the channel's voice 0 track.
Pitch bends belong to a sounding note, so they are written on the track of the
voice that holds that note.

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
