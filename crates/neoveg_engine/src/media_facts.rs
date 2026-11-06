//! What the Media Properties window and the status bar say about media: the audio and video
//! properties of one file or of several at once, and the length of an event while its edge is being
//! trimmed. Text only, pure and tested without a window.

use neoveg_common::timecode::seconds_to_timecode;

/// The facts the program knows about one media file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaFacts {
    pub name: String,
    /// Length in seconds.
    pub duration: f64,
    /// Picture size and frame rate; `None` for sound only.
    pub video: Option<(u32, u32, f64)>,
    /// Sample rate and channel count; `None` for a file with no sound.
    pub audio: Option<(u32, u16)>,
    pub file_bytes: Option<u64>,
}

/// `1.5 KB`, `3.2 MB`, `1.1 GB`.
pub fn size_label(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn channels_label(channels: u16) -> String {
    match channels {
        1 => "mono".into(),
        2 => "estéreo".into(),
        n => format!("{n} canais"),
    }
}

/// The lines for one file.
pub fn single_lines(m: &MediaFacts, timecode_fps: f64) -> Vec<String> {
    let mut lines = vec![format!(
        "Duração: {} ({:.2} s)",
        seconds_to_timecode(m.duration, timecode_fps.max(1.0), false),
        m.duration
    )];
    if let Some((w, h, fps)) = m.video {
        lines.push(format!("Vídeo: {w}×{h} a {fps:.3} quadros por segundo"));
    }
    if let Some((rate, channels)) = m.audio {
        lines.push(format!("Áudio: {rate} Hz, {}", channels_label(channels)));
    }
    if m.video.is_none() && m.audio.is_none() {
        lines.push("Sem imagem nem som conhecidos".into());
    }
    if let Some(bytes) = m.file_bytes {
        lines.push(format!("Tamanho do arquivo: {}", size_label(bytes)));
    }
    lines
}

/// `value` when every item agrees, else `"variados"`.
fn common<T: PartialEq + Copy>(
    values: impl Iterator<Item = T>,
    show: impl Fn(T) -> String,
) -> String {
    let all: Vec<T> = values.collect();
    match all.split_first() {
        Some((first, rest)) if rest.iter().all(|v| v == first) => show(*first),
        Some(_) => "variados".into(),
        None => "nenhum".into(),
    }
}

/// The lines for several files at once: how many, the total length, and for each property the
/// value they share or "variados".
pub fn multiple_lines(items: &[MediaFacts]) -> Vec<String> {
    if items.len() == 1 {
        return single_lines(&items[0], items[0].video.map_or(25.0, |v| v.2));
    }
    let total: f64 = items.iter().map(|m| m.duration).sum();
    let mut lines = vec![
        format!("{} mídias selecionadas", items.len()),
        format!("Duração total: {total:.2} s"),
    ];
    let videos: Vec<_> = items.iter().filter_map(|m| m.video).collect();
    if !videos.is_empty() {
        lines.push(format!(
            "Vídeo ({} arquivos): tamanho {}; taxa {}",
            videos.len(),
            common(videos.iter().map(|v| (v.0, v.1)), |(w, h)| format!(
                "{w}×{h}"
            )),
            common(
                videos.iter().map(|v| (v.2 * 1000.0).round() as i64),
                |f| format!("{:.3} qps", f as f64 / 1000.0)
            ),
        ));
    }
    let audios: Vec<_> = items.iter().filter_map(|m| m.audio).collect();
    if !audios.is_empty() {
        lines.push(format!(
            "Áudio ({} arquivos): {}; {}",
            audios.len(),
            common(audios.iter().map(|a| a.0), |r| format!("{r} Hz")),
            common(audios.iter().map(|a| a.1), channels_label),
        ));
    }
    let bytes: u64 = items.iter().filter_map(|m| m.file_bytes).sum();
    if bytes > 0 {
        lines.push(format!("Tamanho total: {}", size_label(bytes)));
    }
    lines
}

/// The status text while an event edge is trimmed: the new length and how it changed.
pub fn trim_length_label(
    new_duration: f64,
    old_duration: f64,
    fps: f64,
    drop_frame: bool,
) -> String {
    let delta = new_duration - old_duration;
    let change = if delta.abs() < 0.0005 {
        String::new()
    } else {
        format!(" ({delta:+.2} s)")
    };
    format!(
        "Comprimento do evento: {} · {:.2} s{change}",
        seconds_to_timecode(new_duration.max(0.0), fps.max(1.0), drop_frame),
        new_duration
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(name: &str, w: u32, h: u32, fps: f64, d: f64) -> MediaFacts {
        MediaFacts {
            name: name.into(),
            duration: d,
            video: Some((w, h, fps)),
            audio: Some((48_000, 2)),
            file_bytes: Some(2 * 1024 * 1024),
        }
    }

    #[test]
    fn one_file_lists_length_picture_sound_and_size() {
        let lines = single_lines(&video("a.mp4", 1920, 1080, 25.0, 10.5), 25.0);
        assert_eq!(lines[0], "Duração: 00:00:10:12 (10.50 s)");
        assert_eq!(lines[1], "Vídeo: 1920×1080 a 25.000 quadros por segundo");
        assert_eq!(lines[2], "Áudio: 48000 Hz, estéreo");
        assert_eq!(lines[3], "Tamanho do arquivo: 2.0 MB");
        let sound = MediaFacts {
            duration: 3.0,
            audio: Some((44_100, 1)),
            ..MediaFacts::default()
        };
        let lines = single_lines(&sound, 25.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], "Áudio: 44100 Hz, mono");
        assert!(single_lines(&MediaFacts::default(), 25.0)[1].contains("Sem imagem"));
    }

    #[test]
    fn several_files_show_the_shared_value_or_varied() {
        let same = multiple_lines(&[
            video("a", 1920, 1080, 25.0, 4.0),
            video("b", 1920, 1080, 25.0, 6.0),
        ]);
        assert_eq!(same[0], "2 mídias selecionadas");
        assert_eq!(same[1], "Duração total: 10.00 s");
        assert_eq!(
            same[2],
            "Vídeo (2 arquivos): tamanho 1920×1080; taxa 25.000 qps"
        );
        assert_eq!(same[3], "Áudio (2 arquivos): 48000 Hz; estéreo");
        assert_eq!(same[4], "Tamanho total: 4.0 MB");
        let mixed = multiple_lines(&[
            video("a", 1920, 1080, 25.0, 4.0),
            video("b", 1280, 720, 30.0, 6.0),
        ]);
        assert_eq!(
            mixed[2],
            "Vídeo (2 arquivos): tamanho variados; taxa variados"
        );
        // One file is the single-file listing
        assert_eq!(
            multiple_lines(&[video("a", 1920, 1080, 25.0, 4.0)]),
            single_lines(&video("a", 1920, 1080, 25.0, 4.0), 25.0)
        );
    }

    #[test]
    fn the_trim_label_gives_the_new_length_and_what_changed() {
        assert_eq!(
            trim_length_label(3.5, 5.0, 25.0, false),
            "Comprimento do evento: 00:00:03:12 · 3.50 s (-1.50 s)"
        );
        assert_eq!(
            trim_length_label(5.0, 5.0, 25.0, false),
            "Comprimento do evento: 00:00:05:00 · 5.00 s"
        );
        assert!(trim_length_label(-1.0, 5.0, 25.0, false).contains("00:00:00:00"));
        assert_eq!(size_label(512), "512 B");
        assert_eq!(size_label(1536), "1.5 KB");
    }
}
