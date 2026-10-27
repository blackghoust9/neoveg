//! OpenTimelineIO (`.otio`) native JSON interchange: export of the timeline and import of tracks,
//! clips, gaps, dissolve transitions, time-warps and markers/regions into the media already loaded
//! in the project.
//!
//! Conforms to the public ASWF OpenTimelineIO JSON schema (`Timeline.1`, `Stack.1`, `Track.1`,
//! `Clip.1` / `Clip.2`, `Gap.1`, `Transition.1`, `LinearTimeWarp.1`, `Marker.2`,
//! `ExternalReference.1`, `MissingReference.1`, `TimeRange.1`, `RationalTime.1`) in pure Rust with
//! zero Python or C++ dependencies.

use std::collections::HashMap;

use neoveg_common::fade::FadeCurve;
use serde_json::{json, Map, Value};

use crate::command::EditCommand;
use crate::edl::{ImportReport, MediaMatch};
use crate::fcp7::{path_to_url, url_to_path, XmlClip, XmlMedia};
use crate::markers;
use crate::timeline::{Timeline, TrackEvent, TrackId, TrackKind};

/// One clip parsed from an OpenTimelineIO document.
#[derive(Debug, Clone, PartialEq)]
pub struct OtioClip {
    pub xml: XmlClip,
    pub playback_rate: f64,
    pub preserve_pitch: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    pub fade_in_curve: FadeCurve,
    pub fade_out_curve: FadeCurve,
}

/// Parsed OpenTimelineIO timeline ready to be applied to a [`Timeline`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedOtio {
    pub name: String,
    pub fps: f64,
    pub clips: Vec<OtioClip>,
    /// `(kind, track_number_1_based)` of tracks marked `enabled: false`.
    pub muted_tracks: Vec<(TrackKind, usize)>,
    /// `(label, start_seconds, optional_region_duration_seconds)`.
    pub markers: Vec<(String, f64, Option<f64>)>,
    pub warnings: Vec<String>,
}

fn safe_rate(fps: f64) -> f64 {
    if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        30.0
    }
}

fn rational_time(seconds: f64, rate: f64) -> Value {
    let r = safe_rate(rate);
    let ticks = (seconds * r * 1_000_000.0).round() / 1_000_000.0;
    json!({
        "OTIO_SCHEMA": "RationalTime.1",
        "rate": r,
        "value": ticks,
    })
}

fn time_range(start_s: f64, duration_s: f64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "TimeRange.1",
        "start_time": rational_time(start_s.max(0.0), rate),
        "duration": rational_time(duration_s.max(0.0), rate),
    })
}

fn parse_rational_time(v: &Value) -> Option<(f64, f64)> {
    let obj = v.as_object()?;
    let value = obj.get("value")?.as_f64()?;
    let rate = obj.get("rate")?.as_f64()?;
    if !value.is_finite() || !rate.is_finite() || rate <= 0.0 {
        return None;
    }
    Some((value / rate, rate))
}

fn parse_time_range(v: &Value) -> Option<(f64, f64, f64)> {
    let obj = v.as_object()?;
    let (start_s, rate) = parse_rational_time(obj.get("start_time")?)?;
    let (dur_s, _) = parse_rational_time(obj.get("duration")?)?;
    Some((start_s, dur_s, rate))
}

fn schema_name(obj: &Map<String, Value>) -> &str {
    obj.get("OTIO_SCHEMA")
        .and_then(Value::as_str)
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
}

fn file_name_of(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Serializes `timeline` as a formatted OpenTimelineIO (`.otio`) JSON string.
pub fn export(
    timeline: &Timeline,
    media: &[XmlMedia],
    name: &str,
    fps: f64,
    width: u32,
    height: u32,
) -> String {
    let rate = safe_rate(fps);
    let media_by_id: HashMap<u64, &XmlMedia> = media.iter().map(|m| (m.id, m)).collect();

    let mut stack_markers = Vec::new();
    for m in &timeline.markers {
        stack_markers.push(json!({
            "OTIO_SCHEMA": "Marker.2",
            "metadata": {},
            "name": m.name,
            "color": "GREEN",
            "comment": "",
            "marked_range": time_range(m.time, 0.0, rate),
        }));
    }
    for r in &timeline.regions {
        let dur = (r.end - r.start).max(0.0);
        stack_markers.push(json!({
            "OTIO_SCHEMA": "Marker.2",
            "metadata": { "neoveg_region": true },
            "name": r.name,
            "color": "CYAN",
            "comment": "",
            "marked_range": time_range(r.start, dur, rate),
        }));
    }

    let mut otio_tracks = Vec::with_capacity(timeline.tracks.len());
    for track in &timeline.tracks {
        let mut sorted_events = track.events.clone();
        sorted_events.sort_by(|a, b| a.start.total_cmp(&b.start).then_with(|| a.id.cmp(&b.id)));

        // Pairwise partial overlaps (`a.start < b.start < a.end < b.end`) become OTIO Transition.1
        // items centered at the overlap midpoint.
        let n = sorted_events.len();
        let mut overlap_after = vec![0.0_f64; n];
        for i in 0..n.saturating_sub(1) {
            let a_end = sorted_events[i].end();
            let b_start = sorted_events[i + 1].start;
            let b_end = sorted_events[i + 1].end();
            if b_start > sorted_events[i].start + 1e-6
                && b_start < a_end - 1e-6
                && b_end > a_end + 1e-6
            {
                let ov = a_end - b_start;
                if ov < sorted_events[i].duration && ov < sorted_events[i + 1].duration {
                    overlap_after[i] = ov;
                }
            }
        }

        let mut children = Vec::new();
        let mut cursor = 0.0_f64;

        for (i, ev) in sorted_events.iter().enumerate() {
            let in_half = if i > 0 {
                overlap_after[i - 1] * 0.5
            } else {
                0.0
            };
            let out_half = overlap_after[i] * 0.5;

            let seq_start = ev.start + in_half;
            let seq_end = (ev.end() - out_half).max(seq_start);
            let seq_dur = seq_end - seq_start;
            if seq_dur <= 1e-6 {
                continue;
            }

            if seq_start > cursor + 1e-5 {
                children.push(json!({
                    "OTIO_SCHEMA": "Gap.1",
                    "metadata": {},
                    "name": "",
                    "source_range": time_range(0.0, seq_start - cursor, rate),
                    "effects": [],
                    "markers": [],
                    "enabled": true,
                }));
            }

            let media_ref = match ev.media_id.and_then(|id| media_by_id.get(&id)).copied() {
                Some(m) => json!({
                    "OTIO_SCHEMA": "ExternalReference.1",
                    "metadata": { "neoveg_media_name": m.name },
                    "name": m.name,
                    "available_range": time_range(0.0, m.duration.max(seq_dur), rate),
                    "target_url": path_to_url(&m.path),
                }),
                None => json!({
                    "OTIO_SCHEMA": "MissingReference.1",
                    "metadata": {},
                    "name": ev.name,
                    "available_range": null,
                }),
            };

            let mut effects = Vec::new();
            let rate_scale = if ev.playback_rate.is_finite() && ev.playback_rate > 0.0 {
                ev.playback_rate
            } else {
                1.0
            };
            if (rate_scale - 1.0).abs() > 1e-4 {
                effects.push(json!({
                    "OTIO_SCHEMA": "LinearTimeWarp.1",
                    "metadata": { "preserve_pitch": ev.preserve_pitch },
                    "name": "TimeWarp",
                    "effect_name": "LinearTimeWarp",
                    "time_scale": rate_scale,
                }));
            }

            let source_in = ev.trim_in + in_half * rate_scale;
            let clip_meta = serde_json::to_value(ClipNeovegMeta {
                fade_in: ev.fade_in,
                fade_out: ev.fade_out,
                fade_in_curve: ev.fade_in_curve,
                fade_out_curve: ev.fade_out_curve,
            })
            .unwrap_or(Value::Null);

            children.push(json!({
                "OTIO_SCHEMA": "Clip.1",
                "metadata": { "neoveg": clip_meta },
                "name": ev.name,
                "source_range": time_range(source_in, seq_dur, rate),
                "effects": effects,
                "markers": [],
                "enabled": true,
                "media_reference": media_ref,
            }));
            cursor = seq_start + seq_dur;

            if out_half > 1e-6 {
                children.push(json!({
                    "OTIO_SCHEMA": "Transition.1",
                    "metadata": {},
                    "name": "Crossfade",
                    "transition_type": "SMPTE_Dissolve",
                    "in_offset": rational_time(out_half, rate),
                    "out_offset": rational_time(out_half, rate),
                }));
            }
        }

        let kind_str = match track.kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        otio_tracks.push(json!({
            "OTIO_SCHEMA": "Track.1",
            "metadata": {},
            "name": track.name,
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": !track.muted,
            "kind": kind_str,
            "children": children,
        }));
    }

    let doc = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "metadata": {
            "neoveg": {
                "fps": rate,
                "width": width,
                "height": height,
            }
        },
        "name": name,
        "global_start_time": rational_time(0.0, rate),
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "metadata": {},
            "name": "tracks",
            "source_range": null,
            "effects": [],
            "markers": stack_markers,
            "enabled": true,
            "children": otio_tracks,
        }
    });

    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into())
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ClipNeovegMeta {
    #[serde(default)]
    fade_in: f64,
    #[serde(default)]
    fade_out: f64,
    #[serde(default)]
    fade_in_curve: FadeCurve,
    #[serde(default)]
    fade_out_curve: FadeCurve,
}

/// Parses an OpenTimelineIO `.otio` JSON string (`Timeline.1` or `Stack.1`).
pub fn parse(text: &str) -> Result<ParsedOtio, String> {
    let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("JSON OpenTimelineIO inválido: {e}"))?;
    let obj = root
        .as_object()
        .ok_or_else(|| "Raiz do arquivo .otio não é um objeto JSON".to_owned())?;

    let schema = schema_name(obj);
    let (name, stack_obj, global_fps) = match schema {
        "Timeline" => {
            let tl_name = obj
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("Timeline OTIO")
                .to_owned();
            let fps_from_start = obj
                .get("global_start_time")
                .and_then(parse_rational_time)
                .map(|(_, r)| r);
            let fps_from_meta = obj
                .get("metadata")
                .and_then(|m| m.get("neoveg"))
                .and_then(|n| n.get("fps"))
                .and_then(Value::as_f64);
            let tracks = obj
                .get("tracks")
                .and_then(Value::as_object)
                .ok_or_else(|| "Timeline.1 sem campo 'tracks'".to_owned())?;
            (
                tl_name,
                tracks,
                fps_from_start.or(fps_from_meta).unwrap_or(30.0),
            )
        }
        "Stack" => {
            let st_name = obj
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("Stack OTIO")
                .to_owned();
            (st_name, obj, 30.0)
        }
        other => {
            return Err(format!(
                "Esquema OTIO raiz não suportado: '{other}' (esperado Timeline.1 ou Stack.1)"
            ));
        }
    };

    let mut out = ParsedOtio {
        name,
        fps: safe_rate(global_fps),
        clips: Vec::new(),
        muted_tracks: Vec::new(),
        markers: Vec::new(),
        warnings: Vec::new(),
    };
    let mut detected_rate: Option<f64> = None;

    collect_markers(stack_obj, 0.0, &mut out.markers, &mut detected_rate);

    let mut video_index = 0_usize;
    let mut audio_index = 0_usize;
    if let Some(children) = stack_obj.get("children").and_then(Value::as_array) {
        for child in children {
            let Some(track_obj) = child.as_object() else {
                continue;
            };
            if schema_name(track_obj) != "Track" {
                continue;
            }
            let kind_raw = track_obj
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("Video");
            let kind = if kind_raw.eq_ignore_ascii_case("Audio") {
                audio_index += 1;
                TrackKind::Audio
            } else {
                video_index += 1;
                TrackKind::Video
            };
            let track_num = match kind {
                TrackKind::Video => video_index,
                TrackKind::Audio => audio_index,
            };
            let enabled = track_obj
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if !enabled {
                out.muted_tracks.push((kind, track_num));
            }

            parse_track_children(
                track_obj,
                kind,
                track_num,
                &mut out.clips,
                &mut out.markers,
                &mut out.warnings,
                &mut detected_rate,
            );
        }
    }

    if let Some(r) = detected_rate {
        if (out.fps - 30.0).abs() < 1e-6 {
            out.fps = safe_rate(r);
        }
    }

    Ok(out)
}

fn collect_markers(
    obj: &Map<String, Value>,
    offset_s: f64,
    markers_out: &mut Vec<(String, f64, Option<f64>)>,
    detected_rate: &mut Option<f64>,
) {
    let Some(arr) = obj.get("markers").and_then(Value::as_array) else {
        return;
    };
    for m in arr {
        let Some(m_obj) = m.as_object() else {
            continue;
        };
        if !schema_name(m_obj).eq_ignore_ascii_case("Marker") {
            continue;
        }
        let Some((local_start, dur, rate)) = m_obj.get("marked_range").and_then(parse_time_range)
        else {
            continue;
        };
        if detected_rate.is_none() {
            *detected_rate = Some(rate);
        }
        let label = m_obj
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("Marcador")
            .to_owned();
        let region_dur = (dur > 1e-4).then_some(dur);
        markers_out.push((label, (offset_s + local_start).max(0.0), region_dur));
    }
}

fn extract_media_reference(clip_obj: &Map<String, Value>) -> Option<&Map<String, Value>> {
    // OTIO Clip.1 (`media_reference`)
    if let Some(mr) = clip_obj.get("media_reference").and_then(Value::as_object) {
        return Some(mr);
    }
    // OTIO Clip.2 (`media_references` map + `active_media_reference_key`)
    let refs = clip_obj.get("media_references")?.as_object()?;
    let active_key = clip_obj
        .get("active_media_reference_key")
        .and_then(Value::as_str)
        .unwrap_or("DEFAULT_MEDIA");
    refs.get(active_key)
        .or_else(|| refs.get("DEFAULT_MEDIA"))
        .or_else(|| refs.values().next())
        .and_then(Value::as_object)
}

fn parse_track_children(
    track_obj: &Map<String, Value>,
    kind: TrackKind,
    track_num: usize,
    clips_out: &mut Vec<OtioClip>,
    markers_out: &mut Vec<(String, f64, Option<f64>)>,
    warnings: &mut Vec<String>,
    detected_rate: &mut Option<f64>,
) {
    collect_markers(track_obj, 0.0, markers_out, detected_rate);

    let Some(children) = track_obj.get("children").and_then(Value::as_array) else {
        return;
    };

    let mut cursor = 0.0_f64;
    // Pending `(in_offset, out_offset)` from an OTIO `Transition.1` preceding the next clip.
    let mut pending_transition: Option<(f64, f64)> = None;

    for item in children {
        let Some(item_obj) = item.as_object() else {
            continue;
        };
        match schema_name(item_obj) {
            "Gap" => {
                pending_transition = None;
                if let Some((_, dur, rate)) =
                    item_obj.get("source_range").and_then(parse_time_range)
                {
                    if detected_rate.is_none() {
                        *detected_rate = Some(rate);
                    }
                    cursor += dur.max(0.0);
                }
            }
            "Transition" => {
                let in_off = item_obj
                    .get("in_offset")
                    .and_then(parse_rational_time)
                    .map(|(s, _)| s.max(0.0))
                    .unwrap_or(0.0);
                let out_off = item_obj
                    .get("out_offset")
                    .and_then(parse_rational_time)
                    .map(|(s, _)| s.max(0.0))
                    .unwrap_or(0.0);
                if let Some(prev) = clips_out.last_mut() {
                    if prev.xml.kind == kind && prev.xml.track == track_num {
                        prev.xml.end += out_off;
                    }
                }
                pending_transition = Some((in_off, out_off));
            }
            "Clip" => {
                let (source_in, dur, rate) = match item_obj
                    .get("source_range")
                    .and_then(parse_time_range)
                    .or_else(|| {
                        extract_media_reference(item_obj)
                            .and_then(|mr| mr.get("available_range"))
                            .and_then(parse_time_range)
                    }) {
                    Some(r) => r,
                    None => {
                        warnings.push(
                            "Clipe OTIO sem source_range nem available_range ignorado".into(),
                        );
                        pending_transition = None;
                        continue;
                    }
                };
                if detected_rate.is_none() {
                    *detected_rate = Some(rate);
                }
                if dur <= 1e-6 {
                    pending_transition = None;
                    continue;
                }

                let mut playback_rate = 1.0_f64;
                let mut preserve_pitch = true;
                if let Some(fx_arr) = item_obj.get("effects").and_then(Value::as_array) {
                    for fx in fx_arr {
                        if let Some(fx_obj) = fx.as_object() {
                            if schema_name(fx_obj) == "LinearTimeWarp" {
                                if let Some(ts) = fx_obj.get("time_scale").and_then(Value::as_f64) {
                                    if ts.is_finite() && ts > 0.0 {
                                        playback_rate = ts.clamp(0.25, 4.0);
                                    }
                                }
                                if let Some(pp) = fx_obj
                                    .get("metadata")
                                    .and_then(|m| m.get("preserve_pitch"))
                                    .and_then(Value::as_bool)
                                {
                                    preserve_pitch = pp;
                                }
                            }
                        }
                    }
                }

                let (in_off, _) = pending_transition.take().unwrap_or((0.0, 0.0));
                let actual_in_off = in_off.min(cursor).min(source_in / playback_rate);

                let clip_start = (cursor - actual_in_off).max(0.0);
                let clip_end = cursor + dur;
                let adjusted_source_in = (source_in - actual_in_off * playback_rate).max(0.0);

                collect_markers(item_obj, cursor - source_in, markers_out, detected_rate);
                cursor += dur;

                let enabled = item_obj
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                if !enabled {
                    continue;
                }

                let mr = extract_media_reference(item_obj);
                let target_path = mr
                    .and_then(|m| m.get("target_url"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(url_to_path);
                let file_name = target_path.as_deref().and_then(file_name_of).or_else(|| {
                    mr.and_then(|m| m.get("name"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                });
                let clip_name = item_obj
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .or_else(|| file_name.clone())
                    .unwrap_or_else(|| "Clipe".into());

                let meta: ClipNeovegMeta = item_obj
                    .get("metadata")
                    .and_then(|m| m.get("neoveg"))
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                clips_out.push(OtioClip {
                    xml: XmlClip {
                        kind,
                        track: track_num,
                        name: clip_name,
                        file_name,
                        path: target_path,
                        start: clip_start,
                        end: clip_end,
                        source_in: adjusted_source_in,
                    },
                    playback_rate,
                    preserve_pitch,
                    fade_in: meta.fade_in.max(0.0),
                    fade_out: meta.fade_out.max(0.0),
                    fade_in_curve: meta.fade_in_curve,
                    fade_out_curve: meta.fade_out_curve,
                });
            }
            other => {
                pending_transition = None;
                warnings.push(format!("Item OTIO '{other}' ignorado na faixa"));
            }
        }
    }
}

/// Applies the clips, transitions, time-warps and markers of `doc` onto `timeline` as a single
/// undoable [`EditCommand`].
pub fn import_into_timeline(
    timeline: &mut Timeline,
    doc: &ParsedOtio,
    resolve: impl Fn(&XmlClip) -> Option<MediaMatch>,
) -> Option<(EditCommand, ImportReport)> {
    let mut report = ImportReport::default();
    let mut commands = Vec::new();
    let mut tracks: Vec<(TrackKind, usize, TrackId)> = Vec::new();
    let mut events: Vec<(TrackId, TrackEvent)> = Vec::new();

    for item in &doc.clips {
        let clip = &item.xml;
        let Some(media) = resolve(clip) else {
            report
                .skipped
                .push(format!("{}: mídia não encontrada no projeto", clip.name));
            continue;
        };
        let rate = item.playback_rate.clamp(0.25, 4.0);
        let trim_in = clip.source_in.max(0.0);
        let available_timeline = ((media.duration - trim_in).max(0.0)) / rate;
        let duration = (clip.end - clip.start).max(0.0).min(available_timeline);
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
                fade_in: item.fade_in.min(duration),
                fade_out: item.fade_out.min(duration),
                fade_in_curve: item.fade_in_curve,
                fade_out_curve: item.fade_out_curve,
                playback_rate: rate,
                preserve_pitch: item.preserve_pitch,
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

    #[test]
    fn otio_round_trips_tracks_gaps_crossfades_timewarp_and_markers_in_one_undo_step() {
        let mut src = Timeline::new();
        let mut v_track = src.new_track(TrackKind::Video);
        let v_id = v_track.id;
        v_track.events.push(TrackEvent {
            id: src.alloc_id(),
            name: "Plano A".into(),
            media_id: Some(1),
            start: 0.0,
            duration: 5.0,
            trim_in: 1.0,
            ..Default::default()
        });
        // Partially overlaps Plano A on [4.0, 5.0] -> exported as Transition.1(0.5s, 0.5s)
        v_track.events.push(TrackEvent {
            id: src.alloc_id(),
            name: "Plano B".into(),
            media_id: Some(1),
            start: 4.0,
            duration: 5.0,
            trim_in: 2.0,
            ..Default::default()
        });
        src.tracks.push(v_track);

        let mut a_track = src.new_track(TrackKind::Audio);
        a_track.muted = true;
        a_track.events.push(TrackEvent {
            id: src.alloc_id(),
            name: "Locução".into(),
            media_id: Some(2),
            start: 1.5,
            duration: 3.0,
            trim_in: 0.5,
            fade_in: 0.25,
            fade_in_curve: FadeCurve::Smooth,
            playback_rate: 1.5,
            preserve_pitch: false,
            ..Default::default()
        });
        src.tracks.push(a_track);

        let m_id = src.alloc_id();
        src.markers.push(markers::Marker {
            id: m_id,
            time: 2.0,
            name: "Corte".into(),
        });
        let r_id = src.alloc_id();
        src.regions.push(markers::Region {
            id: r_id,
            start: 3.0,
            end: 6.0,
            name: "Refrão".into(),
        });
        let _ = v_id;

        let media = vec![
            XmlMedia {
                id: 1,
                name: "camera.mp4".into(),
                path: "C:/Midia/camera.mp4".into(),
                duration: 30.0,
            },
            XmlMedia {
                id: 2,
                name: "voz.wav".into(),
                path: "C:/Midia/voz.wav".into(),
                duration: 20.0,
            },
        ];

        let json_text = export(&src, &media, "Demo OTIO", 24.0, 1920, 1080);
        assert!(json_text.contains("\"Timeline.1\""));
        assert!(json_text.contains("\"Transition.1\""));
        assert!(json_text.contains("\"LinearTimeWarp.1\""));

        let parsed = parse(&json_text).unwrap();
        assert_eq!(parsed.name, "Demo OTIO");
        assert!((parsed.fps - 24.0).abs() < 1e-6);
        assert_eq!(parsed.clips.len(), 3);
        assert_eq!(parsed.muted_tracks, vec![(TrackKind::Audio, 1)]);

        let mut dst = Timeline::new();
        let mut history = History::new();
        let (cmd, report) = import_into_timeline(&mut dst, &parsed, |c| {
            media
                .iter()
                .find(|m| c.file_name.as_deref() == Some(m.name.as_str()))
                .map(|m| MediaMatch {
                    media_id: m.id,
                    duration: m.duration,
                })
        })
        .unwrap();

        assert_eq!(report.added, 3);
        assert!(report.skipped.is_empty());

        let rev_before = history.revision();
        history.execute(&mut dst, cmd);
        assert_eq!(history.revision(), rev_before + 1);
        assert_eq!(dst.tracks.len(), 2);

        let v_events = &dst.tracks[0].events;
        assert_eq!(v_events.len(), 2);
        assert!((v_events[0].start - 0.0).abs() < 1e-5);
        assert!((v_events[0].duration - 5.0).abs() < 1e-5);
        assert!((v_events[0].trim_in - 1.0).abs() < 1e-5);
        assert!((v_events[1].start - 4.0).abs() < 1e-5);
        assert!((v_events[1].duration - 5.0).abs() < 1e-5);
        assert!((v_events[1].trim_in - 2.0).abs() < 1e-5);

        let a_ev = &dst.tracks[1].events[0];
        assert!(dst.tracks[1].muted);
        assert!((a_ev.start - 1.5).abs() < 1e-5);
        assert!((a_ev.duration - 3.0).abs() < 1e-5);
        assert!((a_ev.playback_rate - 1.5).abs() < 1e-5);
        assert!(!a_ev.preserve_pitch);
        assert!((a_ev.fade_in - 0.25).abs() < 1e-5);
        assert_eq!(a_ev.fade_in_curve, FadeCurve::Smooth);

        assert_eq!(dst.markers.len(), 1);
        assert_eq!(dst.regions.len(), 1);
        assert_eq!(dst.markers[0].name, "Corte");
        assert_eq!(dst.regions[0].name, "Refrão");

        history.undo(&mut dst);
        assert!(dst.tracks.is_empty());
        assert!(dst.markers.is_empty());
        assert!(dst.regions.is_empty());
    }

    #[test]
    fn otio_parses_clip_v2_media_references_and_rejects_invalid_files() {
        let external_otio = r#"{
            "OTIO_SCHEMA": "Timeline.1",
            "name": "Resolve Export",
            "global_start_time": { "OTIO_SCHEMA": "RationalTime.1", "rate": 23.976, "value": 0.0 },
            "tracks": {
                "OTIO_SCHEMA": "Stack.1",
                "name": "tracks",
                "children": [
                    {
                        "OTIO_SCHEMA": "Track.1",
                        "kind": "Video",
                        "name": "V1",
                        "children": [
                            {
                                "OTIO_SCHEMA": "Clip.2",
                                "name": "Take01",
                                "source_range": {
                                    "OTIO_SCHEMA": "TimeRange.1",
                                    "start_time": { "OTIO_SCHEMA": "RationalTime.1", "rate": 24.0, "value": 48.0 },
                                    "duration": { "OTIO_SCHEMA": "RationalTime.1", "rate": 24.0, "value": 72.0 }
                                },
                                "active_media_reference_key": "DEFAULT_MEDIA",
                                "media_references": {
                                    "DEFAULT_MEDIA": {
                                        "OTIO_SCHEMA": "ExternalReference.1",
                                        "target_url": "file:///media/take01.mov"
                                    }
                                }
                            }
                        ]
                    }
                ]
            }
        }"#;

        let parsed = parse(external_otio).unwrap();
        assert_eq!(parsed.name, "Resolve Export");
        assert!((parsed.fps - 23.976).abs() < 1e-4);
        assert_eq!(parsed.clips.len(), 1);
        assert_eq!(parsed.clips[0].xml.file_name.as_deref(), Some("take01.mov"));
        assert!((parsed.clips[0].xml.source_in - 2.0).abs() < 1e-6);
        assert!((parsed.clips[0].xml.end - 3.0).abs() < 1e-6);

        assert!(parse("not json at all").is_err());
        assert!(parse(r#"{"OTIO_SCHEMA": "Clip.1"}"#).is_err());
    }
}
