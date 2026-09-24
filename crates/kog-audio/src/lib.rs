//! Kog's decoding, playback, and radio engine.
//!
//! This crate owns format backends, the decoder registry, the playback
//! pipeline, and random radio. The UI crate depends on it; it never
//! depends on the UI, so engine edits and UI edits compile apart.

pub mod adlmidi;
pub mod adlmidi_decoder;
pub mod adplug;
pub mod adplug_decoder;
pub mod apl;
pub mod apl_decoder;
pub mod archive;
pub mod cover_art;
pub mod cuesheet;
pub mod cuesheet_decoder;
pub mod decoder;
pub mod ffmpeg;
pub mod ffmpeg_decoder;
pub mod gme;
pub mod gme_decoder;
pub mod gsf;
pub mod gsf_decoder;
pub mod hively;
pub mod hively_decoder;
pub mod legacy_id3;
pub mod libvgm;
pub mod libvgm_decoder;
pub mod mt32;
pub mod ncsf;
pub mod ncsf_decoder;
pub mod openmpt;
pub mod openmpt_decoder;
pub mod opl3;
pub mod organya;
pub mod organya_decoder;
pub mod playback;
pub mod playback_order;
pub mod playlist;
pub mod psf;
pub mod psf_decoder;
pub mod qsf;
pub mod qsf_decoder;
pub mod radio;
pub mod sc55;
pub mod sdsf;
pub mod sdsf_decoder;
pub mod settings;
pub mod sfm;
pub mod sfm_decoder;
pub mod sid;
pub mod sid_decoder;
pub mod streaming;
pub mod spessasynth_midi;
pub mod syntrax;
pub mod syntrax_decoder;
pub mod track;
pub mod usf;
pub mod usf_decoder;
pub mod vgmstream;
pub mod vgmstream_decoder;
pub mod visualizer;
