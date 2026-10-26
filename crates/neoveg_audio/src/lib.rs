//! Audio pipeline: decoding (symphonia), hardware output (cpal), real peak and loudness meters and
//! waveforms.

pub mod clap_host;
pub mod decode;
pub mod dsp;
pub mod duck;
pub mod fxmixer;
pub mod meter;
pub mod metronome;
pub mod mixer;
pub mod playback;
pub mod readout;
pub mod record;
pub mod stretch;
pub mod waveform;
pub mod whisper;
