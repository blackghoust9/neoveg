//! Offscreen screenshots of the interface: `neoveg --screenshots <dir>`.
//!
//! The workspace is drawn by egui exactly as in the window, but into a `wgpu` texture that never
//! reaches the screen, and read back to PNG files. Nothing of the user's is touched: the startup
//! screen, the preferences, the audio probe, the decoder worker and the recovery files are all
//! skipped, and the demonstration projects are built in memory (their only file output is a
//! sub-project written to a temporary folder). It needs a `wgpu` adapter (a GPU or a software
//! one); without one the run fails with a clear message instead of capturing the real screen.

use super::*;

use eframe::{egui_wgpu, wgpu};
use neoveg_engine::envelope::{AutomationTarget, EnvelopePoint, EnvelopeShape, TrackEnvelope};
use neoveg_engine::markers;
use neoveg_engine::Track;
use std::path::Path;

/// Frames run before the one that is saved: fonts, SVG icons and the dock settle in the first ones.
const WARM_UP_FRAMES: usize = 9;
/// Seconds of UI time of the first frame. Small, so the caret and the selected-track dot (which
/// blink every 500 ms) are in their visible half and no autosave is due.
const START_TIME: f64 = 0.2;
const FRAME_STEP: f64 = 0.02;

/// File names written by [`run`].
pub const OUTPUTS: [&str; 10] = [
    "ui_main_layout.png",
    "ui_timeline_events.png",
    "ui_multi_project_tabs.png",
    "ui_icon_set.png",
    "ui_welcome_screen.png",
    "ui_project_properties.png",
    "ui_edit_details.png",
    "ui_video_effects.png",
    "ui_pan_crop.png",
    "ui_track_motion.png",
];

/// Renders every screenshot into `dir` (created if needed).
pub fn run(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut gpu = Offscreen::new()?;
    println!("adaptador: {}", gpu.adapter_name);

    let scratch = std::env::temp_dir().join("neoveg_screenshots");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e| format!("cannot create a temp folder: {e}"))?;

    let result = (|| {
        // 1. The whole workspace
        let ctx = new_context();
        let mut app = headless_app(demo_project(), default_dock());
        let real = install_sample_frame(&ctx, &mut app.state);
        println!(
            "previa: {}",
            if real {
                "quadro real do vídeo de amostra"
            } else {
                "sem vídeo de amostra ou ffmpeg: previa preta"
            }
        );
        let image = gpu.capture(&ctx, [1600, 900], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[0]))?;

        // 2. The timeline alone, taller and with the toolbar
        let ctx = new_context();
        let mut app = headless_app(demo_project(), DockState::new(vec![Tab::Timeline]));
        let image = gpu.capture(&ctx, [1400, 520], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[1]))?;

        // 3. Several projects open, one of them a sub-project of the first
        let ctx = new_context();
        let mut app = headless_app(demo_tabs(&ctx, &scratch), default_dock());
        install_sample_frame(&ctx, &mut app.state);
        let image = gpu.capture(&ctx, [1600, 900], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[2]))?;

        // 4. The icon library
        let ctx = new_context();
        let image = gpu.capture(&ctx, [1280, 640], icon_grid)?;
        save(&image, &dir.join(OUTPUTS[3]))?;

        // 5. The welcome screen over the workspace, with a few recent projects
        let ctx = new_context();
        let mut app = headless_app(demo_welcome(&scratch), default_dock());
        let image = gpu.capture(&ctx, [1280, 720], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[4]))?;

        // 6. The project properties window
        let ctx = new_context();
        let mut state = demo_project();
        state
            .project_dialog
            .open_with(&state.project.settings.clone(), false);
        let mut app = headless_app(state, default_dock());
        let image = gpu.capture(&ctx, [1280, 860], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[5]))?;

        // 7. The edit details table and the project notes over the timeline
        let ctx = new_context();
        let mut state = demo_project();
        state.show_details = true;
        state.notes.open = true;
        let mut app = headless_app(state, DockState::new(vec![Tab::Timeline]));
        let image = gpu.capture(&ctx, [1400, 700], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[6]))?;

        // 8. The video effects panel with a chain on the event, its media and the project, and
        // the preview showing the chain
        let ctx = new_context();
        let mut state = demo_project_with_video_effects();
        let chain = preview_chain(&state);
        install_sample_frame_with(&ctx, &mut state, &chain);
        let mut app = headless_app(state, effects_dock());
        let image = gpu.capture(&ctx, [1600, 900], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[7]))?;

        // 9. The Pan/Crop editor on an animated event, with the live preview beside it
        let ctx = new_context();
        let mut state = demo_project_with_pan_crop();
        let source = decode_sample_frame(Path::new(SAMPLE_VIDEO), 3.0)
            .unwrap_or_else(|| neoveg_video::generators::smpte_color_bars(960, 540));
        let (track_id, event_id, shown) = pan_crop_demo_target(&state);
        let result = neoveg_video::pan_crop::apply(&source, &shown, true);
        state.video_texture = Some(ctx.load_texture(
            "preview_frame",
            egui::ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.rgba,
            ),
            egui::TextureOptions::LINEAR,
        ));
        state.pan_crop.open_event(track_id, event_id);
        state.pan_crop.set_source(&ctx, &source);
        let mut app = headless_app(state, default_dock());
        let image = gpu.capture(&ctx, [1280, 760], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[8]))?;

        // 10. The motion of a video track: a picture-in-picture that slides across the frame
        let ctx = new_context();
        let mut state = demo_project_with_track_motion();
        let source = decode_sample_frame(Path::new(SAMPLE_VIDEO), 3.0)
            .unwrap_or_else(|| neoveg_video::generators::smpte_color_bars(960, 540));
        let (track_id, shown) = track_motion_demo_target(&state);
        let placed = neoveg_video::pan_crop::place(&source, &shown, source.width, source.height);
        state.video_texture = Some(ctx.load_texture(
            "preview_frame",
            egui::ColorImage::from_rgba_unmultiplied(
                [placed.width as usize, placed.height as usize],
                &placed.rgba,
            ),
            egui::TextureOptions::LINEAR,
        ));
        state.pan_crop.open_track(track_id);
        let mut app = headless_app(state, default_dock());
        let image = gpu.capture(&ctx, [1280, 760], |ctx| app.show(ctx))?;
        save(&image, &dir.join(OUTPUTS[9]))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn new_context() -> egui::Context {
    let ctx = egui::Context::default();
    ui_theme::apply(&ctx, &Theme::default());
    icons::install(&ctx);
    ctx
}

/// An application as it is after the startup screen, with nothing read from the user's machine.
fn headless_app(state: AppState, dock: DockState<Tab>) -> NeovegApp {
    NeovegApp {
        dock,
        layouts: Default::default(),
        state,
        splash: None,
        gpu: "renderização fora da tela".into(),
        detached: Vec::new(),
        detached_restored: true,
    }
}

// ---- the wgpu side --------------------------------------------------------------------------------

struct Offscreen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
    adapter_name: String,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl Offscreen {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let request = |fallback: bool| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: fallback,
                compatible_surface: None,
            }))
        };
        let adapter = request(false).or_else(|| request(true)).ok_or(
            "nenhum adaptador wgpu (GPU ou renderizador por software) está disponível; \
             instale um driver Vulkan/DX12/Metal ou um renderizador por software",
        )?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("neoveg-screenshots"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|e| format!("cannot open the graphics device: {e}"))?;
        let renderer = egui_wgpu::Renderer::new(&device, FORMAT, None, 1, false);
        Ok(Self {
            device,
            queue,
            renderer,
            adapter_name: format!("{} ({:?})", info.name, info.backend),
        })
    }

    /// Runs `ui` for [`WARM_UP_FRAMES`] + 1 frames at a fixed time step and returns the last one.
    fn capture(
        &mut self,
        ctx: &egui::Context,
        size: [u32; 2],
        mut ui: impl FnMut(&egui::Context),
    ) -> Result<image::RgbaImage, String> {
        let screen =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
        let mut last = None;
        for frame in 0..=WARM_UP_FRAMES {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(START_TIME + frame as f64 * FRAME_STEP),
                ..Default::default()
            };
            let output = ctx.run(input, &mut ui);
            // Every frame's texture changes must be applied, not only the saved frame's
            for (id, delta) in &output.textures_delta.set {
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
            if frame < WARM_UP_FRAMES {
                for id in &output.textures_delta.free {
                    self.renderer.free_texture(id);
                }
            } else {
                last = Some(output);
            }
        }
        let output = last.ok_or("no frame was produced")?;
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let image = self.render(&jobs, size, output.pixels_per_point)?;
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        Ok(image)
    }

    fn render(
        &mut self,
        jobs: &[egui::ClippedPrimitive],
        size: [u32; 2],
        pixels_per_point: f32,
    ) -> Result<image::RgbaImage, String> {
        let [width, height] = size;
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screenshot"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let padded = padded_row_bytes(width);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let extra =
            self.renderer
                .update_buffers(&self.device, &self.queue, &mut encoder, jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, jobs, &screen);
        }
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue
            .submit(extra.into_iter().chain(Some(encoder.finish())));

        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|_| "the readback was dropped".to_owned())?
            .map_err(|e| format!("cannot read the screenshot back: {e}"))?;
        let pixels = strip_row_padding(&slice.get_mapped_range(), width, height);
        readback.unmap();
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| "the pixel buffer has the wrong size".to_owned())
    }
}

/// Bytes per row of a texture copy: `wgpu` wants a multiple of 256.
fn padded_row_bytes(width: u32) -> u32 {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    (width * 4).div_ceil(align) * align
}

/// Removes the end-of-row padding of a texture read back from the GPU.
fn strip_row_padding(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let (row, padded) = ((width * 4) as usize, padded_row_bytes(width) as usize);
    let mut out = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        out.extend_from_slice(&data[y * padded..y * padded + row]);
    }
    out
}

fn save(image: &image::RgbaImage, path: &Path) -> Result<(), String> {
    image
        .save(path)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    println!("gravado {}", path.display());
    Ok(())
}

/// The sample video of the tests: a real 1080p clip with motion and gradients, made with `ffmpeg`.
const SAMPLE_VIDEO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../neoveg_video/tests/assets/sample_video.mp4"
);
/// The decoded frame is scaled to this size (the preview shows it at most this large).
const SAMPLE_FRAME: (u32, u32) = (960, 540);

/// Decodes the frame of `video` at `seconds` with the `ffmpeg` program and returns it as RGBA, scaled
/// to [`SAMPLE_FRAME`]. `None` when the file or the program is missing or the frame cannot be read.
fn decode_sample_frame(video: &Path, seconds: f64) -> Option<neoveg_video::frame::FrameBuffer> {
    if !video.is_file() {
        return None;
    }
    let (width, height) = SAMPLE_FRAME;
    let output = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{seconds:.4}"), "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-vf", &format!("scale={width}:{height}")])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .ok()?;
    let expected = (width * height * 4) as usize;
    let complete = output.status.success() && output.stdout.len() == expected;
    complete.then_some(neoveg_video::frame::FrameBuffer {
        width,
        height,
        rgba: output.stdout,
    })
}

/// Puts the real frame under the cursor into the preview: the frame of the sample video at the
/// cursor time (the whole frame, as the decoder returns it), uploaded as the preview texture, and its
/// scopes. Without the sample file or `ffmpeg` the preview stays as it was and `false` is returned.
fn install_sample_frame(ctx: &egui::Context, state: &mut AppState) -> bool {
    install_sample_frame_with(ctx, state, &[])
}

/// Like [`install_sample_frame`], with the video effect chain run on the frame first.
fn install_sample_frame_with(
    ctx: &egui::Context,
    state: &mut AppState,
    chain: &[neoveg_video::vfx::VideoEffect],
) -> bool {
    // The frame at the cursor: frame index (floor) over the project rate, like the timecode box
    let fps = state.project.settings.fps;
    let seconds = (state.playhead * fps).floor() / fps;
    let Some(mut frame) = decode_sample_frame(Path::new(SAMPLE_VIDEO), seconds) else {
        return false;
    };
    neoveg_video::vfx::apply_chain_cpu(&mut frame, chain);
    state.frame_scopes = Some(Scopes::from_frame(&frame));
    state.frame_scopes_version += 1;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.rgba,
    );
    state.video_texture =
        Some(ctx.load_texture("preview_frame", image, egui::TextureOptions::LINEAR));
    true
}

// ---- demonstration content ---------------------------------------------------------------------------

const RATE: u32 = 8_000;

/// A synthetic stereo recording: a tone shaped by a slow pulse, so its waveform has some life.
fn synthetic_media(name: &str, seconds: f64, freq: f64, pulse: f64, level: f32) -> LoadedMedia {
    let frames = (seconds * f64::from(RATE)) as usize;
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = i as f64 / f64::from(RATE);
        let swell = (0.55 + 0.45 * (std::f64::consts::TAU * pulse * t).sin()).abs();
        let burst = (std::f64::consts::TAU * pulse * 0.23 * t)
            .sin()
            .abs()
            .powf(0.6);
        let v = (std::f64::consts::TAU * freq * t).sin() * swell * burst;
        let left = (v * f64::from(level)) as f32;
        let right = (v * 0.92 * f64::from(level)) as f32;
        samples.extend([left, right]);
    }
    LoadedMedia {
        path: PathBuf::from(name),
        name: name.to_owned(),
        waveform: Arc::new(
            WaveformSource::from_interleaved_f32(RATE, 2, &samples)
                .expect("a synthetic recording is valid"),
        ),
        stereo: Arc::new(samples),
        sample_rate: RATE,
        duration: seconds,
    }
}

/// The recordings of the demonstration project, with their media pool entries.
fn insert_demo_media(state: &mut AppState) {
    let media = [
        (1, synthetic_media("trilha.wav", 30.0, 110.0, 0.35, 0.8)),
        (2, synthetic_media("entrevista.wav", 20.0, 220.0, 0.9, 0.7)),
        (3, synthetic_media("efeito.wav", 4.0, 330.0, 1.6, 0.9)),
    ];
    for (id, media) in media {
        state.media.insert(id, media);
    }
}

fn demo_event(
    state: &mut AppState,
    track: &mut Track,
    name: &str,
    media: u64,
    start: f64,
    duration: f64,
) -> usize {
    let event = TrackEvent {
        id: state.timeline.alloc_id(),
        name: name.to_owned(),
        media_id: Some(media),
        start,
        duration,
        ..Default::default()
    };
    track.events.push(event);
    track.events.len() - 1
}

/// The project shown in the main screenshots: four tracks with crossfades, an envelope, markers
/// and one event of each switch.
fn demo_project() -> AppState {
    let mut s = AppState::new();
    s.project.name = "Documentário".into();
    for (id, name, seconds) in [
        (1, "trilha.wav", 30.0),
        (2, "entrevista.wav", 20.0),
        (3, "efeito.wav", 4.0),
        (4, "abertura.mp4", 24.0),
    ] {
        s.project.media_pool.push(MediaRef {
            subproject: false,
            id,
            path: name.into(),
            duration_seconds: seconds,
        });
    }
    insert_demo_media(&mut s);

    let mut video = s.timeline.new_track(TrackKind::Video);
    video.name = "Vídeo 1".into();
    let a = demo_event(&mut s, &mut video, "Abertura", 4, 0.0, 9.0);
    video.events[a].fade_out = 1.0;
    demo_event(&mut s, &mut video, "Cena 2", 4, 8.0, 12.0);
    demo_event(&mut s, &mut video, "Cena 3", 4, 21.0, 6.0);

    let mut music = s.timeline.new_track(TrackKind::Audio);
    music.name = "Trilha".into();
    let m = demo_event(&mut s, &mut music, "trilha.wav", 1, 0.0, 26.0);
    music.events[m].fade_in = 1.5;
    music.events[m].fade_out = 2.0;
    music.events[m].normalize = true;
    music.events[m].normalize_gain = 1.25;
    let mut volume = TrackEnvelope::new(AutomationTarget::Volume, -3.0);
    for (time, value) in [(6.0, -3.0), (8.0, -14.0), (16.0, -14.0), (18.0, -3.0)] {
        volume.points.push(EnvelopePoint {
            time,
            value,
            shape: EnvelopeShape::Linear,
        });
    }
    music.envelopes.push(volume);

    let mut voice = s.timeline.new_track(TrackKind::Audio);
    voice.name = "Entrevista".into();
    let first = demo_event(&mut s, &mut voice, "entrevista.wav", 2, 1.0, 9.0);
    let second = demo_event(&mut s, &mut voice, "entrevista.wav", 2, 8.0, 10.0);
    voice.events[second].trim_in = 4.0;
    // The second event starts inside the first one: an automatic crossfade over the overlap
    let _ = first;
    let tail = demo_event(&mut s, &mut voice, "entrevista.wav", 2, 19.5, 5.0);
    voice.events[tail].locked = true;

    let mut fx = s.timeline.new_track(TrackKind::Audio);
    fx.name = "Efeitos".into();
    let looped = demo_event(&mut s, &mut fx, "efeito.wav", 3, 2.0, 11.0);
    fx.events[looped].looping = true;
    let muted = demo_event(&mut s, &mut fx, "efeito.wav", 3, 14.0, 3.0);
    fx.events[muted].muted = true;
    let flipped = demo_event(&mut s, &mut fx, "efeito.wav", 3, 18.0, 3.5);
    fx.events[flipped].invert_phase = true;

    let selected_event = (voice.id, voice.events[second].id);
    s.selected_track = Some(voice.id);
    s.timeline.tracks.extend([video, music, voice, fx]);
    s.selection.select_only(selected_event);

    for (time, name) in [(5.0, "Intro"), (14.0, "Corte"), (21.0, "Final")] {
        if let Some(command) = markers::add_marker(&mut s.timeline, time, Some(name)) {
            s.history.execute(&mut s.timeline, command);
        }
    }
    for (id, time, text, label, color) in [
        (1, 3.0, "Entrada da trilha ainda alta", "áudio", 1),
        (2, 16.5, "Trocar este plano", "corte", 3),
    ] {
        s.project.notes.push(neoveg_common::project::ProjectNote {
            id,
            time,
            text: text.into(),
            label: label.into(),
            color,
            resolved: false,
        });
    }
    s.playhead = 9.0;
    s.pixels_per_second = 44.0;
    s.status = "Projeto de demonstração: capturas de tela geradas fora da tela".into();
    s
}

/// The demonstration project with a video effect chain on the first event, on its media and on the
/// project, and that event selected so the panel shows its chain.
fn demo_project_with_video_effects() -> AppState {
    use neoveg_engine::TrackEffect;
    let mut s = demo_project();
    let (track_id, event_id) = {
        let track = s
            .timeline
            .tracks
            .iter_mut()
            .find(|t| t.kind == TrackKind::Video)
            .expect("the demonstration has a video track");
        let mut blur = TrackEffect::new("gaussian_blur");
        blur.params.insert("h_range".into(), 0.012);
        blur.params.insert("v_range".into(), 0.012);
        let mut vignette = TrackEffect::new("vignette");
        vignette.params.insert("inner_radius".into(), 0.35);
        let mut off = TrackEffect::new("sharpen");
        off.bypass = true;
        let event = &mut track.events[0];
        event.video_effects = vec![blur, vignette, off];
        let ids = (track.id, event.id);
        // The track's level envelope: opaque, a dip in the second scene, opaque again
        let mut level = TrackEnvelope::new(AutomationTarget::VideoLevel, 1.0);
        for (time, value) in [(8.0, 1.0), (10.0, 0.35), (14.0, 0.35), (16.0, 1.0)] {
            level.points.push(EnvelopePoint {
                time,
                value,
                shape: EnvelopeShape::Linear,
            });
        }
        track.envelopes.push(level);
        ids
    };
    s.timeline
        .media_video_effects
        .insert(4, vec![TrackEffect::new("unsharp_mask")]);
    s.timeline.video_effects = vec![TrackEffect::new("vignette")];
    s.selected_track = Some(track_id);
    s.selection.select_only((track_id, event_id));
    s.playhead = 3.0;
    s.status = "Efeitos de vídeo: cadeia no evento, na mídia e no projeto".into();
    s
}

/// The demonstration project with a Pan/Crop that moves and zooms during the first video event,
/// the cursor in the middle of the move.
fn demo_project_with_pan_crop() -> AppState {
    use neoveg_common::pan_crop::{PanCrop, PanCropFrame};
    let mut s = demo_project();
    let (track_id, event_id) = {
        let track = s
            .timeline
            .tracks
            .iter_mut()
            .find(|t| t.kind == TrackKind::Video)
            .expect("the demonstration has a video track");
        let mut pan_crop = PanCrop::default();
        pan_crop.upsert_key(
            0.0,
            PanCropFrame {
                x_center: 0.32,
                y_center: 0.42,
                width: 0.62,
                height: 0.62,
                rotation_deg: 0.0,
            },
            1e-3,
        );
        pan_crop.upsert_key(
            6.0,
            PanCropFrame {
                x_center: 0.66,
                y_center: 0.56,
                width: 0.46,
                height: 0.46,
                rotation_deg: 8.0,
            },
            1e-3,
        );
        pan_crop.keys[0].smoothness = 0.6;
        let event = &mut track.events[0];
        event.pan_crop = pan_crop;
        (track.id, event.id)
    };
    s.selected_track = Some(track_id);
    s.selection.select_only((track_id, event_id));
    s.playhead = 2.5;
    s.status = "Pan/Crop do evento: caixa animada entre dois quadros-chave".into();
    s
}

/// The demonstration project with the motion of its video track: the layer shrinks to a third of
/// the frame and slides from the lower right to the upper left, the cursor halfway along.
fn demo_project_with_track_motion() -> AppState {
    use neoveg_common::pan_crop::{PanCrop, PanCropFrame};
    let mut s = demo_project();
    let track_id = {
        let track = s
            .timeline
            .tracks
            .iter_mut()
            .find(|t| t.kind == TrackKind::Video)
            .expect("the demonstration has a video track");
        let mut motion = PanCrop::default();
        let at = |x: f32, y: f32, rotation: f32| PanCropFrame {
            x_center: x,
            y_center: y,
            width: 0.36,
            height: 0.36,
            rotation_deg: rotation,
        };
        motion.upsert_key(2.0, at(0.78, 0.74, 0.0), 1e-3);
        motion.upsert_key(14.0, at(0.24, 0.28, -6.0), 1e-3);
        motion.keys[0].smoothness = 0.7;
        track.motion = motion;
        track.id
    };
    s.selected_track = Some(track_id);
    s.playhead = 8.0;
    s.status = "Movimento da trilha: a camada desliza entre dois quadros-chave".into();
    s
}

/// The track the motion demo edits, and its box at the cursor.
fn track_motion_demo_target(
    state: &AppState,
) -> (
    neoveg_engine::TrackId,
    neoveg_common::pan_crop::PanCropFrame,
) {
    let track_id = state.selected_track.expect("a selected track");
    let track = state.timeline.track(track_id).expect("the track exists");
    (track_id, track.motion.frame_at(state.playhead))
}

/// The event the Pan/Crop demo edits, and the box at the cursor.
fn pan_crop_demo_target(
    state: &AppState,
) -> (
    neoveg_engine::TrackId,
    neoveg_engine::EventId,
    neoveg_common::pan_crop::PanCropFrame,
) {
    let (track_id, event_id) = state.selection.primary().expect("a selected event");
    let event = state
        .timeline
        .track(track_id)
        .and_then(|t| t.events.iter().find(|e| e.id == event_id))
        .expect("the event exists");
    let frame = event.pan_crop.frame_at(state.playhead - event.start);
    (track_id, event_id, frame)
}

/// The chain the preview shows for the first video event: media, event, then project (the track
/// has none), the same order as the worker.
fn preview_chain(state: &AppState) -> Vec<neoveg_video::vfx::VideoEffect> {
    use crate::video_fx_panel::to_worker_chain;
    let track = state
        .timeline
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Video)
        .expect("a video track");
    let event = &track.events[0];
    let mut chain = event
        .media_id
        .and_then(|m| state.timeline.media_video_effects.get(&m))
        .map(|c| to_worker_chain(c))
        .unwrap_or_default();
    chain.extend(to_worker_chain(&event.video_effects));
    chain.extend(to_worker_chain(&track.video_effects));
    chain.extend(to_worker_chain(&state.timeline.video_effects));
    chain
}

/// The preview, the video effects panel and the timeline.
fn effects_dock() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Preview]);
    let surface = dock.main_surface_mut();
    let [top, _timeline] = surface.split_below(NodeIndex::root(), 0.58, vec![Tab::Timeline]);
    surface.split_right(top, 0.60, vec![Tab::VideoFx]);
    dock
}

/// Several projects open at once: the main one, a draft and a sub-project made from two events.
fn demo_tabs(ctx: &egui::Context, scratch: &Path) -> AppState {
    let mut s = demo_project();
    s.project_path = Some(scratch.join("Documentario.neoveg"));
    let voice = s.timeline.tracks[2].id;
    let events: Vec<EventRef> = s.timeline.tracks[2]
        .events
        .iter()
        .take(2)
        .map(|e| (voice, e.id))
        .collect();
    s.selection.select_only(events[0]);
    s.selection.add_all(events.iter().copied());
    s.create_subproject_from_selection();
    let sub = s
        .timeline
        .tracks
        .iter()
        .flat_map(|t| t.events.iter().map(move |e| (t.id, e.id)))
        .find_map(|(track, event)| s.subproject_path_of(track, event));

    // A second, empty project in its own tab
    s.new_tab();
    s.project.name = "Rascunho".into();
    let draft_tab = s.tab_ids().last().copied();
    if let Some(path) = sub {
        s.open_path_in_tab(&path, ctx);
    }
    // Back to the main project, whose recordings are put in memory again
    if let Some(first) = s.tab_ids().first().copied() {
        s.switch_tab(first, ctx);
    }
    let _ = draft_tab;
    insert_demo_media(&mut s);
    s.playhead = 9.0;
    s.pixels_per_second = 44.0;
    s.status = "Três projetos abertos: Documentário, Rascunho e um sub-projeto".into();
    s
}

/// The welcome screen: a few projects that exist on disk (empty files in `scratch`) are listed as recent.
fn demo_welcome(scratch: &Path) -> AppState {
    let mut s = AppState::new();
    s.project.name = "Sem título".into();
    for name in [
        "Documentario_0003",
        "Casamento_Ana_e_Rui",
        "Clipe_Banda_2026",
        "Entrevista_Podcast_12",
    ] {
        let path = scratch.join(format!("{name}.neoveg"));
        let _ = std::fs::write(&path, b"{}");
        s.prefs.add_recent_project(&path);
    }
    s.welcome.open_with(&ProjectSettings::default());
    s
}

/// A page with every icon of the SVG library, one cell each, with its name.
fn icon_grid(ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading(format!(
            "Biblioteca de ícones SVG do Neoveg ({} ícones)",
            icons::SvgIcon::ALL.len()
        ));
        ui.add_space(6.0);
        let ink = ui.visuals().text_color();
        let (cell, icon_size) = (vec2(108.0, 62.0), 28.0);
        let columns = ((ui.available_width() / cell.x) as usize).max(1);
        egui::Grid::new("icons")
            .spacing(vec2(0.0, 0.0))
            .show(ui, |ui| {
                for (i, icon) in icons::SvgIcon::ALL.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(cell, egui::Sense::hover());
                    let art = Rect::from_center_size(
                        rect.center_top() + vec2(0.0, 6.0 + icon_size / 2.0),
                        vec2(icon_size, icon_size),
                    );
                    icons::paint_svg(ui, *icon, art, ink);
                    ui.painter().text(
                        rect.center_bottom() - vec2(0.0, 6.0),
                        Align2::CENTER_BOTTOM,
                        icon.name(),
                        FontId::proportional(10.0),
                        ink.gamma_multiply(0.75),
                    );
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_padded_to_the_copy_alignment_and_the_padding_is_removed() {
        assert_eq!(padded_row_bytes(64), 256);
        assert_eq!(padded_row_bytes(65), 512);
        // 3 pixels wide: 12 bytes of picture in 256-byte rows
        let mut data = vec![0u8; 256 * 2];
        data[..12].copy_from_slice(&[1; 12]);
        data[256..268].copy_from_slice(&[2; 12]);
        data[12] = 99;
        let out = strip_row_padding(&data, 3, 2);
        assert_eq!(out.len(), 24);
        assert!(out[..12].iter().all(|&b| b == 1));
        assert!(out[12..].iter().all(|&b| b == 2));
    }

    #[test]
    fn the_demo_project_shows_each_feature_it_is_meant_to_show() {
        let s = demo_project();
        assert_eq!(s.timeline.tracks.len(), 4);
        let events: Vec<&TrackEvent> = s.timeline.tracks.iter().flat_map(|t| &t.events).collect();
        assert!(events.iter().any(|e| e.muted));
        assert!(events.iter().any(|e| e.locked));
        assert!(events.iter().any(|e| e.looping));
        assert!(events.iter().any(|e| e.normalize));
        assert!(events.iter().any(|e| e.invert_phase));
        assert!(
            !find_crossfades(&s.timeline.tracks[2].events).is_empty(),
            "the voice track crossfades"
        );
        assert!(s.timeline.tracks[1]
            .envelopes
            .iter()
            .any(|e| e.points.len() > 1));
        assert_eq!(s.timeline.markers.len(), 3);
    }

    #[test]
    fn the_demonstration_tabs_hold_three_projects() {
        let dir = std::env::temp_dir().join(format!("neoveg_demo_tabs_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = egui::Context::default();
        let s = demo_tabs(&ctx, &dir);
        assert_eq!(s.tab_ids().len(), 3);
        assert_eq!(s.project.name, "Documentário");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_video_effects_demo_has_a_chain_at_each_level_and_the_preview_runs_them() {
        let s = demo_project_with_video_effects();
        let track = s
            .timeline
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video)
            .unwrap();
        assert_eq!(track.events[0].video_effects.len(), 3);
        assert_eq!(s.timeline.media_video_effects[&4].len(), 1);
        assert_eq!(s.timeline.video_effects.len(), 1);
        let (target, _, scopes) = s.vfx_target();
        assert!(matches!(target, VideoFxTarget::Event { .. }));
        assert!(scopes.event && scopes.media && scopes.track);
        // media (1) + event without the bypassed sharpen (2) + project (1)
        let kinds: Vec<String> = preview_chain(&s).into_iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            ["unsharp_mask", "gaussian_blur", "vignette", "vignette"]
        );
    }

    #[test]
    fn the_pan_crop_demo_moves_between_two_keys_and_edits_a_video_event() {
        let s = demo_project_with_pan_crop();
        let (track_id, event_id, frame) = pan_crop_demo_target(&s);
        assert!(neoveg_engine::pan_crop::get(&s.timeline, track_id, event_id).is_some());
        // 2.5 s into a move from x 0.32 to 0.66 that eases out of the first key
        assert!(frame.x_center > 0.32 && frame.x_center < 0.66, "{frame:?}");
        assert!(frame.width < 0.62 && frame.width > 0.46);
    }

    #[test]
    fn the_track_motion_demo_slides_between_two_keys_and_edits_a_video_track() {
        let s = demo_project_with_track_motion();
        let (track_id, frame) = track_motion_demo_target(&s);
        assert!(neoveg_engine::track_motion::get(&s.timeline, track_id).is_some());
        assert!(frame.x_center < 0.78 && frame.x_center > 0.24, "{frame:?}");
        assert_eq!(frame.width, 0.36);
    }
}
