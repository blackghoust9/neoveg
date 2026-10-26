//! Startup screen: floating 3D shapes, the animated name, an animated film-reel artwork and a
//! progress bar that follows the real startup steps.
//!
//! Everything is drawn with egui shapes: the 3D objects are small meshes rotated and projected in
//! software (flat shaded, sorted far to near), so no model, texture or font file is needed. The
//! artwork (a film reel unspooling a strip of frames under the playhead) is original work made for
//! Neoveg and released with it under the MIT licence.
//!
//! The progress is not a fake timer: the application runs its startup steps one per frame
//! (preferences, graphics, audio device, effects, video decoder, recovery files) and reports each
//! one here. Each step stays on screen for a short minimum time so it can be read, and any key or
//! click skips the pacing.

use std::f32::consts::{PI, TAU};

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Mesh, Pos2, Rect, Rounding, Shape, Stroke,
};

// ---- pacing ----------------------------------------------------------------------------------------

/// Delay before the first step starts, so the intro is seen from the beginning.
const INTRO_S: f64 = 0.55;
/// Shortest time a step stays current, so its label can be read.
const MIN_STEP_S: f64 = 0.30;
/// The screen never closes before this much time has passed.
const MIN_TOTAL_S: f64 = 2.8;
/// Time the finished bar stays full before the screen fades out.
const HOLD_S: f64 = 0.35;
const FADE_IN_S: f64 = 0.45;
const FADE_OUT_S: f64 = 0.55;
/// Fade out when skipped.
const SKIP_FADE_S: f64 = 0.20;

/// The state machine behind the progress bar. It knows nothing about egui: the clock is passed in,
/// so it can be tested step by step.
#[derive(Debug, Clone)]
pub struct Pacing {
    steps: usize,
    started: Option<f64>,
    current: usize,
    running: bool,
    work_done: bool,
    step_started: f64,
    skipped_at: Option<f64>,
    all_done: Option<f64>,
}

impl Pacing {
    pub fn new(steps: usize) -> Self {
        Self {
            steps,
            started: None,
            current: 0,
            running: false,
            work_done: false,
            step_started: 0.0,
            skipped_at: None,
            all_done: None,
        }
    }

    fn start(&mut self, now: f64) -> f64 {
        *self.started.get_or_insert(now)
    }

    /// Any key or click: the steps no longer wait for their minimum time and the screen closes
    /// quickly.
    pub fn skip(&mut self, now: f64) {
        self.skipped_at.get_or_insert(now);
    }

    pub fn skipped(&self) -> bool {
        self.skipped_at.is_some()
    }

    fn advance(&mut self, now: f64) {
        if self.running
            && self.work_done
            && (self.skipped() || now - self.step_started >= MIN_STEP_S)
        {
            self.running = false;
            self.work_done = false;
            self.current += 1;
            if self.current >= self.steps {
                self.all_done.get_or_insert(now);
            }
        }
    }

    /// The step the application must run now (once), or `None`.
    pub fn pending_step(&mut self, now: f64) -> Option<usize> {
        let started = self.start(now);
        self.advance(now);
        if self.running || self.current >= self.steps {
            return None;
        }
        if now - started < INTRO_S && !self.skipped() {
            return None;
        }
        self.running = true;
        self.work_done = false;
        self.step_started = now;
        Some(self.current)
    }

    /// The application finished the step it was given.
    pub fn complete(&mut self, now: f64) {
        self.work_done = true;
        self.advance(now);
    }

    /// Index of the step shown as current (the last one stays after the end).
    pub fn current(&self) -> usize {
        self.current.min(self.steps.saturating_sub(1))
    }

    /// Progress of the bar, `0..=1`: finished steps plus how far the current one has got.
    pub fn progress(&self, now: f64) -> f32 {
        if self.steps == 0 || self.current >= self.steps {
            return 1.0;
        }
        let within = if self.running {
            let time = ((now - self.step_started) / MIN_STEP_S).clamp(0.0, 1.0) as f32;
            // Never the whole step before the work is reported done
            time * if self.work_done { 1.0 } else { 0.6 }
        } else {
            0.0
        };
        ((self.current as f32 + within) / self.steps as f32).clamp(0.0, 1.0)
    }

    /// When the fade-out starts.
    fn closing_at(&self) -> Option<f64> {
        if let Some(skipped) = self.skipped_at {
            return Some(skipped);
        }
        let done = self.all_done?;
        let started = self.started?;
        Some((done + HOLD_S).max(started + MIN_TOTAL_S))
    }

    /// Opacity of the whole screen: fades in at the start, out at the end.
    pub fn opacity(&self, now: f64) -> f32 {
        let Some(started) = self.started else {
            return 0.0;
        };
        let fade_in = ((now - started) / FADE_IN_S).clamp(0.0, 1.0);
        let fade_out = match self.closing_at() {
            Some(at) => {
                let length = if self.skipped() {
                    SKIP_FADE_S
                } else {
                    FADE_OUT_S
                };
                1.0 - ((now - at) / length).clamp(0.0, 1.0)
            }
            None => 1.0,
        };
        (fade_in.min(fade_out)) as f32
    }

    /// `true` once the screen has faded out completely.
    pub fn finished(&self, now: f64) -> bool {
        match self.closing_at() {
            Some(at) => {
                let length = if self.skipped() {
                    SKIP_FADE_S
                } else {
                    FADE_OUT_S
                };
                now >= at + length
            }
            None => false,
        }
    }

    /// Seconds since the screen appeared.
    pub fn elapsed(&self, now: f64) -> f64 {
        self.started.map_or(0.0, |s| (now - s).max(0.0))
    }
}

// ---- easing and the animated name --------------------------------------------------------------------

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Overshoots a little before settling at 1.
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let (c1, c3) = (1.70158_f32, 2.70158_f32);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

const NAME: &str = "NEOVEG";
/// Seconds between one letter starting and the next.
const LETTER_STAGGER_S: f32 = 0.11;
const LETTER_TIME_S: f32 = 0.7;
const NAME_START_S: f32 = 0.30;

/// How one letter looks at `t` seconds after the screen appeared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LetterAnim {
    /// Pixels above its place, as a fraction of the font size.
    pub rise: f32,
    pub scale: f32,
    pub alpha: f32,
}

pub fn letter_anim(index: usize, t: f32) -> LetterAnim {
    let local = (t - NAME_START_S - index as f32 * LETTER_STAGGER_S) / LETTER_TIME_S;
    let p = local.clamp(0.0, 1.0);
    let eased = ease_out_back(p);
    LetterAnim {
        rise: (1.0 - eased) * -0.9,
        scale: 1.0 + (1.0 - p) * 0.9,
        alpha: smoothstep(0.0, 0.35, p),
    }
}

/// Time at which the last letter has landed.
pub fn name_ready_at() -> f32 {
    NAME_START_S + (NAME.chars().count() - 1) as f32 * LETTER_STAGGER_S + LETTER_TIME_S
}

// ---- tiny 3D ----------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    fn scale(self, k: f32) -> V3 {
        V3::new(self.x * k, self.y * k, self.z * k)
    }
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn cross(self, o: V3) -> V3 {
        V3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    fn normalized(self) -> V3 {
        let l = self.length();
        if l > 1e-9 {
            self.scale(1.0 / l)
        } else {
            self
        }
    }
    /// Rotation by `angles` radians around x, then y, then z.
    pub fn rotated(self, angles: V3) -> V3 {
        let (sx, cx) = angles.x.sin_cos();
        let (sy, cy) = angles.y.sin_cos();
        let (sz, cz) = angles.z.sin_cos();
        let v = V3::new(self.x, self.y * cx - self.z * sx, self.y * sx + self.z * cx);
        let v = V3::new(v.x * cy + v.z * sy, v.y, -v.x * sy + v.z * cy);
        V3::new(v.x * cz - v.y * sz, v.x * sz + v.y * cz, v.z)
    }
}

/// A flat convex polygon of a mesh. `inside` is a point that lies behind the face (the centre of
/// the solid, or of the tube for a ring), used to make the normal point outwards.
#[derive(Debug, Clone)]
pub struct Face {
    pub idx: Vec<usize>,
    pub inside: V3,
}

#[derive(Debug, Clone)]
pub struct Mesh3 {
    pub verts: Vec<V3>,
    pub faces: Vec<Face>,
}

impl Mesh3 {
    /// Outward unit normal of `face` for vertices `verts` (the vertices may be rotated copies).
    fn normal_of(&self, verts: &[V3], face: &Face, inside: V3) -> V3 {
        let p: Vec<V3> = face.idx.iter().map(|&i| verts[i]).collect();
        let n = p[1].sub(p[0]).cross(p[2].sub(p[0])).normalized();
        let centroid = p
            .iter()
            .fold(V3::new(0.0, 0.0, 0.0), |a, b| a.add(*b))
            .scale(1.0 / p.len() as f32);
        if n.dot(centroid.sub(inside)) < 0.0 {
            n.scale(-1.0)
        } else {
            n
        }
    }

    pub fn cube() -> Self {
        let v = |x: f32, y: f32, z: f32| V3::new(x, y, z);
        let verts = vec![
            v(-1.0, -1.0, -1.0),
            v(1.0, -1.0, -1.0),
            v(1.0, 1.0, -1.0),
            v(-1.0, 1.0, -1.0),
            v(-1.0, -1.0, 1.0),
            v(1.0, -1.0, 1.0),
            v(1.0, 1.0, 1.0),
            v(-1.0, 1.0, 1.0),
        ];
        let quads = [
            [0, 1, 2, 3],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 3, 7, 4],
            [1, 2, 6, 5],
        ];
        Self {
            verts,
            faces: quads
                .iter()
                .map(|q| Face {
                    idx: q.to_vec(),
                    inside: V3::new(0.0, 0.0, 0.0),
                })
                .collect(),
        }
    }

    pub fn octahedron() -> Self {
        let v = |x: f32, y: f32, z: f32| V3::new(x, y, z);
        let verts = vec![
            v(1.3, 0.0, 0.0),
            v(-1.3, 0.0, 0.0),
            v(0.0, 1.3, 0.0),
            v(0.0, -1.3, 0.0),
            v(0.0, 0.0, 1.3),
            v(0.0, 0.0, -1.3),
        ];
        let mut faces = Vec::new();
        for &x in &[0, 1] {
            for &y in &[2, 3] {
                for &z in &[4, 5] {
                    faces.push(Face {
                        idx: vec![x, y, z],
                        inside: V3::new(0.0, 0.0, 0.0),
                    });
                }
            }
        }
        Self { verts, faces }
    }

    /// A "play" triangle pointing right, as a solid prism.
    pub fn play_prism() -> Self {
        let depth = 0.45;
        let tri = [(-0.85, -1.05), (-0.85, 1.05), (1.15, 0.0)];
        let mut verts = Vec::new();
        for z in [-depth, depth] {
            for (x, y) in tri {
                verts.push(V3::new(x, y, z));
            }
        }
        let inside = V3::new(0.1, 0.0, 0.0);
        let mut faces = vec![
            Face {
                idx: vec![0, 1, 2],
                inside,
            },
            Face {
                idx: vec![3, 4, 5],
                inside,
            },
        ];
        for i in 0..3 {
            let j = (i + 1) % 3;
            faces.push(Face {
                idx: vec![i, j, 3 + j, 3 + i],
                inside,
            });
        }
        Self { verts, faces }
    }

    /// A film can: a short cylinder.
    pub fn film_can() -> Self {
        let n = 16;
        let h = 0.38;
        let mut verts = Vec::new();
        for z in [-h, h] {
            for k in 0..n {
                let a = TAU * k as f32 / n as f32;
                verts.push(V3::new(1.1 * a.cos(), 1.1 * a.sin(), z));
            }
        }
        let inside = V3::new(0.0, 0.0, 0.0);
        let mut faces = vec![
            Face {
                idx: (0..n).collect(),
                inside,
            },
            Face {
                idx: (n..2 * n).collect(),
                inside,
            },
        ];
        for k in 0..n {
            let l = (k + 1) % n;
            faces.push(Face {
                idx: vec![k, l, n + l, n + k],
                inside,
            });
        }
        Self { verts, faces }
    }

    /// A ring (torus) with `major` / `minor` radii.
    pub fn torus(major: f32, minor: f32, seg_u: usize, seg_v: usize) -> Self {
        let mut verts = Vec::new();
        for u in 0..seg_u {
            let a = TAU * u as f32 / seg_u as f32;
            for v in 0..seg_v {
                let b = TAU * v as f32 / seg_v as f32;
                let r = major + minor * b.cos();
                verts.push(V3::new(r * a.cos(), r * a.sin(), minor * b.sin()));
            }
        }
        let at = |u: usize, v: usize| (u % seg_u) * seg_v + (v % seg_v);
        let mut faces = Vec::new();
        for u in 0..seg_u {
            let a = TAU * (u as f32 + 0.5) / seg_u as f32;
            // Centre of the tube at this angle
            let inside = V3::new(major * a.cos(), major * a.sin(), 0.0);
            for v in 0..seg_v {
                faces.push(Face {
                    idx: vec![at(u, v), at(u + 1, v), at(u + 1, v + 1), at(u, v + 1)],
                    inside,
                });
            }
        }
        Self { verts, faces }
    }
}

/// A shape floating on the screen.
#[derive(Debug, Clone)]
struct Floater {
    mesh: usize,
    /// Position on the screen, `-1..1` on both axes from the centre.
    at: (f32, f32),
    /// Distance from the camera: larger is farther, smaller and slower.
    depth: f32,
    /// Size in world units.
    size: f32,
    spin: V3,
    phase: f32,
    bob: f32,
    color: Color32,
}

const BLUE: Color32 = Color32::from_rgb(0x29, 0x80, 0xB9);
const CYAN: Color32 = Color32::from_rgb(0x3A, 0xC7, 0xE0);
const PURPLE: Color32 = Color32::from_rgb(0x8E, 0x5C, 0xD0);
const PINK: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x9A);
const GOLD: Color32 = Color32::from_rgb(0xF1, 0xC4, 0x0F);
const TEAL: Color32 = Color32::from_rgb(0x1A, 0xB3, 0x94);

/// The meshes, in the order [`Floater::mesh`] points into.
fn meshes() -> Vec<Mesh3> {
    vec![
        Mesh3::cube(),
        Mesh3::octahedron(),
        Mesh3::play_prism(),
        Mesh3::torus(1.0, 0.36, 22, 10),
        Mesh3::film_can(),
    ]
}

fn floaters() -> Vec<Floater> {
    let f = |mesh, at, depth, size, spin: (f32, f32, f32), phase, bob, color| Floater {
        mesh,
        at,
        depth,
        size,
        spin: V3::new(spin.0, spin.1, spin.2),
        phase,
        bob,
        color,
    };
    vec![
        f(
            3,
            (-0.86, -0.62),
            3.2,
            0.62,
            (0.30, 0.55, 0.10),
            0.0,
            0.035,
            BLUE,
        ),
        f(
            2,
            (0.84, -0.55),
            3.0,
            0.60,
            (0.15, 0.70, 0.05),
            1.3,
            0.04,
            CYAN,
        ),
        f(
            0,
            (-0.72, 0.58),
            3.6,
            0.46,
            (0.50, 0.35, 0.20),
            2.1,
            0.03,
            PURPLE,
        ),
        f(
            4,
            (0.78, 0.62),
            3.4,
            0.55,
            (0.55, 0.20, 0.08),
            0.7,
            0.035,
            GOLD,
        ),
        f(
            1,
            (-0.38, -0.84),
            4.6,
            0.38,
            (0.25, 0.60, 0.15),
            3.0,
            0.03,
            PINK,
        ),
        f(
            0,
            (0.42, 0.86),
            5.0,
            0.30,
            (0.60, 0.45, 0.25),
            4.2,
            0.025,
            TEAL,
        ),
        f(
            3,
            (0.18, -0.9),
            5.6,
            0.30,
            (0.35, 0.30, 0.45),
            5.1,
            0.025,
            PURPLE,
        ),
        f(
            1,
            (-0.95, 0.05),
            6.2,
            0.26,
            (0.20, 0.50, 0.30),
            0.9,
            0.02,
            CYAN,
        ),
        f(
            2,
            (0.96, 0.08),
            6.0,
            0.26,
            (0.30, 0.65, 0.10),
            2.7,
            0.02,
            PINK,
        ),
        f(
            4,
            (-0.2, 0.9),
            6.6,
            0.22,
            (0.45, 0.25, 0.20),
            3.6,
            0.02,
            BLUE,
        ),
    ]
}

/// A polygon ready to draw.
#[derive(Debug, Clone)]
struct DrawFace {
    depth: f32,
    points: Vec<Pos2>,
    fill: Color32,
    edge: Color32,
}

const SCENE_FOCAL: f32 = 1.05;
/// The shapes decorate the edges; they must not hide the name and the artwork.
const SIZE_SCALE: f32 = 0.58;
/// Direction from a surface towards the light (up, to the left, towards the viewer).
const TO_LIGHT: V3 = V3::new(-0.45, -0.65, -0.62);

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Every visible face of every floater at time `t`, sorted far to near. `pointer` is the mouse
/// position from the centre in `-1..1` (it shifts near shapes more than far ones).
fn scene_faces(rect: Rect, t: f32, pointer: (f32, f32), opacity: f32) -> Vec<DrawFace> {
    let meshes = meshes();
    let half = rect.size() / 2.0;
    let focal = rect.height().min(rect.width()) * SCENE_FOCAL;
    let mut out = Vec::new();
    for fl in floaters() {
        let mesh = &meshes[fl.mesh];
        let parallax = 1.0 / fl.depth;
        let bob = (t * 0.9 + fl.phase).sin() * fl.bob;
        let center = rect.center()
            + vec2(
                fl.at.0 * half.x + pointer.0 * 38.0 * parallax * 3.0,
                (fl.at.1 + bob) * half.y + pointer.1 * 26.0 * parallax * 3.0,
            );
        let angles = V3::new(fl.spin.x, fl.spin.y, fl.spin.z)
            .scale(t)
            .add(V3::new(fl.phase, fl.phase * 0.7, 0.0));
        let rotated: Vec<V3> = mesh
            .verts
            .iter()
            .map(|v| v.scale(fl.size * SIZE_SCALE).rotated(angles))
            .collect();
        let fog = ((fl.depth - 3.0) / 4.0).clamp(0.0, 1.0);
        let alpha = (1.0 - 0.55 * fog) * opacity;
        for face in &mesh.faces {
            let inside = face.inside.scale(fl.size * SIZE_SCALE).rotated(angles);
            let normal = mesh.normal_of(&rotated, face, inside);
            // The viewer looks along +z: a face is visible when its normal points back at them
            if normal.z >= 0.0 {
                continue;
            }
            let lit = 0.28 + 0.72 * normal.dot(TO_LIGHT).max(0.0);
            let shade = lerp_color(Color32::from_rgb(10, 12, 22), fl.color, lit.min(1.0));
            let highlight = lerp_color(shade, Color32::WHITE, 0.10 * lit);
            let mut depth = 0.0;
            let points: Vec<Pos2> = face
                .idx
                .iter()
                .map(|&i| {
                    let v = rotated[i];
                    let z = fl.depth + v.z;
                    depth += z;
                    center + vec2(v.x, v.y) * (focal / z)
                })
                .collect();
            out.push(DrawFace {
                depth: depth / face.idx.len() as f32,
                points,
                fill: with_alpha(highlight, alpha),
                edge: with_alpha(lerp_color(shade, Color32::WHITE, 0.35), alpha * 0.55),
            });
        }
    }
    out.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    out
}

// ---- background ----------------------------------------------------------------------------------------

/// A vertical gradient over `rect`.
fn gradient_rect(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// A horizontal gradient over `rect`.
fn gradient_rect_h(painter: &egui::Painter, rect: Rect, left: Color32, right: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), left);
    mesh.colored_vertex(rect.right_top(), right);
    mesh.colored_vertex(rect.right_bottom(), right);
    mesh.colored_vertex(rect.left_bottom(), left);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// Soft coloured glows that drift behind everything, and tiny stars.
fn paint_background(painter: &egui::Painter, rect: Rect, t: f32, opacity: f32) {
    gradient_rect(
        painter,
        rect,
        with_alpha(Color32::from_rgb(7, 9, 18), opacity),
        with_alpha(Color32::from_rgb(20, 24, 44), opacity),
    );
    let size = rect.width().min(rect.height());
    for (i, (color, radius, speed)) in [(BLUE, 0.7, 0.13), (PURPLE, 0.6, 0.10), (TEAL, 0.5, 0.17)]
        .into_iter()
        .enumerate()
    {
        let a = t * speed + i as f32 * 2.1;
        let center = rect.center()
            + vec2(
                a.cos() * rect.width() * 0.28,
                (a * 1.3).sin() * rect.height() * 0.22,
            );
        // A radial glow is a stack of fading discs
        for k in 0..14 {
            let f = k as f32 / 13.0;
            painter.circle_filled(
                center,
                size * radius * (1.0 - f * 0.85),
                with_alpha(color, 0.010 * opacity),
            );
        }
    }
    // Stars: deterministic positions, slowly drifting up and twinkling
    for i in 0..90u32 {
        let h = |k: u32| {
            let mut x = i
                .wrapping_mul(2_654_435_761)
                .wrapping_add(k.wrapping_mul(40_503));
            x ^= x >> 15;
            x = x.wrapping_mul(2_246_822_519);
            x ^= x >> 13;
            (x % 10_000) as f32 / 10_000.0
        };
        let speed = 0.004 + h(3) * 0.012;
        let y = (h(2) - t * speed).rem_euclid(1.0);
        let twinkle = 0.5 + 0.5 * (t * (1.0 + h(4) * 2.0) + h(5) * TAU).sin();
        painter.circle_filled(
            pos2(
                rect.left() + h(1) * rect.width(),
                rect.top() + y * rect.height(),
            ),
            0.6 + h(6) * 1.4,
            with_alpha(Color32::WHITE, (0.12 + 0.4 * twinkle * h(7)) * opacity),
        );
    }
}

// ---- the film-reel artwork -------------------------------------------------------------------------------

/// The reel with a strip of frames unspooling under the playhead, inside `area`.
fn paint_reel_art(painter: &egui::Painter, area: Rect, t: f32, opacity: f32) {
    let painter = painter.with_clip_rect(area);
    let h = area.height();
    let reel_r = h * 0.46;
    let reel_c = pos2(area.left() + reel_r + 6.0, area.center().y);
    let strip_h = h * 0.50;
    let strip = Rect::from_min_max(
        pos2(reel_c.x, area.center().y - strip_h / 2.0),
        pos2(area.right(), area.center().y + strip_h / 2.0),
    );
    let a = |c: Color32, k: f32| with_alpha(c, k * opacity);

    // Film strip: dark base, sprocket holes along both edges and the frames, all scrolling
    painter.rect_filled(strip, Rounding::ZERO, a(Color32::from_rgb(16, 18, 24), 1.0));
    painter.rect_stroke(
        strip,
        Rounding::ZERO,
        Stroke::new(1.0_f32, a(Color32::from_gray(60), 1.0)),
    );
    let scroll = (t * 64.0) % 1000.0;
    let hole = strip_h * 0.085;
    let pitch = hole * 2.6;
    let edge = strip_h * 0.115;
    let mut x = strip.left() - pitch + scroll % pitch;
    while x < strip.right() {
        for y in [strip.top() + edge * 0.5, strip.bottom() - edge * 0.5] {
            painter.rect_filled(
                Rect::from_center_size(pos2(x, y), vec2(hole, hole * 0.8)),
                Rounding::same(1.5),
                a(Color32::from_rgb(40, 44, 56), 1.0),
            );
        }
        x += pitch;
    }
    let frame_h = strip_h - edge * 2.2;
    let frame_w = frame_h * 1.45;
    let gap = frame_w * 0.14;
    let step = frame_w + gap;
    let playhead_x = area.center().x + area.width() * 0.12;
    let first = ((strip.left() - frame_w - scroll) / step).floor() as i32;
    let palette = [BLUE, PURPLE, TEAL, PINK, GOLD, CYAN];
    for n in first..first + (strip.width() / step) as i32 + 4 {
        let left = strip.left() - scroll + n as f32 * step;
        let frame = Rect::from_min_size(
            pos2(left, strip.center().y - frame_h / 2.0),
            vec2(frame_w, frame_h),
        );
        if frame.right() < strip.left() || frame.left() > strip.right() {
            continue;
        }
        let kind = n.rem_euclid(4);
        let tone = palette[n.rem_euclid(palette.len() as i32) as usize];
        // The frame under the playhead lights up
        let under = smoothstep(frame_w * 0.9, 0.0, (frame.center().x - playhead_x).abs());
        gradient_rect(
            &painter,
            frame,
            a(
                lerp_color(Color32::from_rgb(20, 22, 32), tone, 0.35 + 0.45 * under),
                1.0,
            ),
            a(
                lerp_color(Color32::from_rgb(8, 10, 16), tone, 0.12 + 0.25 * under),
                1.0,
            ),
        );
        let ink = a(lerp_color(tone, Color32::WHITE, 0.55), 0.55 + 0.4 * under);
        match kind {
            0 => {
                // A play triangle
                let c = frame.center();
                let r = frame_h * 0.22;
                painter.add(Shape::convex_polygon(
                    vec![
                        c + vec2(-r * 0.7, -r),
                        c + vec2(-r * 0.7, r),
                        c + vec2(r * 1.1, 0.0),
                    ],
                    ink,
                    Stroke::NONE,
                ));
            }
            1 => {
                // Hills and a sun
                painter.circle_filled(
                    frame.left_top() + vec2(frame_w * 0.72, frame_h * 0.3),
                    frame_h * 0.13,
                    ink,
                );
                painter.add(Shape::convex_polygon(
                    vec![
                        frame.left_bottom(),
                        frame.left_bottom() + vec2(frame_w * 0.35, -frame_h * 0.42),
                        frame.left_bottom() + vec2(frame_w * 0.62, -frame_h * 0.2),
                        frame.left_bottom() + vec2(frame_w * 0.82, -frame_h * 0.36),
                        frame.right_bottom(),
                    ],
                    a(lerp_color(tone, Color32::BLACK, 0.45), 0.9),
                    Stroke::NONE,
                ));
            }
            2 => {
                // A waveform: the audio that goes with the picture
                let bars = 13;
                for b in 0..bars {
                    let w = frame_w * 0.8 / bars as f32;
                    let amp =
                        ((b as f32 * 0.9 + t * 3.0 + n as f32).sin() * 0.5 + 0.5) * 0.7 + 0.15;
                    let bx = frame.left() + frame_w * 0.1 + w * b as f32;
                    painter.rect_filled(
                        Rect::from_center_size(
                            pos2(bx + w * 0.4, frame.center().y),
                            vec2(w * 0.6, frame_h * 0.6 * amp),
                        ),
                        Rounding::ZERO,
                        ink,
                    );
                }
            }
            _ => {
                // Letterbox bars
                for y in [frame.top(), frame.bottom() - frame_h * 0.16] {
                    painter.rect_filled(
                        Rect::from_min_size(pos2(frame.left(), y), vec2(frame_w, frame_h * 0.16)),
                        Rounding::ZERO,
                        a(Color32::BLACK, 0.55),
                    );
                }
                painter.circle_stroke(frame.center(), frame_h * 0.2, Stroke::new(1.5_f32, ink));
            }
        }
        painter.rect_stroke(
            frame,
            Rounding::ZERO,
            Stroke::new(1.0_f32, a(Color32::from_gray(90), 0.7)),
        );
    }

    // The reel, on top of the strip's start: flanges, six rotating holes, hub and bolts
    painter.circle_filled(reel_c + vec2(4.0, 6.0), reel_r, a(Color32::BLACK, 0.35));
    painter.circle_filled(reel_c, reel_r, a(Color32::from_rgb(36, 40, 54), 1.0));
    painter.circle_stroke(reel_c, reel_r, Stroke::new(3.0_f32, a(BLUE, 1.0)));
    painter.circle_stroke(
        reel_c,
        reel_r * 0.86,
        Stroke::new(1.0_f32, a(Color32::from_gray(90), 0.8)),
    );
    let spin = -t * 0.9;
    for k in 0..6 {
        let ang = spin + k as f32 * PI / 3.0;
        let c = reel_c + vec2(ang.cos(), ang.sin()) * reel_r * 0.56;
        painter.circle_filled(c, reel_r * 0.2, a(Color32::from_rgb(10, 12, 18), 1.0));
        painter.circle_stroke(
            c,
            reel_r * 0.2,
            Stroke::new(1.5_f32, a(Color32::from_gray(110), 0.9)),
        );
        // A glint on the edge of each hole
        painter.circle_stroke(
            c - vec2(1.0, 1.0),
            reel_r * 0.2,
            Stroke::new(1.0_f32, a(Color32::WHITE, 0.18)),
        );
    }
    painter.circle_filled(reel_c, reel_r * 0.2, a(CYAN, 1.0));
    painter.circle_filled(reel_c, reel_r * 0.1, a(Color32::from_rgb(14, 16, 24), 1.0));
    for k in 0..4 {
        let ang = spin * 1.5 + k as f32 * PI / 2.0;
        painter.circle_filled(
            reel_c + vec2(ang.cos(), ang.sin()) * reel_r * 0.15,
            reel_r * 0.03,
            a(Color32::WHITE, 0.8),
        );
    }

    // Playhead: a glowing line and the blue pentagonal head of the timeline
    let glow = 0.5 + 0.5 * (t * 2.2).sin();
    for (w, al) in [(9.0_f32, 0.05_f32), (5.0, 0.10), (2.5, 0.25)] {
        painter.line_segment(
            [
                pos2(playhead_x, area.top() + 8.0),
                pos2(playhead_x, area.bottom() - 2.0),
            ],
            Stroke::new(w, a(CYAN, al * (0.6 + 0.4 * glow))),
        );
    }
    painter.line_segment(
        [
            pos2(playhead_x, area.top() + 14.0),
            pos2(playhead_x, area.bottom()),
        ],
        Stroke::new(1.5_f32, a(Color32::from_rgb(0x34, 0x98, 0xDB), 1.0)),
    );
    let top = area.top() + 1.0;
    painter.add(Shape::convex_polygon(
        vec![
            pos2(playhead_x - 7.0, top),
            pos2(playhead_x + 7.0, top),
            pos2(playhead_x + 7.0, top + 10.0),
            pos2(playhead_x, top + 17.0),
            pos2(playhead_x - 7.0, top + 10.0),
        ],
        a(BLUE, 1.0),
        Stroke::new(1.0_f32, a(Color32::from_rgb(0x34, 0x98, 0xDB), 1.0)),
    ));
}

// ---- the name --------------------------------------------------------------------------------------------

/// "NEOVEG" in thick letters that drop in one by one, with a glow, a bevel made of stacked copies
/// and a light that sweeps across once they have landed.
fn paint_name(painter: &egui::Painter, center: Pos2, size: f32, t: f32, opacity: f32) {
    let font = FontId::proportional(size);
    let spacing = size * 0.10;
    let letters: Vec<char> = NAME.chars().collect();
    let widths: Vec<f32> = letters
        .iter()
        .map(|c| {
            painter
                .layout_no_wrap(c.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + spacing * (letters.len() - 1) as f32;
    let sweep_start = name_ready_at() + 0.15;
    // The light crosses the word in 0.9 s, then rests for 2.2 s
    let phase = ((t - sweep_start).max(0.0)) % 3.1;
    let sweep = if t >= sweep_start && phase < 0.9 {
        phase / 0.9
    } else {
        -1.0
    };
    let mut x = center.x - total / 2.0;
    for (i, ch) in letters.iter().enumerate() {
        let w = widths[i];
        let anim = letter_anim(i, t);
        let calm = smoothstep(name_ready_at(), name_ready_at() + 0.6, t);
        let bob = (t * 1.6 + i as f32 * 0.55).sin() * size * 0.018 * calm;
        let pos = pos2(x + w / 2.0, center.y + anim.rise * size + bob);
        let font_i = FontId::proportional(size * anim.scale);
        let along = i as f32 / (letters.len() - 1) as f32;
        let base = lerp_color(BLUE, lerp_color(CYAN, PURPLE, along), along.sqrt());
        let shine = if sweep >= 0.0 {
            smoothstep(0.22, 0.0, (along - sweep * 1.3 + 0.15).abs())
        } else {
            0.0
        };
        let face = lerp_color(base, Color32::WHITE, 0.15 + 0.7 * shine);
        let al = anim.alpha * opacity;
        // Glow
        for k in 0..8 {
            let ang = k as f32 * PI / 4.0;
            painter.text(
                pos + vec2(ang.cos(), ang.sin()) * size * 0.045,
                Align2::CENTER_CENTER,
                ch,
                font_i.clone(),
                with_alpha(base, 0.07 * al),
            );
        }
        // Bevel: copies stacked behind, darker as they go back
        let depth_layers = 9;
        for layer in (1..=depth_layers).rev() {
            let f = layer as f32 / depth_layers as f32;
            painter.text(
                pos + vec2(1.0, 1.0) * layer as f32 * size * 0.0125,
                Align2::CENTER_CENTER,
                ch,
                font_i.clone(),
                with_alpha(
                    lerp_color(Color32::from_rgb(8, 22, 44), base, 0.45 * (1.0 - f)),
                    al,
                ),
            );
        }
        painter.text(pos, Align2::CENTER_CENTER, ch, font_i, with_alpha(face, al));
        x += w + spacing;
    }
}

// ---- the whole screen -------------------------------------------------------------------------------------

pub struct Splash {
    pub pacing: Pacing,
    labels: Vec<String>,
    /// What the last finished step reported (shown under the bar).
    detail: String,
    shown_progress: f32,
    pointer: (f32, f32),
}

impl Splash {
    pub fn new(labels: &[&str]) -> Self {
        Self {
            pacing: Pacing::new(labels.len()),
            labels: labels.iter().map(|s| (*s).to_owned()).collect(),
            detail: String::new(),
            shown_progress: 0.0,
            pointer: (0.0, 0.0),
        }
    }

    /// The application ran the step it was given and says what it found.
    pub fn step_done(&mut self, now: f64, detail: String) {
        let label = self
            .labels
            .get(self.pacing.current())
            .map_or("", String::as_str);
        self.detail = format!("{label}: {detail}");
        self.pacing.complete(now);
    }

    /// Draws the screen; returns `true` once it has faded out and the application can show.
    pub fn show(&mut self, ctx: &egui::Context, now: f64) -> bool {
        if ctx.input(|i| {
            i.pointer.any_pressed()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
        }) {
            self.pacing.skip(now);
        }
        if self.pacing.finished(now) {
            return true;
        }
        let opacity = self.pacing.opacity(now);
        let t = self.pacing.elapsed(now) as f32;

        // Smooth bar and pointer
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let target = self.pacing.progress(now);
        self.shown_progress += (target - self.shown_progress) * (1.0 - (-8.0 * dt).exp());
        if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
            let r = ctx.screen_rect();
            let goal = (
                ((p.x - r.center().x) / r.width() * 2.0).clamp(-1.0, 1.0),
                ((p.y - r.center().y) / r.height() * 2.0).clamp(-1.0, 1.0),
            );
            self.pointer.0 += (goal.0 - self.pointer.0) * (1.0 - (-3.0 * dt).exp());
            self.pointer.1 += (goal.1 - self.pointer.1) * (1.0 - (-3.0 * dt).exp());
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(7, 9, 18)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                self.paint(&painter, rect, t, now, opacity);
            });
        ctx.request_repaint();
        false
    }

    fn paint(&self, painter: &egui::Painter, rect: Rect, t: f32, now: f64, opacity: f32) {
        paint_background(painter, rect, t, opacity);
        for face in scene_faces(rect, t, self.pointer, opacity) {
            painter.add(Shape::convex_polygon(
                face.points,
                face.fill,
                Stroke::new(1.0_f32, face.edge),
            ));
        }

        // Centre column: artwork, name, tagline, progress
        let cx = rect.center().x;
        let unit = rect.height().min(rect.width() * 0.62);
        let art_h = (unit * 0.31).clamp(120.0, 260.0);
        let art_w = (rect.width() * 0.52).clamp(380.0, 760.0);
        let name_size = (unit * 0.20).clamp(54.0, 130.0);
        let block = art_h + name_size * 1.35 + 130.0;
        let top = rect.center().y - block / 2.0;

        let art = Rect::from_min_size(pos2(cx - art_w / 2.0, top), vec2(art_w, art_h));
        let art_in = smoothstep(0.15, 0.9, t);
        paint_reel_art(painter, art, t, opacity * art_in);

        let name_y = art.bottom() + name_size * 0.78;
        paint_name(painter, pos2(cx, name_y), name_size, t, opacity);

        let tagline_in = smoothstep(name_ready_at(), name_ready_at() + 0.5, t);
        painter.text(
            pos2(cx, name_y + name_size * 0.62),
            Align2::CENTER_TOP,
            "Edição de vídeo e áudio",
            FontId::proportional((name_size * 0.2).clamp(13.0, 22.0)),
            with_alpha(Color32::from_gray(190), 0.9 * tagline_in * opacity),
        );

        // Progress bar
        let bar_w = art_w.min(520.0);
        let bar_y = name_y + name_size * 0.62 + 52.0;
        let bar = Rect::from_min_size(pos2(cx - bar_w / 2.0, bar_y), vec2(bar_w, 7.0));
        let label = self
            .labels
            .get(self.pacing.current())
            .map_or("", String::as_str);
        let finished = self.pacing.progress(now) >= 1.0;
        let text = if finished {
            "Pronto".to_owned()
        } else {
            format!("Carregando: {label}…")
        };
        painter.text(
            pos2(bar.left(), bar.top() - 8.0),
            Align2::LEFT_BOTTOM,
            text,
            FontId::proportional(13.0),
            with_alpha(Color32::from_gray(215), opacity),
        );
        painter.text(
            pos2(bar.right(), bar.top() - 8.0),
            Align2::RIGHT_BOTTOM,
            format!("{:.0}%", self.shown_progress * 100.0),
            FontId::monospace(13.0),
            with_alpha(CYAN, opacity),
        );
        painter.rect_filled(
            bar,
            Rounding::ZERO,
            with_alpha(Color32::from_gray(30), opacity),
        );
        let fill = Rect::from_min_max(
            bar.min,
            pos2(bar.left() + bar.width() * self.shown_progress, bar.bottom()),
        );
        if fill.width() > 0.5 {
            gradient_rect_h(
                painter,
                fill,
                with_alpha(BLUE, opacity),
                with_alpha(CYAN, opacity),
            );
            // A light that runs along the filled part
            let sweep = (t * 0.9).fract();
            let sx = fill.left() + fill.width() * sweep;
            let shine = Rect::from_min_max(
                pos2((sx - 22.0).max(fill.left()), fill.top()),
                pos2((sx + 22.0).min(fill.right()), fill.bottom()),
            );
            if shine.width() > 0.0 {
                painter.rect_filled(
                    shine,
                    Rounding::ZERO,
                    with_alpha(Color32::WHITE, 0.28 * opacity),
                );
            }
        }
        painter.rect_stroke(
            bar,
            Rounding::ZERO,
            Stroke::new(1.0_f32, with_alpha(Color32::from_gray(70), opacity)),
        );
        if !self.detail.is_empty() {
            painter.text(
                pos2(bar.left(), bar.bottom() + 9.0),
                Align2::LEFT_TOP,
                &self.detail,
                FontId::proportional(11.5),
                with_alpha(Color32::from_gray(140), opacity),
            );
        }
        if t > 1.4 && !self.pacing.skipped() {
            painter.text(
                pos2(cx, rect.bottom() - 16.0),
                Align2::CENTER_BOTTOM,
                "Clique ou pressione qualquer tecla para pular",
                FontId::proportional(11.0),
                with_alpha(Color32::from_gray(110), 0.7 * opacity),
            );
        }
        painter.text(
            pos2(rect.left() + 14.0, rect.bottom() - 12.0),
            Align2::LEFT_BOTTOM,
            "Arte e animação originais do Neoveg · licença MIT",
            FontId::proportional(10.5),
            with_alpha(Color32::from_gray(95), 0.8 * opacity),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the pacing the way the application does, at 60 frames per second. `work` is how long
    /// each step takes to report done (0 = the same frame).
    fn run(pacing: &mut Pacing, frames: usize, skip_at: Option<f64>) -> (Vec<usize>, f64) {
        let mut ran = Vec::new();
        let mut finished_at = f64::NAN;
        for frame in 0..frames {
            let now = frame as f64 / 60.0;
            if skip_at.is_some_and(|s| now >= s) {
                pacing.skip(now);
            }
            if let Some(step) = pacing.pending_step(now) {
                ran.push(step);
                pacing.complete(now);
            }
            if pacing.finished(now) && finished_at.is_nan() {
                finished_at = now;
            }
        }
        (ran, finished_at)
    }

    #[test]
    fn every_step_runs_once_in_order_and_the_screen_ends() {
        let mut pacing = Pacing::new(6);
        let (ran, finished_at) = run(&mut pacing, 600, None);
        assert_eq!(ran, [0, 1, 2, 3, 4, 5]);
        assert!(finished_at.is_finite());
        assert!(
            finished_at >= MIN_TOTAL_S + FADE_OUT_S - 0.05,
            "{finished_at}"
        );
        assert!(finished_at < 6.0, "it must not drag on: {finished_at}");
    }

    #[test]
    fn a_step_stays_current_for_its_minimum_time() {
        let mut pacing = Pacing::new(3);
        let mut starts = Vec::new();
        for frame in 0..400 {
            let now = f64::from(frame) / 60.0;
            if let Some(step) = pacing.pending_step(now) {
                starts.push((step, now));
                pacing.complete(now);
            }
        }
        assert_eq!(starts.len(), 3);
        for pair in starts.windows(2) {
            assert!(pair[1].1 - pair[0].1 >= MIN_STEP_S - 1e-9, "{pair:?}");
        }
        assert!(starts[0].1 >= INTRO_S - 1e-9, "the intro plays first");
    }

    #[test]
    fn progress_only_goes_up_stays_in_range_and_waits_for_the_work() {
        let mut pacing = Pacing::new(4);
        let mut last = 0.0;
        for frame in 0..400 {
            let now = f64::from(frame) / 60.0;
            if let Some(_step) = pacing.pending_step(now) {
                // The work takes a while: the bar must not reach the end of the step yet
                assert!(pacing.progress(now) < 1.0);
            }
            if pacing.running && !pacing.work_done && now - pacing.step_started > 0.1 {
                pacing.complete(now);
            }
            let p = pacing.progress(now);
            assert!((0.0..=1.0).contains(&p));
            assert!(p >= last - 1e-6, "progress went back: {last} -> {p}");
            last = p;
        }
        assert_eq!(last, 1.0);
    }

    #[test]
    fn a_slow_step_holds_the_bar_below_its_end() {
        let mut pacing = Pacing::new(2);
        // The first call starts the clock; the intro has to pass before a step begins
        assert_eq!(pacing.pending_step(0.0), None);
        let started = pacing.pending_step(1.0).unwrap();
        assert_eq!(started, 0);
        // The step has not reported: even after a long time the bar is short of the first half
        assert!(pacing.progress(5.0) <= 0.3 + 1e-6);
        pacing.complete(5.0);
        assert!(pacing.progress(5.0) >= 0.5 - 1e-6);
    }

    #[test]
    fn skipping_runs_the_remaining_steps_at_once_and_closes_quickly() {
        let mut pacing = Pacing::new(6);
        let (ran, finished_at) = run(&mut pacing, 600, Some(0.1));
        assert_eq!(ran, [0, 1, 2, 3, 4, 5], "no step is lost by skipping");
        assert!(finished_at < 1.0, "{finished_at}");
        assert!(pacing.skipped());
    }

    #[test]
    fn the_screen_fades_in_stays_opaque_and_fades_out() {
        let mut pacing = Pacing::new(2);
        assert_eq!(pacing.opacity(0.0), 0.0, "not started yet");
        run(&mut pacing, 600, None);
        // Replay the same clock to read the opacity at known times
        let mut p = Pacing::new(2);
        let opacity_at = |p: &mut Pacing, now: f64| {
            if p.pending_step(now).is_some() {
                p.complete(now);
            }
            p.opacity(now)
        };
        assert!(opacity_at(&mut p, 0.0) < 0.05);
        assert!(opacity_at(&mut p, 0.2) < 1.0);
        assert!(opacity_at(&mut p, 1.0) > 0.99);
        for k in 1..400 {
            opacity_at(&mut p, 1.0 + f64::from(k) / 60.0);
        }
        assert_eq!(p.opacity(8.0), 0.0);
        assert!(p.finished(8.0));
    }

    #[test]
    fn a_pacing_without_steps_just_runs_the_clock() {
        let mut pacing = Pacing::new(0);
        assert_eq!(pacing.pending_step(0.0), None);
        assert_eq!(pacing.progress(0.0), 1.0);
    }

    #[test]
    fn the_letters_land_one_after_another_with_a_little_overshoot() {
        // Before its turn a letter is invisible; at the end it is in place
        let early = letter_anim(5, 0.1);
        assert_eq!(early.alpha, 0.0);
        let end = name_ready_at() + 0.01;
        for i in 0..NAME.chars().count() {
            let a = letter_anim(i, end);
            assert_eq!((a.alpha, a.scale), (1.0, 1.0));
            assert!(a.rise.abs() < 1e-3);
        }
        // Later letters start later
        let t = NAME_START_S + 0.2;
        assert!(letter_anim(0, t).alpha > letter_anim(3, t).alpha);
        // The easing goes past 1 before settling
        assert!(ease_out_back(0.7) > 1.0);
        assert_eq!((ease_out_back(0.0), ease_out_back(1.0)), (0.0, 1.0));
    }

    #[test]
    fn every_mesh_is_consistent_and_its_normals_point_outwards() {
        for (name, mesh) in [
            ("cube", Mesh3::cube()),
            ("octahedron", Mesh3::octahedron()),
            ("play", Mesh3::play_prism()),
            ("can", Mesh3::film_can()),
            ("torus", Mesh3::torus(1.0, 0.36, 22, 10)),
        ] {
            assert!(!mesh.faces.is_empty(), "{name}");
            for face in &mesh.faces {
                assert!(face.idx.len() >= 3, "{name}");
                assert!(face.idx.iter().all(|&i| i < mesh.verts.len()), "{name}");
                let n = mesh.normal_of(&mesh.verts, face, face.inside);
                assert!((n.length() - 1.0).abs() < 1e-3, "{name}: {n:?}");
                let centroid = face
                    .idx
                    .iter()
                    .fold(V3::new(0.0, 0.0, 0.0), |a, &i| a.add(mesh.verts[i]))
                    .scale(1.0 / face.idx.len() as f32);
                assert!(
                    n.dot(centroid.sub(face.inside)) > 0.0,
                    "{name} faces inwards"
                );
            }
        }
        // A cube has 6 faces and a torus 22 x 10
        assert_eq!(Mesh3::cube().faces.len(), 6);
        assert_eq!(Mesh3::torus(1.0, 0.3, 22, 10).faces.len(), 220);
    }

    #[test]
    fn rotation_keeps_lengths_and_a_full_turn_comes_back() {
        let v = V3::new(0.3, -0.8, 0.5);
        let r = v.rotated(V3::new(0.7, 1.9, -0.4));
        assert!((r.length() - v.length()).abs() < 1e-5);
        let back = v.rotated(V3::new(TAU, TAU, TAU));
        assert!(back.sub(v).length() < 1e-4);
    }

    #[test]
    fn the_scene_draws_only_front_faces_sorted_far_to_near_inside_the_window() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
        let faces = scene_faces(rect, 1.7, (0.0, 0.0), 1.0);
        assert!(faces.len() > 100, "{}", faces.len());
        for pair in faces.windows(2) {
            assert!(pair[0].depth >= pair[1].depth, "not sorted far to near");
        }
        // Everything stays near the window: shapes float around, they do not fly away
        let wide = rect.expand(300.0);
        assert!(faces
            .iter()
            .flat_map(|f| &f.points)
            .all(|p| wide.contains(*p)));
        // Fully transparent means invisible
        assert!(scene_faces(rect, 1.7, (0.0, 0.0), 0.0)
            .iter()
            .all(|f| f.fill.a() == 0));
    }

    #[test]
    fn the_shapes_move_over_time_and_with_the_pointer() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0));
        let sum = |faces: &[DrawFace]| -> f32 {
            faces
                .iter()
                .flat_map(|f| &f.points)
                .map(|p| p.x + p.y)
                .sum()
        };
        let a = scene_faces(rect, 0.0, (0.0, 0.0), 1.0);
        let b = scene_faces(rect, 2.0, (0.0, 0.0), 1.0);
        let c = scene_faces(rect, 0.0, (1.0, 1.0), 1.0);
        assert!((sum(&a) - sum(&b)).abs() > 1.0, "the scene is static");
        assert!((sum(&a) - sum(&c)).abs() > 1.0, "no parallax");
    }

    #[test]
    fn near_shapes_are_bigger_than_far_ones() {
        let all = floaters();
        let near = all
            .iter()
            .min_by(|a, b| a.depth.total_cmp(&b.depth))
            .unwrap();
        let far = all
            .iter()
            .max_by(|a, b| a.depth.total_cmp(&b.depth))
            .unwrap();
        assert!(near.size / near.depth > far.size / far.depth);
        assert!(all.iter().all(|f| f.mesh < meshes().len()));
        assert!(all
            .iter()
            .all(|f| f.at.0.abs() <= 1.0 && f.at.1.abs() <= 1.0));
    }
}
