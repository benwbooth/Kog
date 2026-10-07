# Channel Inspector

Open **View → Channel Inspector** in Qt, the web player, or the terminal.
Qt and the web player also use **Ctrl+Shift+I**. On Android and iOS, open
**Channel Inspector** from Now Playing.

Choose **Keyboards**, **Tracker**, or **Both**. Each musical channel has its
own keyboard. Cyan keys are held; green keys are retained by sustain. Multiple
keys can light together. Fractional pitches have an orange bend marker. Noise,
percussion, and untuned sample activity are labelled without inventing keys.
Each channel also has a live level meter. Qt, Android, iOS, and the terminal
label it **Level** and show a percentage alongside the bar. It uses the same
decoder-reported level as the web meter; depending on the format, this is a
voice envelope, programmed volume, or measured output level.

The tracker includes notes, instruments, volume, effects, and song data.
Original tracker rows are used when available; otherwise it shows MIDI events
or changes in chip state. **Follow playback** keeps the current row visible.
Qt cells have tooltips for long effect strings. The web and mobile keyboards
have expandable controls and effects.

In the terminal, **1/2/3** choose the view, **D** opens full channel/row details,
**F** toggles following, arrows navigate, **[ / ]** change the keyboard octave,
and **Esc** closes the inspector. **Space** still pauses playback.

## What each decoder can report

| Family | Keyboard and tracker data |
| --- | --- |
| MIDI, including supported containers and all four selectable synths | Polyphonic MIDI channels, velocity, sustain/sostenuto, programs, banks, controllers, pitch bend and RPN tuning; tempo, meter, text, and SysEx events. These are sequenced commands, without inferred synth release tails or custom SysEx tuning. |
| HMI/HMP/HMQ/MUS/XMI through libADLMIDI | Notes, controllers, and events from the running legacy sequencer's public callback. |
| OpenMPT modules | Original pattern commands and current mixer pitches, including effects, slides, pitch envelopes, and background NNA voices assigned to their originating channels. |
| Hively/AHX | Actual channel periods and instrument state, original rows, and both effect columns. |
| Organya | Wavetable pitches, original note lengths, volume/pan events, and drum activity. |
| Syntrax | Live voice pitch, instruments, sequence rows, arpeggio and modulation state. |
| AdPlug | OPL2/3 FM and rhythm key gates, frequencies, operator parameters, and player position. Levels represent programmed attenuation. |
| GME: NSF/NSFE, GBS, AY, HES, KSS, SAP, SPC | Live hardware voices and registers, including supported NSF expansion chips. Tonal oscillators have frequency-derived keys. Sample voices retain their playback rates and identities. |
| VGM/VGZ, S98, DRO, GYM | Writes sent to the sound cores producing the audio. Mapped chips expose tone or sample voices, key gates, frequencies, and controls. Other programmable DSP devices expose a labelled register view; their individual voices are not reconstructed. |
| SID/RSID | Three voices per SID, waveform, frequency, pulse width, gate, ADSR, and filter controls. Release envelopes and software digi voices are not reconstructed. |
| NCSF | Sixteen DS voices with source notes, pitch changes, instruments, envelopes, and sequence state. |
| PSF/PSF2, including miniPSF/miniPSF2 | 24/48 SPU voices, live envelopes, sample addresses, rate, volume, ADSR, and reverb. Keys show relative sample pitch, with C4 at normal playback rate (SPU pitch 0x1000); original instrument tuning is unavailable. |
| SNSF/SFM/SPC | Eight SNES DSP voices, sample identity/rate, envelope, noise, echo, and modulation controls. |
| 2SF | Sixteen DS hardware voices, including pitched PSG and sample/noise state. |
| GSF | Four GBA PSG voices and the two Direct Sound mixes. Game-specific software voices inside the mixes are not separated. |
| QSF | Sixteen QSound PCM and three ADPCM voices with playback parameters. |
| SSF/DSF | 32 SCSP or 64 AICA voices, sample playback, envelopes, looping, LFO, and DSP controls. |
| USF | The stereo AI DMA output and its registers/levels. N64 game-specific software sequencer voices are not reconstructed. |
| Mixed recordings | An explicit unavailable state. A stereo recording does not carry the original musical channels. |

A periodic oscillator's frequency maps directly to a piano key:
`69 + 12 * log2(frequency / 440)`. A sample playback rate measures samples per
second, which does not establish the waveform's fundamental frequency. Sample
voices get keys when the format or sequencer supplies tuning or a source note.
PSF/PSF2 additionally provide explicitly labelled relative keys: doubling the
sample playback rate moves up an octave from the C4 reference. This shows
transposition and pitch slides while leaving original sample tuning unknown.
Their fractional sample-rate offsets are not shown as red bend markers or
musical cents. PSF tracker rows follow hardware key-on writes (including
same-pitch retriggers), key-off/release, and changed control registers. Held
notes, continuous volume/pitch automation, envelope progress, and advancing
sample addresses do not create rows; those live values remain in the channel
details and meters. Onset rows include their starting pitch and controls.
The inspector does not estimate fundamentals from recorded audio.

Register displays report observed state and programmed controls, which can
outlast a hardware envelope or sample. They do not claim to recover the
original composition's tracker effects from a register log. State is normally
sampled at up to 200 Hz; shorter changes can fall between snapshots, and some
emulators report at their native frame/block boundary.

## Synchronization and transport

`kog-inspection` defines the shared serializable model. Every native decoder
records from its own running core. No second emulator or muted re-render is
used for inspection. MIDI's event timeline is evaluated at the same consumed
playback position as the audio.

Each loaded track has its own monitor. Bounded queues carry media timestamps;
the view chooses the newest state at or before the audible position. Seek
runs receive a new generation so an old worker cannot replace current state.
Paused playback retains its keys. Helpers retain early frames before their
format header has been handed back to the player.

The stream encoder records metadata while consuming the same PCM it encodes.
Independent compressed one-second windows retain rapid intermediate events,
initial state, changes, and row history. `/api/inspection` uses the audio
locator, codec, bitrate, MIDI render profile, and seek offset. It uses the
server's normal authentication. Clients select the window using their own
audio clock and prefetch the next one. Metadata is counted and evicted with
the encoded audio cache.

Android's native data source also keeps a private temporary recording so
ExoPlayer's longer buffering does not discard the state at the audible
position. Embedded helpers publish through a worker-local feed. Standalone
helpers can use a bounded file ring; matching sequence markers reject partial
writes. Neither transport blocks the renderer on a frontend refresh.

Platform decoder availability remains as documented in the decoder guides;
adding an inspector does not add an otherwise unavailable decoder to a platform.

## Verification

```sh
cargo test -p kog-audio -p kog-server inspection --lib -- --test-threads=1
cargo test -p kog-inspection
cargo build --workspace --bins
bash crates/kog-web/build.sh
bash tests/native/run-channel-inspection.sh
bash android/gradlew -p android :app:compileDebugKotlin
QT_QPA_PLATFORM=offscreen QT_QUICK_CONTROLS_STYLE=Basic \
  QT_QPA_PLATFORMTHEME=basic QT_QUICK_BACKEND=software \
  qmltestrunner -input tests/qml/tst_ChannelInspector.qml
```

The native fixtures check channel counts, actual pitched/unpitched activity,
pause, seeks, and PCM with inspection enabled/disabled. The VGM integration
compares exact PCM for SN76489, YM2612, and AY chips and a seek, and checks
forwarding for every enabled libvgm core. Clock/window tests cover
rapid events, backwards movement, and nonzero stream start positions.

Set `KOG_INSPECTION_FIXTURES` to a temporary directory when running the Rust
tests to export their snapshots and stream windows. Compile
`tests/swift/channel_inspection.swift` with `ios/Kog/ChannelInspection.swift`
and pass those JSON files to verify the Swift decoding and clock contract.
