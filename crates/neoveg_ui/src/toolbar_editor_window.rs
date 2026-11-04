//! *Personalizar barra de ferramentas*: the buttons of the main toolbar in their order, with the
//! buttons that are not in it, so the user can add, remove and reorder them or go back to the
//! standard set. It only reports [`ToolbarAction`]s; `AppState` keeps the preferences.

use eframe::egui;
use neoveg_common::i18n::tr;

use crate::icons::SvgIcon;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolbarAction {
    Add(String),
    Remove(usize),
    Move { index: usize, up: bool },
    Reset,
}

/// What the window needs to know about the items (ids of the toolbar).
pub struct ToolbarView<'a> {
    /// The items of the toolbar, in order.
    pub items: &'a [String],
    /// The buttons that can still be added.
    pub available: &'a [String],
    /// The name shown for an item.
    pub label: &'a dyn Fn(&str) -> String,
    /// The icon of an item, when it has one.
    pub icon: &'a dyn Fn(&str) -> Option<SvgIcon>,
}

fn icon_and_name(ui: &mut egui::Ui, view: &ToolbarView<'_>, id: &str) {
    match (view.icon)(id) {
        Some(icon) => {
            ui.add(icon.sized_image(16.0));
        }
        None => {
            ui.add_space(20.0);
        }
    }
    ui.label((view.label)(id));
}

/// Draws the window while `open`; returns what the user did.
pub fn show(ctx: &egui::Context, open: &mut bool, view: &ToolbarView<'_>) -> Vec<ToolbarAction> {
    let mut actions = Vec::new();
    if !*open {
        return actions;
    }
    let mut window_open = true;
    crate::dialogs::tool_window(ctx, tr("Personalizar barra de ferramentas"))
        .open(&mut window_open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                ui.strong(tr("Botões na barra"));
                egui::ScrollArea::vertical()
                    .id_salt("toolbar_items")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (index, id) in view.items.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(index > 0, |ui| {
                                    if ui.small_button("▲").clicked() {
                                        actions.push(ToolbarAction::Move { index, up: true });
                                    }
                                });
                                ui.add_enabled_ui(index + 1 < view.items.len(), |ui| {
                                    if ui.small_button("▼").clicked() {
                                        actions.push(ToolbarAction::Move { index, up: false });
                                    }
                                });
                                if ui
                                    .small_button("✖")
                                    .on_hover_text(tr("Tirar da barra"))
                                    .clicked()
                                {
                                    actions.push(ToolbarAction::Remove(index));
                                }
                                if id == neoveg_common::toolbar::SEPARATOR {
                                    ui.weak(tr("— separador —"));
                                } else {
                                    icon_and_name(ui, view, id);
                                }
                            });
                        }
                    });
                let ui = &mut cols[1];
                ui.strong(tr("Botões disponíveis"));
                egui::ScrollArea::vertical()
                    .id_salt("toolbar_available")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if ui.small_button(format!("＋ {}", tr("separador"))).clicked() {
                            actions.push(ToolbarAction::Add(
                                neoveg_common::toolbar::SEPARATOR.to_owned(),
                            ));
                        }
                        for id in view.available {
                            ui.horizontal(|ui| {
                                if ui.small_button("＋").clicked() {
                                    actions.push(ToolbarAction::Add(id.clone()));
                                }
                                icon_and_name(ui, view, id);
                            });
                        }
                    });
            });
            ui.separator();
            if ui
                .button(tr("Restaurar a barra padrão"))
                .on_hover_text(tr("Volta aos botões e à ordem de fábrica"))
                .clicked()
            {
                actions.push(ToolbarAction::Reset);
            }
        });
    *open = window_open;
    actions
}
