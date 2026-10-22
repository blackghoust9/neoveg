//! Neoveg Rust Core CLI & FFI Verification Tool
use neoveg_core::{
    analyze_wav_peaks, compile_ffmpeg_filtergraph, detect_auto_crossfades, split_event_at,
    FadeCurve, Timecode, TimelineEvent,
};
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    match cmd {
        "status" => {
            println!(
                r#"{{"ok":true,"engine":"neoveg-core","version":"2.0.0","ffi":"cdylib+cdecl","features":["smpte_timecode","vegas_5_curve_fades","auto_crossfade_solver","pcm_wav_dsp","event_split_cut","ffmpeg_filtergraph_compiler"]}}"#
            );
        }
        "smpte" => {
            let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let fps: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(29.97);
            println!("{}", Timecode::format_smpte(secs, fps, true));
        }
        "analyze-wav" => {
            let Some(path_str) = args.get(2) else {
                eprintln!(r#"{{"ok":false,"error":"Missing WAV path"}}"#);
                std::process::exit(1);
            };
            let buckets: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(128);
            match analyze_wav_peaks(Path::new(path_str), buckets) {
                Ok(res) => {
                    let peaks_str: Vec<String> =
                        res.peaks.iter().map(|p| format!("{:.3}", p)).collect();
                    println!(
                        r#"{{"ok":true,"sample_rate":{},"channels":{},"duration_sec":{:.4},"peak_db":{:.2},"rms_db":{:.2},"peaks":[{}]}}"#,
                        res.sample_rate,
                        res.channels,
                        res.duration_sec,
                        res.peak_db,
                        res.rms_db,
                        peaks_str.join(",")
                    );
                }
                Err(e) => {
                    eprintln!(r#"{{"ok":false,"error":"{}"}}"#, e);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            let ev = TimelineEvent {
                id: "clip_1".into(),
                name: "Demo".into(),
                media_path: "demo.mp4".into(),
                media_type: "video".into(),
                track_index: 0,
                start: 0.0,
                duration: 5.0,
                trim_in: 0.0,
                fade_in: 0.5,
                fade_out: 0.5,
                fade_in_curve: FadeCurve::Smooth,
                fade_out_curve: FadeCurve::Smooth,
                gain: 1.0,
                playback_rate: 1.0,
                muted: false,
            };
            let mut list = vec![ev.clone()];
            let _ = detect_auto_crossfades(&mut list);
            let _ = split_event_at(&ev, 2.5, "clip_2");
            let fg = compile_ffmpeg_filtergraph(&list, 1920, 1080, 29.97);
            println!("Neoveg Rust Core Ready. Filtergraph preview: {}", fg);
        }
    }
}
