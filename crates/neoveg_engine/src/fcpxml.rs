//! Final Cut Pro FCPXML (`<fcpxml version="1.10">`) interchange: export of the timeline and import
//! of sequence clips, connected lanes and markers into the media already loaded in the project.
//!
//! Uses exact rational frame times (`1001/30000s`, `1/25s`, etc.) as required by Apple's FCPXML
//! DTD so NTSC (`23.976`, `29.97`, `59.94`) and integer frame rates round-trip without drift.

use std::collections::{BTreeMap, HashMap};

use neoveg_common::timecode::is_drop_frame_rate;

use crate::command::EditCommand;
use crate::edl::{ImportReport, MediaMatch};
use crate::fcp7::{path_to_url, url_to_path, XmlClip, XmlMedia};
use crate::markers;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// Parsed Final Cut Pro `<fcpxml>` sequence ready to be placed onto a [`Timeline`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFcpxml {
    pub name: String,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub clips: Vec<XmlClip>,
    /// `(kind, track_number_1_based)` of lanes whose clips were all marked `enabled="0"`.
    pub muted_tracks: Vec<(TrackKind, usize)>,
    /// `(label, start_seconds, optional_region_duration_seconds)`.
    pub markers: Vec<(String, f64, Option<f64>)>,
    pub warnings: Vec<String>,
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

fn gcd(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

/// Rational frame duration `(num, den)` in seconds (`num / den` seconds per frame).
fn frame_duration_rational(fps: f64) -> (i64, i64) {
    let safe = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        30.0
    };
    let nominal = safe.round().max(1.0) as i64;
    let ntsc =
        ((safe * 1.001) - nominal as f64).abs() < 0.05 && (safe - nominal as f64).abs() > 1e-3;
    if ntsc {
        (1001, nominal * 1000)
    } else {
        (1, nominal)
    }
}

fn format_rational(seconds: f64, frame_num: i64, frame_den: i64) -> String {
    let exact_fps = frame_den as f64 / frame_num as f64;
    let frames = (seconds.max(0.0) * exact_fps).round() as i64;
    if frames == 0 {
        return "0s".into();
    }
    let num = frames * frame_num;
    let g = gcd(num, frame_den);
    let (rn, rd) = (num / g, frame_den / g);
    if rd == 1 {
        format!("{rn}s")
    } else {
        format!("{rn}/{rd}s")
    }
}

/// Parses an FCPXML rational time attribute (`"0s"`, `"1001/30000s"`, `"12/25s"`, `"3.5s"`).
pub fn parse_rational_seconds(raw: &str) -> Option<f64> {
    let s = raw.trim().strip_suffix('s').unwrap_or(raw.trim()).trim();
    if s.is_empty() {
        return None;
    }
    if let Some((n_str, d_str)) = s.split_once('/') {
        let n: f64 = n_str.trim().parse().ok()?;
        let d: f64 = d_str.trim().parse().ok()?;
        if d.abs() < 1e-12 || !n.is_finite() || !d.is_finite() {
            return None;
        }
        Some(n / d)
    } else {
        let v: f64 = s.parse().ok()?;
        v.is_finite().then_some(v)
    }
}

/// Writes `timeline` as a Final Cut Pro `<fcpxml version="1.10">` document.
pub fn export(
    timeline: &Timeline,
    media: &[XmlMedia],
    name: &str,
    fps: f64,
    width: u32,
    height: u32,
    sample_rate: u32,
) -> String {
    let (frame_num, frame_den) = frame_duration_rational(fps);
    let rat = |sec: f64| format_rational(sec, frame_num, frame_den);
    let g = gcd(frame_num, frame_den);
    let frame_dur = format!("{}/{}s", frame_num / g, frame_den / g);
    let min_seq = frame_num as f64 / frame_den as f64;
    let seq_dur = rat(timeline.duration().max(min_seq));
    let tc_format = if is_drop_frame_rate(fps) { "DF" } else { "NDF" };

    struct AssetRow {
        res_id: String,
        name: String,
        url: String,
        duration: f64,
        has_video: bool,
        has_audio: bool,
    }

    let media_by_id: HashMap<u64, &XmlMedia> = media.iter().map(|m| (m.id, m)).collect();
    let mut assets: Vec<AssetRow> = Vec::new();
    let mut asset_of_media: HashMap<u64, usize> = HashMap::new();
    let mut generated_assets: HashMap<String, usize> = HashMap::new();

    for track in &timeline.tracks {
        for ev in &track.events {
            let needed = ev.trim_in + ev.duration * ev.playback_rate.max(0.25);
            if let Some(mid) = ev.media_id {
                if let Some(&idx) = asset_of_media.get(&mid) {
                    assets[idx].duration = assets[idx].duration.max(needed);
                    if track.kind == TrackKind::Video {
                        assets[idx].has_video = true;
                    } else {
                        assets[idx].has_audio = true;
                    }
                } else {
                    let idx = assets.len();
                    let res_id = format!("r{}", idx + 2);
                    let (aname, url, dur) = match media_by_id.get(&mid) {
                        Some(m) => (
                            m.name.clone(),
                            path_to_url(&m.path),
                            m.duration.max(needed).max(min_seq),
                        ),
                        None => (ev.name.clone(), path_to_url(&ev.name), needed.max(min_seq)),
                    };
                    assets.push(AssetRow {
                        res_id,
                        name: aname,
                        url,
                        duration: dur,
                        has_video: track.kind == TrackKind::Video,
                        has_audio: track.kind == TrackKind::Audio,
                    });
                    asset_of_media.insert(mid, idx);
                }
            } else if let Some(&idx) = generated_assets.get(&ev.name) {
                assets[idx].duration = assets[idx].duration.max(needed);
            } else {
                let idx = assets.len();
                assets.push(AssetRow {
                    res_id: format!("r{}", idx + 2),
                    name: ev.name.clone(),
                    url: path_to_url(&ev.name),
                    duration: needed.max(min_seq),
                    has_video: track.kind == TrackKind::Video,
                    has_audio: track.kind == TrackKind::Audio,
                });
                generated_assets.insert(ev.name.clone(), idx);
            }
        }
    }

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n");
    out.push_str("<fcpxml version=\"1.10\">\n");
    out.push_str("  <resources>\n");
    out.push_str(&format!(
        "    <format id=\"r1\" frameDuration=\"{frame_dur}\" width=\"{}\" height=\"{}\"/>\n",
        width.max(16),
        height.max(16)
    ));
    let sr = sample_rate.max(8_000);
    for a in &assets {
        out.push_str(&format!(
            "    <asset id=\"{}\" name=\"{}\" src=\"{}\" start=\"0s\" duration=\"{}\" hasVideo=\"{}\" hasAudio=\"{}\" audioSources=\"1\" audioChannels=\"2\" audioRate=\"{sr}\"/>\n",
            a.res_id,
            escape(&a.name),
            escape(&a.url),
            rat(a.duration),
            u8::from(a.has_video),
            u8::from(a.has_audio),
        ));
    }
    out.push_str("  </resources>\n");

    let safe_title = escape(if name.trim().is_empty() {
        "Neoveg"
    } else {
        name.trim()
    });
    out.push_str("  <library>\n");
    out.push_str(&format!("    <event name=\"{safe_title}\">\n"));
    out.push_str(&format!("      <project name=\"{safe_title}\">\n"));
    out.push_str(&format!(
        "        <sequence format=\"r1\" duration=\"{seq_dur}\" tcStart=\"0s\" tcFormat=\"{tc_format}\" audioLayout=\"stereo\" audioRate=\"48k\">\n"
    ));
    out.push_str("          <spine>\n");
    out.push_str(&format!(
        "            <gap name=\"Timeline\" offset=\"0s\" start=\"0s\" duration=\"{seq_dur}\">\n"
    ));

    let mut video_lane = 1i32;
    let mut audio_lane = -1i32;
    for track in &timeline.tracks {
        let lane = match track.kind {
            TrackKind::Video => {
                let l = video_lane;
                video_lane += 1;
                l
            }
            TrackKind::Audio => {
                let l = audio_lane;
                audio_lane -= 1;
                l
            }
        };
        let role = match track.kind {
            TrackKind::Video => "video",
            TrackKind::Audio => "dialogue",
        };
        let enabled_attr = if track.muted { " enabled=\"0\"" } else { "" };
        let mut events: Vec<&TrackEvent> = track.events.iter().collect();
        events.sort_by(|a, b| a.start.total_cmp(&b.start));
        for ev in events {
            let idx = ev
                .media_id
                .and_then(|mid| asset_of_media.get(&mid).copied())
                .or_else(|| generated_assets.get(&ev.name).copied())
                .unwrap_or(0);
            let res_id = &assets[idx].res_id;
            out.push_str(&format!(
                "              <asset-clip name=\"{}\" ref=\"{res_id}\" lane=\"{lane}\" offset=\"{}\" start=\"{}\" duration=\"{}\" role=\"{role}\"{enabled_attr}/>\n",
                escape(&ev.name),
                rat(ev.start),
                rat(ev.trim_in),
                rat(ev.duration),
            ));
        }
    }

    for marker in &timeline.markers {
        out.push_str(&format!(
            "              <marker start=\"{}\" duration=\"{frame_dur}\" value=\"{}\"/>\n",
            rat(marker.time),
            escape(&marker.name),
        ));
    }
    for region in &timeline.regions {
        let dur = (region.end - region.start).max(min_seq);
        out.push_str(&format!(
            "              <marker start=\"{}\" duration=\"{}\" value=\"{}\" completed=\"0\"/>\n",
            rat(region.start),
            rat(dur),
            escape(&region.name),
        ));
    }

    out.push_str("            </gap>\n");
    out.push_str("          </spine>\n");
    out.push_str("        </sequence>\n");
    out.push_str("      </project>\n");
    out.push_str("    </event>\n");
    out.push_str("  </library>\n");
    out.push_str("</fcpxml>\n");
    out
}

#[derive(Clone, Debug)]
struct FcpxAsset {
    name: String,
    path: Option<String>,
    file_name: Option<String>,
    has_video: bool,
    has_audio: bool,
}

#[derive(Clone, Debug)]
struct LaneClip {
    clip: XmlClip,
    enabled: bool,
}

/// Parses a Final Cut Pro `<fcpxml>` document.
pub fn parse(text: &str) -> Result<ParsedFcpxml, String> {
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)
        .map_err(|e| format!("FCPXML malformado: {e}"))?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("fcpxml") {
        return Err(format!(
            "elemento raiz esperado <fcpxml>, encontrado <{}>",
            root.tag_name().name()
        ));
    }

    let mut formats: HashMap<String, (f64, u32, u32)> = HashMap::new();
    let mut assets: HashMap<String, FcpxAsset> = HashMap::new();

    if let Some(resources) = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("resources"))
    {
        for child in resources.children().filter(|n| n.is_element()) {
            let tag = child.tag_name().name();
            if tag.eq_ignore_ascii_case("format") {
                if let Some(id) = child.attribute("id") {
                    let fps = child
                        .attribute("frameDuration")
                        .and_then(parse_rational_seconds)
                        .filter(|&d| d > 1e-9)
                        .map_or(30.0, |d| 1.0 / d);
                    let w = child
                        .attribute("width")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1920);
                    let h = child
                        .attribute("height")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1080);
                    formats.insert(id.to_owned(), (fps, w, h));
                }
            } else if tag.eq_ignore_ascii_case("asset") {
                if let Some(id) = child.attribute("id") {
                    let src_raw = child.attribute("src").map(str::to_owned).or_else(|| {
                        child
                            .children()
                            .find(|c| {
                                c.is_element()
                                    && c.tag_name().name().eq_ignore_ascii_case("media-rep")
                            })
                            .and_then(|mr| mr.attribute("src"))
                            .map(str::to_owned)
                    });
                    let path = src_raw.as_deref().map(url_to_path);
                    let file_name = path
                        .as_deref()
                        .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_owned());
                    let name = child
                        .attribute("name")
                        .map(str::to_owned)
                        .or_else(|| file_name.clone())
                        .unwrap_or_else(|| id.to_owned());
                    let has_video = child
                        .attribute("hasVideo")
                        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                    let has_audio = child
                        .attribute("hasAudio")
                        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                    assets.insert(
                        id.to_owned(),
                        FcpxAsset {
                            name,
                            path,
                            file_name,
                            has_video,
                            has_audio,
                        },
                    );
                }
            }
        }
    }

    let project_node = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("project"));
    let project_name = project_node
        .and_then(|p| p.attribute("name"))
        .unwrap_or("Sequência FCPXML")
        .to_owned();

    let seq = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("sequence"))
        .ok_or_else(|| "nenhuma <sequence> encontrada no FCPXML".to_owned())?;
    let (fps, width, height) = seq
        .attribute("format")
        .and_then(|f| formats.get(f).copied())
        .or_else(|| formats.values().next().copied())
        .unwrap_or((30.0, 1920, 1080));

    let spine = seq
        .children()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("spine"))
        .ok_or_else(|| "nenhum <spine> encontrado na <sequence>".to_owned())?;

    let mut lane_map: BTreeMap<(u8, i32), Vec<LaneClip>> = BTreeMap::new();
    let mut parsed_markers = Vec::new();
    let frame_sec = 1.0 / fps.max(1.0);
    let mut spine_cursor = 0.0;
    for item in spine.children().filter(|n| n.is_element()) {
        collect_item(
            item,
            0.0,
            &mut spine_cursor,
            frame_sec,
            &assets,
            &mut lane_map,
            &mut parsed_markers,
        );
    }

    let mut clips = Vec::new();
    let mut muted_tracks = Vec::new();
    let mut video_track_idx = 0usize;
    let mut audio_track_idx = 0usize;

    // Video lanes sorted ascending (0, 1, 2...); audio lanes sorted by |lane| (-1, -2...)
    let mut video_entries: Vec<(i32, Vec<LaneClip>)> = Vec::new();
    let mut audio_entries: Vec<(i32, Vec<LaneClip>)> = Vec::new();
    for ((kind_tag, lane), items) in lane_map {
        if items.is_empty() {
            continue;
        }
        if kind_tag == 0 {
            video_entries.push((lane, items));
        } else {
            audio_entries.push((lane, items));
        }
    }
    video_entries.sort_by_key(|(lane, _)| *lane);
    audio_entries.sort_by_key(|(lane, _)| -*lane);

    for (_, items) in video_entries {
        video_track_idx += 1;
        let all_disabled = items.iter().all(|c| !c.enabled);
        if all_disabled {
            muted_tracks.push((TrackKind::Video, video_track_idx));
        }
        for entry in items {
            if entry.enabled || all_disabled {
                let mut c = entry.clip;
                c.track = video_track_idx;
                clips.push(c);
            }
        }
    }
    for (_, items) in audio_entries {
        audio_track_idx += 1;
        let all_disabled = items.iter().all(|c| !c.enabled);
        if all_disabled {
            muted_tracks.push((TrackKind::Audio, audio_track_idx));
        }
        for entry in items {
            if entry.enabled || all_disabled {
                let mut c = entry.clip;
                c.track = audio_track_idx;
                clips.push(c);
            }
        }
    }

    Ok(ParsedFcpxml {
        name: project_name,
        fps,
        width,
        height,
        clips,
        muted_tracks,
        markers: parsed_markers,
        warnings: Vec::new(),
    })
}

fn collect_item(
    node: roxmltree::Node<'_, '_>,
    anchor_shift: f64,
    spine_cursor: &mut f64,
    frame_sec: f64,
    assets: &HashMap<String, FcpxAsset>,
    lane_map: &mut BTreeMap<(u8, i32), Vec<LaneClip>>,
    parsed_markers: &mut Vec<(String, f64, Option<f64>)>,
) {
    let tag = node.tag_name().name();
    if tag.eq_ignore_ascii_case("marker") || tag.eq_ignore_ascii_case("chapter-marker") {
        if let Some(local_start) = node.attribute("start").and_then(parse_rational_seconds) {
            let t = (anchor_shift + local_start).max(0.0);
            let dur = node
                .attribute("duration")
                .and_then(parse_rational_seconds)
                .filter(|&d| d > frame_sec * 1.5);
            let label = node.attribute("value").unwrap_or("Marcador").to_owned();
            parsed_markers.push((label, t, dur));
        }
        return;
    }

    let enabled = node
        .attribute("enabled")
        .is_none_or(|v| v != "0" && !v.eq_ignore_ascii_case("false"));
    let lane: i32 = node
        .attribute("lane")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let offset = node
        .attribute("offset")
        .and_then(parse_rational_seconds)
        .map(|o| anchor_shift + o)
        .unwrap_or(*spine_cursor);
    let start = node
        .attribute("start")
        .and_then(parse_rational_seconds)
        .unwrap_or(0.0);
    let duration = node
        .attribute("duration")
        .and_then(parse_rational_seconds)
        .unwrap_or(0.0);

    if lane == 0 && duration > 0.0 {
        *spine_cursor = offset + duration;
    }

    let is_clip = tag.eq_ignore_ascii_case("asset-clip")
        || tag.eq_ignore_ascii_case("clip")
        || tag.eq_ignore_ascii_case("video")
        || tag.eq_ignore_ascii_case("audio");
    if is_clip && duration > 1e-6 {
        let asset = node.attribute("ref").and_then(|r| assets.get(r));
        let role = node.attribute("role").unwrap_or("").to_ascii_lowercase();
        let kind = if tag.eq_ignore_ascii_case("audio")
            || lane < 0
            || role.contains("audio")
            || role.contains("dialogue")
            || role.contains("music")
            || role.contains("effects")
            || asset.is_some_and(|a| !a.has_video && a.has_audio)
        {
            TrackKind::Audio
        } else {
            TrackKind::Video
        };
        let kind_tag = match kind {
            TrackKind::Video => 0,
            TrackKind::Audio => 1,
        };
        let name = node
            .attribute("name")
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| asset.map(|a| a.name.clone()))
            .unwrap_or_else(|| "Clipe".into());
        let clip_start = offset.max(0.0);
        lane_map
            .entry((kind_tag, lane))
            .or_default()
            .push(LaneClip {
                clip: XmlClip {
                    kind,
                    track: 1,
                    name,
                    file_name: asset.and_then(|a| a.file_name.clone()),
                    path: asset.and_then(|a| a.path.clone()),
                    start: clip_start,
                    end: clip_start + duration,
                    source_in: start.max(0.0),
                },
                enabled,
            });
    }

    let child_shift = offset - start;
    let mut nested_cursor = offset;
    for child in node.children().filter(|n| n.is_element()) {
        collect_item(
            child,
            child_shift,
            &mut nested_cursor,
            frame_sec,
            assets,
            lane_map,
            parsed_markers,
        );
    }
}

/// Places the clips and markers of `doc` onto `timeline` as a single undoable [`EditCommand`].
pub fn import_into_timeline(
    timeline: &mut Timeline,
    doc: &ParsedFcpxml,
    resolve: impl Fn(&XmlClip) -> Option<MediaMatch>,
) -> Option<(EditCommand, ImportReport)> {
    let mut report = ImportReport::default();
    let mut commands = Vec::new();
    let mut tracks: Vec<(TrackKind, usize, TrackId)> = Vec::new();
    let mut events: Vec<(TrackId, TrackEvent)> = Vec::new();

    for clip in &doc.clips {
        let Some(media) = resolve(clip) else {
            report
                .skipped
                .push(format!("{}: mídia não encontrada no projeto", clip.name));
            continue;
        };
        let trim_in = clip.source_in.max(0.0);
        let available = (media.duration - trim_in).max(0.0);
        let duration = (clip.end - clip.start).max(0.0).min(available);
        if duration <= 1e-6 {
            report
                .skipped
                .push(format!("{}: trecho fora da duração da mídia", clip.name));
            continue;
        }
        let track_id = match tracks
            .iter()
            .find(|(k, idx, _)| *k == clip.kind && *idx == clip.track)
        {
            Some((_, _, id)) => *id,
            None => {
                let mut track = timeline.new_track(clip.kind);
                if doc.muted_tracks.contains(&(clip.kind, clip.track)) {
                    track.muted = true;
                }
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
    if !doc.markers.is_empty() {
        let before = timeline.marks();
        let mut after = before.clone();
        for (label, start, dur) in &doc.markers {
            let id = timeline.alloc_id();
            match dur {
                Some(d) => after.regions.push(markers::Region {
                    id,
                    start: start.max(0.0),
                    end: (*start + *d).max(start.max(0.0) + 1e-3),
                    name: label.clone(),
                }),
                None => after.markers.push(markers::Marker {
                    id,
                    time: start.max(0.0),
                    name: label.clone(),
                }),
            }
        }
        after.markers.sort_by(|a, b| a.time.total_cmp(&b.time));
        after.regions.sort_by(|a, b| a.start.total_cmp(&b.start));
        commands.push(EditCommand::ReplaceMarks { before, after });
    }
    Some((EditCommand::Batch(commands), report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::History;
    use crate::selection::TimeRange;

    fn sample_project() -> (Timeline, Vec<XmlMedia>) {
        let mut tl = Timeline::new();
        let mut v = tl.new_track(TrackKind::Video);
        let mut a = tl.new_track(TrackKind::Audio);
        a.muted = true;
        v.events.push(TrackEvent {
            id: tl.alloc_id(),
            name: "Plano A & <luz>".into(),
            media_id: Some(1),
            start: 0.0,
            duration: 2.002,
            trim_in: 1.001,
            ..Default::default()
        });
        a.events.push(TrackEvent {
            id: tl.alloc_id(),
            name: "Locução".into(),
            media_id: Some(2),
            start: 1.001,
            duration: 3.003,
            trim_in: 0.0,
            ..Default::default()
        });
        tl.tracks.push(v);
        tl.tracks.push(a);
        let mut h = History::new();
        let m = markers::add_marker(&mut tl, 1.001, Some("Corte")).unwrap();
        h.execute(&mut tl, m);
        let r = markers::add_region(&mut tl, TimeRange::new(2.002, 4.004), Some("Bloco")).unwrap();
        h.execute(&mut tl, r);
        let media = vec![
            XmlMedia {
                id: 1,
                name: "plano_a.mov".into(),
                path: "C:\\Mídia\\plano a.mov".into(),
                duration: 30.0,
            },
            XmlMedia {
                id: 2,
                name: "locucao.wav".into(),
                path: "/home/ed/locucao.wav".into(),
                duration: 20.0,
            },
        ];
        (tl, media)
    }

    #[test]
    fn fcpxml_round_trips_ntsc_rational_times_muted_lanes_and_markers() {
        let (tl, media) = sample_project();
        let xml = export(&tl, &media, "Corte Final", 29.97, 1920, 1080, 48_000);
        assert!(xml.contains("<fcpxml version=\"1.10\">"));
        assert!(xml.contains("frameDuration=\"1001/30000s\""));
        assert!(xml.contains("tcFormat=\"DF\""));

        let parsed = parse(&xml).unwrap();
        assert_eq!(parsed.name, "Corte Final");
        assert!((parsed.fps - 30_000.0 / 1001.0).abs() < 1e-4);
        assert_eq!(parsed.clips.len(), 2);
        assert_eq!(parsed.muted_tracks, vec![(TrackKind::Audio, 1)]);
        assert_eq!(parsed.markers.len(), 2);

        let mut dest = Timeline::new();
        let mut history = History::new();
        let (cmd, report) = import_into_timeline(&mut dest, &parsed, |clip| {
            media
                .iter()
                .find(|m| {
                    clip.path
                        .as_deref()
                        .is_some_and(|p| p.replace('\\', "/") == m.path.replace('\\', "/"))
                })
                .map(|m| MediaMatch {
                    media_id: m.id,
                    duration: m.duration,
                })
        })
        .unwrap();
        assert_eq!(report.added, 2);
        assert!(report.skipped.is_empty());
        history.execute(&mut dest, cmd);
        assert_eq!(dest.tracks.len(), 2);
        assert!(dest.tracks[1].muted);
        assert_eq!(dest.markers.len(), 1);
        assert_eq!(dest.regions.len(), 1);
        assert!((dest.tracks[0].events[0].trim_in - 1.001).abs() < 1e-3);

        // Single undo reverts all imported tracks, clips and markers
        history.undo(&mut dest);
        assert!(dest.tracks.is_empty() && dest.markers.is_empty() && dest.regions.is_empty());
    }

    #[test]
    fn parses_primary_storyline_and_nested_connected_clips() {
        let external = r#"<?xml version="1.0" encoding="UTF-8"?>
<fcpxml version="1.9">
  <resources>
    <format id="r1" frameDuration="1/25s" width="1920" height="1080"/>
    <asset id="r2" name="entrevista.mp4" src="file:///media/entrevista.mp4" hasVideo="1" hasAudio="1"/>
    <asset id="r3" name="trilha.wav" src="file:///media/trilha.wav" hasVideo="0" hasAudio="1"/>
  </resources>
  <library>
    <event name="Evento">
      <project name="Doc">
        <sequence format="r1">
          <spine>
            <asset-clip name="Abertura" ref="r2" offset="0s" start="2s" duration="5s">
              <asset-clip name="Trilha" ref="r3" lane="-1" offset="3s" start="0s" duration="4s" role="music"/>
            </asset-clip>
          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>"#;
        let parsed = parse(external).unwrap();
        assert!((parsed.fps - 25.0).abs() < 1e-6);
        assert_eq!(parsed.clips.len(), 2);
        // Connected clip at offset=3s inside parent (offset=0s, start=2s) lands at timeline 1.0s
        let music = parsed
            .clips
            .iter()
            .find(|c| c.kind == TrackKind::Audio)
            .unwrap();
        assert!((music.start - 1.0).abs() < 1e-6);
        assert!(((music.end - music.start) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_non_fcpxml_documents() {
        assert!(parse("<xmeml version=\"5\"/>").is_err());
        assert!(parse("not xml").is_err());
    }
}
