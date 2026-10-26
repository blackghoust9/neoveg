//! Final Cut Pro 7 XML (`xmeml` version 5) interchange: export of the timeline and import of the
//! clips of a sequence into the media that is already in the project.
//!
//! The format is the public interchange XML understood by Premiere Pro, DaVinci Resolve and Final
//! Cut Pro 7; this is Neoveg's own code on it. Only what Neoveg models is written (tracks, clip
//! placement, source in / out, names and media paths); fades, effects and transitions are not.

use crate::command::EditCommand;
use crate::edl::{ImportReport, MediaMatch};
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// A media file of the project, as the export needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlMedia {
    pub id: u64,
    pub name: String,
    pub path: String,
    /// Length of the media, seconds.
    pub duration: f64,
}

/// Frame timebase of `fps` (25, 30, 24...) and whether the rate is the NTSC one (x1000/1001).
fn timebase(fps: f64) -> (u32, bool) {
    let rounded = fps.round().max(1.0);
    let ntsc = (fps - rounded).abs() > 1e-3;
    (rounded as u32, ntsc)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// `C:\Videos\a b.mp4` -> `file://localhost/C%3a/Videos/a%20b.mp4`.
pub fn path_to_url(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let mut out = String::from("file://localhost");
    if !unified.starts_with('/') {
        out.push('/');
    }
    for (i, c) in unified.chars().enumerate() {
        match c {
            ':' if i == 1 => out.push_str("%3a"),
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3f"),
            c => out.push(c),
        }
    }
    out
}

/// The reverse of [`path_to_url`]: decodes percent escapes and drops the `file://` prefix. A Windows
/// drive (`/C:/x`) loses its leading slash.
pub fn url_to_path(url: &str) -> String {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://"))
        .unwrap_or(url);
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                decoded.push(v);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    let text = String::from_utf8_lossy(&decoded).into_owned();
    let b = text.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() {
        text[1..].to_owned()
    } else {
        text
    }
}

fn frames(seconds: f64, fps: f64) -> i64 {
    (seconds * fps).round() as i64
}

fn rate_xml(out: &mut String, indent: &str, base: u32, ntsc: bool) {
    out.push_str(&format!(
        "{indent}<rate><timebase>{base}</timebase><ntsc>{}</ntsc></rate>\n",
        if ntsc { "TRUE" } else { "FALSE" }
    ));
}

/// Writes the timeline as FCP7 XML. `media` describes the project media (events whose media is not
/// listed are written without a file reference).
pub fn export(timeline: &Timeline, media: &[XmlMedia], name: &str, fps: f64) -> String {
    let (base, ntsc) = timebase(fps);
    // Frames are counted at the exact rate (e.g. 29.97), as the timebase says
    let exact = if ntsc {
        f64::from(base) * 1000.0 / 1001.0
    } else {
        f64::from(base)
    };
    let duration_frames = frames(
        timeline
            .tracks
            .iter()
            .flat_map(|t| t.events.iter().map(|e| e.end()))
            .fold(0.0, f64::max),
        exact,
    );

    let mut out = String::new();
    out.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n<xmeml version=\"5\">\n",
    );
    out.push_str(&format!(
        "  <sequence id=\"sequence-1\">\n    <name>{}</name>\n    <duration>{duration_frames}</duration>\n",
        escape(name)
    ));
    rate_xml(&mut out, "    ", base, ntsc);
    out.push_str("    <media>\n");

    let mut files_written: Vec<u64> = Vec::new();
    for (kind, tag) in [(TrackKind::Video, "video"), (TrackKind::Audio, "audio")] {
        out.push_str(&format!("      <{tag}>\n"));
        for track in timeline.tracks.iter().filter(|t| t.kind == kind) {
            out.push_str(&format!(
                "        <track>\n          <name>{}</name>\n",
                escape(&track.name)
            ));
            if track.muted {
                out.push_str("          <enabled>FALSE</enabled>\n");
            }
            for event in &track.events {
                clip_xml(
                    &mut out,
                    event,
                    kind,
                    media,
                    exact,
                    base,
                    ntsc,
                    &mut files_written,
                );
            }
            out.push_str("        </track>\n");
        }
        out.push_str(&format!("      </{tag}>\n"));
    }
    out.push_str("    </media>\n  </sequence>\n</xmeml>\n");
    out
}

#[allow(clippy::too_many_arguments)]
fn clip_xml(
    out: &mut String,
    event: &TrackEvent,
    kind: TrackKind,
    media: &[XmlMedia],
    fps: f64,
    base: u32,
    ntsc: bool,
    files_written: &mut Vec<u64>,
) {
    let start = frames(event.start, fps);
    let end = frames(event.end(), fps).max(start + 1);
    let trim_in = frames(event.trim_in, fps);
    let trim_out = trim_in + (end - start);
    out.push_str(&format!(
        "          <clipitem id=\"clipitem-{}\">\n            <name>{}</name>\n            <enabled>TRUE</enabled>\n            <start>{start}</start>\n            <end>{end}</end>\n            <in>{trim_in}</in>\n            <out>{trim_out}</out>\n",
        event.id,
        escape(&event.name)
    ));
    rate_xml(out, "            ", base, ntsc);
    if let Some(m) = event
        .media_id
        .and_then(|id| media.iter().find(|m| m.id == id))
    {
        if files_written.contains(&m.id) {
            out.push_str(&format!("            <file id=\"file-{}\"/>\n", m.id));
        } else {
            files_written.push(m.id);
            out.push_str(&format!(
                "            <file id=\"file-{}\">\n              <name>{}</name>\n              <pathurl>{}</pathurl>\n              <duration>{}</duration>\n",
                m.id,
                escape(&m.name),
                escape(&path_to_url(&m.path)),
                frames(m.duration, fps)
            ));
            rate_xml(out, "              ", base, ntsc);
            let kind_tag = if kind == TrackKind::Video {
                "<video><samplecharacteristics><width>1920</width><height>1080</height></samplecharacteristics></video>"
            } else {
                "<audio><channelcount>2</channelcount></audio>"
            };
            out.push_str(&format!(
                "              <media>{kind_tag}</media>\n            </file>\n"
            ));
        }
    }
    if kind == TrackKind::Audio {
        out.push_str("            <sourcetrack><mediatype>audio</mediatype><trackindex>1</trackindex></sourcetrack>\n");
    }
    out.push_str("          </clipitem>\n");
}

// ---- import ------------------------------------------------------------------------------------------

/// One clip of an imported sequence. Times are seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlClip {
    pub kind: TrackKind,
    /// Index of the track among the sequence's tracks of the same kind.
    pub track: usize,
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub source_in: f64,
    pub file_name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct XmlDocument {
    pub name: String,
    pub fps: f64,
    pub clips: Vec<XmlClip>,
    /// Problems that did not stop the reading.
    pub warnings: Vec<String>,
}

fn child<'a, 'b>(node: roxmltree::Node<'a, 'b>, tag: &str) -> Option<roxmltree::Node<'a, 'b>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn text_of(node: roxmltree::Node, tag: &str) -> Option<String> {
    child(node, tag)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_owned())
}

fn number_of(node: roxmltree::Node, tag: &str) -> Option<i64> {
    text_of(node, tag)?.parse().ok()
}

/// The frame rate under `node`'s `<rate>`, if it has one.
fn rate_of(node: roxmltree::Node) -> Option<f64> {
    let rate = child(node, "rate")?;
    let base: f64 = text_of(rate, "timebase")?.parse().ok()?;
    if base <= 0.0 {
        return None;
    }
    let ntsc = text_of(rate, "ntsc").is_some_and(|v| v.eq_ignore_ascii_case("true"));
    Some(if ntsc { base * 1000.0 / 1001.0 } else { base })
}

/// Reads an FCP7 XML document (the first `<sequence>`).
pub fn parse(text: &str) -> Result<XmlDocument, String> {
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("XML inválido: {e}"))?;
    let sequence = doc
        .descendants()
        .find(|n| n.has_tag_name("sequence"))
        .ok_or("o arquivo não tem nenhuma sequência (<sequence>)")?;
    let fps = rate_of(sequence).unwrap_or(30.0);
    let mut out = XmlDocument {
        name: text_of(sequence, "name").unwrap_or_default(),
        fps,
        ..Default::default()
    };
    let Some(media) = child(sequence, "media") else {
        return Ok(out);
    };
    // `file` elements may be defined once and referenced by id afterwards
    let mut files: std::collections::HashMap<String, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();
    for file in doc.descendants().filter(|n| n.has_tag_name("file")) {
        if let Some(id) = file.attribute("id") {
            let name = text_of(file, "name");
            let path = text_of(file, "pathurl").map(|u| url_to_path(&u));
            if name.is_some() || path.is_some() {
                files.insert(id.to_owned(), (name, path));
            }
        }
    }
    for (kind, tag) in [(TrackKind::Video, "video"), (TrackKind::Audio, "audio")] {
        let Some(section) = child(media, tag) else {
            continue;
        };
        let mut seen: Vec<(usize, i64, i64, i64, String)> = Vec::new();
        for (track_index, track) in section
            .children()
            .filter(|n| n.has_tag_name("track"))
            .enumerate()
        {
            for item in track.children().filter(|n| n.has_tag_name("clipitem")) {
                let clip_fps = rate_of(item).unwrap_or(fps);
                let (Some(start), Some(end)) = (number_of(item, "start"), number_of(item, "end"))
                else {
                    out.warnings.push("clipe sem início ou fim ignorado".into());
                    continue;
                };
                // A negative start or end marks the edge of a transition: the length comes from
                // the in / out points instead
                let (src_in, src_out) = (
                    number_of(item, "in").unwrap_or(0),
                    number_of(item, "out").unwrap_or(0),
                );
                let (start, end) = if start < 0 || end < 0 {
                    out.warnings.push(format!(
                        "clipe \"{}\" com transição: posição aproximada",
                        text_of(item, "name").unwrap_or_default()
                    ));
                    (start.max(0), start.max(0) + (src_out - src_in).max(1))
                } else {
                    (start, end)
                };
                if end <= start {
                    continue;
                }
                let file = child(item, "file");
                let (file_name, path) = match file {
                    Some(f) => {
                        let own = (
                            text_of(f, "name"),
                            text_of(f, "pathurl").map(|u| url_to_path(&u)),
                        );
                        if own.0.is_some() || own.1.is_some() {
                            own
                        } else {
                            f.attribute("id")
                                .and_then(|id| files.get(id).cloned())
                                .unwrap_or((None, None))
                        }
                    }
                    None => (None, None),
                };
                let name = text_of(item, "name")
                    .or_else(|| file_name.clone())
                    .unwrap_or_else(|| "Clipe".into());
                // Stereo audio comes as one clip per channel: keep a single copy
                if kind == TrackKind::Audio {
                    let key = (
                        track_index / 2,
                        start,
                        end,
                        src_in,
                        path.clone().unwrap_or_else(|| name.clone()),
                    );
                    if seen.contains(&key) {
                        continue;
                    }
                    seen.push(key);
                }
                out.clips.push(XmlClip {
                    kind,
                    track: if kind == TrackKind::Audio {
                        track_index / 2
                    } else {
                        track_index
                    },
                    name,
                    start: start as f64 / fps,
                    end: end as f64 / fps,
                    source_in: src_in as f64 / clip_fps,
                    file_name,
                    path,
                });
            }
        }
    }
    Ok(out)
}

/// Puts the clips of `doc` on new tracks (one per sequence track), at their positions, in one undo
/// step. `resolve` finds the project media of a clip; clips without media are skipped and reported.
pub fn import_into_timeline(
    timeline: &mut Timeline,
    doc: &XmlDocument,
    resolve: impl Fn(&XmlClip) -> Option<MediaMatch>,
) -> Option<(EditCommand, ImportReport)> {
    let mut report = ImportReport::default();
    let mut commands = Vec::new();
    let mut tracks: Vec<(TrackKind, usize, TrackId)> = Vec::new();
    let mut events: Vec<(TrackId, TrackEvent)> = Vec::new();
    for clip in &doc.clips {
        let Some(media) = resolve(clip) else {
            report.skipped.push(format!(
                "\"{}\": mídia não encontrada no projeto",
                clip.file_name.as_deref().unwrap_or(&clip.name)
            ));
            continue;
        };
        let trim_in = clip.source_in.max(0.0);
        let duration = (clip.end - clip.start).min((media.duration - trim_in).max(0.0));
        if duration <= 1e-6 {
            report.skipped.push(format!(
                "\"{}\": o trecho está fora da duração da mídia",
                clip.name
            ));
            continue;
        }
        let track_id = match tracks
            .iter()
            .find(|(k, i, _)| *k == clip.kind && *i == clip.track)
        {
            Some((_, _, id)) => *id,
            None => {
                let track = timeline.new_track(clip.kind);
                let id = track.id;
                commands.push(EditCommand::AddTrack { track });
                tracks.push((clip.kind, clip.track, id));
                id
            }
        };
        events.push((
            track_id,
            TrackEvent {
                id: timeline.alloc_id(),
                name: clip.name.clone(),
                media_id: Some(media.media_id),
                start: clip.start.max(0.0),
                duration,
                trim_in,
                ..Default::default()
            },
        ));
        report.added += 1;
    }
    if events.is_empty() {
        return None;
    }
    for (track_id, event) in events {
        commands.push(EditCommand::AddEvent { track_id, event });
    }
    Some((EditCommand::Batch(commands), report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;

    fn project() -> (Timeline, Vec<XmlMedia>) {
        let mut tl = Timeline::new();
        let mut video = tl.new_track(TrackKind::Video);
        let mut audio = tl.new_track(TrackKind::Audio);
        let add = |tl: &mut Timeline,
                   track: &mut crate::timeline::Track,
                   start: f64,
                   duration: f64,
                   trim_in: f64,
                   media: u64,
                   name: &str| {
            track.events.push(TrackEvent {
                id: tl.alloc_id(),
                name: name.into(),
                media_id: Some(media),
                start,
                duration,
                trim_in,
                ..Default::default()
            });
        };
        add(&mut tl, &mut video, 0.0, 4.0, 2.0, 1, "Entrevista & <cena>");
        add(&mut tl, &mut video, 6.0, 2.4, 0.0, 2, "Cobertura");
        add(&mut tl, &mut audio, 1.0, 3.0, 0.5, 1, "Voz");
        tl.tracks.push(video);
        tl.tracks.push(audio);
        let media = vec![
            XmlMedia {
                id: 1,
                name: "entrevista.mp4".into(),
                path: "C:\\Vídeos\\meu projeto\\entrevista.mp4".into(),
                duration: 60.0,
            },
            XmlMedia {
                id: 2,
                name: "cobertura.mov".into(),
                path: "/home/u/cobertura.mov".into(),
                duration: 30.0,
            },
        ];
        (tl, media)
    }

    #[test]
    fn urls_round_trip_paths_with_spaces_accents_and_drives() {
        for path in [
            "C:/Videos/a b.mp4",
            "/home/u/cobertura 100%.mov",
            "D:/Vídeos/ação#1.mp4",
        ] {
            assert_eq!(url_to_path(&path_to_url(path)), path, "{path}");
        }
        assert_eq!(url_to_path(&path_to_url("C:\\x\\y.mp4")), "C:/x/y.mp4");
        assert_eq!(url_to_path("file://localhost/C%3a/x%20y.mp4"), "C:/x y.mp4");
    }

    #[test]
    fn export_is_well_formed_and_parses_back_to_the_same_clips() {
        let (tl, media) = project();
        let xml = export(&tl, &media, "Meu <projeto>", 29.97);
        let doc = parse(&xml).expect("the exported XML parses");
        assert_eq!(doc.name, "Meu <projeto>");
        assert!((doc.fps - 29.97).abs() < 0.01);
        assert_eq!(doc.clips.len(), 3);
        let first = &doc.clips[0];
        assert_eq!(first.kind, TrackKind::Video);
        assert_eq!(first.name, "Entrevista & <cena>");
        assert!((first.start - 0.0).abs() < 0.05 && (first.end - 4.0).abs() < 0.05);
        assert!((first.source_in - 2.0).abs() < 0.05);
        assert_eq!(
            first.path.as_deref(),
            Some("C:/Vídeos/meu projeto/entrevista.mp4")
        );
        let audio = doc
            .clips
            .iter()
            .find(|c| c.kind == TrackKind::Audio)
            .unwrap();
        assert!((audio.start - 1.0).abs() < 0.05 && (audio.source_in - 0.5).abs() < 0.05);
        // A file used twice is described once and referenced afterwards
        assert_eq!(xml.matches("<pathurl>").count(), 2);
    }

    #[test]
    fn import_places_clips_on_new_tracks_in_one_undo_step() {
        let (src, media) = project();
        let doc = parse(&export(&src, &media, "p", 25.0)).unwrap();
        let mut tl = Timeline::new();
        let mut history = History::new();
        let (command, report) = import_into_timeline(&mut tl, &doc, |clip| {
            let file = clip.file_name.as_deref()?;
            media.iter().find(|m| m.name == file).map(|m| MediaMatch {
                media_id: m.id,
                duration: m.duration,
            })
        })
        .unwrap();
        assert_eq!(report.added, 3);
        assert!(report.skipped.is_empty());
        assert!(history.execute(&mut tl, command));
        assert_eq!(tl.tracks.len(), 2);
        let video = tl
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap();
        assert_eq!(video.events.len(), 2);
        assert!((video.events[0].trim_in - 2.0).abs() < 0.05);
        assert!(history.undo(&mut tl));
        assert!(tl.tracks.is_empty());
    }

    #[test]
    fn clips_without_media_and_stereo_duplicates_are_handled() {
        let xml = r#"<xmeml version="5"><sequence><name>S</name>
          <rate><timebase>25</timebase><ntsc>FALSE</ntsc></rate><media>
          <video><track><clipitem><name>Sem midia</name><start>0</start><end>50</end><in>0</in><out>50</out>
            <file id="f9"><name>falta.mp4</name><pathurl>file://localhost/x/falta.mp4</pathurl></file></clipitem></track></video>
          <audio>
            <track><clipitem><name>A</name><start>0</start><end>100</end><in>0</in><out>100</out>
              <file id="f1"><name>a.wav</name></file></clipitem></track>
            <track><clipitem><name>A</name><start>0</start><end>100</end><in>0</in><out>100</out>
              <file id="f1"/></clipitem></track>
          </audio></media></sequence></xmeml>"#;
        let doc = parse(xml).unwrap();
        // One video clip and a single copy of the stereo pair
        assert_eq!(doc.clips.len(), 2);
        assert_eq!(doc.clips[1].file_name.as_deref(), Some("a.wav"));
        let mut tl = Timeline::new();
        let (_, report) = import_into_timeline(&mut tl, &doc, |c| {
            (c.file_name.as_deref() == Some("a.wav")).then_some(MediaMatch {
                media_id: 5,
                duration: 10.0,
            })
        })
        .unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("falta.mp4"));
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(parse("").is_err());
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<xmeml version=\"5\"></xmeml>")
            .unwrap_err()
            .contains("sequência"));
        // Nothing to place: no command
        let mut tl = Timeline::new();
        assert!(import_into_timeline(&mut tl, &XmlDocument::default(), |_| None).is_none());
    }
}
