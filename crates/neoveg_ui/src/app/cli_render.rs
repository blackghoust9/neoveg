//! `neoveg --render <projeto> <saida>`: opens a project and renders it without a window.

use super::*;
use std::path::Path;
use crate::render_dialog::RenderLaunchRequest;

/// Renders `project` into `output` (the format comes from the extension) and returns when the
/// file is written. The progress goes to the standard output.
pub(crate) fn run(project: &Path, output: &Path) -> Result<(), String> {
    let format = crate::cli::format_for_output(output)
        .ok_or_else(|| tr_args("formato de saída desconhecido: {}", &[&output.display()]))?;
    let ctx = egui::Context::default();
    let mut state = AppState::new();
    if !state.load_project_file(project, None, &ctx) {
        return Err(state.status.clone());
    }
    if state.timeline.duration() <= 1e-3 {
        return Err(tr("O projeto não tem eventos para renderizar.").to_string());
    }
    let settings = state.project.settings.clone();
    let request = RenderLaunchRequest {
        output_path: output.to_path_buf(),
        format,
        width: settings.width,
        height: settings.height,
        fps: settings.fps,
        crf: 18,
        engine: Default::default(),
        burn_captions: false,
        loop_region_only: false,
        save_loudness_log: false,
    };
    state.start_render(request, &ctx);
    let Some(job) = state.render_dialog.active_job.as_ref() else {
        return Err(state.status.clone());
    };
    let mut last = String::new();
    loop {
        let status = job.status.lock().map_err(|e| e.to_string())?.clone();
        let line = format!(
            "{} {}/{}",
            status.phase, status.current_frame, status.total_frames
        );
        if line != last {
            println!("{line}");
            last = line;
        }
        if status.finished {
            return match status.error {
                Some(e) => Err(e),
                None => Ok(()),
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mono 16-bit WAV of `seconds` of a quiet tone at 8 kHz.
    fn write_wav(path: &Path, seconds: u32) {
        let rate = 8_000u32;
        let samples = rate * seconds;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + samples * 2).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(samples * 2).to_le_bytes());
        for i in 0..samples {
            let v = ((i as f32 * 0.05).sin() * 8000.0) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn a_project_renders_to_wav_without_a_window() {
        let dir = std::env::temp_dir().join(format!("neoveg_cli_render_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("tom.wav");
        write_wav(&wav, 1);

        let mut s = AppState::new();
        s.add_track(TrackKind::Audio);
        let track = s.selected_track.unwrap();
        s.project.media_pool.push(MediaRef {
            subproject: false,
            format: None,
            id: 1,
            path: wav.to_string_lossy().into_owned(),
            duration_seconds: 1.0,
        });
        let event = TrackEvent {
            id: s.timeline.alloc_id(),
            media_id: Some(1),
            start: 0.0,
            duration: 1.0,
            ..Default::default()
        };
        s.history.execute(
            &mut s.timeline,
            EditCommand::AddEvent {
                track_id: track,
                event,
            },
        );
        let project = dir.join("p.neoveg");
        neoveg_engine::project_file::save(&project, &s.project, &s.timeline).unwrap();

        let output = dir.join("saida.wav");
        run(&project, &output).unwrap();
        assert_eq!(&std::fs::read(&output).unwrap()[..4], b"RIFF");
        assert!(run(&project, &dir.join("saida.xyz")).is_err());
        assert!(run(&dir.join("nao_existe.neoveg"), &output).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
