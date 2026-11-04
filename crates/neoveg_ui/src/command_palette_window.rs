//! The command palette: a box to type in and the commands that answer it, each with its shortcut.
//! `↑`/`↓` choose, `Enter` runs the chosen command, `Esc` closes. It only draws and reports the
//! row that was chosen; the rows and the running of the command are in `app.rs`.

use eframe::egui;
use neoveg_common::command_palette;
use neoveg_common::i18n::tr;

/// Most rows shown.
const LIMIT: usize = 60;

/// One command of the palette.
pub struct PaletteRow {
    pub label: String,
    pub category: String,
    pub shortcut: String,
    /// The command can run now (a disabled one is shown dimmed and cannot be chosen).
    pub enabled: bool,
    /// What the search looks at: the names in the languages of the interface and the id.
    pub haystack: String,
}

#[derive(Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    selected: usize,
    just_opened: bool,
}

impl PaletteState {
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.just_opened = true;
    }

    /// The rows that answer the query, as indices of `rows`.
    pub fn matches(&self, rows: &[PaletteRow]) -> Vec<usize> {
        let haystacks: Vec<String> = rows.iter().map(|r| r.haystack.clone()).collect();
        command_palette::search(&self.query, &haystacks, LIMIT)
    }

    /// Draws the palette; returns the index in `rows` of the command the user chose.
    pub fn show(&mut self, ctx: &egui::Context, rows: &[PaletteRow]) -> Option<usize> {
        if !self.open {
            return None;
        }
        let mut chosen = None;
        let mut close = false;
        let found = self.matches(rows);
        self.selected = self.selected.min(found.len().saturating_sub(1));
        // The keys act before the box sees them, so the arrows do not move its caret
        let (down, up, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if down && self.selected + 1 < found.len() {
            self.selected += 1;
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if escape {
            close = true;
        }
        if enter {
            if let Some(&row) = found.get(self.selected) {
                if rows[row].enabled {
                    chosen = Some(row);
                }
            }
        }
        egui::Window::new(tr("Paleta de comandos"))
            .id(egui::Id::new("command_palette"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 70.0])
            .fixed_size([560.0, 0.0])
            .show(ctx, |ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr("Digite o nome de um comando…"))
                        .desired_width(f32::INFINITY),
                );
                if self.just_opened {
                    edit.request_focus();
                    self.just_opened = false;
                }
                if edit.changed() {
                    self.selected = 0;
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if found.is_empty() {
                            ui.weak(tr("Nenhum comando encontrado"));
                        }
                        for (slot, &index) in found.iter().enumerate() {
                            let row = &rows[index];
                            let selected = slot == self.selected;
                            let response = ui
                                .add_enabled_ui(row.enabled, |ui| {
                                    ui.horizontal(|ui| {
                                        let label = ui.selectable_label(selected, &row.label);
                                        ui.weak(&row.category);
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.monospace(&row.shortcut);
                                            },
                                        );
                                        label
                                    })
                                    .inner
                                })
                                .inner;
                            if selected && (down || up) {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                chosen = Some(index);
                            }
                        }
                    });
            });
        if chosen.is_some() || close {
            self.open = false;
        }
        chosen
    }
}
