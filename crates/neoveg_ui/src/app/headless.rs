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
pub const OUTPUTS: [&str; 6] = [
    "ui_main_layout.png",
    "ui_timeline_events.png",
    "ui_multi_project_tabs.png",
    "ui_icon_set.png",
    "ui_welcome_screen.png",
    "ui_project_properties.png",
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
    s.playhead = 9.0;
    s.pixels_per_second = 44.0;
    s.status = "Projeto de demonstração: capturas de tela geradas fora da tela".into();
    s
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
}
