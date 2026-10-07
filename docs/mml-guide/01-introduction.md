# 1. Introduction

Kog MML is a text notation for the music Kog plays. It describes MIDI files,
tracker modules, FM synthesis, and the sound chips of game consoles and home
computers in one language. Every song Kog can show in its Channel Inspector
can also be read as Kog MML.

MML, short for Music Macro Language, is a family of notations that began on
1980s home computers and lives on in chiptune composition tools. A line of MML
reads like a melody: `o4 l8 c d e f g4 > c4` plays a C major scale run in
eighth notes and ends on two quarter notes, the last one an octave higher.
Kog MML keeps that style and adds what is needed to describe a recording
exactly: tracks for every voice, bar lines, chip registers, envelopes,
detuning, triplets, and a timing map for songs whose tempo drifts.

## What a score is

Kog does not read the song file to produce MML. It plays the song once more,
silently, with the same decoder and synth settings used for playback, and
watches what the sound hardware does: which voices start and stop, at which
pitch, with which instrument and parameters. The result is a *score*: every
voice's notes on a beat grid, and the parameters that changed while they
played.

The score is then written as MML. Writing and reading are exact inverses: the
text Kog writes reads back into the identical score, event for event. You can
save the text, edit it, or compare two recordings without losing anything the
score holds.

The score is a careful recording, not the composer's source file. Chapter 13,
"How Kog records a score", explains what is kept, what is placed on the beat,
and what is left out.

## How this guide is organised

- Chapter 2 shows where to find the MML view and how to use it.
- Chapters 3 to 5 describe the shape of a file: its headers, tracks and bars.
- Chapters 6 to 9 cover everything that can appear on a track line: notes,
  lengths, rests, and commands.
- Chapters 10 to 12 cover the tables that keep the text short: macros, pitch
  and tuning tables, and transposition for sample-based chips.
- Chapter 13 explains timing and how recordings are made, and chapter 14 goes
  through each family of formats.
- Chapter 15 is a complete grammar, chapter 16 walks through an example, and
  chapter 17 answers common questions.

Examples in this guide are real Kog MML. Text after `;` is a comment.
