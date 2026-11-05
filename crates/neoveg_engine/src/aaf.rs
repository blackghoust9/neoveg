//! AAF (Advanced Authoring Format) interchange: export of the timeline and import of the clips of
//! a composition into the media that is already in the project.
//!
//! The file format is [`neoveg_aaf`] (clean-room AMWA AAF Edit Protocol implementation); this
//! module maps the timeline to its neutral composition model and back. Neoveg models tracks, clip
//! placement, source in and length, names, media paths, overlap transitions, markers, speed (playback rate),
//! fades and curves, volume/opacity, effect chains, embedded media, and nested compositions.
//!
//! The import produces [`AafDocument`], and can also yield the legacy [`XmlDocument`] for
//! compatibility with Final Cut Pro 7 workflows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use neoveg_aaf::model::{
    CClip, CEffect, CFade, CFadeCurve, CItem, CKind, CMarker, CTrack, CTransition, Composition,
    Document, Gain, Source,
};
use neoveg_aaf::time::{FrameRate, Tick};
use neoveg_common::fade::FadeCurve;

use crate::fcp7::{XmlClip, XmlDocument, XmlMedia};
use crate::timeline::{Timeline, TrackEffect, TrackEvent, TrackKind};

/// What the export needs to know about the project besides the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct AafProject<'a> {
    pub name: &'a str,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
}

/// Options controlling AAF export packaging.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AafExportOptions {
    /// 512-byte sectors (compound file version 3) instead of 4096-byte ones.
    pub small_sectors: bool,
    /// Embed picture/audio media directly in AAF EssenceData.
    pub embed_media: bool,
}

/// Media descriptor for AAF export, with optional embedded data and subprojects.
#[derive(Clone, Debug, PartialEq)]
pub struct AafMedia {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub duration: f64,
    pub embedded_audio_wav: Option<Vec<u8>>,
    pub embedded_picture: Option<Vec<u8>>,
    pub subproject: Option<Timeline>,
}

impl From<&XmlMedia> for AafMedia {
    fn from(m: &XmlMedia) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            path: m.path.clone(),
            duration: m.duration,
            embedded_audio_wav: None,
            embedded_picture: None,
            subproject: None,
        }
    }
}

/// A parsed AAF document with full metadata (speed, opacity, fades, effects, nested comps, embedded media).
#[derive(Clone, Debug, PartialEq)]
pub struct AafDocument {
    pub name: String,
    pub fps: f64,
    pub clips: Vec<AafClip>,
    pub nested: Vec<AafComposition>,
    pub embedded_files: Vec<AafEmbeddedFile>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AafClip {
    pub kind: TrackKind,
    pub track: usize,
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub source_in: f64,
    pub file_name: Option<String>,
    pub path: Option<String>,
    pub speed: Option<f64>,
    pub opacity: Option<f64>,
    pub gain_db: Option<f64>,
    pub fade_in_sec: Option<f64>,
    pub fade_in_curve: Option<FadeCurve>,
    pub fade_out_sec: Option<f64>,
    pub fade_out_curve: Option<FadeCurve>,
    pub effects: Vec<TrackEffect>,
    pub nested_index: Option<usize>,
    pub embedded_file_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AafComposition {
    pub name: String,
    pub fps: f64,
    pub clips: Vec<AafClip>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AafEmbeddedFile {
    pub name: String,
    pub kind: TrackKind,
    pub bytes: Vec<u8>,
}

fn to_aaf_fade_curve(c: FadeCurve) -> CFadeCurve {
    match c {
        FadeCurve::Linear => CFadeCurve::Linear,
        FadeCurve::Fast => CFadeCurve::Fast,
        FadeCurve::Slow => CFadeCurve::Slow,
        FadeCurve::Smooth => CFadeCurve::Smooth,
        FadeCurve::Sharp => CFadeCurve::Sharp,
    }
}

fn from_aaf_fade_curve(c: CFadeCurve) -> FadeCurve {
    match c {
        CFadeCurve::Linear => FadeCurve::Linear,
        CFadeCurve::Fast => FadeCurve::Fast,
        CFadeCurve::Slow => FadeCurve::Slow,
        CFadeCurve::Smooth => FadeCurve::Smooth,
        CFadeCurve::Sharp => FadeCurve::Sharp,
    }
}

/// The frame rate as the exact rational AAF wants (`30000/1001` for 29.97).
pub fn frame_rate(fps: f64) -> FrameRate {
    let fps = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        25.0
    };
    for (num, den) in [(24000, 1001), (30000, 1001), (60000, 1001), (120000, 1001)] {
        if (fps - num as f64 / den as f64).abs() < 0.005 {
            return FrameRate::new(num, den);
        }
    }
    if (fps - fps.round()).abs() < 0.005 {
        FrameRate::new(fps.round().max(1.0) as i64, 1)
    } else {
        FrameRate::new((fps * 1000.0).round().max(1.0) as i64, 1000)
    }
}

/// Half of `length`, on a whole frame (the cut of a transition lies on a frame).
fn half_in_frames(length: Tick, rate: FrameRate) -> Tick {
    let frames = length.seconds() * rate.num as f64 / rate.den.max(1) as f64;
    let half = (frames / 2.0).round();
    Tick::from_seconds_f64(half * rate.den.max(1) as f64 / rate.num.max(1) as f64)
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[allow(clippy::too_many_arguments)]
fn build_composition(
    name: &str,
    timeline: &Timeline,
    media: &[AafMedia],
    project: &AafProject<'_>,
    options: &AafExportOptions,
    doc: &mut Document,
    source_of: &mut HashMap<(u64, CKind), usize>,
    warn: &mut impl FnMut(&str),
) -> Result<Composition, String> {
    let rate = frame_rate(project.fps);
    let mut comp = Composition {
        name: name.to_owned(),
        rate,
        sample_rate: project.sample_rate.max(1),
        width: project.width,
        height: project.height,
        start_tc: 0,
        drop: false,
        tracks: Vec::new(),
        markers: timeline
            .markers
            .iter()
            .map(|m| CMarker {
                start: Tick::from_seconds_f64(m.time.max(0.0)),
                duration: Tick::ZERO,
                name: m.name.clone(),
                comment: String::new(),
                color: None,
            })
            .collect(),
    };

    for (kind, ckind) in [
        (TrackKind::Video, CKind::Picture),
        (TrackKind::Audio, CKind::Sound),
    ] {
        let mut number = 0u32;
        for track in timeline.tracks.iter().filter(|t| t.kind == kind) {
            let mut events: Vec<_> = track.events.iter().collect();
            events.sort_by(|a, b| a.start.total_cmp(&b.start));
            let mut items: Vec<CItem> = Vec::new();
            let mut cursor = Tick::ZERO;
            for event in events {
                if event.muted {
                    warn("eventos silenciados não foram exportados");
                    continue;
                }
                let Some(info) = event
                    .media_id
                    .and_then(|id| media.iter().find(|m| m.id == id))
                else {
                    warn("eventos sem mídia de arquivo (títulos, geradores, ajustes) não foram exportados");
                    continue;
                };

                let start = Tick::from_seconds_f64(event.start.max(0.0));
                let mut len = Tick::from_seconds_f64(event.duration.max(0.0));
                if len <= Tick::ZERO {
                    continue;
                }
                let mut at = start;
                if at > cursor {
                    items.push(CItem::Filler(at - cursor));
                } else if at < cursor {
                    let overlap = cursor - at;
                    let previous = items.last().map(CItem::len).unwrap_or(Tick::ZERO);
                    if matches!(items.last(), Some(CItem::Clip(_)))
                        && overlap < previous
                        && overlap < len
                    {
                        items.push(CItem::Transition(CTransition {
                            len: overlap,
                            cut: half_in_frames(overlap, rate),
                            effect: "cross_dissolve".into(),
                        }));
                    } else {
                        warn("eventos sobrepostos além de um crossfade foram encurtados");
                        len = (start + len) - cursor;
                        at = cursor;
                        if len <= Tick::ZERO {
                            continue;
                        }
                    }
                }

                let index = if let Some(&idx) = source_of.get(&(info.id, ckind)) {
                    idx
                } else {
                    let sound = ckind == CKind::Sound;
                    let mut embedded = None;
                    if options.embed_media {
                        embedded = if sound {
                            info.embedded_audio_wav.clone()
                        } else {
                            info.embedded_picture.clone()
                        };
                    }
                    let mut nested = None;
                    if let Some(sub_tl) = &info.subproject {
                        if let Ok(nested_comp) = build_composition(
                            &info.name, sub_tl, media, project, options, doc, source_of, warn,
                        ) {
                            let n_idx = doc.nested.len();
                            doc.nested.push(nested_comp);
                            nested = Some(n_idx);
                        }
                    }

                    let idx = doc.sources.len();
                    doc.sources.push(Source {
                        key: format!("media:{}", info.id),
                        name: info.name.clone(),
                        kind: ckind,
                        path: if nested.is_some() || embedded.is_some() {
                            None
                        } else {
                            Some(info.path.clone())
                        },
                        channel: None,
                        channels: if sound { 2 } else { 0 },
                        file_channels: if sound { 2 } else { 0 },
                        width: if sound { 0 } else { project.width },
                        height: if sound { 0 } else { project.height },
                        frame_rate: rate,
                        sample_rate: if sound { project.sample_rate.max(1) } else { 0 },
                        bits: if sound { 16 } else { 0 },
                        length: Tick::from_seconds_f64(info.duration.max(0.0)),
                        offset: Tick::ZERO,
                        start_tc: None,
                        tc_rate: rate,
                        embedded,
                        markers: Vec::new(),
                        nested,
                    });
                    source_of.insert((info.id, ckind), idx);
                    idx
                };

                let speed = if (event.playback_rate - 1.0).abs() > 1e-4 {
                    Some(event.playback_rate)
                } else {
                    None
                };

                let fade_in = if event.fade_in > 0.0 {
                    Some(CFade {
                        len: Tick::from_seconds_f64(event.fade_in),
                        curve: to_aaf_fade_curve(event.fade_in_curve),
                    })
                } else {
                    None
                };

                let fade_out = if event.fade_out > 0.0 {
                    Some(CFade {
                        len: Tick::from_seconds_f64(event.fade_out),
                        curve: to_aaf_fade_curve(event.fade_out_curve),
                    })
                } else {
                    None
                };

                let gain = if ckind == CKind::Sound && (event.gain_db).abs() > 1e-3 {
                    Some(Gain::Constant(event.gain_linear() as f64))
                } else {
                    None
                };

                let opacity = if ckind == CKind::Picture && (event.opacity - 1.0).abs() > 1e-3 {
                    Some(Gain::Constant(event.opacity as f64))
                } else {
                    None
                };

                let mut effects = Vec::new();
                let raw_effects = if ckind == CKind::Picture {
                    &event.video_effects
                } else {
                    &event.audio_effects
                };
                for eff in raw_effects {
                    if !eff.bypass {
                        let params = eff
                            .params
                            .iter()
                            .map(|(k, v)| (k.clone(), *v as f64))
                            .collect();
                        effects.push(CEffect {
                            id: eff.kind.clone(),
                            name: eff.kind.clone(),
                            params,
                        });
                    }
                }

                items.push(CItem::Clip(CClip {
                    len,
                    source: index,
                    start: Tick::from_seconds_f64(event.trim_in.max(0.0)),
                    gain,
                    opacity,
                    speed,
                    fade_in,
                    fade_out,
                    effects,
                    name: if event.name.is_empty() {
                        file_name(&info.path).to_owned()
                    } else {
                        event.name.clone()
                    },
                }));
                cursor = at + len;
            }
            if items.iter().any(|i| matches!(i, CItem::Clip(_))) {
                number += 1;
                comp.tracks.push(CTrack {
                    kind: ckind,
                    name: format!(
                        "{}{number}",
                        if kind == TrackKind::Video { "V" } else { "A" }
                    ),
                    number,
                    channels: if ckind == CKind::Sound { 2 } else { 1 },
                    items,
                });
            }
        }
    }

    Ok(comp)
}

/// Advanced export of the timeline with customizable options (sector size, embedded media, subprojects).
pub fn export_with_options(
    timeline: &Timeline,
    media: &[AafMedia],
    project: &AafProject<'_>,
    options: &AafExportOptions,
) -> Result<(Vec<u8>, Vec<String>), String> {
    if timeline.tracks.iter().all(|t| t.events.is_empty()) {
        return Err("a linha do tempo está vazia".into());
    }
    let mut warnings: Vec<String> = Vec::new();
    let mut warn = |text: &str| {
        if !warnings.iter().any(|w| w == text) {
            warnings.push(text.to_owned());
        }
    };
    let mut doc = Document::default();
    let mut source_of: HashMap<(u64, CKind), usize> = HashMap::new();

    let comp = build_composition(
        project.name,
        timeline,
        media,
        project,
        options,
        &mut doc,
        &mut source_of,
        &mut warn,
    )?;

    if comp.tracks.is_empty() {
        return Err("nenhum evento da linha do tempo tem mídia de arquivo para exportar".into());
    }
    doc.compositions.push(comp);

    let (bytes, report) =
        neoveg_aaf::write_aaf(&doc, options.small_sectors).map_err(|e| e.to_string())?;

    warnings.extend(
        report
            .entries
            .into_iter()
            .filter(|e| e.level == neoveg_aaf::Level::Warning)
            .map(|e| e.message),
    );
    Ok((bytes, warnings))
}

/// Writes the timeline as an AAF file; the second value lists what was left out or approximated.
/// `small_sectors` writes 512-byte sectors (compound file version 3).
pub fn export(
    timeline: &Timeline,
    media: &[XmlMedia],
    project: &AafProject<'_>,
    small_sectors: bool,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let aaf_media: Vec<AafMedia> = media.iter().map(AafMedia::from).collect();
    let options = AafExportOptions {
        small_sectors,
        embed_media: false,
    };
    export_with_options(timeline, &aaf_media, project, &options)
}

fn parse_comp_clips(
    comp: &Composition,
    doc: &Document,
    source_to_embedded: &HashMap<usize, usize>,
) -> Vec<AafClip> {
    let mut clips = Vec::new();
    let (mut video, mut audio) = (0usize, 0usize);
    for track in &comp.tracks {
        let (kind, track_idx) = match track.kind {
            CKind::Picture => {
                video += 1;
                (TrackKind::Video, video - 1)
            }
            CKind::Sound => {
                audio += 1;
                (TrackKind::Audio, audio - 1)
            }
        };
        let mut cursor = Tick::ZERO;
        for item in &track.items {
            match item {
                CItem::Filler(len) => cursor += *len,
                CItem::Transition(t) => cursor -= t.len,
                CItem::Clip(clip) => {
                    let source = doc.sources.get(clip.source);
                    let file_name_val = source
                        .and_then(|s| s.path.as_deref())
                        .map(|p| file_name(p).to_owned())
                        .or_else(|| source.map(|s| s.name.clone()));
                    let path = source.and_then(|s| s.path.clone());
                    let offset = source.map(|s| s.offset).unwrap_or(Tick::ZERO);
                    let source_in = (clip.start - offset).seconds().max(0.0);
                    let speed = clip.speed;
                    let opacity = clip.opacity.as_ref().map(|g| match g {
                        Gain::Constant(v) => *v,
                        Gain::Varying { points, .. } => points.first().map(|p| p.1).unwrap_or(1.0),
                    });
                    let gain_db = clip.gain.as_ref().map(|g| match g {
                        Gain::Constant(v) => {
                            if *v <= 1e-4 {
                                -60.0
                            } else {
                                (20.0 * v.log10()).clamp(-60.0, 12.0)
                            }
                        }
                        Gain::Varying { points, .. } => {
                            let v = points.first().map(|p| p.1).unwrap_or(1.0);
                            if v <= 1e-4 {
                                -60.0
                            } else {
                                (20.0 * v.log10()).clamp(-60.0, 12.0)
                            }
                        }
                    });
                    let fade_in_sec = clip.fade_in.as_ref().map(|f| f.len.seconds());
                    let fade_in_curve = clip.fade_in.as_ref().map(|f| from_aaf_fade_curve(f.curve));
                    let fade_out_sec = clip.fade_out.as_ref().map(|f| f.len.seconds());
                    let fade_out_curve =
                        clip.fade_out.as_ref().map(|f| from_aaf_fade_curve(f.curve));
                    let effects = clip
                        .effects
                        .iter()
                        .map(|eff| {
                            let mut te = TrackEffect::new(&eff.id);
                            for (k, v) in &eff.params {
                                te.params.insert(k.clone(), *v as f32);
                            }
                            te
                        })
                        .collect();
                    let nested_index = source.and_then(|s| s.nested);
                    let embedded_file_index = source_to_embedded.get(&clip.source).copied();

                    clips.push(AafClip {
                        kind,
                        track: track_idx,
                        name: if clip.name.is_empty() {
                            source.map(|s| s.name.clone()).unwrap_or_default()
                        } else {
                            clip.name.clone()
                        },
                        start: cursor.seconds(),
                        end: (cursor + clip.len).seconds(),
                        source_in,
                        file_name: file_name_val,
                        path,
                        speed,
                        opacity,
                        gain_db,
                        fade_in_sec,
                        fade_in_curve,
                        fade_out_sec,
                        fade_out_curve,
                        effects,
                        nested_index,
                        embedded_file_index,
                    });
                    cursor += clip.len;
                }
            }
        }
    }
    clips
}

/// Reads an AAF file into structured [`AafDocument`] including nested compositions, speed, fades, and embedded essence.
pub fn parse_aaf_document(bytes: &[u8]) -> Result<AafDocument, String> {
    let (doc, report) = neoveg_aaf::read_aaf(bytes).map_err(|e| e.to_string())?;
    let comp = doc
        .compositions
        .first()
        .ok_or_else(|| "o arquivo AAF não tem composição".to_string())?;

    let mut embedded_files = Vec::new();
    let mut source_to_embedded: HashMap<usize, usize> = HashMap::new();
    for (s_idx, s) in doc.sources.iter().enumerate() {
        if let Some(bytes) = &s.embedded {
            let name = if !s.name.is_empty() {
                s.name.clone()
            } else {
                format!("midia_embutida_{s_idx}")
            };
            let kind = if s.kind == CKind::Picture {
                TrackKind::Video
            } else {
                TrackKind::Audio
            };
            let emb_idx = embedded_files.len();
            embedded_files.push(AafEmbeddedFile {
                name,
                kind,
                bytes: bytes.clone(),
            });
            source_to_embedded.insert(s_idx, emb_idx);
        }
    }

    let mut out = AafDocument {
        name: comp.name.clone(),
        fps: comp.rate.num as f64 / comp.rate.den.max(1) as f64,
        clips: parse_comp_clips(comp, &doc, &source_to_embedded),
        nested: Vec::new(),
        embedded_files,
        warnings: report.entries.into_iter().map(|e| e.message).collect(),
    };

    for nested_comp in &doc.nested {
        out.nested.push(AafComposition {
            name: nested_comp.name.clone(),
            fps: nested_comp.rate.num as f64 / nested_comp.rate.den.max(1) as f64,
            clips: parse_comp_clips(nested_comp, &doc, &source_to_embedded),
        });
    }

    Ok(out)
}

/// Writes all embedded media files extracted from an AAF document into `target_dir`.
pub fn materialize_embedded_media(
    aaf_doc: &AafDocument,
    target_dir: &Path,
) -> Result<Vec<(usize, PathBuf)>, String> {
    if !target_dir.is_dir() {
        std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    }
    let mut materialized = Vec::new();
    for (idx, emb) in aaf_doc.embedded_files.iter().enumerate() {
        let mut file_name = emb.name.clone();
        if !file_name.contains('.') {
            let ext = match emb.kind {
                TrackKind::Video => "bin",
                TrackKind::Audio => "wav",
            };
            file_name.push('.');
            file_name.push_str(ext);
        }
        let safe_name = file_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let dest = target_dir.join(format!("{idx:02}_{safe_name}"));
        std::fs::write(&dest, &emb.bytes).map_err(|e| e.to_string())?;
        materialized.push((idx, dest));
    }
    Ok(materialized)
}

/// Imports clips from [`AafDocument`] directly into `timeline`, resolving media paths against `media_map`.
pub fn import_aaf_into_timeline(
    aaf_doc: &AafDocument,
    timeline: &mut Timeline,
    media_map: &HashMap<String, u64>,
) {
    let mut video_track_map: HashMap<usize, usize> = HashMap::new();
    let mut audio_track_map: HashMap<usize, usize> = HashMap::new();

    for clip in &aaf_doc.clips {
        let (track_map, kind) = match clip.kind {
            TrackKind::Video => (&mut video_track_map, TrackKind::Video),
            TrackKind::Audio => (&mut audio_track_map, TrackKind::Audio),
        };
        let target_track_idx = *track_map.entry(clip.track).or_insert_with(|| {
            let mut tr = timeline.new_track(kind);
            tr.name = format!(
                "{}{}",
                if kind == TrackKind::Video {
                    "Vídeo "
                } else {
                    "Áudio "
                },
                clip.track + 1
            );
            timeline.tracks.push(tr);
            timeline.tracks.len() - 1
        });

        let media_id = clip
            .path
            .as_ref()
            .and_then(|p| media_map.get(p).copied())
            .or_else(|| {
                clip.file_name
                    .as_ref()
                    .and_then(|f| media_map.get(f).copied())
            });

        let event = TrackEvent {
            id: timeline.alloc_id(),
            name: clip.name.clone(),
            media_id,
            start: clip.start,
            duration: (clip.end - clip.start).max(0.0),
            trim_in: clip.source_in,
            fade_in: clip.fade_in_sec.unwrap_or(0.0),
            fade_out: clip.fade_out_sec.unwrap_or(0.0),
            fade_in_curve: clip.fade_in_curve.unwrap_or_default(),
            fade_out_curve: clip.fade_out_curve.unwrap_or_default(),
            playback_rate: clip.speed.unwrap_or(1.0),
            opacity: clip.opacity.unwrap_or(1.0) as f32,
            gain_db: clip.gain_db.unwrap_or(0.0) as f32,
            video_effects: if clip.kind == TrackKind::Video {
                clip.effects.clone()
            } else {
                Vec::new()
            },
            audio_effects: if clip.kind == TrackKind::Audio {
                clip.effects.clone()
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        if let Some(t) = timeline.tracks.get_mut(target_track_idx) {
            t.events.push(event);
        }
    }
}

/// Reads the first composition of an AAF file into the clips it places. Times are seconds.
pub fn parse(bytes: &[u8]) -> Result<XmlDocument, String> {
    let aaf_doc = parse_aaf_document(bytes)?;
    let out = XmlDocument {
        name: aaf_doc.name,
        fps: aaf_doc.fps,
        clips: aaf_doc
            .clips
            .into_iter()
            .map(|c| XmlClip {
                kind: c.kind,
                track: c.track,
                name: c.name,
                start: c.start,
                end: c.end,
                source_in: c.source_in,
                file_name: c.file_name,
                path: c.path,
            })
            .collect(),
        warnings: aaf_doc.warnings,
    };
    Ok(out)
}

/// Whether `bytes` look like an AAF file.
pub fn sniff(bytes: &[u8]) -> bool {
    neoveg_aaf::sniff(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TrackEvent;

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
        add(&mut tl, &mut video, 0.0, 4.0, 2.0, 1, "Entrevista");
        // Overlaps the first by one second: a crossfade
        add(&mut tl, &mut video, 3.0, 2.0, 0.0, 2, "Cobertura");
        add(&mut tl, &mut video, 8.0, 1.0, 5.0, 1, "Final");
        add(&mut tl, &mut audio, 1.0, 3.0, 0.5, 1, "Voz");
        tl.tracks.push(video);
        tl.tracks.push(audio);
        tl.markers.push(crate::markers::Marker {
            id: 1,
            time: 2.0,
            name: "Ponto".into(),
        });
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

    fn settings() -> AafProject<'static> {
        AafProject {
            name: "Meu filme",
            fps: 25.0,
            width: 1920,
            height: 1080,
            sample_rate: 48_000,
        }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.005
    }

    #[test]
    fn roundtrip_timeline_to_aaf_and_back() {
        let (tl, media) = project();
        let (bytes, warnings) = export(&tl, &media, &settings(), false).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(sniff(&bytes));

        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.name, "Meu filme");
        assert!(near(doc.fps, 25.0));

        let video: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.kind == TrackKind::Video)
            .collect();
        let audio: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.kind == TrackKind::Audio)
            .collect();
        assert_eq!(video.len(), 3);
        assert_eq!(audio.len(), 1);

        let expect = [
            (0.0, 4.0, 2.0, "Entrevista"),
            (3.0, 5.0, 0.0, "Cobertura"),
            (8.0, 9.0, 5.0, "Final"),
        ];
        for (clip, (start, end, trim, name)) in video.iter().zip(expect) {
            assert!(near(clip.start, start) && near(clip.end, end), "{clip:?}");
            assert!(near(clip.source_in, trim), "{clip:?}");
            assert_eq!(clip.name, name);
            assert_eq!(clip.track, 0);
        }
        assert!(
            near(audio[0].start, 1.0) && near(audio[0].end, 4.0) && near(audio[0].source_in, 0.5)
        );
        assert_eq!(
            video[0].path.as_deref().map(|p| p.replace('\\', "/")),
            Some("C:/Vídeos/meu projeto/entrevista.mp4".to_string())
        );
        assert_eq!(video[0].file_name.as_deref(), Some("entrevista.mp4"));
        assert_eq!(video[1].path.as_deref(), Some("/home/u/cobertura.mov"));
    }

    #[test]
    fn roundtrip_speed_fades_volume_effects_embedded_media_and_subprojects() {
        let mut tl = Timeline::new();
        let mut v_track = tl.new_track(TrackKind::Video);
        let mut a_track = tl.new_track(TrackKind::Audio);

        let mut v_effect = TrackEffect::new("ofx.blur");
        v_effect.params.insert("radius".into(), 10.0);

        let v_event = TrackEvent {
            id: 1,
            name: "Video 1".into(),
            media_id: Some(10),
            start: 0.0,
            duration: 5.0,
            trim_in: 1.0,
            playback_rate: 1.25,
            opacity: 0.8,
            fade_in: 1.0,
            fade_in_curve: FadeCurve::Smooth,
            fade_out: 1.5,
            fade_out_curve: FadeCurve::Sharp,
            video_effects: vec![v_effect],
            ..Default::default()
        };
        v_track.events.push(v_event);

        let mut a_effect = TrackEffect::new("vst.eq");
        a_effect.params.insert("gain".into(), 3.5);

        let a_event = TrackEvent {
            id: 2,
            name: "Audio 1".into(),
            media_id: Some(20),
            start: 0.0,
            duration: 5.0,
            trim_in: 0.0,
            playback_rate: 1.0,
            gain_db: -6.0,
            fade_in: 0.5,
            fade_in_curve: FadeCurve::Fast,
            fade_out: 0.5,
            fade_out_curve: FadeCurve::Slow,
            audio_effects: vec![a_effect],
            ..Default::default()
        };
        a_track.events.push(a_event);

        tl.tracks.push(v_track);
        tl.tracks.push(a_track);

        // Subproject timeline
        let mut sub_tl = Timeline::new();
        let mut sub_track = sub_tl.new_track(TrackKind::Video);
        sub_track.events.push(TrackEvent {
            id: 100,
            name: "SubEvent".into(),
            media_id: Some(30),
            start: 0.0,
            duration: 3.0,
            trim_in: 0.0,
            ..Default::default()
        });
        sub_tl.tracks.push(sub_track);

        let media = vec![
            AafMedia {
                id: 10,
                name: "clip1.mp4".into(),
                path: "C:/media/clip1.mp4".into(),
                duration: 10.0,
                embedded_audio_wav: None,
                embedded_picture: Some(vec![0xAA, 0xBB, 0xCC, 0xDD]),
                subproject: None,
            },
            AafMedia {
                id: 20,
                name: "audio.wav".into(),
                path: "C:/media/audio.wav".into(),
                duration: 10.0,
                embedded_audio_wav: Some(neoveg_aaf::encode_wav(&[0; 4800], 1, 48000, 16)),
                embedded_picture: None,
                subproject: None,
            },
            AafMedia {
                id: 30,
                name: "nested_subproject.neoveg".into(),
                path: "C:/media/nested.neoveg".into(),
                duration: 5.0,
                embedded_audio_wav: None,
                embedded_picture: None,
                subproject: Some(sub_tl),
            },
        ];

        let options = AafExportOptions {
            small_sectors: false,
            embed_media: true,
        };

        let (bytes, warnings) = export_with_options(&tl, &media, &settings(), &options).unwrap();
        assert!(warnings.is_empty(), "Warnings: {warnings:?}");

        let aaf_doc = parse_aaf_document(&bytes).unwrap();
        assert_eq!(aaf_doc.clips.len(), 2);

        let v_clip = aaf_doc
            .clips
            .iter()
            .find(|c| c.kind == TrackKind::Video)
            .unwrap();
        assert_eq!(v_clip.speed, Some(1.25));
        assert!(near(v_clip.opacity.unwrap(), 0.8));
        assert_eq!(v_clip.fade_in_sec, Some(1.0));
        assert_eq!(v_clip.fade_in_curve, Some(FadeCurve::Smooth));
        assert_eq!(v_clip.fade_out_sec, Some(1.5));
        assert_eq!(v_clip.fade_out_curve, Some(FadeCurve::Sharp));
        assert_eq!(v_clip.effects.len(), 1);
        assert_eq!(v_clip.effects[0].kind, "ofx.blur");
        assert_eq!(v_clip.effects[0].params.get("radius"), Some(&10.0));

        let a_clip = aaf_doc
            .clips
            .iter()
            .find(|c| c.kind == TrackKind::Audio)
            .unwrap();
        assert!(near(a_clip.gain_db.unwrap(), -6.0));
        assert_eq!(a_clip.fade_in_sec, Some(0.5));
        assert_eq!(a_clip.fade_in_curve, Some(FadeCurve::Fast));
        assert_eq!(a_clip.fade_out_sec, Some(0.5));
        assert_eq!(a_clip.fade_out_curve, Some(FadeCurve::Slow));
        assert_eq!(a_clip.effects.len(), 1);
        assert_eq!(a_clip.effects[0].kind, "vst.eq");
        assert_eq!(a_clip.effects[0].params.get("gain"), Some(&3.5));

        // Verify embedded files
        assert_eq!(aaf_doc.embedded_files.len(), 2);

        // Verify import into timeline
        let mut imported_tl = Timeline::new();
        let mut map = HashMap::new();
        map.insert("C:/media/clip1.mp4".into(), 1001);
        map.insert("C:/media/audio.wav".into(), 1002);
        import_aaf_into_timeline(&aaf_doc, &mut imported_tl, &map);
        assert_eq!(imported_tl.tracks.len(), 2);
    }

    #[test]
    fn both_sector_sizes_work_and_what_cannot_be_written_is_reported() {
        let (mut tl, media) = project();
        for small in [false, true] {
            let (bytes, _) = export(&tl, &media, &settings(), small).unwrap();
            assert!(sniff(&bytes));
            assert_eq!(parse(&bytes).unwrap().clips.len(), 4);
        }
        let id = tl.alloc_id();
        tl.tracks[0].events.push(TrackEvent {
            id,
            name: "Título".into(),
            media_id: None,
            start: 10.0,
            duration: 2.0,
            ..Default::default()
        });
        let (bytes, warnings) = export(&tl, &media, &settings(), false).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(parse(&bytes).unwrap().clips.len(), 4);
        assert!(export(&Timeline::new(), &media, &settings(), false).is_err());
        assert!(export(&tl, &[], &settings(), false).is_err());
    }

    #[test]
    fn a_file_that_is_not_aaf_is_refused_without_a_crash() {
        assert!(!sniff(b"not a compound file"));
        assert!(parse(b"not a compound file").is_err());
        assert!(parse(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0, 0]).is_err());
    }

    #[test]
    fn the_frame_rate_is_an_exact_rational() {
        assert_eq!(
            (frame_rate(29.97).num, frame_rate(29.97).den),
            (30000, 1001)
        );
        assert_eq!(
            (frame_rate(23.976).num, frame_rate(23.976).den),
            (24000, 1001)
        );
        assert_eq!((frame_rate(25.0).num, frame_rate(25.0).den), (25, 1));
        assert_eq!(
            (frame_rate(f64::NAN).num, frame_rate(f64::NAN).den),
            (25, 1)
        );
    }
}
