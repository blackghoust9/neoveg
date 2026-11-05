//! What the editor and the process that hosts a VST3 plug-in say to each other (see
//! [`crate::vst3_isolated`]): frames on the pipes of the child process.
//!
//! A frame is `MAGIC`, one byte for the kind, four bytes for the length of the payload (little
//! endian) and the payload. The magic is what a reader looks for: a plug-in that prints to the
//! output of its process puts bytes between the frames, and they are skipped until the next magic.
//! The control messages are JSON; the audio is binary (planar 32-bit floats, little endian), so a
//! block costs one copy each way.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

/// Starts every frame.
pub const MAGIC: [u8; 4] = *b"NV3\x01";

/// The biggest payload a reader accepts (a frame of 64 MiB is already a broken one).
pub const MAX_PAYLOAD: usize = 64 * 1024 * 1024;

/// The kinds of frame.
pub mod kind {
    /// Editor to child: open a plug-in (JSON [`super::Open`]).
    pub const OPEN: u8 = 1;
    /// Editor to child: process a block of audio (binary, see [`super::encode_block`]).
    pub const PROCESS: u8 = 2;
    /// Editor to child: forget the state of the plug-in.
    pub const RESET: u8 = 3;
    /// Editor to child: scan a module (JSON [`super::Scan`]).
    pub const SCAN: u8 = 4;
    /// Editor to child: end.
    pub const QUIT: u8 = 5;
    /// Child to editor: the plug-in is open (JSON [`super::Ready`]).
    pub const READY: u8 = 10;
    /// Child to editor: the processed block (binary).
    pub const PROCESSED: u8 = 11;
    /// Child to editor: what a scan found (JSON `Vec<ScanItem>`).
    pub const SCANNED: u8 = 12;
    /// Child to editor: something failed (the payload is the text).
    pub const ERROR: u8 = 13;
    /// Child to editor: done, nothing to report.
    pub const DONE: u8 = 14;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Open {
    pub path: String,
    pub class_id: String,
    pub sample_rate: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ready {
    /// The plug-in's own latency in samples.
    pub latency: u32,
    /// Its parameters: the key the effect chain stores and the default.
    pub params: Vec<(String, f32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scan {
    pub path: String,
}

/// A parameter of a plug-in a scan found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanParam {
    pub id: String,
    pub vst_id: u32,
    pub name: String,
    pub default: f32,
    pub stepped: bool,
    pub is_bypass: bool,
}

/// A plug-in a scan found (the data of `Vst3PluginInfo`, in a form that crosses the pipe).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanItem {
    pub kind: String,
    pub class_id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: String,
    pub audio_inputs: u32,
    pub audio_outputs: u32,
    pub params: Vec<ScanParam>,
    pub latency: u32,
}

/// Writes one frame and flushes.
pub fn write_frame(out: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    out.write_all(&MAGIC)?;
    out.write_all(&[kind])?;
    out.write_all(&(payload.len() as u32).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Reads the next frame: bytes before the magic are skipped, `Ok(None)` is the end of the stream.
pub fn read_frame(input: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut matched = 0;
    let mut byte = [0u8; 1];
    // Look for the magic one byte at a time, so that junk of any length is passed over
    while matched < MAGIC.len() {
        if input.read(&mut byte)? == 0 {
            return Ok(None);
        }
        matched = if byte[0] == MAGIC[matched] {
            matched + 1
        } else if byte[0] == MAGIC[0] {
            1
        } else {
            0
        };
    }
    let mut head = [0u8; 5];
    input.read_exact(&mut head)?;
    let len = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    if len > MAX_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "quadro grande demais",
        ));
    }
    let mut payload = vec![0u8; len];
    input.read_exact(&mut payload)?;
    Ok(Some((head[0], payload)))
}

/// A block of audio and the parameter changes that go with it, as the payload of a
/// [`kind::PROCESS`] frame: the number of changes, each as a length, the key and the value, then the
/// number of channels, the number of frames and the samples channel after channel.
pub fn encode_block(changes: &[(String, f32)], channels: &[&mut [f32]]) -> Vec<u8> {
    let frames = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let mut out = Vec::with_capacity(8 + channels.len() * frames * 4 + changes.len() * 16);
    out.extend_from_slice(&(changes.len() as u16).to_le_bytes());
    for (key, value) in changes {
        let key = key.as_bytes();
        out.extend_from_slice(&(key.len().min(u16::MAX as usize) as u16).to_le_bytes());
        out.extend_from_slice(&key[..key.len().min(u16::MAX as usize)]);
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.push(channels.len() as u8);
    out.extend_from_slice(&(frames as u32).to_le_bytes());
    for channel in channels {
        for sample in &channel[..frames] {
            out.extend_from_slice(&sample.to_le_bytes());
        }
    }
    out
}

/// The parameter changes and the planar samples of an encoded block (`None` for one that is cut
/// short or whose lengths do not add up).
#[allow(clippy::type_complexity)]
pub fn decode_block(bytes: &[u8]) -> Option<(Vec<(String, f32)>, Vec<Vec<f32>>)> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let part = bytes.get(at..at.checked_add(n)?)?;
        at += n;
        Some(part)
    };
    let count = u16::from_le_bytes(take(2)?.try_into().ok()?) as usize;
    let mut changes = Vec::with_capacity(count);
    for _ in 0..count {
        let len = u16::from_le_bytes(take(2)?.try_into().ok()?) as usize;
        let key = String::from_utf8_lossy(take(len)?).into_owned();
        let value = f32::from_le_bytes(take(4)?.try_into().ok()?);
        changes.push((key, value));
    }
    let channels = *take(1)?.first()? as usize;
    let frames = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
    let mut audio = Vec::with_capacity(channels);
    for _ in 0..channels {
        let raw = take(frames.checked_mul(4)?)?;
        audio.push(
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect(),
        );
    }
    (at == bytes.len()).then_some((changes, audio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_goes_through_a_pipe_and_comes_out_the_same() {
        let mut pipe = Vec::new();
        write_frame(&mut pipe, kind::OPEN, b"hello").unwrap();
        write_frame(&mut pipe, kind::QUIT, b"").unwrap();
        let mut reader = pipe.as_slice();
        assert_eq!(
            read_frame(&mut reader).unwrap(),
            Some((kind::OPEN, b"hello".to_vec()))
        );
        assert_eq!(
            read_frame(&mut reader).unwrap(),
            Some((kind::QUIT, Vec::new()))
        );
        assert_eq!(read_frame(&mut reader).unwrap(), None);
    }

    #[test]
    fn what_a_plugin_prints_between_frames_is_skipped() {
        let mut pipe = b"a line the plug-in printed\n".to_vec();
        write_frame(&mut pipe, kind::DONE, b"x").unwrap();
        // Junk that starts like the magic and is not
        pipe.extend_from_slice(b"NV3 NV NNV3\x02 ");
        write_frame(&mut pipe, kind::DONE, b"y").unwrap();
        let mut reader = pipe.as_slice();
        assert_eq!(
            read_frame(&mut reader).unwrap(),
            Some((kind::DONE, b"x".to_vec()))
        );
        assert_eq!(
            read_frame(&mut reader).unwrap(),
            Some((kind::DONE, b"y".to_vec()))
        );
    }

    #[test]
    fn a_frame_cut_short_or_too_big_is_an_error() {
        let mut pipe = Vec::new();
        write_frame(&mut pipe, kind::PROCESS, &[1, 2, 3, 4]).unwrap();
        pipe.truncate(pipe.len() - 2);
        assert!(read_frame(&mut pipe.as_slice()).is_err());
        let mut huge = MAGIC.to_vec();
        huge.push(kind::PROCESS);
        huge.extend_from_slice(&(MAX_PAYLOAD as u32 + 1).to_le_bytes());
        assert!(read_frame(&mut huge.as_slice()).is_err());
    }

    #[test]
    fn a_block_and_its_parameter_changes_round_trip() {
        let mut left = vec![0.5f32, -0.25, 1.0];
        let mut right = vec![0.0f32, 0.125, -1.0];
        let changes = vec![("17".to_string(), 0.75f32), ("3".to_string(), 0.0)];
        let bytes = encode_block(&changes, &[&mut left, &mut right]);
        let (got_changes, audio) = decode_block(&bytes).unwrap();
        assert_eq!(got_changes, changes);
        assert_eq!(audio, vec![vec![0.5, -0.25, 1.0], vec![0.0, 0.125, -1.0]]);
        // Channels of different lengths use the shortest
        let mut short = vec![9.0f32];
        let bytes = encode_block(&[], &[&mut left, &mut short]);
        assert_eq!(decode_block(&bytes).unwrap().1, vec![vec![0.5], vec![9.0]]);
    }

    #[test]
    fn a_broken_block_is_refused_not_trusted() {
        let mut left = vec![0.5f32; 4];
        let bytes = encode_block(&[], &[&mut left]);
        assert!(
            decode_block(&bytes[..bytes.len() - 1]).is_none(),
            "cut short"
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(decode_block(&longer).is_none(), "trailing bytes");
        assert!(decode_block(&[]).is_none());
    }
}
