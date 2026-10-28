//! Background frame decoding, LRU frame caching, playback lookahead prefetch, and multi-track
//! CPU/GPU compositing. The UI sends requests and polls replies; it never decodes video itself.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use neoveg_common::chroma::ChromaKey;
use neoveg_common::composite::CompositeMode;
use neoveg_common::grade::Grade;

use crate::cache::LruFrameCache;
use crate::composite::{composite_nodes_cpu, CompositeNode, DecodedLayer, GpuCompositor};
use crate::frame::FrameBuffer;
use crate::generators::{
    decode_generator_at, is_generator_manifest_path, is_time_varying_generator_path,
    probe_generator_manifest,
};
use crate::info::{fit_size, VideoInfo};
use crate::lut::Lut;
use crate::pan_crop::{self, PanCropRequest};
use crate::still::{
    decode_image, decode_sequence_at, is_image_path, is_sequence_manifest_path, probe_image,
    probe_sequence_manifest,
};
use crate::vfx::{self, VideoEffect};
use crate::vfx_gpu::VfxGpu;
use neoveg_common::pan_crop::PanCropFrame;

pub struct FrameRequest {
    pub media_id: u64,
    pub path: PathBuf,
    /// Time inside the source media.
    pub seconds: f64,
    /// Maximum decoded width (0 = source size).
    pub max_width: u32,
    /// Color table applied to the decoded picture (done on the worker thread).
    pub lut: Option<Arc<Lut>>,
    /// Primary color correction, applied before the LUT.
    pub grade: Grade,
    /// Chroma key (green/blue screen), applied before primary grading and LUT.
    pub chroma_key: ChromaKey,
}

pub struct FrameReply {
    pub media_id: u64,
    pub seconds: f64,
    pub result: Result<FrameBuffer, String>,
}

/// One clip participating in a multi-track composite frame.
#[derive(Debug, Clone)]
pub struct VideoClipRequest {
    pub media_id: u64,
    pub path: PathBuf,
    /// Source-local timestamp in seconds.
    pub seconds: f64,
    /// Source frame rate (`> 0.0`) used to quantize source cache keys.
    pub source_fps: f64,
    /// Clip playback speed multiplier (`1.0` normal) used by playback lookahead prefetch.
    pub playback_rate: f64,
    /// Combined event fade-in / fade-out / overlap crossfade opacity (`0.0..=1.0`).
    pub event_opacity: f32,
    /// Whether project chroma key applies to this clip (enabled for foreground clips).
    pub apply_chroma: bool,
    /// Event effect chain (bypassed effects already left out), run after the chroma key and
    /// before the event's opacity and crossfade.
    pub effects: Vec<VideoEffect>,
    /// Pan/Crop of the event, run right after the chroma key and before the effects.
    pub pan_crop: Option<PanCropRequest>,
}

/// Recursive track compositing request node (single track or parent/child group).
#[derive(Debug, Clone)]
pub enum TrackCompositeNode {
    Track {
        /// Active clips on this track ordered from earliest `start` (bottom) to latest (top).
        clips: Vec<VideoClipRequest>,
        mode: CompositeMode,
        level: f32,
        /// Track effect chain, run on the composited layer of the track.
        effects: Vec<VideoEffect>,
        /// Color the layer fades to and how far (`0.0` leaves it alone), run after the effects.
        fade_color: [u8; 3],
        fade_amount: f32,
        /// Where the layer of the track sits in the frame (`None` leaves it where it is), run last.
        motion: Option<PanCropFrame>,
    },
    Group {
        children: Vec<TrackCompositeNode>,
        parent_clips: Vec<VideoClipRequest>,
        parent_mode: CompositeMode,
        group_mode: CompositeMode,
        group_level: f32,
    },
}

/// Full timeline preview composite request evaluated off the UI thread.
#[derive(Debug, Clone)]
pub struct CompositeFrameRequest {
    pub timeline_seconds: f64,
    pub fps: f64,
    pub playing: bool,
    pub shuttle_speed: f64,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub max_width: u32,
    pub nodes: Vec<TrackCompositeNode>,
    pub lut: Option<Arc<Lut>>,
    pub grade: Grade,
    pub chroma_key: ChromaKey,
    /// Project effect chain, run on the composited picture before the color correction and the LUT.
    pub project_effects: Vec<VideoEffect>,
}

/// One decoded source frame for the Pan/Crop editor: the picture as the decoder returns it, with
/// no grading, key, effects or Pan/Crop.
#[derive(Debug, Clone)]
pub struct SourceRequest {
    pub path: PathBuf,
    pub seconds: f64,
    /// Maximum decoded width (0 = source size).
    pub max_width: u32,
}

pub struct SourceReply {
    pub path: PathBuf,
    pub seconds: f64,
    pub result: Result<FrameBuffer, String>,
}

pub struct CompositeFrameReply {
    pub timeline_seconds: f64,
    pub result: Result<FrameBuffer, String>,
}

enum WorkerJob {
    Single(FrameRequest),
    Composite(CompositeFrameRequest),
    Source(SourceRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceCacheKey {
    path: PathBuf,
    frame_tick: i64,
    max_width: u32,
}

impl SourceCacheKey {
    fn for_source(path: &Path, seconds: f64, source_fps: f64, max_width: u32) -> Self {
        let frame_tick = if is_image_path(path)
            || (is_generator_manifest_path(path) && !is_time_varying_generator_path(path))
        {
            // Static images and time-invariant generators decode once and reuse across timestamps.
            0
        } else {
            let fps = if source_fps.is_finite() && source_fps > 0.0 {
                source_fps
            } else {
                29.97
            };
            (seconds.max(0.0) * fps).round() as i64
        };
        Self {
            path: path.to_path_buf(),
            frame_tick,
            max_width,
        }
    }
}

/// Probes still images and `.neovegseq` manifests natively in pure Rust, and container videos via
/// FFmpeg when compiled with `--features ffmpeg`.
pub fn probe(path: &Path) -> Result<VideoInfo, String> {
    probe_with_defaults(path, 29.97, 5.0)
}

/// Probes media using the caller's project `fps` and `still_seconds` default duration for static
/// images.
pub fn probe_with_defaults(path: &Path, fps: f64, still_seconds: f64) -> Result<VideoInfo, String> {
    if is_image_path(path) {
        return probe_image(path, fps, still_seconds);
    }
    if is_sequence_manifest_path(path) {
        return probe_sequence_manifest(path);
    }
    if is_generator_manifest_path(path) {
        return probe_generator_manifest(path);
    }
    #[cfg(feature = "ffmpeg")]
    {
        crate::decode::probe(path)
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = path;
        Err(DISABLED.into())
    }
}

#[cfg(not(feature = "ffmpeg"))]
const DISABLED: &str =
    "decodificação de vídeo desabilitada: compile com `--features ffmpeg` (veja o README)";

/// Handle to the background video decoding and compositing worker thread.
pub struct FrameWorker {
    tx: Sender<WorkerJob>,
    pub rx: Receiver<FrameReply>,
    pub composite_rx: Receiver<CompositeFrameReply>,
    pub source_rx: Receiver<SourceReply>,
}

impl FrameWorker {
    /// Starts the worker thread. Always succeeds so still images and image sequences work even in
    /// builds without `--features ffmpeg`.
    pub fn spawn(on_reply: impl Fn() + Send + 'static) -> Result<Self, String> {
        let (tx, jobs) = channel::<WorkerJob>();
        let (single_tx, rx) = channel::<FrameReply>();
        let (composite_tx, composite_rx) = channel::<CompositeFrameReply>();
        let (source_tx, source_rx) = channel::<SourceReply>();
        std::thread::Builder::new()
            .name("neoveg-video-decode".into())
            .spawn(move || run(jobs, single_tx, composite_tx, source_tx, on_reply))
            .map_err(|e| format!("cannot start video thread: {e}"))?;
        Ok(Self {
            tx,
            rx,
            composite_rx,
            source_rx,
        })
    }

    pub fn request(&self, request: FrameRequest) {
        let _ = self.tx.send(WorkerJob::Single(request));
    }

    pub fn request_composite(&self, request: CompositeFrameRequest) {
        let _ = self.tx.send(WorkerJob::Composite(request));
    }

    /// Asks for one raw source frame (the Pan/Crop editor's background).
    pub fn request_source(&self, request: SourceRequest) {
        let _ = self.tx.send(WorkerJob::Source(request));
    }
}

/// Synchronous composite renderer intended for background *Make Movie* export threads. Reuses
/// decoded-frame LRU caching, GPU/CPU compositing, chroma key, primary grading, and 3D LUTs.
pub struct OfflineFrameRenderer {
    state: WorkerState,
}

impl Default for OfflineFrameRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl OfflineFrameRenderer {
    pub fn new() -> Self {
        Self {
            state: WorkerState::new(),
        }
    }

    pub fn render_frame(&mut self, request: &CompositeFrameRequest) -> Result<FrameBuffer, String> {
        handle_composite(&mut self.state, request)
    }
}

struct WorkerState {
    /// Decoded raw frames per source file (`96 MiB` budget).
    source_cache: LruFrameCache<SourceCacheKey>,
    /// Final composited + graded output frames (`48 MiB` budget).
    composite_cache: LruFrameCache<u64>,
    gpu_compositor: Option<GpuCompositor>,
    gpu_probed: bool,
    /// Effect chain pipelines, built on the compositor's device the first time they are needed.
    vfx_gpu: Option<VfxGpu>,
    #[cfg(feature = "ffmpeg")]
    decoders: std::collections::HashMap<PathBuf, crate::decode::VideoDecoder>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            source_cache: LruFrameCache::new(96 * 1024 * 1024),
            composite_cache: LruFrameCache::new(48 * 1024 * 1024),
            gpu_compositor: None,
            gpu_probed: false,
            vfx_gpu: None,
            #[cfg(feature = "ffmpeg")]
            decoders: std::collections::HashMap::new(),
        }
    }

    fn ensure_gpu(&mut self) -> Option<&GpuCompositor> {
        if !self.gpu_probed {
            self.gpu_probed = true;
            self.gpu_compositor = GpuCompositor::try_headless();
        }
        self.gpu_compositor.as_ref()
    }

    /// What a clip needs after its picture is decoded and keyed: the Pan/Crop, then the effect
    /// chain. A clip with neither returns the same `Arc`, so the cached source frame is never
    /// copied or changed.
    fn finish_clip(
        &mut self,
        frame: Arc<FrameBuffer>,
        clip: &VideoClipRequest,
    ) -> Arc<FrameBuffer> {
        let frame = match clip.pan_crop.filter(|p| !p.is_identity()) {
            Some(p) => Arc::new(pan_crop::apply(&frame, &p.frame, p.stretch_to_fill)),
            None => frame,
        };
        self.apply_effects(frame, &clip.effects)
    }

    /// Runs an effect chain on `frame`: on the GPU when there is an adapter and the picture is big
    /// enough to be worth the transfer, on the CPU otherwise or when the GPU refuses the picture.
    /// A chain with nothing to do returns the same `Arc`, so the cached source frame is never
    /// copied or changed.
    fn apply_effects(
        &mut self,
        frame: Arc<FrameBuffer>,
        chain: &[VideoEffect],
    ) -> Arc<FrameBuffer> {
        let resolved = vfx::resolve_chain(chain);
        if resolved.is_empty() || frame.width == 0 || frame.height == 0 {
            return frame;
        }
        if frame.width * frame.height >= 64 * 64 {
            if self.vfx_gpu.is_none() {
                self.vfx_gpu = self.ensure_gpu().map(|g| {
                    let (device, queue) = g.device_queue();
                    VfxGpu::new(device, queue)
                });
            }
            if let Some(gpu) = &self.vfx_gpu {
                match gpu.process(&frame, &resolved) {
                    Ok(done) => return Arc::new(done),
                    Err(e) => tracing::debug!("[DEBUG] video effects on the CPU: {e}"),
                }
            }
        }
        let mut copy = (*frame).clone();
        vfx::apply_chain_cpu(&mut copy, chain);
        Arc::new(copy)
    }

    fn decode_raw_cached(
        &mut self,
        path: &Path,
        seconds: f64,
        source_fps: f64,
        max_width: u32,
    ) -> Result<Arc<FrameBuffer>, String> {
        let key = SourceCacheKey::for_source(path, seconds, source_fps, max_width);
        if let Some(hit) = self.source_cache.get(&key) {
            return Ok(hit);
        }

        let decoded = if is_image_path(path) {
            decode_image(path, max_width)
        } else if is_sequence_manifest_path(path) {
            decode_sequence_at(path, seconds, max_width)
        } else if is_generator_manifest_path(path) {
            decode_generator_at(path, seconds, max_width)
        } else {
            self.decode_video_container(path, seconds, max_width)
        }?;

        let arc = Arc::new(decoded);
        self.source_cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    fn decode_video_container(
        &mut self,
        path: &Path,
        seconds: f64,
        max_width: u32,
    ) -> Result<FrameBuffer, String> {
        #[cfg(feature = "ffmpeg")]
        {
            use crate::decode::VideoDecoder;
            const MAX_OPEN_DECODERS: usize = 6;
            if !self.decoders.contains_key(path) {
                if self.decoders.len() >= MAX_OPEN_DECODERS {
                    self.decoders.clear();
                }
                self.decoders
                    .insert(path.to_path_buf(), VideoDecoder::open(path)?);
            }
            let res = self
                .decoders
                .get_mut(path)
                .ok_or_else(|| "decoder unavailable".to_string())
                .and_then(|d| d.frame_at(seconds, max_width));
            if res.is_err() {
                self.decoders.remove(path);
            }
            res
        }
        #[cfg(not(feature = "ffmpeg"))]
        {
            let _ = (path, seconds, max_width);
            Err(DISABLED.into())
        }
    }
}

fn run(
    jobs: Receiver<WorkerJob>,
    single_tx: Sender<FrameReply>,
    composite_tx: Sender<CompositeFrameReply>,
    source_tx: Sender<SourceReply>,
    on_reply: impl Fn(),
) {
    let mut state = WorkerState::new();
    let mut pending_next: Option<WorkerJob> = None;

    loop {
        let first = match pending_next.take() {
            Some(job) => job,
            None => match jobs.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
        };

        let mut latest_single: Option<FrameRequest> = None;
        let mut latest_composite: Option<CompositeFrameRequest> = None;
        let mut latest_source: Option<SourceRequest> = None;
        match first {
            WorkerJob::Single(r) => latest_single = Some(r),
            WorkerJob::Composite(r) => latest_composite = Some(r),
            WorkerJob::Source(r) => latest_source = Some(r),
        }
        // Scrubbing floods the queue: keep only the newest pending job of each kind.
        while let Ok(newer) = jobs.try_recv() {
            match newer {
                WorkerJob::Single(r) => latest_single = Some(r),
                WorkerJob::Composite(r) => latest_composite = Some(r),
                WorkerJob::Source(r) => latest_source = Some(r),
            }
        }

        if let Some(req) = latest_source {
            let result = state
                .decode_raw_cached(&req.path, req.seconds, 29.97, req.max_width)
                .map(|raw| (*raw).clone());
            if let Err(e) = &result {
                tracing::warn!("[WARN] source frame failed for {}: {e}", req.path.display());
            }
            let reply = SourceReply {
                path: req.path.clone(),
                seconds: req.seconds,
                result,
            };
            if source_tx.send(reply).is_err() {
                break;
            }
            on_reply();
        }

        if let Some(req) = latest_single {
            let result = handle_single(&mut state, &req);
            if let Err(e) = &result {
                tracing::warn!("[WARN] video decode failed for {}: {e}", req.path.display());
            }
            let reply = FrameReply {
                media_id: req.media_id,
                seconds: req.seconds,
                result,
            };
            if single_tx.send(reply).is_err() {
                break;
            }
            on_reply();
        }

        if let Some(req) = latest_composite {
            let result = handle_composite(&mut state, &req);
            if let Err(e) = &result {
                tracing::warn!("[WARN] composite video decode failed: {e}");
            }
            let reply = CompositeFrameReply {
                timeline_seconds: req.timeline_seconds,
                result,
            };
            if composite_tx.send(reply).is_err() {
                break;
            }
            on_reply();

            // Playback lookahead prefetch: while idle during active playback, decode the next 1..=2
            // frames along the shuttle direction into `source_cache` without blocking new UI scrubs.
            if req.playing && req.shuttle_speed.abs() > 1e-3 {
                pending_next = prefetch_lookahead(&mut state, &req, &jobs);
            }
        }
    }
}

fn handle_single(state: &mut WorkerState, req: &FrameRequest) -> Result<FrameBuffer, String> {
    let raw = state.decode_raw_cached(&req.path, req.seconds, 29.97, req.max_width)?;
    let mut frame = (*raw).clone();
    crate::chroma::apply_chroma_key(&mut frame, &req.chroma_key);
    crate::grading::apply_look(&mut frame, &req.grade, req.lut.as_deref());
    Ok(frame)
}

fn hash_f32(h: &mut impl Hasher, v: f32) {
    v.to_bits().hash(h);
}

/// Feeds a clip's Pan/Crop into the cache key.
fn hash_pan_crop(h: &mut impl Hasher, pan_crop: Option<&PanCropRequest>) {
    let Some(p) = pan_crop else {
        0u8.hash(h);
        return;
    };
    1u8.hash(h);
    p.stretch_to_fill.hash(h);
    for v in [
        p.frame.x_center,
        p.frame.y_center,
        p.frame.width,
        p.frame.height,
        p.frame.rotation_deg,
    ] {
        hash_f32(h, v);
    }
}

fn hash_node(h: &mut impl Hasher, node: &TrackCompositeNode, max_width: u32) {
    match node {
        TrackCompositeNode::Track {
            clips,
            mode,
            level,
            effects,
            fade_color,
            fade_amount,
            motion,
        } => {
            0u8.hash(h);
            mode.code().hash(h);
            hash_f32(h, *level);
            vfx::hash_chain(effects, h);
            fade_color.hash(h);
            hash_f32(h, *fade_amount);
            match motion {
                Some(m) => {
                    1u8.hash(h);
                    for v in [m.x_center, m.y_center, m.width, m.height, m.rotation_deg] {
                        hash_f32(h, v);
                    }
                }
                None => 0u8.hash(h),
            }
            clips.len().hash(h);
            for clip in clips {
                let sk = SourceCacheKey::for_source(
                    &clip.path,
                    clip.seconds,
                    clip.source_fps,
                    max_width,
                );
                sk.hash(h);
                hash_f32(h, clip.event_opacity);
                clip.apply_chroma.hash(h);
                vfx::hash_chain(&clip.effects, h);
                hash_pan_crop(h, clip.pan_crop.as_ref());
            }
        }
        TrackCompositeNode::Group {
            children,
            parent_clips,
            parent_mode,
            group_mode,
            group_level,
        } => {
            1u8.hash(h);
            parent_mode.code().hash(h);
            group_mode.code().hash(h);
            hash_f32(h, *group_level);
            children.len().hash(h);
            for c in children {
                hash_node(h, c, max_width);
            }
            parent_clips.len().hash(h);
            for clip in parent_clips {
                let sk = SourceCacheKey::for_source(
                    &clip.path,
                    clip.seconds,
                    clip.source_fps,
                    max_width,
                );
                sk.hash(h);
                hash_f32(h, clip.event_opacity);
                clip.apply_chroma.hash(h);
                vfx::hash_chain(&clip.effects, h);
                hash_pan_crop(h, clip.pan_crop.as_ref());
            }
        }
    }
}

fn composite_request_signature(req: &CompositeFrameRequest, out_w: u32, out_h: u32) -> u64 {
    let mut h = DefaultHasher::new();
    out_w.hash(&mut h);
    out_h.hash(&mut h);
    req.max_width.hash(&mut h);
    req.nodes.len().hash(&mut h);
    for n in &req.nodes {
        hash_node(&mut h, n, req.max_width);
    }
    hash_f32(&mut h, req.grade.exposure);
    hash_f32(&mut h, req.grade.temperature);
    hash_f32(&mut h, req.grade.tint);
    hash_f32(&mut h, req.grade.lift);
    hash_f32(&mut h, req.grade.contrast);
    hash_f32(&mut h, req.grade.gamma);
    hash_f32(&mut h, req.grade.saturation);
    vfx::hash_chain(&req.project_effects, &mut h);
    req.chroma_key.enabled.hash(&mut h);
    if req.chroma_key.enabled {
        for c in req.chroma_key.key_color {
            hash_f32(&mut h, c);
        }
        hash_f32(&mut h, req.chroma_key.similarity);
        hash_f32(&mut h, req.chroma_key.smoothness);
        hash_f32(&mut h, req.chroma_key.spill);
        req.chroma_key.show_matte.hash(&mut h);
    }
    if let Some(lut) = &req.lut {
        (Arc::as_ptr(lut) as usize).hash(&mut h);
    } else {
        0usize.hash(&mut h);
    }
    h.finish()
}

fn decode_clip_stack(
    state: &mut WorkerState,
    clips: &[VideoClipRequest],
    canvas_w: u32,
    canvas_h: u32,
    max_width: u32,
    chroma: &ChromaKey,
) -> Result<Option<Arc<FrameBuffer>>, String> {
    if clips.is_empty() {
        return Ok(None);
    }
    if clips.len() == 1 && (clips[0].event_opacity - 1.0).abs() < 1e-4 {
        let clip = &clips[0];
        let raw = state.decode_raw_cached(&clip.path, clip.seconds, clip.source_fps, max_width)?;
        let frame = if clip.apply_chroma && !chroma.is_disabled() {
            let mut keyed = (*raw).clone();
            crate::chroma::apply_chroma_key(&mut keyed, chroma);
            Arc::new(keyed)
        } else {
            raw
        };
        return Ok(Some(state.finish_clip(frame, clip)));
    }

    // Multiple overlapping clips on the same track (crossfade) or partial event fade-in/out:
    // composite onto a transparent track buffer so lower tracks remain visible outside/under fades.
    let mut layers = Vec::with_capacity(clips.len());
    for clip in clips {
        if clip.event_opacity <= 0.0 {
            continue;
        }
        let raw = state.decode_raw_cached(&clip.path, clip.seconds, clip.source_fps, max_width)?;
        let frame = if clip.apply_chroma && !chroma.is_disabled() {
            let mut keyed = (*raw).clone();
            crate::chroma::apply_chroma_key(&mut keyed, chroma);
            Arc::new(keyed)
        } else {
            raw
        };
        let frame = state.finish_clip(frame, clip);
        layers.push(CompositeNode::Layer(DecodedLayer {
            frame,
            mode: CompositeMode::SourceAlpha,
            opacity: clip.event_opacity.clamp(0.0, 1.0),
        }));
    }
    if layers.is_empty() {
        return Ok(None);
    }
    let merged = composite_nodes_cpu(canvas_w, canvas_h, [0, 0, 0, 0], &layers);
    Ok(Some(Arc::new(merged)))
}

fn build_decoded_tree(
    state: &mut WorkerState,
    nodes: &[TrackCompositeNode],
    canvas_w: u32,
    canvas_h: u32,
    max_width: u32,
    chroma: &ChromaKey,
) -> Result<Vec<CompositeNode>, String> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            TrackCompositeNode::Track {
                clips,
                mode,
                level,
                effects,
                fade_color,
                fade_amount,
                motion,
            } => {
                if *level <= 0.0 {
                    continue;
                }
                if let Some(frame) =
                    decode_clip_stack(state, clips, canvas_w, canvas_h, max_width, chroma)?
                {
                    let frame = state.apply_effects(frame, effects);
                    let frame = if *fade_amount > 0.0 {
                        let mut faded = (*frame).clone();
                        crate::grading::fade_to_color(&mut faded, *fade_color, *fade_amount);
                        Arc::new(faded)
                    } else {
                        frame
                    };
                    let frame = match motion.filter(|m| !m.is_full()) {
                        Some(m) => Arc::new(pan_crop::place(&frame, &m, canvas_w, canvas_h)),
                        None => frame,
                    };
                    out.push(CompositeNode::Layer(DecodedLayer {
                        frame,
                        mode: *mode,
                        opacity: level.clamp(0.0, 1.0),
                    }));
                }
            }
            TrackCompositeNode::Group {
                children,
                parent_clips,
                parent_mode,
                group_mode,
                group_level,
            } => {
                if *group_level <= 0.0 {
                    continue;
                }
                let decoded_children =
                    build_decoded_tree(state, children, canvas_w, canvas_h, max_width, chroma)?;
                let parent_layer =
                    decode_clip_stack(state, parent_clips, canvas_w, canvas_h, max_width, chroma)?
                        .map(|frame| DecodedLayer {
                            frame,
                            mode: *parent_mode,
                            opacity: 1.0,
                        });
                if decoded_children.is_empty() && parent_layer.is_none() {
                    continue;
                }
                out.push(CompositeNode::Group {
                    children: decoded_children,
                    parent_layer,
                    group_mode: *group_mode,
                    group_opacity: group_level.clamp(0.0, 1.0),
                });
            }
        }
    }
    Ok(out)
}

fn uses_non_trivial_blend(nodes: &[CompositeNode]) -> bool {
    nodes.len() > 1
        || nodes.iter().any(|n| match n {
            CompositeNode::Layer(l) => l.mode != CompositeMode::SourceAlpha || l.opacity < 0.999,
            CompositeNode::Group { .. } => true,
        })
}

fn handle_composite(
    state: &mut WorkerState,
    req: &CompositeFrameRequest,
) -> Result<FrameBuffer, String> {
    let (out_w, out_h) = fit_size(
        req.canvas_width.max(2),
        req.canvas_height.max(2),
        req.max_width,
    );
    let sig = composite_request_signature(req, out_w, out_h);
    if let Some(cached) = state.composite_cache.get(&sig) {
        return Ok((*cached).clone());
    }

    let decoded_tree = build_decoded_tree(
        state,
        &req.nodes,
        out_w,
        out_h,
        req.max_width,
        &req.chroma_key,
    )?;

    let composited = if decoded_tree.is_empty() {
        FrameBuffer::solid(out_w, out_h, [0, 0, 0, 255])
    } else if uses_non_trivial_blend(&decoded_tree) && (out_w * out_h) >= 64 * 64 {
        if let Some(gpu) = state.ensure_gpu() {
            gpu.composite_nodes_gpu(out_w, out_h, [0, 0, 0, 255], &decoded_tree)
                .unwrap_or_else(|_| {
                    composite_nodes_cpu(out_w, out_h, [0, 0, 0, 255], &decoded_tree)
                })
        } else {
            composite_nodes_cpu(out_w, out_h, [0, 0, 0, 255], &decoded_tree)
        }
    } else {
        composite_nodes_cpu(out_w, out_h, [0, 0, 0, 255], &decoded_tree)
    };

    let composited = if req.project_effects.is_empty() {
        composited
    } else {
        // The project chain sees the whole picture; the cache keeps the result of the chain
        let done = state.apply_effects(Arc::new(composited), &req.project_effects);
        Arc::try_unwrap(done).unwrap_or_else(|shared| (*shared).clone())
    };
    let mut composited = composited;
    crate::grading::apply_look(&mut composited, &req.grade, req.lut.as_deref());
    state
        .composite_cache
        .insert(sig, Arc::new(composited.clone()));
    Ok(composited)
}

fn collect_prefetch_clips<'a>(node: &'a TrackCompositeNode, out: &mut Vec<&'a VideoClipRequest>) {
    match node {
        TrackCompositeNode::Track { clips, .. } => out.extend(clips.iter()),
        TrackCompositeNode::Group {
            children,
            parent_clips,
            ..
        } => {
            for c in children {
                collect_prefetch_clips(c, out);
            }
            out.extend(parent_clips.iter());
        }
    }
}

fn prefetch_lookahead(
    state: &mut WorkerState,
    req: &CompositeFrameRequest,
    jobs: &Receiver<WorkerJob>,
) -> Option<WorkerJob> {
    let fps = if req.fps.is_finite() && req.fps > 0.0 {
        req.fps
    } else {
        29.97
    };
    let mut clips = Vec::new();
    for node in &req.nodes {
        collect_prefetch_clips(node, &mut clips);
    }

    for step in 1..=2 {
        let dt_timeline = (step as f64 / fps) * req.shuttle_speed;
        for clip in &clips {
            if let Ok(arrived) = jobs.try_recv() {
                return Some(arrived);
            }
            if is_image_path(&clip.path) {
                continue;
            }
            let next_seconds = (clip.seconds + dt_timeline * clip.playback_rate).max(0.0);
            let key = SourceCacheKey::for_source(
                &clip.path,
                next_seconds,
                clip.source_fps,
                req.max_width,
            );
            if !state.source_cache.contains(&key) {
                let _ = state.decode_raw_cached(
                    &clip.path,
                    next_seconds,
                    clip.source_fps,
                    req.max_width,
                );
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::still::detect_image_sequence;
    use image::{ImageBuffer, Rgba};
    use std::time::Duration;

    fn write_png(path: &Path, w: u32, h: u32, color: [u8; 4]) {
        ImageBuffer::from_pixel(w, h, Rgba(color))
            .save(path)
            .unwrap();
    }

    #[test]
    fn worker_always_spawns_and_decodes_stills_sequences_and_prefetches_ahead() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_seq_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let colors = [
            [250, 10, 10, 255],
            [10, 250, 10, 255],
            [10, 10, 250, 255],
            [240, 240, 10, 255],
        ];
        for (i, c) in colors.iter().enumerate() {
            write_png(&dir.join(format!("anim_{:04}.png", i + 1)), 48, 36, *c);
        }
        let seq = detect_image_sequence(&dir.join("anim_0001.png"), 25.0)
            .unwrap()
            .unwrap();
        let manifest = dir.join("anim.neovegseq");
        seq.save_manifest(&manifest).unwrap();

        let worker = FrameWorker::spawn(|| {}).unwrap();
        let make_req = |t: f64, playing: bool| CompositeFrameRequest {
            timeline_seconds: t,
            fps: 25.0,
            playing,
            shuttle_speed: 1.0,
            canvas_width: 48,
            canvas_height: 36,
            max_width: 0,
            nodes: vec![TrackCompositeNode::Track {
                clips: vec![VideoClipRequest {
                    media_id: 1,
                    path: manifest.clone(),
                    seconds: t,
                    source_fps: 25.0,
                    playback_rate: 1.0,
                    event_opacity: 1.0,
                    apply_chroma: false,
                    effects: Vec::new(),
                    pan_crop: None,
                }],
                mode: CompositeMode::SourceAlpha,
                level: 1.0,
                effects: Vec::new(),
                fade_color: [0, 0, 0],
                fade_amount: 0.0,
                motion: None,
            }],
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
            project_effects: Vec::new(),
        };

        // Request frame 0 while playing = true -> triggers lookahead prefetch of frames 1 and 2.
        worker.request_composite(make_req(0.0, true));
        let r0 = worker
            .composite_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(r0.result.unwrap().pixel(4, 4), colors[0]);

        // Give the background thread a brief moment to complete its 2-frame lookahead prefetch,
        // then remove frame 2 from disk to prove subsequent playback hits `source_cache`!
        std::thread::sleep(Duration::from_millis(120));
        std::fs::remove_file(dir.join("anim_0002.png")).unwrap();

        worker.request_composite(make_req(0.04, false));
        let r1 = worker
            .composite_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(
            r1.result.unwrap().pixel(4, 4),
            colors[1],
            "frame 1 must be served from the prefetched worker cache even after disk deletion"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn effect_request(
        path: &Path,
        clip_fx: Vec<VideoEffect>,
        track_fx: Vec<VideoEffect>,
    ) -> CompositeFrameRequest {
        CompositeFrameRequest {
            timeline_seconds: 0.0,
            fps: 25.0,
            playing: false,
            shuttle_speed: 1.0,
            canvas_width: 128,
            canvas_height: 96,
            max_width: 0,
            nodes: vec![TrackCompositeNode::Track {
                clips: vec![VideoClipRequest {
                    media_id: 1,
                    path: path.to_path_buf(),
                    seconds: 0.0,
                    source_fps: 25.0,
                    playback_rate: 1.0,
                    event_opacity: 1.0,
                    apply_chroma: false,
                    effects: clip_fx,
                    pan_crop: None,
                }],
                mode: CompositeMode::SourceAlpha,
                level: 1.0,
                effects: track_fx,
                fade_color: [0, 0, 0],
                fade_amount: 0.0,
                motion: None,
            }],
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
            project_effects: Vec::new(),
        }
    }

    #[test]
    fn event_and_track_effects_change_the_frame_without_touching_the_cached_source() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_fx_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("split.png");
        let mut img = ImageBuffer::from_pixel(128, 96, Rgba([0u8, 0, 0, 255]));
        for y in 0..96 {
            for x in 64..128 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        img.save(&path).unwrap();

        let blur = vec![VideoEffect::new(vfx::GAUSSIAN_BLUR).with("h_range", 0.05)];
        let mut renderer = OfflineFrameRenderer::new();
        let plain = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(
            plain.pixel(63, 40),
            [0, 0, 0, 255],
            "hard edge without effects"
        );

        let on_event = renderer
            .render_frame(&effect_request(&path, blur.clone(), Vec::new()))
            .unwrap();
        assert!(
            on_event.pixel(63, 40)[0] > 20,
            "event blur softens the edge"
        );
        let on_track = renderer
            .render_frame(&effect_request(&path, Vec::new(), blur))
            .unwrap();
        assert!(
            on_track.pixel(63, 40)[0] > 20,
            "track blur softens the edge"
        );

        // The cached source frame and the composite cache were not poisoned by the effects
        let again = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(again, plain);

        // One changed value is a different frame (the cache key follows the parameters)
        let wider = vec![VideoEffect::new(vfx::GAUSSIAN_BLUR).with("h_range", 0.01)];
        let narrow = renderer
            .render_frame(&effect_request(&path, wider, Vec::new()))
            .unwrap();
        assert_ne!(narrow, on_event);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_effects_act_on_the_whole_picture_and_follow_the_cache_key() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_pfx_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("white.png");
        write_png(&path, 128, 96, [255, 255, 255, 255]);

        let mut renderer = OfflineFrameRenderer::new();
        let plain = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(plain.pixel(0, 0), [255, 255, 255, 255]);

        let mut with_vignette = effect_request(&path, Vec::new(), Vec::new());
        with_vignette.project_effects = vec![VideoEffect::new(vfx::VIGNETTE)];
        let dark = renderer.render_frame(&with_vignette).unwrap();
        assert!(dark.pixel(0, 0)[0] < 30, "the corner is dark");
        assert_eq!(
            dark.pixel(64, 48),
            [255, 255, 255, 255],
            "the centre is not"
        );

        // The same request without the chain is a different frame (the cache key follows it)
        let again = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(again, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_track_fade_to_color_paints_the_layer_and_follows_the_cache_key() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_fade_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("white.png");
        write_png(&path, 128, 96, [255, 255, 255, 255]);

        let mut renderer = OfflineFrameRenderer::new();
        let plain = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(plain.pixel(10, 10), [255, 255, 255, 255]);

        let faded = |amount: f32| {
            let mut request = effect_request(&path, Vec::new(), Vec::new());
            if let TrackCompositeNode::Track {
                fade_color,
                fade_amount,
                ..
            } = &mut request.nodes[0]
            {
                *fade_color = [255, 0, 0];
                *fade_amount = amount;
            }
            request
        };
        let half = renderer.render_frame(&faded(0.5)).unwrap();
        assert_eq!(half.pixel(10, 10), [255, 128, 128, 255]);
        let full = renderer.render_frame(&faded(1.0)).unwrap();
        assert_eq!(full.pixel(10, 10), [255, 0, 0, 255]);
        // Back to no fade is the original picture again (neither cache served a stale frame)
        let again = renderer.render_frame(&faded(0.0)).unwrap();
        assert_eq!(again, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_clip_pan_crop_frames_the_picture_and_follows_the_cache_key() {
        use neoveg_common::pan_crop::PanCropFrame;
        let dir = std::env::temp_dir().join(format!("neoveg_worker_pc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("split.png");
        let mut img = ImageBuffer::from_pixel(128, 96, Rgba([255u8, 0, 0, 255]));
        for y in 0..96 {
            for x in 64..128 {
                img.put_pixel(x, y, Rgba([0, 0, 255, 255]));
            }
        }
        img.save(&path).unwrap();

        let with_box = |x_center: f32| {
            let mut request = effect_request(&path, Vec::new(), Vec::new());
            if let TrackCompositeNode::Track { clips, .. } = &mut request.nodes[0] {
                clips[0].pan_crop = Some(PanCropRequest {
                    frame: PanCropFrame {
                        x_center,
                        width: 0.5,
                        ..PanCropFrame::FULL
                    },
                    stretch_to_fill: true,
                });
            }
            request
        };
        let mut renderer = OfflineFrameRenderer::new();
        let plain = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(plain.pixel(10, 10), [255, 0, 0, 255]);
        assert_eq!(plain.pixel(120, 10), [0, 0, 255, 255]);

        // The left half of the picture fills the frame: red everywhere
        let left = renderer.render_frame(&with_box(0.25)).unwrap();
        assert_eq!(left.pixel(120, 10), [255, 0, 0, 255]);
        // Moving the box is a different frame, not a cached one
        let right = renderer.render_frame(&with_box(0.75)).unwrap();
        assert_eq!(right.pixel(10, 10), [0, 0, 255, 255]);
        // And the cached source frame was not changed by any of it
        let again = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(again, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_worker_returns_the_raw_source_frame_on_its_own_channel() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_src_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("green.png");
        write_png(&path, 40, 30, [0, 255, 0, 255]);

        let worker = FrameWorker::spawn(|| {}).unwrap();
        worker.request_source(SourceRequest {
            path: path.clone(),
            seconds: 0.0,
            max_width: 0,
        });
        let reply = worker
            .source_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the worker answers");
        assert_eq!(reply.path, path);
        let frame = reply.result.unwrap();
        assert_eq!((frame.width, frame.height), (40, 30));
        assert_eq!(frame.pixel(5, 5), [0, 255, 0, 255]);
        assert!(
            worker.rx.try_recv().is_err(),
            "nothing leaks into the preview channel"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_motion_of_a_track_places_its_layer_in_the_frame_and_follows_the_cache_key() {
        let dir = std::env::temp_dir().join(format!("neoveg_worker_motion_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("white.png");
        write_png(&path, 128, 96, [255, 255, 255, 255]);

        let placed = |x_center: f32| {
            let mut request = effect_request(&path, Vec::new(), Vec::new());
            if let TrackCompositeNode::Track { motion, .. } = &mut request.nodes[0] {
                *motion = Some(PanCropFrame {
                    x_center,
                    y_center: 0.5,
                    width: 0.3,
                    height: 0.3,
                    rotation_deg: 0.0,
                });
            }
            request
        };
        let mut renderer = OfflineFrameRenderer::new();
        let plain = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(plain.pixel(10, 10), [255, 255, 255, 255]);

        // A small box on the left: white there, the black canvas (nothing below) elsewhere
        let left = renderer.render_frame(&placed(0.2)).unwrap();
        assert_eq!(left.pixel(26, 48), [255, 255, 255, 255]);
        assert_eq!(left.pixel(100, 48), [0, 0, 0, 255]);
        // Moving the box is a different frame, not the cached one
        let right = renderer.render_frame(&placed(0.8)).unwrap();
        assert_eq!(right.pixel(26, 48), [0, 0, 0, 255]);
        assert_eq!(right.pixel(102, 48), [255, 255, 255, 255]);
        let again = renderer
            .render_frame(&effect_request(&path, Vec::new(), Vec::new()))
            .unwrap();
        assert_eq!(again, plain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(feature = "ffmpeg"))]
    #[test]
    fn video_container_without_ffmpeg_reports_clear_portuguese_message() {
        assert!(probe(Path::new("x.mp4")).unwrap_err().contains("ffmpeg"));
    }
}

#[cfg(all(test, feature = "ffmpeg"))]
mod ffmpeg_tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn worker_decodes_requested_frames_off_thread() {
        let path = std::env::temp_dir().join(format!("neoveg_worker_{}.mp4", std::process::id()));
        let made = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg("testsrc=duration=2:size=320x240:rate=25")
            .args(["-c:v", "mpeg4"])
            .arg(&path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !made {
            return;
        }

        let worker = FrameWorker::spawn(|| {}).unwrap();
        worker.request(FrameRequest {
            media_id: 7,
            path: path.clone(),
            seconds: 0.4,
            max_width: 160,
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
            project_effects: Vec::new(),
        });
        let reply = worker.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(reply.media_id, 7);
        let frame = reply.result.unwrap();
        assert_eq!((frame.width, frame.height), (160, 120));

        worker.request(FrameRequest {
            media_id: 7,
            path: PathBuf::from("missing.mp4"),
            seconds: 0.0,
            max_width: 0,
            lut: None,
            grade: Grade::default(),
            chroma_key: ChromaKey::default(),
            project_effects: Vec::new(),
        });
        assert!(worker
            .rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .is_err());
        let _ = std::fs::remove_file(path);
    }
}
