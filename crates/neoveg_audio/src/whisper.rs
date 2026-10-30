//! Pure-Rust automatic speech-to-subtitles transcription via OpenAI Whisper (`candle-transformers`).
//!
//! Loads local Hugging Face Whisper model folders containing `config.json`, `tokenizer.json`, and
//! either `model.safetensors` (FP32/FP16/BF16 converted to `f32`) or quantized `.gguf` weights,
//! mixes or resamples timeline audio to 16 kHz mono, computes Slaney-normalized log-mel
//! spectrograms, and runs timestamp-guided encoder-decoder inference on a background worker thread.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self, Config};

use crate::mixer::{self, MixClip};

/// Whisper's native audio sample rate in Hz.
pub const WHISPER_SAMPLE_RATE: u32 = whisper::SAMPLE_RATE as u32;

/// Selectable transcription languages (`code`, human-readable label).
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("pt", "Português"),
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("zh", "中文"),
    ("ru", "Русский"),
];

/// One timed subtitle segment produced by Whisper.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
}

/// Renders active timeline [`MixClip`]s between `start_s` and `end_s` directly into a 16 kHz mono
/// buffer suitable for Whisper inference.
pub fn mix_to_mono_16k(clips: &[MixClip], start_s: f64, end_s: f64) -> Vec<f32> {
    let start_s = start_s.max(0.0);
    let end_s = end_s.max(start_s);
    let total_samples = ((end_s - start_s) * WHISPER_SAMPLE_RATE as f64).round() as usize;
    let mut out = Vec::with_capacity(total_samples);
    let inv_rate = 1.0 / WHISPER_SAMPLE_RATE as f64;
    for i in 0..total_samples {
        let t = start_s + i as f64 * inv_rate;
        let (l, r) = mixer::mix_frame(clips, t);
        out.push(((l + r) * 0.5_f32).clamp(-1.0_f32, 1.0_f32));
    }
    out
}

/// Resamples a mono `f32` slice from `source_rate` Hz to 16 kHz using linear interpolation.
pub fn resample_mono_to_16k(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 {
        return Vec::new();
    }
    if source_rate == WHISPER_SAMPLE_RATE {
        return samples.to_vec();
    }
    let ratio = source_rate as f64 / WHISPER_SAMPLE_RATE as f64;
    let out_len = ((samples.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        if idx >= samples.len() {
            break;
        }
        let s0 = samples[idx];
        let s1 = samples.get(idx + 1).copied().unwrap_or(s0);
        let frac = (pos - idx as f64) as f32;
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

fn hz_to_mel_slaney(freq_hz: f64) -> f64 {
    const F_MIN: f64 = 0.0;
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    const MIN_LOG_MEL: f64 = (MIN_LOG_HZ - F_MIN) / F_SP;
    let logstep = 6.4_f64.ln() / 27.0;
    if freq_hz >= MIN_LOG_HZ {
        MIN_LOG_MEL + (freq_hz / MIN_LOG_HZ).ln() / logstep
    } else {
        (freq_hz - F_MIN) / F_SP
    }
}

fn mel_to_hz_slaney(mel: f64) -> f64 {
    const F_MIN: f64 = 0.0;
    const F_SP: f64 = 200.0 / 3.0;
    const MIN_LOG_HZ: f64 = 1000.0;
    const MIN_LOG_MEL: f64 = (MIN_LOG_HZ - F_MIN) / F_SP;
    let logstep = 6.4_f64.ln() / 27.0;
    if mel >= MIN_LOG_MEL {
        MIN_LOG_HZ * (logstep * (mel - MIN_LOG_MEL)).exp()
    } else {
        F_MIN + F_SP * mel
    }
}

/// Builds the Slaney-normalized triangular mel filterbank `(num_mel_bins, 1 + n_fft / 2)` used by
/// OpenAI Whisper (`80` bins for tiny/base/small/medium/large-v2, `128` bins for large-v3/turbo).
pub fn slaney_mel_filters(num_mel_bins: usize, n_fft: usize, sample_rate: f64) -> Vec<f32> {
    let n_freqs = 1 + n_fft / 2;
    if num_mel_bins == 0 || n_fft == 0 || sample_rate <= 0.0 {
        return Vec::new();
    }
    let fft_freqs: Vec<f64> = (0..n_freqs)
        .map(|k| k as f64 * sample_rate / n_fft as f64)
        .collect();
    let mel_min = hz_to_mel_slaney(0.0);
    let mel_max = hz_to_mel_slaney(sample_rate * 0.5);
    let n_pts = num_mel_bins + 2;
    let hz_pts: Vec<f64> = (0..n_pts)
        .map(|i| {
            let mel = mel_min + (mel_max - mel_min) * (i as f64) / ((n_pts - 1) as f64);
            mel_to_hz_slaney(mel)
        })
        .collect();

    let mut filters = vec![0.0_f32; num_mel_bins * n_freqs];
    for m in 0..num_mel_bins {
        let f_left = hz_pts[m];
        let f_center = hz_pts[m + 1];
        let f_right = hz_pts[m + 2];
        let left_span = (f_center - f_left).max(1e-12);
        let right_span = (f_right - f_center).max(1e-12);
        let enorm = 2.0 / (f_right - f_left).max(1e-12);
        for (k, &f) in fft_freqs.iter().enumerate() {
            let lower = (f - f_left) / left_span;
            let upper = (f_right - f) / right_span;
            let weight = lower.min(upper).max(0.0) * enorm;
            filters[m * n_freqs + k] = weight as f32;
        }
    }
    filters
}

fn gpt2_unicode_to_byte_map() -> HashMap<char, u8> {
    let mut direct = [false; 256];
    for b in b'!'..=b'~' {
        direct[b as usize] = true;
    }
    for b in 0xA1_u8..=0xAC_u8 {
        direct[b as usize] = true;
    }
    for b in 0xAE_u8..=0xFF_u8 {
        direct[b as usize] = true;
    }

    let mut map = HashMap::with_capacity(256);
    let mut extra = 0_u32;
    for b in 0_u8..=255_u8 {
        let ch = if direct[b as usize] {
            char::from(b)
        } else {
            let c = char::from_u32(256 + extra).unwrap_or('\u{FFFD}');
            extra += 1;
            c
        };
        map.insert(ch, b);
    }
    map
}

/// Pure-Rust loader for Hugging Face `tokenizer.json` files used by Whisper models.
#[derive(Debug, Clone)]
pub struct WhisperTokenizer {
    token_to_id: HashMap<String, u32>,
    id_to_token: HashMap<u32, String>,
    unicode_to_byte: HashMap<char, u8>,
    pub sot_token: u32,
    pub transcribe_token: u32,
    pub translate_token: Option<u32>,
    pub eot_token: u32,
    pub no_timestamps_token: u32,
    pub no_speech_tokens: Vec<u32>,
    pub timestamp_begin: u32,
}

impl WhisperTokenizer {
    /// Parses a Hugging Face `tokenizer.json` string without requiring C++ tokenizer dependencies.
    pub fn from_json_str(json: &str) -> Result<Self, String> {
        let root: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Falha ao ler tokenizer.json: {e}"))?;

        let mut token_to_id = HashMap::new();
        let mut id_to_token = HashMap::new();

        if let Some(vocab) = root
            .get("model")
            .and_then(|m| m.get("vocab"))
            .and_then(|v| v.as_object())
        {
            for (tok, val) in vocab {
                if let Some(id) = val.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    token_to_id.insert(tok.clone(), id);
                    id_to_token.insert(id, tok.clone());
                }
            }
        }

        if let Some(added) = root.get("added_tokens").and_then(|a| a.as_array()) {
            for entry in added {
                let id = entry
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .and_then(|n| u32::try_from(n).ok());
                let content = entry.get("content").and_then(|v| v.as_str());
                if let (Some(id), Some(content)) = (id, content) {
                    token_to_id.insert(content.to_owned(), id);
                    id_to_token.insert(id, content.to_owned());
                }
            }
        }

        let req = |name: &str| -> Result<u32, String> {
            token_to_id
                .get(name)
                .copied()
                .ok_or_else(|| format!("Token obrigatório ausente em tokenizer.json: {name}"))
        };

        let sot_token = req(whisper::SOT_TOKEN)?;
        let transcribe_token = req(whisper::TRANSCRIBE_TOKEN)?;
        let translate_token = token_to_id.get(whisper::TRANSLATE_TOKEN).copied();
        let eot_token = req(whisper::EOT_TOKEN)?;
        let no_timestamps_token = req(whisper::NO_TIMESTAMPS_TOKEN)?;
        let timestamp_begin = token_to_id
            .get("<|0.00|>")
            .copied()
            .unwrap_or(no_timestamps_token.saturating_add(1));
        let no_speech_tokens = whisper::NO_SPEECH_TOKENS
            .iter()
            .filter_map(|t| token_to_id.get(*t).copied())
            .collect();

        Ok(Self {
            token_to_id,
            id_to_token,
            unicode_to_byte: gpt2_unicode_to_byte_map(),
            sot_token,
            transcribe_token,
            translate_token,
            eot_token,
            no_timestamps_token,
            no_speech_tokens,
            timestamp_begin,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let json = fs::read_to_string(path)
            .map_err(|e| format!("Não foi possível abrir {}: {e}", path.display()))?;
        Self::from_json_str(&json)
    }

    pub fn token_id(&self, token: &str) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }

    /// Looks up a Whisper language token such as `"pt"` -> `"<|pt|>"`.
    pub fn language_token(&self, lang: &str) -> Option<u32> {
        let clean = lang.trim().to_ascii_lowercase();
        let code = clean.split(['-', '_']).next().unwrap_or(&clean);
        self.token_to_id.get(&format!("<|{code}|>")).copied()
    }

    /// If `id` is a Whisper timestamp token (`<|0.00|>` ..= `<|30.00|>`), returns its offset in
    /// seconds (`0.02 s` resolution).
    pub fn timestamp_seconds(&self, id: u32) -> Option<f64> {
        if id >= self.timestamp_begin && id <= self.timestamp_begin + 1500 {
            Some((id - self.timestamp_begin) as f64 * 0.02)
        } else {
            None
        }
    }

    pub fn is_special(&self, id: u32) -> bool {
        if id >= self.eot_token {
            return true;
        }
        self.id_to_token
            .get(&id)
            .is_some_and(|s| s.starts_with("<|") && s.ends_with("|>"))
    }

    /// Decodes GPT-2 byte-level BPE token IDs into valid UTF-8 text, ignoring special/timestamp IDs.
    pub fn decode_tokens(&self, ids: &[u32]) -> String {
        let mut bytes = Vec::new();
        for &id in ids {
            if self.timestamp_seconds(id).is_some() || self.is_special(id) {
                continue;
            }
            if let Some(tok) = self.id_to_token.get(&id) {
                for ch in tok.chars() {
                    if let Some(&b) = self.unicode_to_byte.get(&ch) {
                        bytes.push(b);
                    } else {
                        let mut utf8 = [0_u8; 4];
                        bytes.extend_from_slice(ch.encode_utf8(&mut utf8).as_bytes());
                    }
                }
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

enum WhisperWeights {
    Standard(whisper::model::Whisper),
    Quantized(whisper::quantized_model::Whisper),
}

impl WhisperWeights {
    fn config(&self) -> &Config {
        match self {
            Self::Standard(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    fn reset_kv_cache(&mut self) {
        match self {
            Self::Standard(m) => m.reset_kv_cache(),
            Self::Quantized(m) => m.reset_kv_cache(),
        }
    }

    fn encode(&mut self, mel: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Standard(m) => m.encoder.forward(mel, true),
            Self::Quantized(m) => m.encoder.forward(mel, true),
        }
    }

    fn decode_last_logits(
        &mut self,
        tokens: &Tensor,
        audio_features: &Tensor,
        flush_kv_cache: bool,
    ) -> candle_core::Result<Vec<f32>> {
        let seq_len = tokens.dim(1)?;
        let logits = match self {
            Self::Standard(m) => {
                let ys = m.decoder.forward(tokens, audio_features, flush_kv_cache)?;
                m.decoder.final_linear(&ys.i((.., seq_len - 1..))?)?
            }
            Self::Quantized(m) => {
                let ys = m.decoder.forward(tokens, audio_features, flush_kv_cache)?;
                m.decoder.final_linear(&ys.i((.., seq_len - 1..))?)?
            }
        };
        logits.squeeze(0)?.squeeze(0)?.to_vec1::<f32>()
    }
}

/// Loaded Whisper model + tokenizer + Slaney mel filterbank ready for transcription.
pub struct WhisperEngine {
    weights: WhisperWeights,
    tokenizer: WhisperTokenizer,
    mel_filters: Vec<f32>,
    device: Device,
}

fn find_file_with_ext(dir: &Path, preferred: &str, ext: &str) -> Option<PathBuf> {
    let direct = dir.join(preferred);
    if direct.is_file() {
        return Some(direct);
    }
    let mut candidates: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(ext))
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

impl WhisperEngine {
    /// Loads a Whisper model folder (`config.json`, `tokenizer.json`, and `model.safetensors` or
    /// `*.gguf`). If `model_path` points to a file inside the folder, its parent directory is used.
    pub fn load_dir(model_path: &Path) -> Result<Self, String> {
        let dir = if model_path.is_file() {
            model_path.parent().unwrap_or(model_path)
        } else {
            model_path
        };
        if !dir.is_dir() {
            return Err(format!(
                "Pasta do modelo Whisper não encontrada: {}",
                dir.display()
            ));
        }

        let config_path = dir.join("config.json");
        let config_str = fs::read_to_string(&config_path).map_err(|e| {
            format!(
                "Arquivo config.json ausente em {} ({e}). Selecione uma pasta de modelo Whisper (ex.: openai/whisper-small).",
                dir.display()
            )
        })?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| format!("Falha ao interpretar config.json: {e}"))?;

        let tokenizer_path = dir.join("tokenizer.json");
        let tokenizer = WhisperTokenizer::from_file(&tokenizer_path)?;

        let device = Device::Cpu;
        let weights =
            if let Some(st_path) = find_file_with_ext(dir, "model.safetensors", "safetensors") {
                let raw_tensors = candle_core::safetensors::load(&st_path, &device)
                    .map_err(|e| format!("Falha ao carregar pesos {}: {e}", st_path.display()))?;
                let mut f32_tensors = HashMap::with_capacity(raw_tensors.len());
                for (name, tensor) in raw_tensors {
                    let converted = if tensor.dtype() == whisper::DTYPE {
                        tensor
                    } else {
                        tensor.to_dtype(whisper::DTYPE).map_err(|e| {
                            format!("Falha ao converter tensor {name} para F32: {e}")
                        })?
                    };
                    f32_tensors.insert(name, converted);
                }
                let vb = VarBuilder::from_tensors(f32_tensors, whisper::DTYPE, &device);
                let model = whisper::model::Whisper::load(&vb, config.clone())
                    .map_err(|e| format!("Falha ao montar modelo Whisper: {e}"))?;
                WhisperWeights::Standard(model)
            } else if let Some(gguf_path) = find_file_with_ext(dir, "model.gguf", "gguf") {
                let vb = whisper::quantized_model::VarBuilder::from_gguf(&gguf_path, &device)
                    .map_err(|e| format!("Falha ao carregar GGUF {}: {e}", gguf_path.display()))?;
                let model = whisper::quantized_model::Whisper::load(&vb, config.clone())
                    .map_err(|e| format!("Falha ao montar modelo Whisper quantizado: {e}"))?;
                WhisperWeights::Quantized(model)
            } else {
                return Err(format!(
                    "Nenhum arquivo de pesos (model.safetensors ou *.gguf) encontrado em {}",
                    dir.display()
                ));
            };

        let mel_filters = slaney_mel_filters(
            config.num_mel_bins,
            whisper::N_FFT,
            whisper::SAMPLE_RATE as f64,
        );

        Ok(Self {
            weights,
            tokenizer,
            mel_filters,
            device,
        })
    }

    /// Constructs an engine from already-loaded components (used by unit tests with synthetic tiny
    /// models).
    pub fn from_model_and_tokenizer(
        model: whisper::model::Whisper,
        tokenizer: WhisperTokenizer,
    ) -> Self {
        let mel_filters = slaney_mel_filters(
            model.config.num_mel_bins,
            whisper::N_FFT,
            whisper::SAMPLE_RATE as f64,
        );
        Self {
            weights: WhisperWeights::Standard(model),
            tokenizer,
            mel_filters,
            device: Device::Cpu,
        }
    }

    fn detect_language(&mut self, audio_features: &Tensor) -> Result<Option<u32>, String> {
        let candidates: Vec<u32> = SUPPORTED_LANGUAGES
            .iter()
            .filter_map(|(code, _)| self.tokenizer.language_token(code))
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }
        let sot = Tensor::new(&[self.tokenizer.sot_token], &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| e.to_string())?;
        let logits = self
            .weights
            .decode_last_logits(&sot, audio_features, true)
            .map_err(|e| e.to_string())?;
        let best = candidates.into_iter().max_by(|&a, &b| {
            let la = logits.get(a as usize).copied().unwrap_or(f32::NEG_INFINITY);
            let lb = logits.get(b as usize).copied().unwrap_or(f32::NEG_INFINITY);
            la.total_cmp(&lb)
        });
        Ok(best)
    }

    /// Transcribes 16 kHz mono PCM samples into timed subtitle segments, adding `offset_s` to all
    /// timestamps. Checks `cancel` frequently and reports normalized progress `0.0..=1.0`.
    pub fn transcribe(
        &mut self,
        samples_16k: &[f32],
        offset_s: f64,
        language: Option<&str>,
        cancel: &AtomicBool,
        mut on_progress: impl FnMut(f32),
    ) -> Result<Vec<TranscriptSegment>, String> {
        on_progress(0.0_f32);
        if samples_16k.is_empty() {
            on_progress(1.0_f32);
            return Ok(Vec::new());
        }

        let total_chunks = samples_16k.len().div_ceil(whisper::N_SAMPLES).max(1);
        let mut all_segments = Vec::new();
        let requested_lang = language
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"));

        for chunk_idx in 0..total_chunks {
            if cancel.load(Ordering::Relaxed) {
                return Err("Transcrição cancelada.".into());
            }

            let sample_start = chunk_idx * whisper::N_SAMPLES;
            let sample_end = (sample_start + whisper::N_SAMPLES).min(samples_16k.len());
            let raw_slice = &samples_16k[sample_start..sample_end];
            let chunk_duration_s = raw_slice.len() as f64 / WHISPER_SAMPLE_RATE as f64;
            let chunk_offset_s = offset_s + (sample_start as f64 / WHISPER_SAMPLE_RATE as f64);

            // Skip silent 30-second windows to avoid Whisper silence hallucinations and wasted CPU.
            let energy: f64 = raw_slice
                .iter()
                .map(|&s| (s as f64) * (s as f64))
                .sum::<f64>()
                / (raw_slice.len().max(1) as f64);
            if energy.sqrt() < 1e-4 {
                on_progress(((chunk_idx + 1) as f32) / (total_chunks as f32));
                continue;
            }

            let mut padded = vec![0.0_f32; whisper::N_SAMPLES];
            padded[..raw_slice.len()].copy_from_slice(raw_slice);

            let cfg = self.weights.config().clone();
            let mel_vec = whisper::audio::pcm_to_mel(&cfg, &padded, &self.mel_filters);
            let mel_frames = mel_vec.len() / cfg.num_mel_bins;
            let mel = Tensor::from_vec(mel_vec, (1, cfg.num_mel_bins, mel_frames), &self.device)
                .and_then(|t| t.narrow(2, 0, whisper::N_FRAMES.min(mel_frames)))
                .map_err(|e| format!("Erro ao montar espectrograma mel: {e}"))?;

            if cancel.load(Ordering::Relaxed) {
                return Err("Transcrição cancelada.".into());
            }

            self.weights.reset_kv_cache();
            let audio_features = self
                .weights
                .encode(&mel)
                .map_err(|e| format!("Erro no encoder Whisper: {e}"))?;

            let lang_token = match requested_lang {
                Some(code) => self.tokenizer.language_token(code),
                None => self.detect_language(&audio_features)?,
            };

            let mut tokens = vec![self.tokenizer.sot_token];
            if let Some(lt) = lang_token {
                tokens.push(lt);
            }
            tokens.push(self.tokenizer.transcribe_token);
            let prompt_len = tokens.len();

            let max_steps = cfg
                .max_target_positions
                .saturating_sub(prompt_len + 1)
                .min(224);

            let mut skip_chunk = false;
            for step in 0..max_steps {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Transcrição cancelada.".into());
                }

                let tokens_t = Tensor::new(tokens.as_slice(), &self.device)
                    .and_then(|t| t.unsqueeze(0))
                    .map_err(|e| format!("Erro nos tokens do decoder: {e}"))?;
                let mut logits = self
                    .weights
                    .decode_last_logits(&tokens_t, &audio_features, step == 0)
                    .map_err(|e| format!("Erro no decoder Whisper: {e}"))?;

                if step == 0 && !self.tokenizer.no_speech_tokens.is_empty() {
                    let no_speech_prob =
                        softmax_subset_prob(&logits, &self.tokenizer.no_speech_tokens);
                    if no_speech_prob > whisper::NO_SPEECH_THRESHOLD {
                        skip_chunk = true;
                        break;
                    }
                }

                self.apply_decoding_masks(&mut logits, &tokens[prompt_len..]);

                let next_token = argmax(&logits);
                if next_token == self.tokenizer.eot_token {
                    break;
                }
                tokens.push(next_token);

                if has_runaway_repetition(&tokens[prompt_len..]) {
                    break;
                }
            }

            if !skip_chunk {
                let segs = extract_segments_from_tokens(
                    &self.tokenizer,
                    &tokens[prompt_len..],
                    chunk_offset_s,
                    chunk_duration_s,
                );
                all_segments.extend(segs);
            }

            on_progress(((chunk_idx + 1) as f32) / (total_chunks as f32));
        }

        on_progress(1.0_f32);
        Ok(all_segments)
    }

    fn apply_decoding_masks(&self, logits: &mut [f32], sampled: &[u32]) {
        let cfg = self.weights.config();
        for &tok in &cfg.suppress_tokens {
            if let Some(slot) = logits.get_mut(tok as usize) {
                *slot = f32::NEG_INFINITY;
            }
        }
        for &tok in &[
            self.tokenizer.sot_token,
            self.tokenizer.transcribe_token,
            self.tokenizer.no_timestamps_token,
        ] {
            if let Some(slot) = logits.get_mut(tok as usize) {
                *slot = f32::NEG_INFINITY;
            }
        }
        if let Some(tr) = self.tokenizer.translate_token {
            if let Some(slot) = logits.get_mut(tr as usize) {
                *slot = f32::NEG_INFINITY;
            }
        }
        for &ns in &self.tokenizer.no_speech_tokens {
            if let Some(slot) = logits.get_mut(ns as usize) {
                *slot = f32::NEG_INFINITY;
            }
        }

        let ts_begin = self.tokenizer.timestamp_begin as usize;
        let ts_end = (ts_begin + 1501).min(logits.len());
        if ts_begin >= logits.len() {
            return;
        }
        for slot in logits.iter_mut().skip(ts_end) {
            *slot = f32::NEG_INFINITY;
        }

        if sampled.is_empty() {
            // First generated token of every window starts at <|0.00|>.
            for (idx, slot) in logits.iter_mut().enumerate() {
                if idx != ts_begin {
                    *slot = f32::NEG_INFINITY;
                }
            }
            return;
        }

        let last_was_ts = sampled
            .last()
            .is_some_and(|&t| t >= self.tokenizer.timestamp_begin);
        let penult_was_ts =
            sampled.len() >= 2 && sampled[sampled.len() - 2] >= self.tokenizer.timestamp_begin;

        let last_ts = sampled
            .iter()
            .rev()
            .copied()
            .find(|&t| t >= self.tokenizer.timestamp_begin);

        if last_was_ts {
            if penult_was_ts || sampled.len() == 1 {
                // Just opened a segment (`<|0.00|>` or `<|t_end|><|t_start|>`): require text or EOT.
                for slot in &mut logits[ts_begin..ts_end] {
                    *slot = f32::NEG_INFINITY;
                }
                if sampled.len() == 1 {
                    if let Some(eot) = logits.get_mut(self.tokenizer.eot_token as usize) {
                        *eot = f32::NEG_INFINITY;
                    }
                }
            } else {
                // Just closed a segment (`... text <|t_end|>`): require next `<|t_start|>` or EOT.
                let eot = self.tokenizer.eot_token as usize;
                for (idx, slot) in logits[..ts_begin].iter_mut().enumerate() {
                    if idx != eot {
                        *slot = f32::NEG_INFINITY;
                    }
                }
                if let Some(prev_ts) = last_ts {
                    let min_next = (prev_ts as usize).min(ts_end);
                    for slot in &mut logits[ts_begin..min_next] {
                        *slot = f32::NEG_INFINITY;
                    }
                }
            }
        } else if let Some(prev_ts) = last_ts {
            // Inside segment text: timestamps must strictly advance beyond `prev_ts`.
            let min_end = ((prev_ts + 1) as usize).min(ts_end);
            for slot in &mut logits[ts_begin..min_end] {
                *slot = f32::NEG_INFINITY;
            }
            // OpenAI Whisper rule: if total timestamp probability exceeds max text token prob,
            // force a timestamp so cues close cleanly.
            let (ts_prob_sum, max_text_prob) = timestamp_vs_text_probs(logits, ts_begin, ts_end);
            if ts_prob_sum > max_text_prob {
                for slot in &mut logits[..ts_begin] {
                    *slot = f32::NEG_INFINITY;
                }
            }
        }
    }
}

/// Convenience entry point for worker threads: loads the Whisper model at `model_dir`, mixes
/// `clips` to 16 kHz mono over `0.0..duration_s`, and returns timed subtitle segments.
pub fn transcribe_clips(
    model_dir: &Path,
    clips: &[MixClip],
    duration_s: f64,
    language: Option<&str>,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(f32),
) -> Result<Vec<TranscriptSegment>, String> {
    on_progress(0.02_f32);
    let mut engine = WhisperEngine::load_dir(model_dir)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Transcrição cancelada.".into());
    }
    on_progress(0.08_f32);
    let end_s = if duration_s > 0.0 {
        duration_s
    } else {
        mixer::plan_end(clips)
    };
    let samples = mix_to_mono_16k(clips, 0.0, end_s);
    engine.transcribe(&samples, 0.0, language, cancel, |p| {
        on_progress(0.08_f32 + p * 0.92_f32);
    })
}

fn argmax(logits: &[f32]) -> u32 {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(idx, _)| idx as u32)
        .unwrap_or(0)
}

fn softmax_subset_prob(logits: &[f32], subset: &[u32]) -> f64 {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !max_logit.is_finite() {
        return 0.0;
    }
    let denom: f64 = logits
        .iter()
        .filter(|&&v| v.is_finite())
        .map(|&v| ((v - max_logit) as f64).exp())
        .sum();
    if denom <= 0.0 {
        return 0.0;
    }
    let num: f64 = subset
        .iter()
        .filter_map(|&id| logits.get(id as usize))
        .filter(|&&v| v.is_finite())
        .map(|&v| ((v - max_logit) as f64).exp())
        .sum();
    num / denom
}

fn timestamp_vs_text_probs(logits: &[f32], ts_begin: usize, ts_end: usize) -> (f64, f64) {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !max_logit.is_finite() {
        return (0.0, 0.0);
    }
    let mut denom = 0.0_f64;
    let mut ts_sum = 0.0_f64;
    let mut max_text = 0.0_f64;
    for (idx, &v) in logits.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let e = ((v - max_logit) as f64).exp();
        denom += e;
        if idx >= ts_begin && idx < ts_end {
            ts_sum += e;
        } else if e > max_text {
            max_text = e;
        }
    }
    if denom <= 0.0 {
        (0.0, 0.0)
    } else {
        (ts_sum / denom, max_text / denom)
    }
}

fn has_runaway_repetition(sampled: &[u32]) -> bool {
    for pat_len in 1..=6 {
        if sampled.len() >= pat_len * 6 {
            let tail = &sampled[sampled.len() - pat_len * 6..];
            let first = &tail[..pat_len];
            if tail.chunks_exact(pat_len).all(|c| c == first) {
                return true;
            }
        }
    }
    false
}

fn extract_segments_from_tokens(
    tokenizer: &WhisperTokenizer,
    sampled: &[u32],
    chunk_offset_s: f64,
    chunk_duration_s: f64,
) -> Vec<TranscriptSegment> {
    let mut out = Vec::new();
    let mut seg_start: Option<f64> = None;
    let mut seg_tokens: Vec<u32> = Vec::new();
    let max_local = chunk_duration_s.max(0.1);

    let push_seg =
        |out: &mut Vec<TranscriptSegment>, t0_local: f64, t1_local: f64, tokens: &[u32]| {
            let text = tokenizer.decode_tokens(tokens).trim().to_owned();
            if text.is_empty() {
                return;
            }
            let start = chunk_offset_s + t0_local.clamp(0.0, max_local);
            let end = (chunk_offset_s + t1_local.clamp(0.0, max_local)).max(start + 0.12);
            out.push(TranscriptSegment {
                start_s: start,
                end_s: end,
                text,
            });
        };

    for &id in sampled {
        if let Some(ts) = tokenizer.timestamp_seconds(id) {
            if !seg_tokens.is_empty() {
                push_seg(&mut out, seg_start.unwrap_or(0.0), ts, &seg_tokens);
                seg_tokens.clear();
                seg_start = None;
            } else {
                seg_start = Some(ts);
            }
        } else if !tokenizer.is_special(id) {
            seg_tokens.push(id);
        }
    }

    if !seg_tokens.is_empty() {
        push_seg(&mut out, seg_start.unwrap_or(0.0), max_local, &seg_tokens);
    }

    out
}

#[cfg(test)]
mod tests {
    use crate::pcm::PcmStore;

    use neoveg_common::fade::FadeCurve;

    use super::*;

    fn synthetic_tokenizer_json() -> String {
        // Encode UTF-8 bytes of " Olá ação!" through the exact GPT-2 byte->unicode table.
        let inv = gpt2_unicode_to_byte_map();
        let mut byte_to_ch = ['\0'; 256];
        for (ch, b) in inv {
            byte_to_ch[b as usize] = ch;
        }
        let encode_gpt2 = |s: &str| -> String {
            s.as_bytes()
                .iter()
                .map(|&b| byte_to_ch[b as usize])
                .collect()
        };

        let ola = encode_gpt2(" Olá");
        let acao = encode_gpt2(" ação!");

        serde_json::json!({
            "model": {
                "vocab": {
                    ola: 10,
                    acao: 11,
                    "a": 12
                }
            },
            "added_tokens": [
                { "id": 50, "content": "<|endoftext|>" },
                { "id": 51, "content": "<|startoftranscript|>" },
                { "id": 52, "content": "<|pt|>" },
                { "id": 53, "content": "<|en|>" },
                { "id": 54, "content": "<|transcribe|>" },
                { "id": 55, "content": "<|translate|>" },
                { "id": 56, "content": "<|nospeech|>" },
                { "id": 57, "content": "<|notimestamps|>" },
                { "id": 58, "content": "<|0.00|>" }
            ]
        })
        .to_string()
    }

    #[test]
    fn slaney_mel_filterbank_has_expected_shape_non_negative_weights_and_peak_order() {
        let filters_80 = slaney_mel_filters(80, whisper::N_FFT, whisper::SAMPLE_RATE as f64);
        assert_eq!(filters_80.len(), 80 * 201);
        assert!(filters_80.iter().all(|&w| w >= 0.0 && w.is_finite()));

        let filters_128 = slaney_mel_filters(128, whisper::N_FFT, whisper::SAMPLE_RATE as f64);
        assert_eq!(filters_128.len(), 128 * 201);

        let mut prev_peak = 0_usize;
        for bin in 0..80 {
            let row = &filters_80[bin * 201..(bin + 1) * 201];
            let (peak_idx, &peak_val) = row
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap();
            assert!(peak_val > 0.0, "mel bin {bin} should have positive energy");
            assert!(
                peak_idx >= prev_peak,
                "mel peaks must increase monotonically ({peak_idx} < {prev_peak})"
            );
            prev_peak = peak_idx;
        }
    }

    #[test]
    fn gpt2_byte_tokenizer_decodes_multibyte_portuguese_utf8_and_parses_timestamps() {
        let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
        assert_eq!(tok.language_token("pt"), Some(52));
        assert_eq!(tok.language_token("pt-BR"), Some(52));
        assert_eq!(tok.timestamp_seconds(58), Some(0.0));
        assert_eq!(tok.timestamp_seconds(58 + 125), Some(2.5));

        let decoded = tok.decode_tokens(&[58, 10, 11, 58 + 100, 50]);
        assert_eq!(decoded, " Olá ação!");

        let segs = extract_segments_from_tokens(
            &tok,
            &[58, 10, 11, 58 + 90, 58 + 100, 10, 58 + 150],
            5.0,
            4.0,
        );
        assert_eq!(segs.len(), 2);
        assert!((segs[0].start_s - 5.0).abs() < 1e-6);
        assert!((segs[0].end_s - 6.8).abs() < 1e-6);
        assert_eq!(segs[0].text, "Olá ação!");
        assert!((segs[1].start_s - 7.0).abs() < 1e-6);
        assert!((segs[1].end_s - 8.0).abs() < 1e-6);
        assert_eq!(segs[1].text, "Olá");
    }

    #[test]
    fn mix_to_mono_16k_renders_timeline_clips_at_16khz() {
        let clip = MixClip {
            samples: PcmStore::shared(vec![0.5_f32; 48_000 * 2]),
            source_rate: 48_000,
            start: 0.25,
            duration: 0.5,
            trim_in: 0.0,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: FadeCurve::Linear,
            fade_out_curve: FadeCurve::Linear,
            cross_in: 0.0,
            cross_in_curve: FadeCurve::Linear,
            cross_out: 0.0,
            cross_out_curve: FadeCurve::Linear,
            track: 0,
            loop_len: 0.0,
            channels: Default::default(),
        };
        let mono = mix_to_mono_16k(&[clip], 0.0, 1.0);
        assert_eq!(mono.len(), 16_000);
        assert!(mono[100].abs() < 1e-6);
        assert!((mono[8_000] - 0.5_f32).abs() < 1e-4);
        assert!(mono[15_000].abs() < 1e-6);
    }

    #[test]
    fn synthetic_tiny_whisper_model_runs_encoder_decoder_and_honors_cancellation() {
        let tok = WhisperTokenizer::from_json_str(&synthetic_tokenizer_json()).unwrap();
        let cfg = Config {
            num_mel_bins: 80,
            max_source_positions: 1500,
            d_model: 16,
            encoder_attention_heads: 2,
            encoder_layers: 1,
            vocab_size: 1600,
            max_target_positions: 16,
            decoder_attention_heads: 2,
            decoder_layers: 1,
            suppress_tokens: vec![],
        };
        let vb = VarBuilder::zeros(whisper::DTYPE, &Device::Cpu);
        let model = whisper::model::Whisper::load(&vb, cfg).unwrap();
        let mut engine = WhisperEngine::from_model_and_tokenizer(model, tok);

        // 0.5 s of 440 Hz tone at 16 kHz (above silence threshold).
        let samples: Vec<f32> = (0..8_000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 16_000.0).sin() * 0.4_f32)
            .collect();

        let cancel = AtomicBool::new(false);
        let mut last_progress = 0.0_f32;
        let res = engine
            .transcribe(&samples, 1.5, Some("pt"), &cancel, |p| {
                last_progress = p;
            })
            .unwrap();
        assert!((last_progress - 1.0_f32).abs() < 1e-6);
        // Zero weights produce deterministic tokens without panicking; segments if non-empty stay >= offset.
        for seg in &res {
            assert!(seg.start_s >= 1.5);
            assert!(seg.end_s > seg.start_s);
        }

        // Cancelled run stops immediately with Err.
        cancel.store(true, Ordering::Relaxed);
        let err = engine.transcribe(&samples, 0.0, Some("pt"), &cancel, |_| {});
        assert!(err.is_err());
    }
}
