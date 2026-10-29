//! AAF (Advanced Authoring Format) interchange: export of the timeline and import of the clips of
//! a composition into the media that is already in the project.
//!
//! The file format is [`neoveg_aaf`] (adapted from FilmCraft); this module maps the timeline to
//! its neutral composition model and back. What Neoveg models is written: tracks, clip placement,
//! source in and length, names, media paths, the overlap of two events as a transition and the
//! markers. Speed, fades, volume and effects are not.
//!
//! The import produces the same [`XmlDocument`] the Final Cut Pro 7 XML import does, so the clips
//! are placed by [`crate::fcp7::import_into_timeline`].

use std::collections::HashMap;

use neoveg_aaf::model::{
    CClip, CItem, CKind, CMarker, CTrack, CTransition, Composition, Document, Source,
};
use neoveg_aaf::time::{FrameRate, Tick};

use crate::fcp7::{XmlClip, XmlDocument, XmlMedia};
use crate::timeline::{Timeline, TrackKind};

/// What the export needs to know about the project besides the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct AafProject<'a> {
    pub name: &'a str,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
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

/// Writes the timeline as an AAF file; the second value lists what was left out or approximated.
/// `small_sectors` writes 512-byte sectors (compound file version 3).
pub fn export(
    timeline: &Timeline,
    media: &[XmlMedia],
    project: &AafProject<'_>,
    small_sectors: bool,
) -> Result<(Vec<u8>, Vec<String>), String> {
    if timeline.tracks.iter().all(|t| t.events.is_empty()) {
        return Err("a linha do tempo está vazia".into());
    }
    let rate = frame_rate(project.fps);
    let mut warnings: Vec<String> = Vec::new();
    let mut warn = |text: &str| {
        if !warnings.iter().any(|w| w == text) {
            warnings.push(text.to_owned());
        }
    };
    let mut doc = Document::default();
    let mut source_of: HashMap<(u64, CKind), usize> = HashMap::new();
    let mut comp = Composition {
        name: project.name.to_owned(),
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
                if (event.playback_rate - 1.0).abs() > 1e-9 {
                    warn("a velocidade dos eventos não foi exportada");
                }
                let start = Tick::from_seconds_f64(event.start.max(0.0));
                let mut len = Tick::from_seconds_f64(event.duration.max(0.0));
                if len <= Tick::ZERO {
                    continue;
                }
                let mut at = start;
                if at > cursor {
                    items.push(CItem::Filler(at - cursor));
                } else if at < cursor {
                    // Two events that overlap are a transition: it spans the overlap
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
                let index = *source_of.entry((info.id, ckind)).or_insert_with(|| {
                    let sound = ckind == CKind::Sound;
                    doc.sources.push(Source {
                        key: format!("media:{}", info.id),
                        name: info.name.clone(),
                        kind: ckind,
                        path: Some(info.path.clone()),
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
                        embedded: None,
                        markers: Vec::new(),
                        nested: None,
                    });
                    doc.sources.len() - 1
                });
                items.push(CItem::Clip(CClip {
                    len,
                    source: index,
                    start: Tick::from_seconds_f64(event.trim_in.max(0.0)),
                    gain: None,
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
    if comp.tracks.is_empty() {
        return Err("nenhum evento da linha do tempo tem mídia de arquivo para exportar".into());
    }
    doc.compositions.push(comp);
    let (bytes, report) = neoveg_aaf::write_aaf(&doc, small_sectors).map_err(|e| e.to_string())?;
    // Only what was dropped or approximated, not the assumptions the writer notes
    warnings.extend(
        report
            .entries
            .into_iter()
            .filter(|e| e.level == neoveg_aaf::Level::Warning)
            .map(|e| e.message),
    );
    Ok((bytes, warnings))
}

/// Reads the first composition of an AAF file into the clips it places. Times are seconds.
pub fn parse(bytes: &[u8]) -> Result<XmlDocument, String> {
    let (doc, report) = neoveg_aaf::read_aaf(bytes).map_err(|e| e.to_string())?;
    let comp = doc
        .compositions
        .first()
        .ok_or_else(|| "o arquivo AAF não tem composição".to_string())?;
    let mut out = XmlDocument {
        name: comp.name.clone(),
        fps: comp.rate.num as f64 / comp.rate.den.max(1) as f64,
        clips: Vec::new(),
        warnings: report.entries.into_iter().map(|e| e.message).collect(),
    };
    let (mut video, mut audio) = (0usize, 0usize);
    for track in &comp.tracks {
        let (kind, index) = match track.kind {
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
                    match source {
                        Some(s) if s.nested.is_none() => out.clips.push(XmlClip {
                            kind,
                            track: index,
                            name: if clip.name.is_empty() {
                                s.name.clone()
                            } else {
                                clip.name.clone()
                            },
                            start: cursor.seconds(),
                            end: (cursor + clip.len).seconds(),
                            source_in: (clip.start - s.offset).seconds().max(0.0),
                            file_name: s.path.as_deref().map(|p| file_name(p).to_owned()),
                            path: s.path.clone(),
                        }),
                        _ => out.warnings.push(
                            "um clipe de composição aninhada ou sem mídia foi ignorado".into(),
                        ),
                    }
                    cursor += clip.len;
                }
            }
        }
    }
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

    #[test]
    fn the_timeline_goes_out_as_aaf_and_the_clips_come_back_where_they_were() {
        let (tl, media) = project();
        let (bytes, warnings) = export(&tl, &media, &settings(), false).unwrap();
        assert!(sniff(&bytes), "a compound file with the AAF root");
        assert!(warnings.is_empty(), "{warnings:?}");
        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.name, "Meu filme");
        assert!((doc.fps - 25.0).abs() < 1e-9);
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
        assert_eq!((video.len(), audio.len()), (3, 1));
        let near = |a: f64, b: f64| (a - b).abs() < 1.0 / 25.0;
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
        // The paths come back whole, with accents and spaces
        assert_eq!(
            video[0].path.as_deref().map(|p| p.replace('\\', "/")),
            Some("C:/Vídeos/meu projeto/entrevista.mp4".to_string())
        );
        assert_eq!(video[0].file_name.as_deref(), Some("entrevista.mp4"));
        assert_eq!(video[1].path.as_deref(), Some("/home/u/cobertura.mov"));
    }

    #[test]
    fn both_sector_sizes_work_and_what_cannot_be_written_is_reported() {
        let (mut tl, media) = project();
        for small in [false, true] {
            let (bytes, _) = export(&tl, &media, &settings(), small).unwrap();
            assert!(sniff(&bytes));
            assert_eq!(parse(&bytes).unwrap().clips.len(), 4);
        }
        // An event without media and one with another speed are reported, not written
        let id = tl.alloc_id();
        tl.tracks[0].events.push(TrackEvent {
            id,
            name: "Título".into(),
            media_id: None,
            start: 10.0,
            duration: 2.0,
            ..Default::default()
        });
        tl.tracks[0].events[0].playback_rate = 2.0;
        let (bytes, warnings) = export(&tl, &media, &settings(), false).unwrap();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert_eq!(parse(&bytes).unwrap().clips.len(), 4);
        // Nothing to write
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
