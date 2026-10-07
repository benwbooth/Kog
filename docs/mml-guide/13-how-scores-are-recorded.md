# 13. How Kog records a score

This chapter explains what happens between pressing play and reading MML, and
so what a score can and cannot tell you.

## Frames

Kog plays the song again with the decoder used for playback, with the audio
discarded. Every few milliseconds (up to 200 times a second, or at the chip's
own frame rate) the decoder reports a *frame*: for every voice, whether it is
sounding, its pitch, instrument, level, pan and parameters.

Changes shorter than one frame are not seen. A drum that lasts two
milliseconds may be missed, and two writes to the same register within a
frame leave only the second.

## From frames to notes

For each voice, Kog compares each frame with the previous one:

- A voice starts sounding, or a new pitch appears: a note starts.
- The voice stops (its key is released) or its pitch moves to another
  semitone: the note ends. The release is where the note ends; the fading
  tail after release is not part of the note.
- The pitch moves within a semitone: a bend.
- The chip reports a new key-on at the same pitch: the note ends and a new
  one starts.
- An unpitched voice starts: a hit.

Instrument, level, pan and parameter changes are recorded at the frame where
they happen.

## Measurements are left out

Some values change on nearly every frame because they are measurements rather
than settings: the address a sample voice is currently reading, an envelope's
exact level as it decays, a measured output level. For each channel Kog counts
how often each value changes while the voice sounds. Values that change on
more than half of those frames are left out of the score. Level and pan of
such channels are written at each key-on only.

## Finding the beat

Frames are spaced in milliseconds, but music moves in beats. To write note
values instead of milliseconds, Kog finds the beat:

1. It collects the start time of every note in the song.
2. It looks for the longest step (between about 45 milliseconds and a slow
   quarter note) that nearly every gap between starts is a whole number of.
   Promising steps are refined to a fraction of a frame, because a step that
   is slightly wrong drifts across a long song.
3. It decides how many steps make a quarter note by trying 1, 2, 3, 4, 6, 8,
   12 and 16 and keeping the choice that writes the most gaps as plain note
   values. A step that divides beats into threes therefore produces triplets
   rather than awkward ties. Ties are broken toward a tempo near 125 BPM.

MIDI files state their tempo and time signature, so this step uses those.

## Following the beat

Real recordings are not perfectly steady. Tempo drifts between sections,
chips report key-ons a few milliseconds early or late, and long held chords
end on a slightly different beat than a fixed grid predicts. Kog walks
through the song's note starts in order and places each on the nearest beat,
or half, third or quarter of a beat, if it is close enough. A start that lines
up with a whole beat also nudges the length of the beat, so slow drift is
followed instead of accumulating. Over long gaps the window is wider, because
a few percent of tempo drift adds up over a held chord.

Starts that fit no beat line are kept at a quarter of a beat's resolution.
Releases are snapped more loosely, to the nearest whole beat when they are
within half a beat of it, so a note let go just before the next one ends on
the beat. Commands between notes are placed the same way; commands during a
note keep full resolution so that envelopes and vibrato stay intact.

## The timing map

Because the beat is followed rather than assumed, the time of each beat is
known. Kog keeps the points where the real timing departs from a straight
line by more than 5 milliseconds, and writes them as `#TIMING`. The views use
it to highlight the right notes even when the tempo drifts.

## Bars

Kog places bar lines where the biggest chords start: it counts how many voices
start on each beat of the bar and picks the beat with the most, weighting
chords heavily. `#PICKUP` makes the first bar longer to line the rest up.

## Finishing

Before the score is written, Kog:

- drops all but the last value of a setting written twice at the same moment;
- moves registers that only follow the pitch into `#PITCH` tables;
- records each key's usual detune in `#TUNE`;
- turns repeated changes within notes into macros;
- shifts relative-pitch instruments by octaves (`#TRANSPOSE`);
- converts hexadecimal register values to decimal.

The score is then written as text. Reading that text gives back exactly the
same score.

## Speed

Recording speed depends on the decoder. NES, Game Boy and similar chips record
hundreds of times faster than real time. PlayStation music runs an emulated
processor and records about five times faster than real time. The first bars
appear after a few seconds of recorded audio, and the view updates as the rest
arrives.
