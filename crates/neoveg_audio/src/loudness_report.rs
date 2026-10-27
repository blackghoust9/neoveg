//! Offline whole-project BS.1770-4 / EBU R128 loudness measurement, `<name>_loud.txt` sidecar log
//! generation, and pure-Rust stereo RIFF WAVE mixdown export.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::dsp::loudness::LoudnessMeter;
use crate::fxmixer::{BlockMixer, MAX_BLOCK};
use crate::readout::LoudnessSnapshot;

/// One 1-second row in a `<name>_loud.txt` loudness log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessTimelineRow {
    pub time_seconds: f64,
    pub momentary_lufs: f64,
    pub short_term_lufs: f64,
    pub true_peak_dbtp: f64,
}

/// Complete offline loudness report for a timeline range `[start_seconds, end_seconds]`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoudnessReport {
    pub sample_rate: u32,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub summary: LoudnessSnapshot,
    pub rows: Vec<LoudnessTimelineRow>,
}

fn fmt_val(v: f64, unit: &str) -> String {
    if v.is_finite() {
        format!("{v:+.1} {unit}")
    } else {
        format!("-inf {unit}")
    }
}

/// Computes `<stem>_loud.txt` next to `target_path` (e.g. `filme.mp4` -> `filme_loud.txt`).
pub fn sidecar_loudness_path(target_path: &Path) -> PathBuf {
    let stem = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("projeto");
    let filename = format!("{stem}_loud.txt");
    match target_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(filename),
        _ => PathBuf::from(filename),
    }
}

/// Measures the offline loudness of `mixer` over `[start_seconds, end_seconds]` without playing to
/// hardware. Includes every track FX chain, CLAP plug-in, PDC alignment, automation curve, ducking,
/// and master bus FX chain configured on `mixer`.
pub fn measure_mixer_offline(
    mixer: &mut BlockMixer,
    sample_rate: u32,
    start_seconds: f64,
    end_seconds: f64,
    master_gain: f32,
) -> LoudnessReport {
    let rate = sample_rate.max(8_000);
    let rate_f = f64::from(rate);
    let t0 = start_seconds.max(0.0);
    let t1 = end_seconds.max(t0);
    let total_frames = ((t1 - t0) * rate_f).round() as usize;
    let step = 1.0 / rate_f;

    let mut meter = LoudnessMeter::new(rate_f, 2);
    let mut rows = Vec::new();
    if total_frames == 0 {
        return LoudnessReport {
            sample_rate: rate,
            start_seconds: t0,
            end_seconds: t1,
            summary: LoudnessSnapshot::SILENT,
            rows,
        };
    }

    let g = if master_gain.is_finite() {
        master_gain.max(0.0)
    } else {
        1.0
    };
    mixer.prime(t0, step);

    let mut interleaved = [0.0_f32; MAX_BLOCK * 2];
    let mut done = 0usize;
    let mut next_row_frames = rate as usize;

    while done < total_frames {
        let n = (total_frames - done).min(MAX_BLOCK);
        let base_t = t0 + done as f64 * step;
        mixer.begin(n);
        for i in 0..n {
            mixer.add_frame(i, base_t + i as f64 * step);
        }
        mixer.finish(n);
        let (ol, or) = mixer.out();
        for i in 0..n {
            interleaved[i * 2] = ol[i] * g;
            interleaved[i * 2 + 1] = or[i] * g;
        }
        meter.process_interleaved(&interleaved[..n * 2]);
        done += n;

        if done >= next_row_frames || done == total_frames {
            rows.push(LoudnessTimelineRow {
                time_seconds: t0 + done as f64 * step,
                momentary_lufs: meter.momentary(),
                short_term_lufs: meter.short_term(),
                true_peak_dbtp: meter.true_peak_dbtp(),
            });
            while next_row_frames <= done {
                next_row_frames += rate as usize;
            }
        }
    }

    let summary = LoudnessSnapshot {
        momentary_lufs: meter.momentary(),
        short_term_lufs: meter.short_term(),
        integrated_lufs: meter.integrated(),
        range_lu: meter.loudness_range(),
        true_peak_dbtp: meter.true_peak_dbtp(),
        max_momentary_lufs: meter.max_momentary(),
        max_short_term_lufs: meter.max_short_term(),
    };

    LoudnessReport {
        sample_rate: rate,
        start_seconds: t0,
        end_seconds: t1,
        summary,
        rows,
    }
}

/// Formats `report` as a human-readable `<name>_loud.txt` loudness log (ITU-R BS.1770-4 / EBU R128).
pub fn format_loudness_log(title: &str, report: &LoudnessReport) -> String {
    let s = &report.summary;
    let duration = (report.end_seconds - report.start_seconds).max(0.0);
    let mut out = String::new();
    out.push_str("Neoveg — Relatório de Loudness (ITU-R BS.1770-4 / EBU R128)\n");
    out.push_str("===========================================================\n");
    out.push_str(&format!("Projeto / Arquivo : {title}\n"));
    out.push_str(&format!(
        "Taxa de amostragem: {} Hz (Estéreo)\n",
        report.sample_rate
    ));
    out.push_str(&format!(
        "Intervalo medido  : {:.3} s .. {:.3} s (duração {:.3} s)\n\n",
        report.start_seconds, report.end_seconds, duration
    ));
    out.push_str("Resumo do Programa:\n");
    out.push_str(&format!(
        "  Integrado (I)        : {}\n",
        fmt_val(s.integrated_lufs, "LUFS")
    ));
    out.push_str(&format!("  Faixa de Loudness    : {:.1} LU\n", s.range_lu));
    out.push_str(&format!(
        "  Pico Verdadeiro Máx. : {}\n",
        fmt_val(s.true_peak_dbtp, "dBTP")
    ));
    out.push_str(&format!(
        "  Momentâneo Máx. (M)  : {}\n",
        fmt_val(s.max_momentary_lufs, "LUFS")
    ));
    out.push_str(&format!(
        "  Curto Prazo Máx. (S) : {}\n\n",
        fmt_val(s.max_short_term_lufs, "LUFS")
    ));
    out.push_str("Histórico por segundo:\n");
    out.push_str("  Tempo (s) |   Momentâneo (M) |  Curto Prazo (S) | Pico Acum. (TP)\n");
    out.push_str("  ----------+------------------+------------------+----------------\n");
    for row in &report.rows {
        out.push_str(&format!(
            "  {:>9.2} | {:>16} | {:>16} | {:>15}\n",
            row.time_seconds,
            fmt_val(row.momentary_lufs, "LUFS"),
            fmt_val(row.short_term_lufs, "LUFS"),
            fmt_val(row.true_peak_dbtp, "dBTP"),
        ));
    }
    out
}

/// Saves `report` to `log_path` (`UTF-8`, LF line endings).
pub fn save_loudness_log(
    log_path: &Path,
    title: &str,
    report: &LoudnessReport,
) -> Result<(), String> {
    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
        }
    }
    let text = format_loudness_log(title, report);
    std::fs::write(log_path, text)
        .map_err(|e| format!("falha ao gravar relatório {}: {e}", log_path.display()))
}

/// Renders `mixer` offline over `[start_seconds, end_seconds]` into a stereo 16-bit PCM `.wav`
/// file while simultaneously measuring its BS.1770-4 loudness report.
pub fn render_mixer_to_wav_pcm16(
    mixer: &mut BlockMixer,
    wav_path: &Path,
    sample_rate: u32,
    start_seconds: f64,
    end_seconds: f64,
    master_gain: f32,
) -> Result<LoudnessReport, String> {
    if let Some(parent) = wav_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("falha ao criar pasta {}: {e}", parent.display()))?;
        }
    }
    let rate = sample_rate.max(8_000);
    let rate_f = f64::from(rate);
    let t0 = start_seconds.max(0.0);
    let t1 = end_seconds.max(t0);
    let total_frames = ((t1 - t0) * rate_f).round() as usize;
    let step = 1.0 / rate_f;

    let data_bytes = (total_frames as u64) * 4;
    if data_bytes > u64::from(u32::MAX - 44) {
        return Err("duração excede o limite de 4 GiB do formato WAV padrão".into());
    }
    let data_len = data_bytes as u32;
    let riff_len = 36u32.saturating_add(data_len);

    let file = std::fs::File::create(wav_path)
        .map_err(|e| format!("falha ao criar WAV {}: {e}", wav_path.display()))?;
    let mut writer = std::io::BufWriter::new(file);

    // 44-byte canonical stereo 16-bit PCM RIFF/WAVE header.
    let byte_rate = rate * 4;
    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_len.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM = 1
    header[22..24].copy_from_slice(&2u16.to_le_bytes()); // 2 channels
    header[24..28].copy_from_slice(&rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&4u16.to_le_bytes()); // block align
    header[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_len.to_le_bytes());
    writer
        .write_all(&header)
        .map_err(|e| format!("falha ao escrever cabeçalho WAV: {e}"))?;

    let mut meter = LoudnessMeter::new(rate_f, 2);
    let mut rows = Vec::new();
    let g = if master_gain.is_finite() {
        master_gain.max(0.0)
    } else {
        1.0
    };
    if total_frames > 0 {
        mixer.prime(t0, step);
    }

    let mut interleaved = [0.0_f32; MAX_BLOCK * 2];
    let mut pcm_bytes = [0u8; MAX_BLOCK * 4];
    let mut done = 0usize;
    let mut next_row_frames = rate as usize;

    while done < total_frames {
        let n = (total_frames - done).min(MAX_BLOCK);
        let base_t = t0 + done as f64 * step;
        mixer.begin(n);
        for i in 0..n {
            mixer.add_frame(i, base_t + i as f64 * step);
        }
        mixer.finish(n);
        let (ol, or) = mixer.out();
        for i in 0..n {
            let l = ol[i] * g;
            let r = or[i] * g;
            interleaved[i * 2] = l;
            interleaved[i * 2 + 1] = r;
            let li = (l.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            let ri = (r.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            let b = i * 4;
            pcm_bytes[b..b + 2].copy_from_slice(&li.to_le_bytes());
            pcm_bytes[b + 2..b + 4].copy_from_slice(&ri.to_le_bytes());
        }
        writer
            .write_all(&pcm_bytes[..n * 4])
            .map_err(|e| format!("falha ao gravar amostras WAV: {e}"))?;
        meter.process_interleaved(&interleaved[..n * 2]);
        done += n;

        if done >= next_row_frames || done == total_frames {
            rows.push(LoudnessTimelineRow {
                time_seconds: t0 + done as f64 * step,
                momentary_lufs: meter.momentary(),
                short_term_lufs: meter.short_term(),
                true_peak_dbtp: meter.true_peak_dbtp(),
            });
            while next_row_frames <= done {
                next_row_frames += rate as usize;
            }
        }
    }

    writer
        .flush()
        .map_err(|e| format!("falha ao finalizar WAV: {e}"))?;

    let summary = if total_frames == 0 {
        LoudnessSnapshot::SILENT
    } else {
        LoudnessSnapshot {
            momentary_lufs: meter.momentary(),
            short_term_lufs: meter.short_term(),
            integrated_lufs: meter.integrated(),
            range_lu: meter.loudness_range(),
            true_peak_dbtp: meter.true_peak_dbtp(),
            max_momentary_lufs: meter.max_momentary(),
            max_short_term_lufs: meter.max_short_term(),
        }
    };

    Ok(LoudnessReport {
        sample_rate: rate,
        start_seconds: t0,
        end_seconds: t1,
        summary,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fxmixer::FxPlan;
    use crate::mixer::MixClip;
    use std::sync::Arc;

    fn sine_clip(freq: f64, amp: f64, rate: u32, seconds: f64) -> MixClip {
        let frames = (f64::from(rate) * seconds) as usize;
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let v = (amp * (2.0 * std::f64::consts::PI * freq * i as f64 / f64::from(rate)).sin())
                as f32;
            samples.push(v);
            samples.push(v);
        }
        MixClip {
            samples: Arc::new(samples),
            source_rate: rate,
            start: 0.0,
            duration: seconds,
            gain: 1.0,
            track: 0,
            ..Default::default()
        }
    }

    #[test]
    fn sidecar_path_appends_loud_txt_suffix() {
        assert_eq!(
            sidecar_loudness_path(Path::new("/renders/master_cut.mp4")),
            PathBuf::from("/renders/master_cut_loud.txt")
        );
        assert_eq!(
            sidecar_loudness_path(Path::new("episodio01.neoveg")),
            PathBuf::from("episodio01_loud.txt")
        );
    }

    #[test]
    fn offline_measurement_and_wav_bounce_match_bs1770_calibration_and_write_loud_txt() {
        let rate = 48_000u32;
        let clip = sine_clip(997.0, 0.1, rate, 4.0);
        let mut mixer = BlockMixer::new(vec![clip], &FxPlan::default(), rate as f32);

        let dir = std::env::temp_dir().join(format!("neoveg_loud_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("mixdown.wav");

        let report = render_mixer_to_wav_pcm16(&mut mixer, &wav, rate, 0.0, 4.0, 1.0).unwrap();
        assert!((report.summary.integrated_lufs - -20.0).abs() < 0.35);
        assert!((report.summary.true_peak_dbtp - -20.0).abs() < 0.5);
        assert_eq!(report.rows.len(), 4);

        let loud_txt = sidecar_loudness_path(&wav);
        save_loudness_log(&loud_txt, "mixdown.wav", &report).unwrap();
        let log_content = std::fs::read_to_string(&loud_txt).unwrap();
        assert!(log_content.contains("ITU-R BS.1770-4 / EBU R128"));
        assert!(log_content.contains("Integrado (I)"));
        assert!(log_content.contains("-20.0 LUFS"));

        // Verify the rendered WAV decodes back via Symphonia with the expected frame count.
        let decoded = crate::decode::decode_file(&wav).unwrap();
        assert_eq!(decoded.sample_rate, rate);
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.frame_count(), 48_000 * 4);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
