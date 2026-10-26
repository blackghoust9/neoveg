//! Video pipeline: RGBA frame buffers, procedural generators, GPU upload (wgpu) and FFmpeg decoding.

pub mod frame;
pub mod generators;
pub mod gpu;
pub mod info;
pub mod lut;
pub mod worker;

#[cfg(feature = "ffmpeg")]
pub mod decode;

pub use frame::FrameBuffer;
pub use info::VideoInfo;
