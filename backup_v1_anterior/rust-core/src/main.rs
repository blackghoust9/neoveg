//! OpenVegas Core CLI & IPC Bridge
//!
//! Commands:
//!   openvegas-core status
//!   openvegas-core smpte <seconds> <fps>
//!   openvegas-core analyze-wav <wav_path> [buckets]
//!   openvegas-core demo-filtergraph

use openvegas_core::{
    analyze_wav_peaks, compile_ffmpeg_filtergraph, detect_auto_crossfades, FadeCurve, Timecode,
    TimelineEvent,
};
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    match cmd {
        "status" => {
            println!(
                r#"{{"engine":"openvegas-core","language":"Rust","version":"1.0.0","features":["smpte_timecode","vegas_5_curve_fades","auto_crossfade_solver","pcm_wav_dsp","ffmpeg_filtergraph_compiler"]}}"#
            );
        }
        "smpte" => {
            let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let fps: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(29.97);
            let tc = Timecode::format_smpte(secs, fps, true);
            let mb = Timecode::format_measures_beats(secs, 120.0, 4);
            println!(
                r#"{{"seconds":{:.4},"fps":{:.2},"smpte":"{}","measures_beats":"{}"}}"#,
                secs, fps, tc, mb
            );
        }
        "analyze-wav" => {
            let Some(wav_path) = args.get(2) else {
                eprintln!(r#"{{"error":"Missing wav_path argument"}}"#);
                std::process::exit(1);
            };
            let buckets: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(128);
            match analyze_wav_peaks(Path::new(wav_path), buckets) {
                Ok(res) => {
                    let peaks_str: Vec<String> =
                        res.peaks.iter().map(|p| format!("{:.3}", p)).collect();
                    println!(
                        r#"{{"sample_rate":{},"channels":{},"duration_sec":{:.4},"peak_db":{:.2},"rms_db":{:.2},"peaks":[{}]}}"#,
                        res.sample_rate,
                        res.channels,
                        res.duration_sec,
                        res.peak_db,
                        res.rms_db,
                        peaks_str.join(",")
                    );
                }
                Err(err) => {
                    eprintln!(r#"{{"error":"{}"}}"#, err.replace('"', "'"));
                    std::process::exit(1);
                }
            }
        }
        "demo-filtergraph" => {
            let mut sample_events = vec![
                TimelineEvent {
                    id: "clip_1".into(),
                    name: "Intro Scene".into(),
                    media_path: "scene1.mp4".into(),
                    media_type: "video".into(),
                    track_index: 0,
                    start: 0.0,
                    duration: 5.0,
                    trim_in: 0.0,
                    fade_in: 0.5,
                    fade_out: 1.0,
                    fade_in_curve: FadeCurve::Smooth,
                    fade_out_curve: FadeCurve::Smooth,
                    gain: 1.0,
                    playback_rate: 1.0,
                    muted: false,
                },
                TimelineEvent {
                    id: "clip_2".into(),
                    name: "Main B-Roll".into(),
                    media_path: "scene2.mp4".into(),
                    media_type: "video".into(),
                    track_index: 0,
                    start: 4.0,
                    duration: 6.0,
                    trim_in: 0.0,
                    fade_in: 1.0,
                    fade_out: 0.5,
                    fade_in_curve: FadeCurve::Smooth,
                    fade_out_curve: FadeCurve::Fast,
                    gain: 1.0,
                    playback_rate: 1.0,
                    muted: false,
                },
            ];
            let xfades = detect_auto_crossfades(&mut sample_events);
            let fg = compile_ffmpeg_filtergraph(&sample_events, 1920, 1080, 30.0);
            println!(
                r#"{{"crossfades_detected":{},"filter_complex":"{}"}}"#,
                xfades.len(),
                fg.replace('"', "\\\"")
            );
        }
        _ => {
            eprintln!(r#"{{"error":"Unknown command"}}"#);
            std::process::exit(1);
        }
    }
}
