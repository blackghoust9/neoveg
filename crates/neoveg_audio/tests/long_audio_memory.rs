//! Memory of loading a long recording, the old way (whole file, a stereo copy, a waveform copy)
//! against the new one (a store that spills to disk, shared with the waveform).
//!
//! Both tests are ignored: they write a 10-minute WAV and are meant to be run one at a time while
//! the process is watched from outside, e.g.
//! `long_audio_memory.exe --ignored --exact old_way_whole_file_three_copies`.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use neoveg_audio::decode::{decode_file, decode_to_store};
use neoveg_audio::waveform::WaveformSource;

const RATE: u32 = 48_000;
const SECONDS: u32 = 600;

/// A 16-bit stereo WAV of a slow sweep, written in pieces so making it needs little memory.
fn make_wav() -> PathBuf {
    let path = std::env::temp_dir().join(format!("neoveg_long_{}.wav", std::process::id()));
    let frames = RATE * SECONDS;
    let data_len = frames * 4;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    out.write_all(b"RIFF").unwrap();
    out.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    out.write_all(b"WAVEfmt ").unwrap();
    out.write_all(&16u32.to_le_bytes()).unwrap();
    out.write_all(&1u16.to_le_bytes()).unwrap();
    out.write_all(&2u16.to_le_bytes()).unwrap();
    out.write_all(&RATE.to_le_bytes()).unwrap();
    out.write_all(&(RATE * 4).to_le_bytes()).unwrap();
    out.write_all(&4u16.to_le_bytes()).unwrap();
    out.write_all(&16u16.to_le_bytes()).unwrap();
    out.write_all(b"data").unwrap();
    out.write_all(&data_len.to_le_bytes()).unwrap();
    for i in 0..frames {
        let v = ((i as f32 * 0.05).sin() * 12_000.0) as i16;
        out.write_all(&v.to_le_bytes()).unwrap();
        out.write_all(&v.to_le_bytes()).unwrap();
    }
    out.flush().unwrap();
    path
}

#[test]
#[ignore]
fn old_way_whole_file_three_copies() {
    let path = make_wav();
    // What the program used to do: the decoded file, its stereo copy, and the waveform's own copy
    let audio = decode_file(&path).unwrap();
    let stereo = Arc::new(audio.to_stereo());
    let waveform = WaveformSource::from_interleaved_f32(audio.sample_rate, 2, &stereo).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    println!("{} {}", stereo.len(), waveform.frame_count());
    let _ = std::fs::remove_file(path);
}

#[test]
#[ignore]
fn new_way_store_shared_with_the_waveform() {
    let path = make_wav();
    let audio = decode_to_store(&path).unwrap();
    let on_disk = audio.store.is_on_disk();
    let store = Arc::new(audio.store);
    let waveform = WaveformSource::from_pcm(audio.sample_rate, Arc::clone(&store)).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    println!("{on_disk} {} {}", store.frames(), waveform.frame_count());
    let _ = std::fs::remove_file(path);
}
