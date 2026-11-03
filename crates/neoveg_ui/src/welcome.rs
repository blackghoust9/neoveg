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

use eframe::egui::{self, Color32, RichText};
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

#[derive(Debug, Clone, Default)]
pub struct WelcomeState {
    pub open: bool,
    /// The settings the new project will have, edited with the lists.
    pub draft: ProjectSettings,
    pub layout: StartLayout,
    pub chosen_template: Option<String>,
    /// The small card with four buttons, instead of the whole screen.
    pub compact: bool,
}

impl WelcomeState {
    /// Shows the screen. `defaults` are the settings new projects start with. With no recent
    /// project to pick from, the small card comes first and the form opens from its first button.
    pub fn open_with(&mut self, defaults: &ProjectSettings, has_recents: bool) {
        self.draft = defaults.clone();
        self.chosen_template = None;
        self.compact = !has_recents;
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
        crate::dialogs::dialog(ctx, tr("Bem-vindo ao Neoveg"))
            .open(&mut open)
            .resizable(false)
            .default_width(720.0)
            .show(ctx, |ui| {
                ui.heading(info.greeting());
                ui.weak(tr_args(
                    "Versão {} · última atualização {}",
                    &[&info.version, &info.updated],
                ));
                ui.add_space(6.0);
                if self.compact {
                    self.compact_card(ui, &mut actions);
                } else {
                    ui.columns(2, |columns| {
                        self.recent_column(&mut columns[0], recents, &mut actions);
                        self.new_project_column(
                            &mut columns[1],
                            recents.is_empty(),
                            custom_templates,
                            &mut actions,
                        );
                    });
                    ui.separator();
                    tools_and_links(ui, &mut actions);
                }
                ui.separator();
                let mut show = show_at_startup;
                if ui
                    .checkbox(&mut show, tr("Mostrar esta janela ao iniciar o programa"))
                    .changed()
                {
                    actions.push(WelcomeAction::ShowAtStartup(show));
                }
            });
        if !open {
            actions.push(WelcomeAction::Close);
        }
        actions
    }

    /// The small card: four buttons.
    fn compact_card(&mut self, ui: &mut egui::Ui, actions: &mut Vec<WelcomeAction>) {
        let wide = egui::vec2(ui.available_width().min(300.0), 30.0);
        for (label, what) in [
            ("Novo Projeto", Card::New),
            ("Abrir Projeto", Card::Open),
            ("Como Usar?", Card::Help),
            ("Sair", Card::Quit),
        ] {
            if ui.add_sized(wide, egui::Button::new(tr(label))).clicked() {
                match what {
                    Card::New => self.compact = false,
                    Card::Open => actions.push(WelcomeAction::OpenFile),
                    Card::Help => actions.push(WelcomeAction::Learn(Link::HowToUse)),
                    Card::Quit => actions.push(WelcomeAction::Quit),
                }
            }
        }
    }

    fn recent_column(
        &mut self,
        ui: &mut egui::Ui,
        recents: &[RecentEntry],
        actions: &mut Vec<WelcomeAction>,
    ) {
        ui.heading(tr("Projetos recentes"));
        ui.add_space(4.0);
        if recents.is_empty() {
            ui.weak(tr(
                "Nenhum projeto recente ainda. Crie um projeto ao lado ou abra um arquivo.",
            ));
        }
        egui::ScrollArea::vertical()
            .id_salt("welcome_recents")
            .max_height(260.0)
            .show(ui, |ui| {
                for entry in recents {
                    ui.horizontal(|ui| {
                        let title = if entry.exists {
                            RichText::new(&entry.name).strong()
                        } else {
                            RichText::new(tr_args("{} (arquivo não encontrado)", &[&entry.name]))
                                .color(Color32::from_rgb(200, 120, 120))
                        };
                        let response = ui
                            .add_enabled(entry.exists, egui::Button::new(title).frame(false))
                            .on_hover_text(&entry.path);
                        if response.clicked() {
                            actions.push(WelcomeAction::OpenRecent(entry.path.clone()));
                        }
                        if ui
                            .small_button("x")
                            .on_hover_text(tr("Esquecer este projeto"))
                            .clicked()
                        {
                            actions.push(WelcomeAction::ForgetRecent(entry.path.clone()));
                        }
                    });
                    ui.weak(entry.short_folder());
                    ui.add_space(2.0);
                }
            });
        ui.add_space(6.0);
        if ui.button(tr("Abrir projeto…")).clicked() {
            actions.push(WelcomeAction::OpenFile);
        }
    }

    fn new_project_column(
        &mut self,
        ui: &mut egui::Ui,
        emphasise_layouts: bool,
        custom_templates: &[ProjectTemplate],
        actions: &mut Vec<WelcomeAction>,
    ) {
        ui.heading(tr("Novo projeto"));
        ui.add_space(4.0);
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

/// What a button of the small card does.
#[derive(Clone, Copy)]
enum Card {
    New,
    Open,
    Help,
    Quit,
}

/// The row of tools and the *Learn* links under the two columns.
fn tools_and_links(ui: &mut egui::Ui, actions: &mut Vec<WelcomeAction>) {
    ui.horizontal_wrapped(|ui| {
        for (label, action) in [
            (
                "Abrir pasta de projetos…",
                WelcomeAction::OpenProjectsFolder,
            ),
            ("Configurações do Usuário", WelcomeAction::UserSettings),
            (
                "Configurar presets de resolução",
                WelcomeAction::ResolutionPresets,
            ),
            ("Verificar atualizações", WelcomeAction::CheckUpdates),
        ] {
            if ui.button(tr(label)).clicked() {
                actions.push(action);
            }
        }
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(tr("Aprender")).strong());
        for (label, link) in [
            ("Como usar >>", Link::HowToUse),
            ("Atualizações >>", Link::Updates),
            ("O que há de novo? >>", Link::WhatsNew),
        ] {
            if ui.link(tr(label)).on_hover_text(link.url()).clicked() {
                actions.push(WelcomeAction::Learn(link));
            }
        }
    });
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
        state.open_with(&defaults, true);
        assert!(state.open);
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
}
