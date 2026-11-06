//! Audio recording from an input device (the default one, or the one the user chose) into a 32-bit
//! float WAV file, with optional monitoring: what is being recorded is also played on the default
//! output, through a lock-free ring, so the performer hears it.
//!
//! The input callback never allocates, locks or does I/O: it converts the samples into one of a
//! fixed pool of preallocated chunks and hands it to a writer thread through a bounded channel;
//! the writer returns each chunk to the pool after writing it. If the disk cannot keep up the
//! chunks run out and the overrun is counted (reported when recording stops) instead of blocking.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
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
    /// The armed track target for the recording (`None`: untracked or default).
    pub track_id: Option<u64>,
}

impl Recording {
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate.max(1))
    }
}

/// How a recording is made.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordOptions {
    /// The input device by name; `None` (or a name that is gone) is the default device.
    pub device: Option<String>,
    /// Play what is recorded on the default output while recording.
    pub monitor: bool,
    /// Gain of the monitor, `0.0..=2.0`.
    pub monitor_volume: f32,
    /// The armed track target for the recording (`None`: untracked or default).
    pub track_id: Option<u64>,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            device: None,
            monitor: false,
            monitor_volume: 1.0,
            track_id: None,
        }
    }
}

/// What happened to the monitor of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorStatus {
    Off,
    On,
    /// Asked for, but not possible (the reason).
    Unavailable(String),
}

/// Names of the input devices of the default host, the default one first.
pub fn list_inputs() -> Vec<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut names: Vec<String> = host
        .input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    names.sort();
    names.dedup();
    if let Some(default) = default {
        names.retain(|n| *n != default);
        names.insert(0, default);
    }
    names
}

/// The device called `name`, or the default input when `name` is `None` or no longer there.
fn input_device(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if let Some(name) = name {
        if let Some(found) = host
            .input_devices()
            .ok()
            .and_then(|mut d| d.find(|d| d.name().is_ok_and(|n| n == name)))
        {
            return Ok(found);
        }
    }
    host.default_input_device()
        .ok_or_else(|| "nenhum dispositivo de entrada de áudio".to_string())
}

/// The input the recording would use (the device called `name`, else the default one).
pub fn probe_input(name: Option<&str>) -> Result<InputInfo, String> {
    let device = input_device(name)?;
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

// ---- the monitor --------------------------------------------------------------------------------

/// Samples the ring holds (power of two): about 0.7 s of stereo at 48 kHz.
const RING_SAMPLES: usize = 1 << 16;
/// Most channels the monitor carries.
const MONITOR_MAX_CHANNELS: usize = 8;

/// A single-producer single-consumer ring of `f32` samples: the input callback writes, the output
/// callback reads, neither waits for the other.
pub struct MonitorRing {
    data: Vec<AtomicU32>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl MonitorRing {
    pub fn new() -> Self {
        Self {
            data: (0..RING_SAMPLES).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Samples waiting to be read.
    pub fn available(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Acquire))
    }

    /// Writes as many of `samples` as fit and returns how many were taken.
    pub fn push(&self, samples: &[f32]) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let free = RING_SAMPLES - head.wrapping_sub(self.tail.load(Ordering::Acquire));
        let n = samples.len().min(free);
        for (i, s) in samples[..n].iter().enumerate() {
            self.data[head.wrapping_add(i) % RING_SAMPLES].store(s.to_bits(), Ordering::Relaxed);
        }
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Reads up to `out.len()` samples and returns how many were read.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let n = out
            .len()
            .min(self.head.load(Ordering::Acquire).wrapping_sub(tail));
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = f32::from_bits(
                self.data[tail.wrapping_add(i) % RING_SAMPLES].load(Ordering::Relaxed),
            );
        }
        self.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    /// Throws away all but the newest `keep` samples (the monitor must stay close to the live sound).
    pub fn skip_to_latest(&self, keep: usize) {
        let available = self.available();
        if available > keep {
            let tail = self.tail.load(Ordering::Relaxed);
            self.tail
                .store(tail.wrapping_add(available - keep), Ordering::Release);
        }
    }
}

impl Default for MonitorRing {
    fn default() -> Self {
        Self::new()
    }
}

/// Puts one input frame on an output frame of another channel count: mono is heard on every
/// channel, two channels into one are averaged, otherwise channels go to the same numbers.
pub fn map_frame(input: &[f32], out: &mut [f32], gain: f32) {
    match (input.len(), out.len()) {
        (0, _) => out.fill(0.0),
        (1, _) => out.fill(input[0] * gain),
        (_, 1) => out[0] = (input[0] + input[1]) * 0.5 * gain,
        _ => {
            for (c, o) in out.iter_mut().enumerate() {
                *o = input.get(c).copied().unwrap_or(0.0) * gain;
            }
        }
    }
}

fn build_monitor_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    ring: Arc<MonitorRing>,
    in_channels: usize,
    gain: f32,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let out_channels = usize::from(config.channels).max(1);
    // Far enough behind the live sound to be steady, no more than ~85 ms of it
    let keep = in_channels * 4096;
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _| {
                ring.skip_to_latest(keep);
                let mut frame = [0.0f32; MONITOR_MAX_CHANNELS];
                let mut mapped = [0.0f32; MONITOR_MAX_CHANNELS];
                for chunk in out.chunks_mut(out_channels) {
                    let got = ring.pop(&mut frame[..in_channels]);
                    let n = chunk.len().min(MONITOR_MAX_CHANNELS);
                    if got == in_channels {
                        map_frame(&frame[..in_channels], &mut mapped[..n], gain);
                    } else {
                        mapped[..n].fill(0.0);
                    }
                    for (o, v) in chunk.iter_mut().zip(&mapped[..n]) {
                        *o = T::from_sample(*v);
                    }
                }
            },
            |e| tracing::error!("[ERROR] audio monitor stream: {e}"),
            None,
        )
        .map_err(|e| format!("não foi possível abrir a saída do monitor: {e}"))
}

/// Opens the monitor on the default output for an input of `in_channels` at `rate`.
fn open_monitor(
    ring: Arc<MonitorRing>,
    rate: u32,
    in_channels: u16,
    gain: f32,
) -> Result<cpal::Stream, String> {
    if usize::from(in_channels) > MONITOR_MAX_CHANNELS {
        return Err("entrada com canais demais para monitorar".into());
    }
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("nenhum dispositivo de saída de áudio")?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("saída de áudio indisponível: {e}"))?;
    if supported.sample_rate().0 != rate {
        return Err(format!(
            "a entrada ({rate} Hz) e a saída ({} Hz) têm taxas diferentes",
            supported.sample_rate().0
        ));
    }
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = usize::from(in_channels);
    let stream = match format {
        SampleFormat::F32 => build_monitor_stream::<f32>(&device, &config, ring, channels, gain),
        SampleFormat::I16 => build_monitor_stream::<i16>(&device, &config, ring, channels, gain),
        SampleFormat::U16 => build_monitor_stream::<u16>(&device, &config, ring, channels, gain),
        SampleFormat::I32 => build_monitor_stream::<i32>(&device, &config, ring, channels, gain),
        other => Err(format!("formato de saída não suportado: {other:?}")),
    }?;
    stream
        .play()
        .map_err(|e| format!("não foi possível iniciar o monitor: {e}"))?;
    Ok(stream)
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
    /// The monitor's output stream, while there is one.
    monitor_stream: Option<cpal::Stream>,
    monitor: MonitorStatus,
    track_id: Option<u64>,
    monitor_ring: Option<Arc<MonitorRing>>,
    data_sender: Option<SyncSender<Vec<f32>>>,
    free_receiver: Option<Receiver<Vec<f32>>>,
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
    free: Receiver<Vec<f32>>,
    data: SyncSender<Vec<f32>>,
    channels: u16,
    monitor: Option<Arc<MonitorRing>>,
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
                    if let Some(ring) = &monitor {
                        ring.push(&chunk);
                    }
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
    /// Starts recording the input chosen in `options` into `path` (a `.wav`).
    pub fn start(path: &Path, options: &RecordOptions) -> Result<Self, String> {
        let device = input_device(options.device.as_deref())?;
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
        // The monitor is a bonus: when it cannot be had the recording goes on without it
        let mut monitor = MonitorStatus::Off;
        let mut monitor_stream = None;
        let mut ring = None;
        if options.monitor {
            let shared_ring = Arc::new(MonitorRing::new());
            match open_monitor(
                shared_ring.clone(),
                info.sample_rate,
                info.channels,
                options.monitor_volume.clamp(0.0, 2.0),
            ) {
                Ok(stream) => {
                    monitor_stream = Some(stream);
                    monitor = MonitorStatus::On;
                    ring = Some(shared_ring);
                }
                Err(reason) => monitor = MonitorStatus::Unavailable(reason),
            }
        }
        let stream = match format {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                &config,
                shared.clone(),
                free_rx,
                data_tx,
                channels,
                ring.clone(),
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                &config,
                shared.clone(),
                free_rx,
                data_tx,
                channels,
                ring.clone(),
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                &config,
                shared.clone(),
                free_rx,
                data_tx,
                channels,
                ring.clone(),
            ),
            SampleFormat::I32 => build_stream::<i32>(
                &device,
                &config,
                shared.clone(),
                free_rx,
                data_tx,
                channels,
                ring.clone(),
            ),
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
            monitor_stream,
            monitor,
            track_id: options.track_id,
            monitor_ring: ring,
            data_sender: None,
            free_receiver: None,
        })
    }

    /// Creates a mock loopback recorder for headless environments.
    /// It records synthetic audio frames to a temporary WAV file, maintains peak levels,
    /// frame counts, and pushes to a live memory [`MonitorRing`].
    pub fn create_mock_loopback() -> Self {
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = temp_dir.join(format!("neoveg_mock_loopback_{timestamp}.wav"));
        let options = RecordOptions {
            device: Some("Mock Loopback Device".to_string()),
            monitor: true,
            monitor_volume: 1.0,
            track_id: Some(1),
        };
        Self::create_mock_loopback_with(&path, &options)
            .expect("mock loopback recorder initialization")
    }

    /// Creates a mock loopback recorder at the given `path` with `options`.
    pub fn create_mock_loopback_with(path: &Path, options: &RecordOptions) -> Result<Self, String> {
        let sample_rate = 48_000;
        let channels = 2;
        let info = InputInfo {
            device: options
                .device
                .clone()
                .unwrap_or_else(|| "Mock Loopback Device".into()),
            sample_rate,
            channels,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("pasta de gravação: {e}"))?;
        }
        let file = File::create(path)
            .map_err(|e| format!("não foi possível criar {}: {e}", path.display()))?;
        let writer = WavWriter::new(BufWriter::new(file), channels, sample_rate)
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

        let monitor_ring = if options.monitor {
            Some(Arc::new(MonitorRing::new()))
        } else {
            None
        };
        let monitor = if options.monitor {
            MonitorStatus::On
        } else {
            MonitorStatus::Off
        };

        let writer_thread = std::thread::Builder::new()
            .name("neoveg-mock-record-writer".into())
            .spawn(move || {
                let mut writer = writer;
                while let Ok(chunk) = data_rx.recv() {
                    writer
                        .write_samples(&chunk)
                        .map_err(|e| format!("falha ao gravar o arquivo: {e}"))?;
                    let _ = free_tx.try_send(chunk);
                }
                Ok(writer)
            })
            .map_err(|e| format!("thread de gravação mock: {e}"))?;

        let mut rec = Self {
            stream: None,
            shared,
            writer: Some(writer_thread),
            path: path.to_owned(),
            info,
            monitor_stream: None,
            monitor,
            track_id: options.track_id,
            monitor_ring,
            data_sender: Some(data_tx),
            free_receiver: Some(free_rx),
        };

        // Feed an initial chunk of synthetic audio (960 stereo frames = 20ms of 440 Hz tone)
        let mut initial_samples = Vec::with_capacity(1920);
        for i in 0..960 {
            let sample = (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48000.0).sin() * 0.5;
            initial_samples.push(sample);
            initial_samples.push(sample);
        }
        rec.push_mock_samples(&initial_samples);

        Ok(rec)
    }

    /// Feeds mock samples directly into the recording and monitor pipelines.
    pub fn push_mock_samples(&mut self, samples: &[f32]) {
        if self.shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let channels = self.info.channels.max(1);
        for part in samples.chunks(CHUNK_SAMPLES) {
            let chunk = if let Some(free_rx) = &self.free_receiver {
                match free_rx.try_recv() {
                    Ok(mut c) => {
                        convert_chunk(part, &mut c);
                        c
                    }
                    Err(_) => {
                        self.shared.overruns.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                }
            } else {
                let mut c = Vec::with_capacity(part.len());
                convert_chunk(part, &mut c);
                c
            };

            let peak = chunk.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
            if let Some(ring) = &self.monitor_ring {
                ring.push(&chunk);
            }
            self.shared
                .peak_bits
                .fetch_max(peak.to_bits(), Ordering::Relaxed);
            self.shared.frames.fetch_add(
                (chunk.len() / usize::from(channels)) as u64,
                Ordering::Relaxed,
            );
            if let Some(data_tx) = &self.data_sender {
                if data_tx.try_send(chunk).is_err() {
                    self.shared.overruns.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// Whether the recording is being monitored.
    pub fn monitor(&self) -> &MonitorStatus {
        &self.monitor
    }

    /// Status of the monitor.
    pub fn monitor_status(&self) -> MonitorStatus {
        self.monitor.clone()
    }

    /// The live monitor ring buffer, if monitoring is enabled.
    pub fn monitor_ring(&self) -> Option<Arc<MonitorRing>> {
        self.monitor_ring.clone()
    }

    /// The armed track target id associated with this recording session, if any.
    pub fn track_id(&self) -> Option<u64> {
        self.track_id
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
        drop(self.monitor_stream.take());
        drop(self.data_sender.take());
        drop(self.free_receiver.take());
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
            track_id: self.track_id,
        })
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        drop(self.stream.take());
        drop(self.monitor_stream.take());
        drop(self.data_sender.take());
        drop(self.free_receiver.take());
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
            track_id: None,
        };
        assert!((r.seconds() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_ring_keeps_order_wraps_around_and_never_takes_more_than_it_holds() {
        let ring = MonitorRing::new();
        assert_eq!(ring.available(), 0);
        assert_eq!(ring.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 2];
        assert_eq!(ring.pop(&mut out), 2);
        assert_eq!(out, [1.0, 2.0]);
        let mut rest = [0.0; 4];
        assert_eq!(ring.pop(&mut rest), 1, "only what was written");
        assert_eq!(rest[0], 3.0);
        // Many more samples than the ring holds, a little at a time: the order survives the wrap
        let mut next = 0.0f32;
        let mut expected = 0.0f32;
        for _ in 0..(RING_SAMPLES / 100 * 3) {
            let block: Vec<f32> = (0..100)
                .map(|_| {
                    next += 1.0;
                    next
                })
                .collect();
            assert_eq!(ring.push(&block), 100);
            let mut back = [0.0; 100];
            assert_eq!(ring.pop(&mut back), 100);
            for v in back {
                expected += 1.0;
                assert_eq!(v, expected);
            }
        }
        // A full ring refuses the rest
        let big = vec![0.5; RING_SAMPLES + 10];
        assert_eq!(ring.push(&big), RING_SAMPLES);
        assert_eq!(ring.push(&[1.0]), 0);
    }

    #[test]
    fn the_monitor_skips_old_sound_to_stay_close_to_the_live_one() {
        let ring = MonitorRing::new();
        let block: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        ring.push(&block);
        ring.skip_to_latest(100);
        assert_eq!(ring.available(), 100);
        let mut out = [0.0; 1];
        ring.pop(&mut out);
        assert_eq!(out[0], 900.0, "the newest 100 are kept");
        ring.skip_to_latest(500);
        assert_eq!(
            ring.available(),
            99,
            "nothing is thrown away when it is close enough"
        );
    }

    #[test]
    fn a_frame_is_mapped_to_the_channels_of_the_output() {
        let mut out = [9.0; 2];
        map_frame(&[0.5], &mut out, 1.0);
        assert_eq!(out, [0.5, 0.5], "mono is heard on both sides");
        map_frame(&[0.2, 0.4], &mut out, 0.5);
        assert_eq!(out, [0.1, 0.2]);
        let mut mono = [9.0; 1];
        map_frame(&[0.2, 0.6], &mut mono, 1.0);
        assert!(
            (mono[0] - 0.4).abs() < 1e-6,
            "two channels into one are averaged"
        );
        let mut six = [9.0; 6];
        map_frame(&[1.0, -1.0], &mut six, 1.0);
        assert_eq!(six, [1.0, -1.0, 0.0, 0.0, 0.0, 0.0]);
        map_frame(&[], &mut out, 1.0);
        assert_eq!(out, [0.0, 0.0]);
    }

    #[test]
    fn the_default_options_record_the_default_input_without_monitor() {
        let options = RecordOptions::default();
        assert!(options.device.is_none() && !options.monitor && options.track_id.is_none());
        assert_eq!(options.monitor_volume, 1.0);
        // A list of devices is always a list (empty on a machine with none), default first
        let names = list_inputs();
        let mut sorted = names.clone();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len(), "no name twice");
    }

    #[test]
    fn interrupted_recording_workflow_with_input_selection_track_arming_and_monitoring() {
        // 1. Input device selection & probing
        let _inputs = list_inputs();
        let _probed = probe_input(None);

        let chosen_device = "Mock Loopback Audio Input".to_string();
        let target_track_id = 42u64;

        // 2. Track arming and record options configuration
        let mut options = RecordOptions::default();
        options.device = Some(chosen_device.clone());
        options.monitor = true;
        options.monitor_volume = 1.2;
        options.track_id = Some(target_track_id);

        assert_eq!(options.device.as_deref(), Some("Mock Loopback Audio Input"));
        assert!(options.monitor);
        assert_eq!(options.monitor_volume, 1.2);
        assert_eq!(options.track_id, Some(42));

        // 3. Start recording with mock loopback and live memory ring buffer monitoring
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!(
            "test_recording_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut recorder = Recorder::create_mock_loopback_with(&temp_file, &options)
            .expect("failed to create mock loopback recorder");

        // Verify device and track association
        assert_eq!(recorder.info().device, chosen_device);
        assert_eq!(recorder.info().sample_rate, 48_000);
        assert_eq!(recorder.info().channels, 2);
        assert_eq!(recorder.track_id(), Some(target_track_id));
        assert_eq!(recorder.monitor_status(), MonitorStatus::On);

        // Verify memory ring buffer monitoring
        let ring = recorder
            .monitor_ring()
            .expect("monitor ring should be present");
        assert!(
            ring.available() > 0,
            "initial synthetic samples populated the ring buffer"
        );

        let mut monitor_buf = [0.0f32; 256];
        let read = ring.pop(&mut monitor_buf);
        assert!(read > 0, "read samples from live monitor ring");
        assert!(
            monitor_buf.iter().any(|&s| s != 0.0),
            "monitored audio is non-silent"
        );

        // Feed dynamic test samples (e.g. 1000 stereo samples of known peak 0.85)
        let mut test_samples = Vec::new();
        for _ in 0..500 {
            test_samples.push(0.85f32);
            test_samples.push(-0.85f32);
        }
        recorder.push_mock_samples(&test_samples);

        // Verify live peak metering
        let peak = recorder.take_peak();
        assert!(
            (peak - 0.85).abs() < 1e-4,
            "peak was accurately tracked: {peak}"
        );
        // Peak is reset after taking
        assert_eq!(recorder.take_peak(), 0.0, "peak is reset after take_peak");

        // Verify frames and duration elapsed
        assert!(recorder.frames() >= 1460); // 960 initial + 500 added
        assert!(recorder.seconds() > 0.0);

        // Stop recording
        let recording = recorder.stop().expect("failed to stop recording cleanly");
        assert_eq!(recording.track_id, Some(target_track_id));
        assert_eq!(recording.sample_rate, 48_000);
        assert_eq!(recording.channels, 2);
        assert_eq!(recording.overruns, 0);
        assert!(recording.frames >= 1460);
        assert!(recording.path.exists());

        // Verify written WAV file through WaveformSource parser
        let wav_bytes = std::fs::read(&recording.path).expect("read recorded file");
        let source = WaveformSource::from_wav_bytes(wav_bytes).expect("WAV must be valid");
        assert_eq!(source.sample_rate, 48_000);
        assert_eq!(source.frame_count() as u64, recording.frames);

        // Clean up temp file
        let _ = std::fs::remove_file(&recording.path);

        // 4. Also verify default parameterless create_mock_loopback()
        let loopback = Recorder::create_mock_loopback();
        assert_eq!(loopback.monitor_status(), MonitorStatus::On);
        assert!(loopback.frames() > 0);
        let path = loopback.path.clone();
        let res = loopback.stop().expect("stop default mock loopback");
        assert!(res.frames > 0);
        let _ = std::fs::remove_file(path);
    }
}
