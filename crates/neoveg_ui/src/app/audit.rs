//! Scan of the whole interface, in every language, without a GPU (`--ignored` test).
//!
//! Each scene (a panel, a window or the whole workspace) is drawn for a few frames; the text that egui
//! lays out is read back from the shapes of the last frame, with its rectangle and the clip rectangle
//! it is drawn in. From that the scan reports what no unit test sees: text cut by its box, text outside
//! the screen, glyphs the font does not have, widget ids that clash, scenes that panic. It also writes
//! every text per language to a JSON file, so a script can compare the languages (a text that is the
//! same in Portuguese and in English is a string that never went through `tr`).
//!
//! ```text
//! NEOVEG_AUDIT_OUT=target/audit cargo test -p neoveg_ui audit_the_interface -- --ignored --nocapture
//! ```

use super::*;
use neoveg_common::i18n;
use std::collections::BTreeMap;

struct Item {
    text: String,
    rect: egui::Rect,
    clip: egui::Rect,
    tofu: String,
}

fn collect(ctx: &egui::Context, shape: &egui::Shape, clip: egui::Rect, items: &mut Vec<Item>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect(ctx, s, clip, items);
            }
        }
        egui::Shape::Text(text) => {
            let galley = &text.galley;
            let string = galley.text().to_string();
            if string.trim().is_empty() {
                return;
            }
            let rect = galley.rect.translate(text.pos.to_vec2());
            let font = galley
                .job
                .sections
                .first()
                .map(|s| s.format.font_id.clone())
                .unwrap_or_default();
            let tofu: String = ctx.fonts(|f| {
                string
                    .chars()
                    .filter(|c| !c.is_whitespace() && !c.is_control() && !f.has_glyph(&font, *c))
                    .collect()
            });
            items.push(Item {
                text: string,
                rect,
                clip,
                tofu,
            });
        }
        _ => {}
    }
}

/// How a scene builds its application state and dock.
type Build = Box<dyn Fn(&egui::Context, &Path) -> (AppState, DockState<Tab>)>;

type Scene = (&'static str, [u32; 2], Build);

type BuildFn = dyn Fn(&egui::Context, &Path) -> (AppState, DockState<Tab>);

fn single(tab: Tab) -> DockState<Tab> {
    DockState::new(vec![tab])
}

fn scenes() -> Vec<Scene> {
    let mut v: Vec<Scene> = Vec::new();
    macro_rules! scene {
        ($name:expr, $size:expr, |$ctx:ident, $dir:ident| $body:expr) => {
            v.push(($name, $size, Box::new(|$ctx, $dir| $body)));
        };
    }
    scene!("workspace", [1600, 900], |_c, _d| (
        demo_project(),
        default_dock()
    ));
    scene!("workspace_small", [1024, 640], |_c, _d| (
        demo_project(),
        default_dock()
    ));
    scene!("workspace_empty", [1600, 900], |_c, _d| (
        AppState::new(),
        default_dock()
    ));
    scene!("tabs", [1600, 900], |c, d| (
        demo_tabs(c, d),
        default_dock()
    ));
    scene!("welcome", [1280, 720], |_c, d| (
        demo_welcome(d),
        default_dock()
    ));
    for (name, tab) in [
        ("panel_preview", Tab::Preview),
        ("panel_timeline", Tab::Timeline),
        ("panel_media", Tab::Media),
        ("panel_master", Tab::MasterBus),
        ("panel_loudness", Tab::Loudness),
        ("panel_audio_fx", Tab::AudioFx),
        ("panel_video_fx", Tab::VideoFx),
        ("panel_transitions", Tab::Transitions),
        ("panel_scopes", Tab::Scopes),
    ] {
        v.push((
            name,
            [900, 600],
            Box::new(move |_c, _d| (demo_project(), single(tab))),
        ));
    }
    scene!("effects_dock", [1600, 900], |_c, _d| (
        demo_project_with_video_effects(),
        effects_dock()
    ));
    scene!("transitions_dock", [1600, 900], |_c, _d| (
        demo_project_with_transition(),
        transitions_dock()
    ));
    scene!("velocity", [1600, 900], |_c, _d| (
        demo_project_with_velocity(),
        effects_dock()
    ));
    scene!("project_dialog", [1280, 860], |_c, _d| {
        let mut s = demo_project();
        s.project_dialog
            .open_with(&s.project.settings.clone(), false);
        (s, default_dock())
    });
    scene!("details_and_notes", [1400, 700], |_c, _d| {
        let mut s = demo_project();
        s.show_details = true;
        s.notes.open = true;
        (s, single(Tab::Timeline))
    });
    scene!("preferences", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_preferences = true;
        (s, default_dock())
    });
    scene!("render_dialog", [1280, 860], |_c, _d| {
        let mut s = demo_project();
        s.render_dialog.open = true;
        (s, default_dock())
    });
    scene!("grade", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_grade = true;
        (s, default_dock())
    });
    scene!("chroma", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_chroma = true;
        (s, default_dock())
    });
    scene!("inspector", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_inspector = true;
        (s, default_dock())
    });
    scene!("metronome", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_metronome = true;
        (s, default_dock())
    });
    scene!("credits", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_credits = true;
        (s, default_dock())
    });
    scene!("keymap", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.show_keymap = true;
        (s, default_dock())
    });
    scene!("log_viewer", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.log_viewer.open = true;
        (s, default_dock())
    });
    scene!("generator", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.generator_dialog.open = true;
        (s, default_dock())
    });
    scene!("slideshow", [1280, 760], |_c, _d| {
        let mut s = demo_project();
        s.slideshow.open = true;
        (s, default_dock())
    });
    scene!("pan_crop_event", [1280, 760], |_c, _d| {
        let mut s = demo_project_with_pan_crop();
        let (track, event, _) = pan_crop_demo_target(&s);
        s.pan_crop.open_event(track, event);
        (s, default_dock())
    });
    scene!("pan_crop_track", [1280, 760], |_c, _d| {
        let mut s = demo_project_with_track_motion();
        let (track, _) = track_motion_demo_target(&s);
        s.pan_crop.open_track(track);
        (s, default_dock())
    });
    v
}

/// Runs frames of one application with the input the test gives it.
struct Driver {
    ctx: egui::Context,
    app: NeovegApp,
    screen: egui::Rect,
    frame: usize,
    items: Vec<Item>,
}

impl Driver {
    fn new(tag: &str, size: [u32; 2], scratch: &Path, build: &BuildFn) -> Self {
        let ctx = new_context();
        crate::fonts::install_blocking(&ctx, tag);
        let (mut state, dock) = build(&ctx, scratch);
        state.prefs.language = tag.to_owned();
        Self {
            app: headless_app(state, dock),
            ctx,
            screen: egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32, size[1] as f32),
            ),
            frame: 0,
            items: Vec::new(),
        }
    }

    /// One frame with `events`; the text of that frame is kept in `items`.
    fn step(&mut self, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(self.screen),
            time: Some(START_TIME + self.frame as f64 * FRAME_STEP),
            events,
            ..Default::default()
        };
        self.frame += 1;
        let app = &mut self.app;
        let output = self.ctx.run(input, |ctx| app.show(ctx));
        self.items.clear();
        for clipped in &output.shapes {
            collect(
                &self.ctx,
                &clipped.shape,
                clipped.clip_rect,
                &mut self.items,
            );
        }
    }

    fn settle(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step(Vec::new());
        }
    }

    /// Moves the pointer to `pos` and leaves it there for a few frames (hover opens submenus).
    fn hover(&mut self, pos: egui::Pos2) {
        self.step(vec![egui::Event::PointerMoved(pos)]);
        self.settle(3);
    }

    fn click(&mut self, pos: egui::Pos2) {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.step(vec![egui::Event::PointerMoved(pos)]);
        self.step(vec![button(true)]);
        self.step(vec![button(false)]);
        self.settle(4);
    }
}

fn rect_json(r: egui::Rect) -> serde_json::Value {
    serde_json::json!([r.min.x, r.min.y, r.max.x, r.max.y])
}

/// A text box wider than the area it is clipped to (a label cut at the side of its panel).
fn cut_sideways(item: &Item) -> bool {
    item.clip.is_positive()
        && (item.rect.min.x < item.clip.min.x - 1.0 || item.rect.max.x > item.clip.max.x + 1.0)
}

/// Every check on the text of one scene; findings go to `report`, the texts to the returned list.
fn analyze(
    name: &str,
    items: &[Item],
    screen: egui::Rect,
    report: &mut Vec<String>,
) -> Vec<serde_json::Value> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique: Vec<&Item> = Vec::new();
    for item in items {
        let key = (
            item.text.clone(),
            item.rect.min.x as i32,
            item.rect.min.y as i32,
        );
        if seen.insert(key) {
            unique.push(item);
        }
    }
    for item in &unique {
        let shown: String = item.text.chars().take(60).collect();
        if !item.tofu.is_empty() {
            report.push(format!("[{name}] NO GLYPH {:?} in {shown:?}", item.tofu));
        }
        if cut_sideways(item) {
            report.push(format!(
                "[{name}] CUT at the side: {shown:?} box {:.0}..{:.0} clip {:.0}..{:.0}",
                item.rect.min.x, item.rect.max.x, item.clip.min.x, item.clip.max.x
            ));
        }
        let outside = item.rect.min.x < screen.min.x - 2.0
            || item.rect.min.y < screen.min.y - 2.0
            || item.rect.max.x > screen.max.x + 2.0
            || item.rect.max.y > screen.max.y + 2.0;
        if outside
            && item.clip.intersects(item.rect)
            && item.clip.contains_rect(item.rect.intersect(screen))
        {
            report.push(format!(
                "[{name}] OFF SCREEN: {shown:?} at {:.0},{:.0}..{:.0},{:.0}",
                item.rect.min.x, item.rect.min.y, item.rect.max.x, item.rect.max.y
            ));
        }
        if item.text.contains("clash") || item.text.contains("ID ") && item.text.contains('⚠') {
            report.push(format!("[{name}] WIDGET ID CLASH: {shown:?}"));
        }
    }
    // Two different texts drawn on top of each other, visible in both (inside their clip rectangles)
    for (i, a) in unique.iter().enumerate() {
        for b in &unique[i + 1..] {
            if a.text == b.text || a.text.contains(&b.text) || b.text.contains(&a.text) {
                continue;
            }
            let (ra, rb) = (a.rect.intersect(a.clip), b.rect.intersect(b.clip));
            if !ra.is_positive() || !rb.is_positive() {
                continue;
            }
            let both = ra.intersect(rb);
            if !both.is_positive() {
                continue;
            }
            let smaller = ra.area().min(rb.area());
            if smaller > 0.0 && both.area() / smaller > 0.35 {
                let (sa, sb): (String, String) = (
                    a.text.chars().take(30).collect(),
                    b.text.chars().take(30).collect(),
                );
                report.push(format!(
                    "[{name}] OVERLAP: {sa:?} over {sb:?} at {:.0},{:.0}",
                    both.min.x, both.min.y
                ));
            }
        }
    }
    unique
        .iter()
        .map(|item| {
            serde_json::json!({
                "t": item.text,
                "r": rect_json(item.rect),
                "c": rect_json(item.clip),
            })
        })
        .collect()
}

/// Opens every menu of the menu bar and hovers each entry that appears, so the submenus open too;
/// returns the text seen, one list per menu.
fn scan_menus(tag: &str, scratch: &Path) -> Vec<Vec<Item>> {
    let build = |_c: &egui::Context, _d: &Path| (demo_project(), default_dock());
    let mut probe = Driver::new(tag, [1600, 900], scratch, &build);
    probe.settle(WARM_UP_FRAMES + 1);
    // The menu bar is the first row of text at the top of the window
    let mut bar: Vec<(f32, egui::Pos2)> = probe
        .items
        .iter()
        .filter(|i| i.rect.min.y < 26.0 && i.rect.height() < 24.0)
        .map(|i| (i.rect.min.x, i.rect.center()))
        .collect();
    bar.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut menus = Vec::new();
    for (_, label) in bar {
        let mut d = Driver::new(tag, [1600, 900], scratch, &build);
        d.settle(WARM_UP_FRAMES + 1);
        let base: std::collections::BTreeSet<String> =
            d.items.iter().map(|i| i.text.clone()).collect();
        d.click(label);
        let mut seen: BTreeMap<(String, i32, i32), Item> = BTreeMap::new();
        let keep = |items: &mut Vec<Item>, seen: &mut BTreeMap<(String, i32, i32), Item>| {
            for item in items.drain(..) {
                let key = (
                    item.text.clone(),
                    item.rect.min.x as i32,
                    item.rect.min.y as i32,
                );
                seen.entry(key).or_insert(item);
            }
        };
        let mut now: Vec<Item> = std::mem::take(&mut d.items);
        keep(&mut now, &mut seen);
        // Hover each new entry, twice over so a submenu's own submenus open as well
        let mut hovered = std::collections::BTreeSet::new();
        for _ in 0..3 {
            let targets: Vec<egui::Pos2> = seen
                .values()
                .filter(|i| !base.contains(&i.text) && i.rect.min.y > 22.0)
                .map(|i| i.rect.center())
                .filter(|p| hovered.insert((p.x as i32, p.y as i32)))
                .collect();
            if targets.is_empty() {
                break;
            }
            for pos in targets {
                d.hover(pos);
                let mut now = std::mem::take(&mut d.items);
                keep(&mut now, &mut seen);
            }
        }
        menus.push(
            seen.into_values()
                .filter(|i| !base.contains(&i.text))
                .collect(),
        );
    }
    menus
}

#[test]
#[ignore = "scans the whole interface in every language; run on demand"]
fn audit_the_interface() {
    let out = std::env::var("NEOVEG_AUDIT_OUT").unwrap_or_else(|_| "target/audit".into());
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap();
    let scratch = std::env::temp_dir().join(format!("neoveg_audit_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let only: Option<Vec<String>> = std::env::var("NEOVEG_AUDIT_LANGS")
        .ok()
        .map(|s| s.split(',').map(str::to_owned).collect());

    let mut summary: BTreeMap<String, usize> = BTreeMap::new();
    for tag in i18n::LANGUAGES {
        if only.as_ref().is_some_and(|o| !o.iter().any(|t| t == tag)) {
            continue;
        }
        i18n::set_language(tag);
        let mut report: Vec<String> = Vec::new();
        let mut texts = serde_json::Map::new();
        for (name, size, build) in scenes() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut d = Driver::new(tag, size, &scratch, &*build);
                d.settle(WARM_UP_FRAMES + 1);
                (d.items, d.screen)
            }));
            let Ok((items, screen)) = result else {
                report.push(format!("[{name}] PANIC while drawing the scene"));
                continue;
            };
            let list = analyze(name, &items, screen, &mut report);
            texts.insert(name.to_owned(), serde_json::Value::Array(list));
        }
        let menus =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scan_menus(tag, &scratch)));
        match menus {
            Ok(menus) => {
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
                for (n, items) in menus.iter().enumerate() {
                    let name = format!("menu_{n:02}");
                    let list = analyze(&name, items, screen, &mut report);
                    texts.insert(name, serde_json::Value::Array(list));
                }
            }
            Err(_) => report.push("[menus] PANIC while opening the menus".into()),
        }
        let path = out.join(format!("texts_{tag}.json"));
        std::fs::write(&path, serde_json::to_string(&texts).unwrap()).unwrap();
        report.sort();
        report.dedup();
        std::fs::write(out.join(format!("findings_{tag}.txt")), report.join("\n")).unwrap();
        summary.insert(tag.to_owned(), report.len());
        println!("{tag}: {} findings", report.len());
    }
    let _ = std::fs::remove_dir_all(&scratch);
    i18n::set_language("pt-BR");
    println!("summary: {summary:?}");
}
