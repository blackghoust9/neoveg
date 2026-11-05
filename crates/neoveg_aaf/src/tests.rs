use crate::model::{
    CClip, CEffect, CFade, CFadeCurve, CItem, CKind, CTrack, Composition, Document, Gain, Source,
};
    use crate::time::{FrameRate, Tick};
    use crate::{read_aaf, write_aaf};

    #[test]
    fn roundtrip_speed_opacity_fades_and_effects() {
        let mut doc = Document::default();
        let clip_len = Tick::from_seconds_f64(2.0);
        let fade_in_len = Tick::from_seconds_f64(0.5);
        let fade_out_len = Tick::from_seconds_f64(0.5);

        let source = Source {
            key: "source_video_1".into(),
            name: "clip1.mp4".into(),
            kind: CKind::Picture,
            path: Some("C:/media/clip1.mp4".into()),
            channel: None,
            channels: 0,
            file_channels: 0,
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_24,
            sample_rate: 0,
            bits: 8,
            length: Tick::from_seconds_f64(10.0),
            offset: Tick::ZERO,
            start_tc: None,
            tc_rate: FrameRate::FPS_24,
            embedded: None,
            markers: Vec::new(),
            nested: None,
        };
        doc.sources.push(source);

        let clip = CClip {
            source: 0,
            len: clip_len,
            start: Tick::ZERO,
            gain: None,
            opacity: Some(Gain::Constant(0.75)),
            speed: Some(1.5),
            fade_in: Some(CFade {
                len: fade_in_len,
                curve: CFadeCurve::Smooth,
            }),
            fade_out: Some(CFade {
                len: fade_out_len,
                curve: CFadeCurve::Sharp,
            }),
            effects: vec![CEffect {
                id: "ofx.gaussian_blur".into(),
                name: "Gaussian Blur".into(),
                params: vec![("radius".into(), 15.5)],
            }],
            name: "Shot 01".into(),
        };

        let track = CTrack {
            kind: CKind::Picture,
            name: "Video 1".into(),
            number: 1,
            channels: 0,
            items: vec![CItem::Clip(clip)],
        };

        let comp = Composition {
            name: "Timeline 1".into(),
            rate: FrameRate::FPS_24,
            sample_rate: 48000,
            width: 1920,
            height: 1080,
            start_tc: 0,
            drop: false,
            tracks: vec![track],
            markers: Vec::new(),
        };

        doc.compositions.push(comp);

        let (bytes, report) = write_aaf(&doc, false).expect("write_aaf failed");
        assert!(!bytes.is_empty());
        assert!(
            !report.has_warnings(),
            "report had warnings: {:?}",
            report.entries
        );

        let (read_doc, read_report) = read_aaf(&bytes).expect("read_aaf failed");
        assert!(
            !read_report.has_warnings(),
            "read report warnings: {:?}",
            read_report.entries
        );
        assert_eq!(read_doc.compositions.len(), 1);
        let read_comp = &read_doc.compositions[0];
        assert_eq!(read_comp.tracks.len(), 1);
        let read_track = &read_comp.tracks[0];
        assert_eq!(read_track.items.len(), 1);

        if let CItem::Clip(c) = &read_track.items[0] {
            assert_eq!(c.name, "Shot 01");
            assert_eq!(c.len, clip_len);
            // Check speed
            assert!(c.speed.is_some(), "speed was not read");
            let spd = c.speed.unwrap();
            assert!((spd - 1.5).abs() < 1e-3, "expected speed 1.5, got {spd}");

            // Check opacity
            assert!(c.opacity.is_some(), "opacity was not read");
            match c.opacity.as_ref().unwrap() {
                Gain::Constant(val) => assert!(
                    (val - 0.75).abs() < 1e-2,
                    "expected opacity 0.75, got {val}"
                ),
                _ => panic!("expected constant opacity"),
            }

            // Check fades
            assert_eq!(
                c.fade_in,
                Some(CFade {
                    len: fade_in_len,
                    curve: CFadeCurve::Smooth,
                })
            );
            assert_eq!(
                c.fade_out,
                Some(CFade {
                    len: fade_out_len,
                    curve: CFadeCurve::Sharp,
                })
            );

            // Check effects
            assert_eq!(c.effects.len(), 1);
            assert_eq!(c.effects[0].id, "ofx.gaussian_blur");
            assert_eq!(c.effects[0].name, "Gaussian Blur");
            assert_eq!(c.effects[0].params.len(), 1);
            assert_eq!(c.effects[0].params[0].0, "radius");
            assert!((c.effects[0].params[0].1 - 15.5).abs() < 1e-3);
        } else {
            panic!("expected CItem::Clip");
        }
    }

    #[test]
    fn roundtrip_embedded_media_and_nested_compositions() {
        let mut doc = Document::default();
        let clip_len = Tick::from_seconds_f64(1.0);

        // 1. Nested composition
        let nested_clip = CClip {
            source: 0,
            len: clip_len,
            start: Tick::ZERO,
            gain: None,
            opacity: None,
            speed: None,
            fade_in: None,
            fade_out: None,
            effects: Vec::new(),
            name: "Nested Inner Clip".into(),
        };
        let nested_comp = Composition {
            name: "SubComp 1".into(),
            rate: FrameRate::FPS_24,
            sample_rate: 48000,
            width: 1920,
            height: 1080,
            start_tc: 0,
            drop: false,
            tracks: vec![CTrack {
                kind: CKind::Picture,
                name: "V1".into(),
                number: 1,
                channels: 0,
                items: vec![CItem::Clip(nested_clip)],
            }],
            markers: Vec::new(),
        };
        doc.nested.push(nested_comp);

        // 2. Embedded sound source
        let pcm_data = vec![0u8; 9600]; // 100ms 48kHz stereo 16-bit
        let wav_data = crate::support::encode_wav(&pcm_data, 1, 48000, 16);
        let audio_src = Source {
            key: "embedded_audio_1".into(),
            name: "embedded.wav".into(),
            kind: CKind::Sound,
            path: None,
            channel: None,
            channels: 1,
            file_channels: 1,
            width: 0,
            height: 0,
            frame_rate: FrameRate::FPS_25,
            sample_rate: 48000,
            bits: 16,
            length: clip_len,
            offset: Tick::ZERO,
            start_tc: None,
            tc_rate: FrameRate::FPS_25,
            embedded: Some(wav_data.clone()),
            markers: Vec::new(),
            nested: None,
        };
        doc.sources.push(audio_src);

        // 3. Embedded picture source (e.g. JPEG bytes)
        let pic_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]; // fake JPEG
        let pic_src = Source {
            key: "embedded_pic_1".into(),
            name: "embedded_logo.jpg".into(),
            kind: CKind::Picture,
            path: None,
            channel: None,
            channels: 0,
            file_channels: 0,
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_24,
            sample_rate: 0,
            bits: 8,
            length: clip_len,
            offset: Tick::ZERO,
            start_tc: None,
            tc_rate: FrameRate::FPS_24,
            embedded: Some(pic_bytes.clone()),
            markers: Vec::new(),
            nested: None,
        };
        doc.sources.push(pic_src);

        // Root timeline referencing both embedded sources
        let audio_clip = CClip {
            source: 0,
            len: clip_len,
            start: Tick::ZERO,
            gain: Some(Gain::Constant(0.9)),
            opacity: None,
            speed: None,
            fade_in: None,
            fade_out: None,
            effects: Vec::new(),
            name: "Embedded Audio Clip".into(),
        };
        let pic_clip = CClip {
            source: 1,
            len: clip_len,
            start: Tick::ZERO,
            gain: None,
            opacity: Some(Gain::Constant(1.0)),
            speed: None,
            fade_in: None,
            fade_out: None,
            effects: Vec::new(),
            name: "Embedded Pic Clip".into(),
        };

        let root_comp = Composition {
            name: "Master Timeline".into(),
            rate: FrameRate::FPS_24,
            sample_rate: 48000,
            width: 1920,
            height: 1080,
            start_tc: 0,
            drop: false,
            tracks: vec![
                CTrack {
                    kind: CKind::Picture,
                    name: "Video 1".into(),
                    number: 1,
                    channels: 0,
                    items: vec![CItem::Clip(pic_clip)],
                },
                CTrack {
                    kind: CKind::Sound,
                    name: "Audio 1".into(),
                    number: 1,
                    channels: 1,
                    items: vec![CItem::Clip(audio_clip)],
                },
            ],
            markers: Vec::new(),
        };
        doc.compositions.push(root_comp);

        let (bytes, report) = write_aaf(&doc, false).expect("write_aaf with embedded media failed");
        assert!(!bytes.is_empty());
        assert!(
            !report.has_warnings(),
            "write warnings: {:?}",
            report.entries
        );

        let (read_doc, read_report) =
            read_aaf(&bytes).expect("read_aaf with embedded media failed");
        assert!(
            !read_report.has_warnings(),
            "read warnings: {:?}",
            read_report.entries
        );

        // Verify embedded essence was retrieved in sources
        let embedded_audio = read_doc
            .sources
            .iter()
            .find(|s| s.kind == CKind::Sound && s.embedded.is_some());
        assert!(
            embedded_audio.is_some(),
            "Embedded audio essence should be parsed"
        );

        let embedded_picture = read_doc
            .sources
            .iter()
            .find(|s| s.kind == CKind::Picture && s.embedded.is_some());
        assert!(
            embedded_picture.is_some(),
            "Embedded picture essence should be parsed"
        );
        assert_eq!(
            embedded_picture.unwrap().embedded.as_ref().unwrap(),
            &pic_bytes
        );
    }
