//! Video pipeline: RGBA frame buffers, procedural generators, GPU upload (wgpu) and FFmpeg decoding.

pub mod cache;
pub mod chroma;
pub mod composite;
pub mod frame;
pub mod generators;
pub mod gpu;
pub mod grading;
pub mod info;
pub mod lut;
pub mod pan_crop;
pub mod render;
pub mod scopes;
pub mod still;
pub mod transitions;
pub mod vfx;
pub mod vfx_gpu;
pub mod worker;

#[cfg(feature = "ffmpeg")]
pub mod decode;

pub use frame::FrameBuffer;
pub use info::VideoInfo;
