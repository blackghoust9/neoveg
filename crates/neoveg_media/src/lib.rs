// Adapted from `craft-media` (storytold/craft-libs, MIT OR Apache-2.0); see third_party/filmcraft/README.md.
// Same allowances as the FilmCraft workspace this code comes from
#![allow(clippy::needless_range_loop, clippy::type_complexity)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]
//! Media containers and bit-level I/O for Neoveg.
//!
//! - `bitstream`: MSB-first bit reader and writer, Exp-Golomb codes, emulation-prevention escaping
//!   and NAL unit splitting.
//! - `isobmff`: a clean-room ISO BMFF (MP4) and QuickTime (MOV) demuxer and muxer, and a box reader
//!   for other ISO BMFF formats.
//!
//! Codecs stay in their own crates (`neoveg_h264`, `neoveg_aac`).

pub mod bitstream;
pub mod isobmff;
