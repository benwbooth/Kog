# 14. Formats and chips

Every format Kog can inspect can be written as MML. What appears in the score
depends on what the decoder can see. Parameter names are the ones shown in
the Channel Inspector; this chapter describes what to expect.

## MIDI

MIDI files, including all four selectable synths and supported containers.
Notes come straight from the sequence, so lengths, velocities, tempo
(`#TEMPO` without `inferred`) and time signature (`#BAR`) are those of the
file. Each MIDI channel is one track, with chords and held notes written as
chords (chapter 6).
Instruments are bank and program (`@"Bank 0:0 \u{b7} Program 1"`); controllers, sustain, pitch bend
and pressure appear as parameters and bends.

## HMI, HMP, HMQ, MUS, XMI

Legacy game MIDI formats played through libADLMIDI. Notes, controllers and
events come from the running sequencer, much like MIDI.

## Tracker modules (OpenMPT)

MOD, S3M, XM, IT and the other OpenMPT formats. Pitches are the mixer's real
playing pitches, so slides and vibrato effects appear as bends and macros.
Background voices kept by "new note actions" are assigned to the channel that
started them, so a note ringing in the background shows up as a held note in
a chord.

## Hively and AHX

Amiga-style chip trackers. Notes come from the real channel periods, with the
instrument and both effect columns available as parameters.

## Organya

Cave Story's format. Wavetable pitches, original note lengths, volume and pan
events, and drum hits.

## Syntrax

Live voice pitch, instruments, arpeggio and modulation state.

## AdPlug (OPL2 and OPL3)

FM synthesis. Key gates and frequencies give notes; operator settings appear
as parameters. Levels are the programmed attenuation, not the sound's actual
loudness. Rhythm-mode drums are hits.

## Game Music Emu: NSF, NSFE, GBS, AY, HES, KSS, SAP, SPC

Live hardware voices and registers, including NES expansion chips. Tonal
voices get keys from their frequency, which is where `#TUNE` tables come from:
fixed period registers make each key a few cents off. Period and frequency
registers usually become `#PITCH` tables. Envelopes and duty sequences become
macros. Noise channels are hits.

## VGM, VGZ, S98, DRO, GYM

Register logs played through the matching sound cores. Mapped chips give
voices, key gates, frequencies and controls. On the Sega PSG the level is
the attenuation, so it is not repeated as a parameter. On the YM2612 the
level is operator 4's total level, so the operator levels list operators 1
to 3, and only channel 3 shows its special mode (the timer bits drivers
rewrite constantly are left out).

### Drums on the Mega Drive DAC

The YM2612's sixth channel can play 8-bit samples through its DAC, which is
how most Mega Drive music plays drums. The DAC has no key-on, so Kog finds
each hit itself: sound starting again after a quiet moment, or a sample
restarting from its first bytes. Each sample is told apart by its opening
bytes and numbered in the order it is first heard. The track is a drum map,
like MIDI percussion: sample 1 is `c` in octave 2 (key 36), sample 2 `c+`,
and so on, so a kick and snare pattern reads `o2 c c+ c c c+`. The keys are
sample numbers, not pitches. The tracker writes them `S01`, `S02`, and the
keyboard lights one key per sample.

Drivers that mix several samples into the DAC in software (XGM, GEMS) are
heard as one stream, so overlapping drums show as a single hit. Programmable DSP devices without
individual voices appear only as register parameters.

## SID and RSID

Three voices per SID chip: waveform, frequency, pulse width, gate, ADSR and
filter. Release tails and software sample playback are not reconstructed.

## PlayStation: PSF, PSF2, miniPSF, miniPSF2

24 or 48 SPU sample voices. Keys are relative sample pitch (chapter 12) and
instruments are sample addresses (`@"ADPCM 065010"`). The SPU reports
key-ons, so retriggers are exact and legato is marked. Sample read positions
and raw envelope levels are measurements and are left out. Key-on reports
can arrive tens of milliseconds late, which the beat tracking absorbs.

## SNES: SNSF, SFM, SPC

Eight DSP voices with sample identity and rate, envelope, noise, echo and
pitch modulation.

## Nintendo DS: 2SF and NCSF

Sixteen hardware voices. NCSF also knows the sequence's source notes, so its
pitches are real keys; 2SF voices are hardware pitch and sample state.

## GBA: GSF

Four PSG voices and the two Direct Sound mixes. Software voices mixed by the
game into Direct Sound cannot be separated and appear as one mixed track.

## QSound, Saturn and Dreamcast: QSF, SSF, DSF

QSound PCM and ADPCM voices, 32 SCSP voices, or 64 AICA voices with sample
playback, envelopes, looping and LFO settings.

## Nintendo 64: USF

Only the stereo output and its registers are visible; the game's own
sequencer voices are mixed before Kog sees them.

## Recordings

MP3, FLAC and other recorded audio have no voices to read. The MML view says
so instead of showing a score.
