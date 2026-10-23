//! Audio pipeline: decoding (symphonia), hardware output (cpal), real peak meters and waveforms.

pub mod decode;
pub mod meter;
pub mod mixer;
pub mod playback;
pub mod waveform;
