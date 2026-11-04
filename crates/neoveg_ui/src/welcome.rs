//! The welcome screen: shown when the program starts to pick a recent project, open a file or
//! create a new project with the frame size and frame rate chosen on the spot.
//!
//! The reference manuals describe it as a dialog that quickly lets the user choose a project to
//! open or create a new one, with a list for the frame rate and another for the resolution, two
//! window layouts to choose from when there are no recent projects, and a preference that turns
//! the screen off. The window returns [`WelcomeAction`]s; the application carries them out.

use neoveg_common::clock::DayPart;
use neoveg_common::i18n::tr;
use neoveg_common::i18n::tr_args;
use neoveg_common::i18n::tr_text;
use std::path::Path;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Painter, Rect, RichText, Rounding,
    Sense, Stroke,
};
use neoveg_common::presets::{
    builtin_templates, frame_rate_of, resolution_named, ProjectTemplate, FRAME_RATES, RESOLUTIONS,
};
use neoveg_common::project::ProjectSettings;

use crate::external::Link;

/// The window arrangement a new project starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartLayout {
    /// Preview and timeline give most of the room to the picture.
    #[default]
    Video,
    /// The mixer, loudness and effects get the room: for sound work.
    Audio,
}

impl StartLayout {
    pub const ALL: [StartLayout; 2] = [StartLayout::Video, StartLayout::Audio];

    pub fn label(self) -> &'static str {
        match self {
            StartLayout::Video => "Edição de vídeo",
            StartLayout::Audio => "Edição de áudio e mixagem",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            StartLayout::Video => "Visualização grande, mídia à esquerda e linha do tempo embaixo.",
            StartLayout::Audio => {
                "Linha do tempo alta, barramento master e efeitos de áudio em destaque."
            }
        }
    }
}

/// The name of the operating system account: the full name when it has one, otherwise the user
/// name.
pub fn account_name() -> String {
    neoveg_common::sys_info::account_name()
}

/// The folder to show for "open the projects folder": the folder of the newest recent project that
/// still exists, otherwise the documents folder, otherwise the home folder.
pub fn projects_folder(recent: &[String]) -> std::path::PathBuf {
    recent
        .iter()
        .filter_map(|p| Path::new(p).parent().map(Path::to_path_buf))
        .find(|dir| dir.is_dir())
        .or_else(|| {
            let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
            let home = std::path::PathBuf::from(home);
            let documents = home.join("Documents");
            Some(if documents.is_dir() { documents } else { home })
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// One line of the recent projects list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    pub path: String,
    /// File name without the extension.
    pub name: String,
    pub folder: String,
    pub exists: bool,
}

impl RecentEntry {
    /// The last two folders of the path, enough to tell projects apart in a narrow list
    /// (the whole path is in the tooltip).
    pub fn short_folder(&self) -> String {
        let parts: Vec<&str> = self
            .folder
            .split(['/', '\\'])
            .filter(|p| !p.is_empty())
            .collect();
        let from = parts.len().saturating_sub(2);
        parts[from..].join("/")
    }
}

/// Turns the stored paths into list entries; `exists` tells whether a file is still there.
pub fn recent_entries(paths: &[String], exists: &dyn Fn(&Path) -> bool) -> Vec<RecentEntry> {
    paths
        .iter()
        .map(|text| {
            let path = Path::new(text);
            RecentEntry {
                path: text.clone(),
                name: path
                    .file_stem()
                    .map_or_else(|| text.clone(), |s| s.to_string_lossy().into_owned()),
                folder: path
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                exists: exists(path),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum WelcomeAction {
    OpenRecent(String),
    ForgetRecent(String),
    /// Choose a project file to open.
    OpenFile,
    NewProject {
        settings: Box<ProjectSettings>,
        layout: StartLayout,
    },
    /// The "show at startup" checkbox changed.
    ShowAtStartup(bool),
    /// Show the folder of the projects in the file manager.
    OpenProjectsFolder,
    /// Open the preferences, where the name of the user is.
    UserSettings,
    /// Open the list of the templates the user saved.
    ResolutionPresets,
    /// Tell the version in use and open the page of the published versions.
    CheckUpdates,
    /// One of the *Learn* links.
    Learn(Link),
    /// Leave the program (the button of the small card).
    Quit,
    Close,
}

/// What the header of the screen says: whom it greets and which version this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WelcomeInfo {
    /// The name after the greeting.
    pub name: String,
    /// The hour on the clock, which decides between good morning, afternoon and night.
    pub hour: u32,
    pub version: String,
    /// Date of the build, `dd/mm/aaaa`.
    pub updated: String,
}

impl WelcomeInfo {
    /// The greeting with the name: "Bom dia, Ana".
    pub fn greeting(&self) -> String {
        tr_args(DayPart::of_hour(self.hour).greeting(), &[&self.name])
    }
}

/// What the window is showing besides the screen itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Hub,
    NewProject,
}

#[derive(Debug, Clone, Default)]
pub struct WelcomeState {
    pub open: bool,
    /// The settings the new project will have, edited with the lists.
    pub draft: ProjectSettings,
    pub layout: StartLayout,
    pub chosen_template: Option<String>,
    /// The small dark screen with the three actions, instead of the whole screen.
    pub compact: bool,
    page: Page,
}

// ---- the look ----------------------------------------------------------------------------------------
//
// Both screens follow the design of the program (`layouts-neoveg.vectorcraft`): `wc-scr-1` is the
// whole screen and `wc-scr-2` the small dark one. The sizes below are in the units of that drawing,
// scaled to the width of the window.

/// The drawing of the whole screen is 1687 x 1532; the window is `FULL_W` points wide.
const FULL_W: f32 = 760.0;
const FULL_SCALE: f32 = FULL_W / 1687.0;
const FULL_H: f32 = 1532.0 * FULL_SCALE;
const HEADER_H: f32 = 379.0 * FULL_SCALE;
const DIVIDER_X: f32 = (893.0 - 114.0) * FULL_SCALE;
const FOOTER_Y: f32 = (2631.0 - 1293.0) * FULL_SCALE;
const HEAD_SIZE: f32 = 54.5 * FULL_SCALE;
const ITEM_SIZE: f32 = 35.0 * FULL_SCALE;
const ROW_H: f32 = 33.0;
const PAD: f32 = 24.0;
/// The drawing of the small screen is 2104 x 1536; the window is `COMPACT_W` points wide.
const COMPACT_W: f32 = 640.0;
const COMPACT_SCALE: f32 = COMPACT_W / 2104.0;
const COMPACT_H: f32 = 1536.0 * COMPACT_SCALE;
/// The most recent projects the whole screen lists.
const RECENT_SHOWN: usize = 6;

const BLACK: Color32 = Color32::BLACK;
const WHITE: Color32 = Color32::WHITE;
const BODY_BG: Color32 = Color32::from_rgb(0xE3, 0xE3, 0xE3);
const INK: Color32 = Color32::from_rgb(0x53, 0x53, 0x53);
const INK_HOVER: Color32 = Color32::from_rgb(0x1B, 0x6F, 0xD6);
const ICON_DARK: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
const ICON_FOLD: Color32 = Color32::from_rgb(0x5E, 0x5E, 0x5E);
const FOLDER_YELLOW: Color32 = Color32::from_rgb(0xF5, 0xA6, 0x23);
const MISSING: Color32 = Color32::from_rgb(200, 90, 90);
const SYNC_BOX: Color32 = Color32::from_rgb(0x64, 0x64, 0x64);

/// A document with a folded corner and the "Nv" mark.
fn paint_file_icon(painter: &Painter, rect: Rect, body: Color32) {
    let fold = rect.width() * 0.32;
    painter.add(egui::Shape::convex_polygon(
        vec![
            rect.left_top(),
            pos2(rect.right() - fold, rect.top()),
            pos2(rect.right(), rect.top() + fold),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
        body,
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(rect.right() - fold, rect.top()),
            pos2(rect.right(), rect.top() + fold),
            pos2(rect.right() - fold, rect.top() + fold),
        ],
        ICON_FOLD,
        Stroke::NONE,
    ));
    painter.text(
        rect.left_top() + vec2(2.0, 3.0),
        Align2::LEFT_TOP,
        "Nv",
        FontId::proportional(rect.height() * 0.3),
        WHITE,
    );
}

/// A folder (`mark` puts the "Nv" mark on it).
fn paint_folder_icon(painter: &Painter, rect: Rect, color: Color32, mark: bool) {
    let tab = Rect::from_min_size(rect.min, vec2(rect.width() * 0.45, rect.height() * 0.35));
    painter.rect_filled(tab, Rounding::same(2.0), color);
    let body = Rect::from_min_max(
        pos2(rect.left(), rect.top() + rect.height() * 0.22),
        rect.max,
    );
    painter.rect_filled(body, Rounding::same(2.0), color);
    if mark {
        painter.text(
            rect.left_top() + vec2(2.0, 1.0),
            Align2::LEFT_TOP,
            "Nv",
            FontId::proportional(rect.height() * 0.3),
            WHITE,
        );
    }
}

/// Two sheets of text, one over the other.
fn paint_pages_icon(painter: &Painter, rect: Rect) {
    let back = Rect::from_min_max(
        rect.min + vec2(rect.width() * 0.2, 0.0),
        rect.max - vec2(0.0, rect.height() * 0.15),
    );
    let front = Rect::from_min_max(
        rect.min + vec2(0.0, rect.height() * 0.15),
        rect.max - vec2(rect.width() * 0.2, 0.0),
    );
    painter.rect_filled(back, Rounding::same(1.0), ICON_DARK);
    painter.rect_filled(front, Rounding::same(1.0), ICON_DARK);
    painter.rect_stroke(front, Rounding::same(1.0), Stroke::new(1.0_f32, BODY_BG));
    for i in 0..3 {
        let y = front.top() + front.height() * (0.3 + 0.22 * i as f32);
        painter.line_segment(
            [pos2(front.left() + 3.0, y), pos2(front.right() - 3.0, y)],
            Stroke::new(1.0_f32, WHITE),
        );
    }
}

/// A gray box with a circled arrow and two arrows going both ways: the settings.
fn paint_sync_icon(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, Rounding::same(2.0), SYNC_BOX);
    let c = pos2(rect.left() + rect.width() * 0.27, rect.center().y);
    let r = rect.height() * 0.36;
    painter.circle_filled(c, r, BLACK);
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(c.x - r * 0.3, c.y - r * 0.5),
            pos2(c.x + r * 0.55, c.y),
            pos2(c.x - r * 0.3, c.y + r * 0.5),
        ],
        WHITE,
        Stroke::NONE,
    ));
    let (x0, x1) = (rect.left() + rect.width() * 0.55, rect.right() - 4.0);
    let stroke = Stroke::new(2.0_f32, BLACK);
    for (y, right) in [
        (rect.center().y - 3.5, true),
        (rect.center().y + 3.5, false),
    ] {
        painter.line_segment([pos2(x0, y), pos2(x1, y)], stroke);
        let (tip, back) = if right {
            (x1, x1 - 4.0)
        } else {
            (x0, x0 + 4.0)
        };
        painter.line_segment([pos2(tip, y), pos2(back, y - 3.0)], stroke);
        painter.line_segment([pos2(tip, y), pos2(back, y + 3.0)], stroke);
    }
}

/// How a row of the screen draws its icon.
#[derive(Clone, Copy)]
enum Icon {
    File,
    FolderYellow,
    FolderMark,
    Pages,
    Sync,
}

impl Icon {
    fn size(self) -> egui::Vec2 {
        match self {
            Icon::Sync => vec2(36.0, 18.0),
            Icon::Pages => vec2(20.0, 22.0),
            Icon::File => vec2(17.0, 22.0),
            Icon::FolderYellow | Icon::FolderMark => vec2(22.0, 18.0),
        }
    }

    fn paint(self, painter: &Painter, rect: Rect) {
        match self {
            Icon::File => paint_file_icon(painter, rect, ICON_DARK),
            Icon::FolderYellow => paint_folder_icon(painter, rect, FOLDER_YELLOW, false),
            Icon::FolderMark => paint_folder_icon(painter, rect, ICON_DARK, true),
            Icon::Pages => paint_pages_icon(painter, rect),
            Icon::Sync => paint_sync_icon(painter, rect),
        }
    }
}

/// One clickable row: the icon in a column of its own, then the text. Hovering colours the text.
fn link_row(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    enabled: bool,
    ink: Color32,
) -> egui::Response {
    const ICON_COLUMN: f32 = 40.0;
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(ITEM_SIZE), ink);
    let width = ICON_COLUMN + 8.0 + galley.size().x;
    let (rect, response) = ui.allocate_exact_size(
        vec2(width, ROW_H),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let color = if enabled && response.hovered() {
        INK_HOVER
    } else {
        ink
    };
    icon.paint(
        ui.painter(),
        Rect::from_center_size(
            pos2(rect.left() + ICON_COLUMN / 2.0, rect.center().y),
            icon.size(),
        ),
    );
    ui.painter().text(
        pos2(rect.left() + ICON_COLUMN + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(ITEM_SIZE),
        color,
    );
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A heading of a column; `clickable` makes it a link (the heading *Configurações do Usuário*
/// opens the settings).
fn heading(ui: &mut egui::Ui, text: &str, clickable: bool) -> egui::Response {
    let response = ui.add(
        egui::Label::new(RichText::new(text).size(HEAD_SIZE).strong().color(INK)).sense(
            if clickable {
                Sense::click()
            } else {
                Sense::hover()
            },
        ),
    );
    if clickable {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

impl WelcomeState {
    /// Shows the screen. `defaults` are the settings new projects start with. With no recent
    /// project to pick from, the small card comes first and the form opens from its first button.
    pub fn open_with(&mut self, defaults: &ProjectSettings, has_recents: bool) {
        self.draft = defaults.clone();
        self.chosen_template = None;
        self.compact = !has_recents;
        self.page = Page::Hub;
        self.open = true;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        recents: &[RecentEntry],
        custom_templates: &[ProjectTemplate],
        show_at_startup: bool,
        info: &WelcomeInfo,
    ) -> Vec<WelcomeAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        let (width, height) = match (self.page, self.compact) {
            (Page::NewProject, _) => (FULL_W, FULL_H),
            (Page::Hub, true) => (COMPACT_W, COMPACT_H),
            (Page::Hub, false) => (FULL_W, FULL_H),
        };
        let frame = egui::Frame::window(&ctx.style()).inner_margin(0.0);
        crate::dialogs::dialog(ctx, tr("Bem-vindo ao Neoveg"))
            .open(&mut open)
            .resizable(false)
            .frame(frame)
            .fixed_size(vec2(width, height))
            .show(ctx, |ui| match (self.page, self.compact) {
                (Page::NewProject, _) => {
                    self.new_project_page(ui, recents.is_empty(), custom_templates, &mut actions);
                }
                (Page::Hub, true) => {
                    self.compact_hub(ui, recents, show_at_startup, info, &mut actions);
                }
                (Page::Hub, false) => {
                    self.full_hub(ui, recents, show_at_startup, info, &mut actions);
                }
            });
        if !open {
            actions.push(WelcomeAction::Close);
        }
        actions
    }

    /// The whole screen, as in `wc-scr-1` of the design.
    fn full_hub(
        &mut self,
        ui: &mut egui::Ui,
        recents: &[RecentEntry],
        show_at_startup: bool,
        info: &WelcomeInfo,
        actions: &mut Vec<WelcomeAction>,
    ) {
        let (rect, _) = ui.allocate_exact_size(vec2(FULL_W, FULL_H), Sense::hover());
        let painter = ui.painter_at(rect);
        // The header: name, version, date of the last update, and the greeting
        let header = Rect::from_min_size(rect.min, vec2(FULL_W, HEADER_H));
        painter.rect_filled(header, Rounding::ZERO, BLACK);
        painter.text(
            header.min + vec2(23.0, 20.0),
            Align2::LEFT_TOP,
            "NEOVEG",
            FontId::proportional(52.0),
            WHITE,
        );
        for (i, line) in [version_text(info), updated_text(info)].iter().enumerate() {
            painter.text(
                header.min + vec2(27.0, 80.0 + 22.0 * i as f32),
                Align2::LEFT_TOP,
                line,
                FontId::proportional(15.5),
                WHITE,
            );
        }
        painter.text(
            pos2(header.right() - 24.0, header.top() + 40.0),
            Align2::RIGHT_TOP,
            info.greeting(),
            FontId::proportional(19.0),
            WHITE,
        );
        // The body, its two columns and the footer
        let body = Rect::from_min_max(pos2(rect.left(), header.bottom()), rect.max);
        painter.rect_filled(body, Rounding::ZERO, BODY_BG);
        let line = Stroke::new(1.5_f32, INK);
        let divider_x = rect.left() + DIVIDER_X;
        painter.line_segment(
            [
                pos2(divider_x, header.bottom() + 20.0),
                pos2(divider_x, rect.top() + FOOTER_Y),
            ],
            line,
        );
        painter.line_segment(
            [
                pos2(rect.left(), rect.top() + FOOTER_Y),
                pos2(rect.right(), rect.top() + FOOTER_Y),
            ],
            line,
        );

        let top = header.bottom() + 18.0;
        let height = FOOTER_Y - HEADER_H - 24.0;
        let left = Rect::from_min_size(
            pos2(rect.left() + PAD, top),
            vec2(DIVIDER_X - PAD * 1.5, height),
        );
        let right = Rect::from_min_size(
            pos2(divider_x + PAD, top),
            vec2(FULL_W - DIVIDER_X - PAD * 2.0, height),
        );
        let mut column = ui.new_child(egui::UiBuilder::new().max_rect(left));
        column.set_clip_rect(left.expand(2.0));
        self.left_column(&mut column, recents, actions);
        let mut column = ui.new_child(egui::UiBuilder::new().max_rect(right));
        column.set_clip_rect(right.expand(2.0));
        self.right_column(&mut column, actions);

        // The footer: the preference that turns the screen off
        let footer = Rect::from_min_max(
            pos2(rect.left() + PAD, rect.top() + FOOTER_Y + 8.0),
            rect.max,
        );
        let mut strip = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(footer)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        show_at_startup_box(&mut strip, show_at_startup, actions);
    }

    fn left_column(
        &mut self,
        ui: &mut egui::Ui,
        recents: &[RecentEntry],
        actions: &mut Vec<WelcomeAction>,
    ) {
        if heading(ui, tr("Configurações do Usuário"), true).clicked() {
            actions.push(WelcomeAction::UserSettings);
        }
        if link_row(
            ui,
            Icon::Sync,
            tr("Configurar presets de resolução"),
            true,
            INK,
        )
        .clicked()
        {
            actions.push(WelcomeAction::ResolutionPresets);
        }
        if link_row(ui, Icon::Sync, tr("Verificar atualizações"), true, INK).clicked() {
            actions.push(WelcomeAction::CheckUpdates);
        }
        ui.add_space(14.0);
        heading(ui, tr("Abrir um Projeto Recente"), false);
        if recents.is_empty() {
            ui.weak(tr(
                "Nenhum projeto recente ainda. Crie um projeto ao lado ou abra um arquivo.",
            ));
        }
        for entry in recents.iter().take(RECENT_SHOWN) {
            let text = if entry.exists {
                entry.name.clone()
            } else {
                tr_args("{} (arquivo não encontrado)", &[&entry.name])
            };
            let ink = if entry.exists { INK } else { MISSING };
            let response = link_row(ui, Icon::File, &text, entry.exists, ink)
                .on_hover_text(format!("{}\n{}", entry.short_folder(), entry.path));
            if response.clicked() {
                actions.push(WelcomeAction::OpenRecent(entry.path.clone()));
            }
            response.context_menu(|ui| {
                if ui.button(tr("Esquecer este projeto")).clicked() {
                    actions.push(WelcomeAction::ForgetRecent(entry.path.clone()));
                    ui.close_menu();
                }
            });
        }
        ui.add_space(6.0);
        if link_row(ui, Icon::FolderYellow, tr("Abrir projeto…"), true, INK).clicked() {
            actions.push(WelcomeAction::OpenFile);
        }
    }

    fn right_column(&mut self, ui: &mut egui::Ui, actions: &mut Vec<WelcomeAction>) {
        heading(ui, tr("Criar novo"), false);
        if link_row(ui, Icon::File, tr("Criar novo projeto…"), true, INK).clicked() {
            self.page = Page::NewProject;
        }
        if link_row(
            ui,
            Icon::FolderMark,
            tr("Abrir pasta de projetos…"),
            true,
            INK,
        )
        .clicked()
        {
            actions.push(WelcomeAction::OpenProjectsFolder);
        }
        ui.add_space(24.0);
        heading(ui, tr("Aprender"), false);
        for (label, link) in [
            ("Como usar >>", Link::HowToUse),
            ("Atualizações >>", Link::Updates),
            ("O que há de novo? >>", Link::WhatsNew),
        ] {
            if link_row(ui, Icon::Pages, tr(label), true, INK)
                .on_hover_text(link.url())
                .clicked()
            {
                actions.push(WelcomeAction::Learn(link));
            }
        }
    }

    /// The small dark screen, as in `wc-scr-2` of the design.
    fn compact_hub(
        &mut self,
        ui: &mut egui::Ui,
        recents: &[RecentEntry],
        show_at_startup: bool,
        info: &WelcomeInfo,
        actions: &mut Vec<WelcomeAction>,
    ) {
        let (rect, _) = ui.allocate_exact_size(vec2(COMPACT_W, COMPACT_H), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, Rounding::ZERO, BLACK);
        // The header: the mark and the name
        let mark = Rect::from_min_size(
            pos2(rect.center().x - 80.0, rect.top() + 12.0),
            vec2(28.0, 28.0),
        );
        crate::splash::paint_gradient(&painter, mark, 6.0, 1.0);
        painter.text(
            mark.center(),
            Align2::CENTER_CENTER,
            "Nv",
            FontId::proportional(14.0),
            WHITE,
        );
        painter.text(
            pos2(mark.right() + 10.0, mark.center().y),
            Align2::LEFT_CENTER,
            "Neoveg",
            FontId::proportional(24.0),
            WHITE,
        );
        // The actions, one under the other with the icon over the name
        let column_w = 265.0 * COMPACT_SCALE;
        let mut y = rect.top() + 150.0 * COMPACT_SCALE;
        for (icon, label, what) in [
            (Icon::File, "Novo Projeto", Card::New),
            (Icon::FolderMark, "Abrir Projeto", Card::Open),
            (Icon::Pages, "Como Usar?", Card::Help),
        ] {
            let cell = Rect::from_min_size(pos2(rect.left(), y), vec2(column_w, 62.0));
            let response = ui
                .interact(cell, egui::Id::new(("welcome_card", label)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            icon.paint(
                &painter,
                Rect::from_center_size(pos2(cell.center().x, cell.top() + 16.0), icon.size()),
            );
            painter.text(
                pos2(cell.center().x, cell.top() + 38.0),
                Align2::CENTER_TOP,
                tr(label),
                FontId::proportional(11.0),
                if response.hovered() {
                    Color32::from_rgb(0x8A, 0xB8, 0xFF)
                } else {
                    WHITE
                },
            );
            if response.clicked() {
                match what {
                    Card::New => self.page = Page::NewProject,
                    Card::Open => actions.push(WelcomeAction::OpenFile),
                    Card::Help => actions.push(WelcomeAction::Learn(Link::HowToUse)),
                }
            }
            y += 66.0;
        }
        // The blue panel: the greeting, the version and the recent projects
        let panel = Rect::from_min_max(
            pos2(rect.left() + column_w + 12.0, rect.top() + 48.0),
            pos2(
                rect.right() - 12.0,
                rect.top() + 265.0 * COMPACT_SCALE * 3.4,
            ),
        );
        crate::splash::paint_gradient(&painter, panel, 10.0, 1.0);
        let mut list = ui.new_child(egui::UiBuilder::new().max_rect(panel.shrink(12.0)));
        list.set_clip_rect(panel);
        list.label(
            RichText::new(info.greeting())
                .strong()
                .size(16.0)
                .color(WHITE),
        );
        list.label(RichText::new(version_text(info)).color(WHITE));
        list.label(RichText::new(updated_text(info)).color(WHITE));
        list.add_space(6.0);
        if recents.is_empty() {
            list.label(RichText::new(tr("Nenhum projeto recente ainda.")).color(WHITE));
        }
        for entry in recents.iter().take(RECENT_SHOWN) {
            let text = if entry.exists {
                entry.name.clone()
            } else {
                tr_args("{} (arquivo não encontrado)", &[&entry.name])
            };
            let response = list
                .add_enabled(
                    entry.exists,
                    egui::Button::new(RichText::new(text).color(WHITE)).frame(false),
                )
                .on_hover_text(&entry.path);
            if response.clicked() {
                actions.push(WelcomeAction::OpenRecent(entry.path.clone()));
            }
        }
        // The footer: the preference that turns the screen off, and Sair
        let footer = Rect::from_min_max(
            pos2(rect.left() + 12.0, rect.bottom() - 34.0),
            pos2(rect.right() - 12.0, rect.bottom() - 6.0),
        );
        let mut strip = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(footer)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        strip.visuals_mut().override_text_color = Some(WHITE);
        show_at_startup_box(&mut strip, show_at_startup, actions);
        strip.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(tr("Sair")).clicked() {
                actions.push(WelcomeAction::Quit);
            }
        });
    }

    /// The form of a new project, in the same window.
    fn new_project_page(
        &mut self,
        ui: &mut egui::Ui,
        no_recents: bool,
        custom_templates: &[ProjectTemplate],
        actions: &mut Vec<WelcomeAction>,
    ) {
        egui::Frame::none()
            .inner_margin(egui::Margin::same(PAD))
            .show(ui, |ui| {
                ui.set_min_height(FULL_H - 2.0 * PAD);
                ui.horizontal(|ui| {
                    if ui.button(format!("← {}", tr("Voltar"))).clicked() {
                        self.page = Page::Hub;
                    }
                    ui.heading(tr("Novo projeto"));
                });
                ui.separator();
                self.new_project_form(ui, no_recents, custom_templates, actions);
            });
    }

    fn new_project_form(
        &mut self,
        ui: &mut egui::Ui,
        emphasise_layouts: bool,
        custom_templates: &[ProjectTemplate],
        actions: &mut Vec<WelcomeAction>,
    ) {
        let builtin = builtin_templates();
        ui.horizontal(|ui| {
            ui.label(tr("Modelo:"));
            egui::ComboBox::from_id_salt("welcome_template")
                .selected_text(
                    self.chosen_template
                        .clone()
                        .unwrap_or_else(|| tr("(escolha)").into()),
                )
                .width(230.0)
                .show_ui(ui, |ui| {
                    for t in builtin.iter().chain(custom_templates) {
                        if ui
                            .selectable_label(
                                self.chosen_template.as_deref() == Some(t.name.as_str()),
                                &t.name,
                            )
                            .clicked()
                        {
                            self.draft = t.settings.clone();
                            self.chosen_template = Some(t.name.clone());
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(tr("Resolução:"));
            let selected = resolution_named(self.draft.width, self.draft.height).map_or_else(
                || format!("{}x{}", self.draft.width, self.draft.height),
                |r| tr_text(r.name).into_owned(),
            );
            egui::ComboBox::from_id_salt("welcome_resolution")
                .selected_text(selected)
                .width(230.0)
                .show_ui(ui, |ui| {
                    let mut group = "";
                    for r in RESOLUTIONS {
                        if r.group != group {
                            group = r.group;
                            ui.weak(tr_text(group));
                        }
                        let on = (r.width, r.height) == (self.draft.width, self.draft.height);
                        if ui
                            .selectable_label(
                                on,
                                format!("{} — {}x{}", tr_text(r.name), r.width, r.height),
                            )
                            .clicked()
                        {
                            self.draft.width = r.width;
                            self.draft.height = r.height;
                            self.chosen_template = None;
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(tr("Taxa de quadros:"));
            let selected = frame_rate_of(self.draft.fps).map_or_else(
                || format!("{:.3}", self.draft.fps),
                |r| tr_text(r.label).into_owned(),
            );
            egui::ComboBox::from_id_salt("welcome_fps")
                .selected_text(selected)
                .width(230.0)
                .show_ui(ui, |ui| {
                    for rate in FRAME_RATES {
                        if ui
                            .selectable_label(
                                (rate.fps - self.draft.fps).abs() < 0.005,
                                tr_text(rate.label),
                            )
                            .clicked()
                        {
                            self.draft.fps = rate.fps;
                            self.chosen_template = None;
                        }
                    }
                });
        });
        ui.add_space(6.0);
        let heading = if emphasise_layouts {
            tr("Escolha o layout das janelas")
        } else {
            tr("Layout das janelas")
        };
        ui.label(RichText::new(heading).strong());
        for layout in StartLayout::ALL {
            ui.radio_value(&mut self.layout, layout, tr(layout.label()))
                .on_hover_text(tr(layout.description()));
        }
        ui.add_space(8.0);
        if ui.button(tr("Criar projeto")).clicked() {
            actions.push(WelcomeAction::NewProject {
                settings: Box::new(self.draft.clone()),
                layout: self.layout,
            });
        }
    }
}

/// The line with the version in the header.
fn version_text(info: &WelcomeInfo) -> String {
    tr_args("Versão {}", &[&info.version])
}

/// The line with the date of the last update in the header.
fn updated_text(info: &WelcomeInfo) -> String {
    tr_args("Atualizado em {}", &[&info.updated])
}

/// The checkbox that turns the screen off.
fn show_at_startup_box(ui: &mut egui::Ui, show_at_startup: bool, actions: &mut Vec<WelcomeAction>) {
    let mut show = show_at_startup;
    if ui
        .checkbox(&mut show, tr("Mostrar esta janela ao iniciar o programa"))
        .changed()
    {
        actions.push(WelcomeAction::ShowAtStartup(show));
    }
}

/// What a button of the small screen does.
#[derive(Clone, Copy)]
enum Card {
    New,
    Open,
    Help,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_entries_split_the_path_and_flag_missing_files() {
        let paths = vec![
            "C:/obras/filme.neoveg".to_owned(),
            "C:/obras/sumiu.neoveg".to_owned(),
        ];
        let entries = recent_entries(&paths, &|p| p.ends_with("filme.neoveg"));
        assert_eq!(entries[0].name, "filme");
        assert_eq!(entries[0].folder, "C:/obras");
        assert!(entries[0].exists && !entries[1].exists);
        assert!(recent_entries(&[], &|_| true).is_empty());
    }

    #[test]
    fn the_short_folder_keeps_the_last_two_folders_of_either_style_of_path() {
        let entries = recent_entries(
            &[
                "C:\\Users\\ana\\Videos\\Filmes\\a.neoveg".to_owned(),
                "/home/ana/obras/a.neoveg".to_owned(),
                "a.neoveg".to_owned(),
            ],
            &|_| true,
        );
        assert_eq!(entries[0].short_folder(), "Videos/Filmes");
        assert_eq!(entries[1].short_folder(), "ana/obras");
        assert_eq!(entries[2].short_folder(), "");
    }

    #[test]
    fn opening_starts_from_the_given_defaults() {
        let mut state = WelcomeState::default();
        let defaults = ProjectSettings {
            width: 1280,
            height: 720,
            ..ProjectSettings::default()
        };
        state.chosen_template = Some("antigo".into());
        state.page = Page::NewProject;
        state.open_with(&defaults, true);
        assert!(state.open);
        assert_eq!(state.page, Page::Hub, "it opens on the first page");
        assert!(!state.compact, "recent projects: the whole screen");
        assert_eq!((state.draft.width, state.draft.height), (1280, 720));
        assert!(state.chosen_template.is_none());
    }

    #[test]
    fn with_no_recent_project_the_small_card_comes_first() {
        let mut state = WelcomeState::default();
        state.open_with(&ProjectSettings::default(), false);
        assert!(state.compact);
        state.open_with(&ProjectSettings::default(), true);
        assert!(!state.compact);
    }

    #[test]
    fn the_greeting_follows_the_hour_and_names_the_user() {
        let at = |hour| WelcomeInfo {
            name: "Ana".into(),
            hour,
            version: "2.1.0".into(),
            updated: "03/11/2026".into(),
        };
        assert_eq!(at(8).greeting(), "Bom dia, Ana");
        assert_eq!(at(14).greeting(), "Boa tarde, Ana");
        assert_eq!(at(21).greeting(), "Boa noite, Ana");
        assert_eq!(at(3).greeting(), "Boa noite, Ana");
    }

    #[test]
    fn the_projects_folder_is_the_one_of_the_newest_project_that_still_exists() {
        let here = std::env::temp_dir();
        let recent = vec![
            "/pasta/que/sumiu/a.neoveg".to_owned(),
            here.join("b.neoveg").to_string_lossy().into_owned(),
        ];
        assert_eq!(projects_folder(&recent), here);
        // Nothing left: a folder that exists all the same
        assert!(projects_folder(&[]).is_dir() || projects_folder(&[]).as_os_str() == ".");
    }

    #[test]
    fn both_layouts_are_described() {
        for layout in StartLayout::ALL {
            assert!(!layout.label().is_empty() && !layout.description().is_empty());
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_design_proportions_are_kept() {
        // wc-scr-1 is 1687 x 1532 and wc-scr-2 is 2104 x 1536
        assert!((FULL_W / FULL_H - 1687.0 / 1532.0).abs() < 1e-3);
        assert!((COMPACT_W / COMPACT_H - 2104.0 / 1536.0).abs() < 1e-3);
        // The header is a quarter of the whole screen and the footer starts under the columns
        assert!(HEADER_H > FULL_H * 0.2 && HEADER_H < FULL_H * 0.3);
        assert!(FOOTER_Y > HEADER_H && FOOTER_Y < FULL_H);
        assert!(DIVIDER_X > FULL_W * 0.3 && DIVIDER_X < FULL_W * 0.6);
    }

    #[test]
    fn the_header_lines_carry_the_version_and_the_date() {
        let info = WelcomeInfo {
            name: "Ana".into(),
            hour: 9,
            version: "2.1.0".into(),
            updated: "03/11/2026".into(),
        };
        assert!(version_text(&info).contains("2.1.0"));
        assert!(updated_text(&info).contains("03/11/2026"));
    }
}
