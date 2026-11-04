//! Audio recording from the default input device into a 32-bit float WAV file.
//!
//! The input callback never allocates, locks or does I/O: it converts the samples into one of a
//! fixed pool of preallocated chunks and hands it to a writer thread through a bounded channel;
//! the writer returns each chunk to the pool after writing it. If the disk cannot keep up the
//! chunks run out and the overrun is counted (reported when recording stops) instead of blocking.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

/// Samples per pooled chunk and number of chunks (about 3 s of stereo at 48 kHz in total).
const CHUNK_SAMPLES: usize = 8192;
const CHUNKS: usize = 64;

/// The input the recording will use.
#[derive(Debug, Clone, PartialEq)]
pub struct InputInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// What a finished recording is.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    /// Frames written (one sample per channel).
    pub frames: u64,
    /// Chunks the writer could not take in time (0 on a healthy disk).
    pub overruns: u64,
}

impl Recording {
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate.max(1))
    }
}

/// The default input device, when there is one.
pub fn probe_input() -> Result<InputInfo, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("nenhum dispositivo de entrada de áudio")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("entrada de áudio indisponível: {e}"))?;
    Ok(InputInfo {
        device: device.name().unwrap_or_else(|_| "entrada padrão".into()),
        sample_rate: config.sample_rate().0,
        channels: config.channels(),
    })
}

// ---- WAV writing ---------------------------------------------------------------------------------

/// The 44-byte header of an IEEE-float WAV holding `data_bytes` of samples.
pub fn wav_float_header(channels: u16, sample_rate: u32, data_bytes: u32) -> [u8; 44] {
    let block_align = channels * 4;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data_bytes).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&3u16.to_le_bytes()); // IEEE float
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    h[32..34].copy_from_slice(&block_align.to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    h
}

/// Writes float samples after a placeholder header and patches the sizes when finished.
pub struct WavWriter<W: Write + Seek> {
    out: W,
    channels: u16,
    sample_rate: u32,
    data_bytes: u64,
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new(mut out: W, channels: u16, sample_rate: u32) -> std::io::Result<Self> {
        out.write_all(&wav_float_header(channels, sample_rate, 0))?;
        Ok(Self {
            out,
            channels,
            sample_rate,
            data_bytes: 0,
        })
    }

    pub fn write_samples(&mut self, samples: &[f32]) -> std::io::Result<()> {
        for s in samples {
            self.out.write_all(&s.to_le_bytes())?;
        }
        self.data_bytes += samples.len() as u64 * 4;
        Ok(())
    }

    /// Frames written so far.
    pub fn frames(&self) -> u64 {
        self.data_bytes / (4 * u64::from(self.channels.max(1)))
    }

    /// Patches the header with the real size and returns the writer.
    pub fn finish(mut self) -> std::io::Result<W> {
        // A WAV cannot hold more than 4 GiB of data; the header is clamped, the file stays valid
        let bytes = u32::try_from(self.data_bytes).unwrap_or(u32::MAX);
        self.out.seek(SeekFrom::Start(0))?;
        self.out
            .write_all(&wav_float_header(self.channels, self.sample_rate, bytes))?;
        self.out.seek(SeekFrom::End(0))?;
        self.out.flush()?;
        Ok(self.out)
    }
}

// ---- the input callback -------------------------------------------------------------------------

/// Converts `data` to `f32` into `out` (cleared first, never grown past its capacity) and returns the
/// peak of what was converted.
pub fn convert_chunk<T: Copy>(data: &[T], out: &mut Vec<f32>) -> f32
where
    f32: FromSample<T>,
{
    out.clear();
    let room = out.capacity().min(data.len());
    let mut peak = 0.0f32;
    for s in &data[..room] {
        let v = f32::from_sample(*s);
        peak = peak.max(v.abs());
        out.push(v);
    }
    peak
}

struct Shared {
    /// Bit pattern of the highest absolute sample since the last read (floats `>= 0` order like ints).
    peak_bits: AtomicU32,
    frames: AtomicU64,
    overruns: AtomicU64,
    stop: AtomicBool,
}

/// A recording in progress. Dropping it stops it and discards nothing: the file stays on disk.
pub struct Recorder {
    stream: Option<cpal::Stream>,
    shared: Arc<Shared>,
    writer: Option<JoinHandle<Result<WavWriter<BufWriter<File>>, String>>>,
    path: PathBuf,
    info: InputInfo,
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
    free: Receiver<Vec<f32>>,
    data: SyncSender<Vec<f32>>,
    channels: u16,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Copy,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |input: &[T], _| {
                // Long callbacks are cut into pooled chunks
                for part in input.chunks(CHUNK_SAMPLES) {
                    let Ok(mut chunk) = free.try_recv() else {
                        shared.overruns.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    let peak = convert_chunk(part, &mut chunk);
                    shared
                        .peak_bits
                        .fetch_max(peak.to_bits(), Ordering::Relaxed);
                    shared.frames.fetch_add(
                        (chunk.len() / usize::from(channels.max(1))) as u64,
                        Ordering::Relaxed,
                    );
                    if data.try_send(chunk).is_err() {
                        shared.overruns.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            |e| tracing::error!("[ERROR] audio input stream: {e}"),
            None,
        )
        .map_err(|e| format!("não foi possível abrir a entrada de áudio: {e}"))
}

impl Recorder {
    /// Starts recording the default input into `path` (a `.wav`).
    pub fn start(path: &Path) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("nenhum dispositivo de entrada de áudio")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("entrada de áudio indisponível: {e}"))?;
        let info = InputInfo {
            device: device.name().unwrap_or_else(|_| "entrada padrão".into()),
            sample_rate: supported.sample_rate().0,
            channels: supported.channels(),
        };
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("pasta de gravação: {e}"))?;
        }
        let file = File::create(path)
            .map_err(|e| format!("não foi possível criar {}: {e}", path.display()))?;
        let writer = WavWriter::new(BufWriter::new(file), info.channels, info.sample_rate)
            .map_err(|e| format!("não foi possível escrever o WAV: {e}"))?;

        let shared = Arc::new(Shared {
            peak_bits: AtomicU32::new(0),
            frames: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        });
        let (data_tx, data_rx) = sync_channel::<Vec<f32>>(CHUNKS);
        let (free_tx, free_rx) = sync_channel::<Vec<f32>>(CHUNKS);
        for _ in 0..CHUNKS {
            let _ = free_tx.try_send(Vec::with_capacity(CHUNK_SAMPLES));
        }

        let channels = info.channels;
        let stream = match format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, shared.clone(), free_rx, data_tx, channels)
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, shared.clone(), free_rx, data_tx, channels)
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &config, shared.clone(), free_rx, data_tx, channels)
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&device, &config, shared.clone(), free_rx, data_tx, channels)
            }
            other => Err(format!("formato de entrada não suportado: {other:?}")),
        }?;
        stream
            .play()
            .map_err(|e| format!("não foi possível iniciar a gravação: {e}"))?;

        let writer_thread = std::thread::Builder::new()
            .name("neoveg-record-writer".into())
            .spawn(move || {
                let mut writer = writer;
                // Ends when the stream (the only sender) is dropped and the queue is empty
                while let Ok(chunk) = data_rx.recv() {
                    writer
                        .write_samples(&chunk)
                        .map_err(|e| format!("falha ao gravar o arquivo: {e}"))?;
                    let _ = free_tx.try_send(chunk);
                }
                Ok(writer)
            })
            .map_err(|e| format!("thread de gravação: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            shared,
            writer: Some(writer_thread),
            path: path.to_owned(),
            info,
        })
    }

    pub fn info(&self) -> &InputInfo {
        &self.info
    }

    /// Frames captured so far.
    pub fn frames(&self) -> u64 {
        self.shared.frames.load(Ordering::Relaxed)
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.info.sample_rate.max(1))
    }

    /// Highest absolute sample since the last call (a real input level, `0.0..=1.0`+).
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.shared.peak_bits.swap(0, Ordering::Relaxed))
    }

    /// Stops, finishes the file and returns what was recorded.
    pub fn stop(mut self) -> Result<Recording, String> {
        self.shared.stop.store(true, Ordering::Relaxed);
        // Dropping the stream drops the callback and with it the only sender
        drop(self.stream.take());
        let writer = self
            .writer
            .take()
            .ok_or("a gravação já tinha terminado")?
            .join()
            .map_err(|_| "a thread de gravação falhou".to_string())??;
        let frames = writer.frames();
        writer
            .finish()
            .map_err(|e| format!("não foi possível finalizar o WAV: {e}"))?;
        Ok(Recording {
            path: self.path.clone(),
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            frames,
            overruns: self.shared.overruns.load(Ordering::Relaxed),
        })
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        drop(self.stream.take());
        if let Some(writer) = self.writer.take() {
            if let Ok(Ok(w)) = writer.join() {
                let _ = w.finish();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waveform::WaveformSource;
    use std::io::Cursor;

    #[test]
    fn the_header_describes_a_float_wav() {
        let h = wav_float_header(2, 48_000, 960);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 36 + 960);
        assert_eq!(u16::from_le_bytes(h[20..22].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(h[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 48_000);
        assert_eq!(
            u32::from_le_bytes(h[28..32].try_into().unwrap()),
            48_000 * 8
        );
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 960);
    }

    #[test]
    fn a_written_file_reads_back_through_the_waveform_loader() {
        let mut writer = WavWriter::new(Cursor::new(Vec::new()), 2, 48_000).unwrap();
        let tone: Vec<f32> = (0..4800)
            .flat_map(|n| {
                let v = (n as f32 * 0.05).sin() * 0.5;
                [v, -v]
            })
            .collect();
        writer.write_samples(&tone[..4000]).unwrap();
        writer.write_samples(&tone[4000..]).unwrap();
        assert_eq!(writer.frames(), 4800);
        let bytes = writer.finish().unwrap().into_inner();
        assert_eq!(bytes.len(), 44 + 4800 * 8);
        let source = WaveformSource::from_wav_bytes(bytes).expect("a valid WAV");
        assert_eq!(source.frame_count(), 4800);
        assert_eq!(source.sample_rate, 48_000);
    }

    #[test]
    fn conversion_measures_the_peak_and_never_grows_the_chunk() {
        let mut chunk = Vec::with_capacity(4);
        let peak = convert_chunk(&[0i16, 16384, -32768, 100, 200, 300], &mut chunk);
        assert_eq!(chunk.len(), 4, "capped at the pooled capacity");
        assert!((peak - 1.0).abs() < 1e-4, "{peak}");
        assert!((chunk[1] - 0.5).abs() < 1e-3);
        let peak = convert_chunk(&[0.25f32, -0.75], &mut chunk);
        assert_eq!(chunk, vec![0.25, -0.75]);
        assert_eq!(peak, 0.75);
        assert_eq!(convert_chunk::<f32>(&[], &mut chunk), 0.0);
        assert!(chunk.is_empty());
    }

    #[test]
    fn positive_floats_compare_like_their_bit_patterns() {
        // The peak meter relies on fetch_max over bit patterns
        let values = [0.0f32, 1e-9, 0.001, 0.5, 0.999, 1.0, 3.5];
        for pair in values.windows(2) {
            assert!(pair[0].to_bits() < pair[1].to_bits());
        }
    }

    #[test]
    fn a_recording_reports_its_length() {
        let r = Recording {
            path: PathBuf::from("a.wav"),
            sample_rate: 48_000,
            channels: 2,
            frames: 96_000,
            overruns: 0,
        };
        assert!((r.seconds() - 2.0).abs() < 1e-9);
    }
}
