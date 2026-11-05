//! Ferramentas > Gerenciador de plug-ins: the VST3 plug-ins the editor knows, what each one
//! reports (channels, latency, parameters), which ones failed and why, which ones the user turned
//! off, and the choice of running them in a process of their own.
//!
//! The scans and the failures come from `neoveg_audio::vst3_isolated`: every scan runs in a child
//! process, so a plug-in that crashes or hangs while it is read shows up here as a failure with a
//! reason, and the editor goes on. The window returns [`ManagerAction`]s for what is kept in the
//! preferences; the application carries them out. What can be decided without a window (the state
//! of a plug-in, the latency in milliseconds) is in plain functions with tests.

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui::{self, Color32, RichText};
use neoveg_audio::vst3_host::{self, Vst3PluginInfo};
use neoveg_audio::vst3_isolated;
use neoveg_common::i18n::{tr, tr_args};

/// What the user changed in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerAction {
    /// Run the plug-ins in a child process (or not).
    SetIsolation(bool),
    /// Turn the module at this path off (`true`) or on again (`false`).
    SetBlocked(String, bool),
}

/// Where a plug-in stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Ready,
    /// The user turned its module off.
    Off,
    /// It failed, with the reason.
    Failed(String),
}

/// The state of `plugin`: a failure is told before the rest, because it is what the user has to see.
pub fn state_of(
    plugin: &Vst3PluginInfo,
    blocked: &[String],
    failures: &[(String, String)],
) -> PluginState {
    let module = vst3_host::module_key(&plugin.path);
    if let Some((_, reason)) = failures
        .iter()
        .find(|(key, _)| *key == plugin.kind || *key == module)
    {
        return PluginState::Failed(reason.clone());
    }
    if blocked
        .iter()
        .any(|b| std::path::Path::new(b) == plugin.path)
    {
        return PluginState::Off;
    }
    PluginState::Ready
}

/// The latency of `samples` at `rate` Hz as a text: `"0"`, `"64 (1.3 ms)"`.
pub fn latency_text(samples: u32, rate: f64) -> String {
    if samples == 0 {
        "0".into()
    } else {
        format!(
            "{samples} ({:.1} ms)",
            f64::from(samples) * 1000.0 / rate.max(1.0)
        )
    }
}

/// The plug-ins in the order of the list: by name, then by vendor.
pub fn sorted(mut plugins: Vec<Vst3PluginInfo>) -> Vec<Vst3PluginInfo> {
    plugins.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.vendor.to_lowercase().cmp(&b.vendor.to_lowercase()))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    plugins
}

/// The modules that failed to scan (what is not the failure of a plug-in already listed): the
/// kinds that are only a module path, with the reason.
pub fn failed_modules(failures: &[(String, String)]) -> Vec<(String, String)> {
    failures
        .iter()
        .filter(|(key, _)| vst3_host::parse_vst3_kind(key).is_none())
        .cloned()
        .collect()
}

#[derive(Default)]
pub struct PluginManager {
    pub open: bool,
    scan: Option<mpsc::Receiver<Result<usize, String>>>,
    status: String,
}

impl PluginManager {
    /// Scans the folders of the system in the background (in child processes).
    fn start_system_scan(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.scan = Some(rx);
        self.status = tr("Procurando plug-ins VST3 nas pastas do sistema…").to_owned();
        let _ = std::thread::Builder::new()
            .name("neoveg-plugin-manager-scan".into())
            .spawn(move || {
                let _ = tx.send(Ok(vst3_host::scan_system_plugins().len()));
            });
    }

    /// Scans one module the user chose.
    fn start_file_scan(&mut self, path: PathBuf) {
        if self.scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.scan = Some(rx);
        self.status = tr_args("Lendo {}…", &[&path.display()]);
        let _ = std::thread::Builder::new()
            .name("neoveg-plugin-manager-file".into())
            .spawn(move || {
                let _ = tx.send(vst3_host::scan_vst3_file(&path).map(|found| found.len()));
            });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.scan else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(count)) => {
                self.scan = None;
                self.status = tr_args(
                    "{} plug-ins na lista.",
                    &[&vst3_host::cached_plugins().len().max(count).to_string()],
                );
            }
            Ok(Err(reason)) => {
                self.scan = None;
                self.status = tr_args("A leitura falhou: {}", &[&reason]);
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200))
            }
            Err(mpsc::TryRecvError::Disconnected) => self.scan = None,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        isolated: bool,
        blocked: &[String],
    ) -> Vec<ManagerAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        self.poll(ctx);
        let mut open = self.open;
        crate::dialogs::dialog(ctx, tr("Gerenciador de plug-ins"))
            .open(&mut open)
            .default_size([760.0, 440.0])
            .show(ctx, |ui| {
                let mut run_isolated = isolated;
                if ui
                    .checkbox(
                        &mut run_isolated,
                        tr("Rodar os plug-ins VST3 em processo isolado"),
                    )
                    .on_hover_text(tr("Cada plug-in roda e é lido num processo próprio: se ele cair ou travar, o editor continua e o plug-in aparece aqui como falho. Vale para os plug-ins abertos depois da mudança."))
                    .changed()
                {
                    actions.push(ManagerAction::SetIsolation(run_isolated));
                }
                if isolated && vst3_isolated::host_exe().is_none() {
                    ui.colored_label(
                        Color32::from_rgb(231, 76, 60),
                        tr("O programa neoveg_vst3_host não está ao lado do editor: os plug-ins rodam no processo do editor."),
                    );
                }
                ui.horizontal(|ui| {
                    let scanning = self.scan.is_some();
                    ui.add_enabled_ui(!scanning, |ui| {
                        if ui.button(tr("Procurar plug-ins")).clicked() {
                            self.start_system_scan();
                        }
                        if ui.button(tr("Ler um arquivo .vst3…")).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Plug-in VST3", &["vst3"])
                                .pick_file()
                            {
                                self.start_file_scan(path);
                            }
                        }
                    });
                    if ui.button(tr("Esquecer as falhas")).clicked() {
                        for (key, _) in vst3_isolated::crashes() {
                            vst3_isolated::forget_crash(&key);
                        }
                    }
                    if scanning {
                        ui.spinner();
                    }
                });
                if !self.status.is_empty() {
                    ui.weak(&self.status);
                }
                ui.separator();

                let failures = vst3_isolated::crashes();
                let plugins = sorted(vst3_host::cached_plugins());
                if plugins.is_empty() {
                    ui.weak(tr("Nenhum plug-in VST3 na lista. Use Procurar plug-ins."));
                }
                egui::ScrollArea::both().auto_shrink([false, true]).show(ui, |ui| {
                    egui::Grid::new("plugin_manager_grid")
                        .striped(true)
                        .num_columns(8)
                        .show(ui, |ui| {
                            for head in ["Usar", "Nome", "Fabricante", "Versão", "Canais", "Latência (amostras)", "Parâmetros", "Estado"] {
                                ui.label(RichText::new(tr(head)).strong());
                            }
                            ui.end_row();
                            for plugin in &plugins {
                                let state = state_of(plugin, blocked, &failures);
                                let path = plugin.path.to_string_lossy().into_owned();
                                let mut on = state != PluginState::Off;
                                if ui
                                    .checkbox(&mut on, "")
                                    .on_hover_text(tr("Desligado, o plug-in não aparece nas listas de efeitos e não é aberto."))
                                    .changed()
                                {
                                    actions.push(ManagerAction::SetBlocked(path.clone(), !on));
                                }
                                ui.label(&plugin.name).on_hover_text(&path);
                                ui.label(&plugin.vendor);
                                ui.label(&plugin.version);
                                ui.label(format!("{} → {}", plugin.audio_inputs, plugin.audio_outputs));
                                ui.label(latency_text(plugin.latency, 48_000.0));
                                ui.label(plugin.params.len().to_string());
                                match &state {
                                    PluginState::Ready => ui.label(tr("Pronto")),
                                    PluginState::Off => ui.weak(tr("Desligado")),
                                    PluginState::Failed(reason) => ui
                                        .colored_label(
                                            Color32::from_rgb(231, 76, 60),
                                            tr_args("Falhou: {}", &[reason]),
                                        )
                                        .on_hover_text(reason),
                                };
                                ui.end_row();
                            }
                        });
                    let modules = failed_modules(&failures);
                    if !modules.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(tr("Arquivos que não puderam ser lidos")).strong());
                        for (key, reason) in modules {
                            let path = key.strip_prefix(vst3_host::VST3_KIND_PREFIX).unwrap_or(&key);
                            ui.horizontal(|ui| {
                                ui.colored_label(Color32::from_rgb(231, 76, 60), path);
                                ui.weak(&reason);
                                if ui.small_button(tr("Tentar de novo")).clicked() {
                                    vst3_isolated::forget_crash(&key);
                                    self.start_file_scan(PathBuf::from(path));
                                }
                            });
                        }
                    }
                });
            });
        self.open = open;
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(name: &str, vendor: &str, path: &str, class: &str) -> Vst3PluginInfo {
        Vst3PluginInfo {
            kind: format!("vst3:{path}::{class:0>32}"),
            class_id: format!("{class:0>32}"),
            name: name.into(),
            vendor: vendor.into(),
            version: "1.0".into(),
            path: PathBuf::from(path),
            audio_inputs: 2,
            audio_outputs: 2,
            params: Vec::new(),
            latency: 0,
        }
    }

    #[test]
    fn a_failure_is_told_before_a_switch_and_a_switch_before_ready() {
        let p = plugin("Gain", "Acme", "C:/VST3/Gain.vst3", "1");
        assert_eq!(state_of(&p, &[], &[]), PluginState::Ready);
        let off = vec!["C:/VST3/Gain.vst3".to_string()];
        assert_eq!(state_of(&p, &off, &[]), PluginState::Off);
        // The failure of the plug-in itself and the one of its module both count
        let by_kind = vec![(p.kind.clone(), "caiu".to_string())];
        assert_eq!(
            state_of(&p, &off, &by_kind),
            PluginState::Failed("caiu".into())
        );
        let by_module = vec![(vst3_host::module_key(&p.path), "travou".to_string())];
        assert_eq!(
            state_of(&p, &[], &by_module),
            PluginState::Failed("travou".into())
        );
        // Another plug-in's failure is not this one's
        let other = vec![("vst3:C:/VST3/Other.vst3".to_string(), "x".to_string())];
        assert_eq!(state_of(&p, &[], &other), PluginState::Ready);
    }

    #[test]
    fn the_latency_shows_samples_and_milliseconds() {
        assert_eq!(latency_text(0, 48_000.0), "0");
        assert_eq!(latency_text(480, 48_000.0), "480 (10.0 ms)");
        assert_eq!(latency_text(64, 48_000.0), "64 (1.3 ms)");
    }

    #[test]
    fn the_list_is_in_the_order_of_names_without_regard_to_case() {
        let list = sorted(vec![
            plugin("delay", "B", "b.vst3", "3"),
            plugin("Chorus", "A", "a.vst3", "2"),
            plugin("Chorus", "0 first", "z.vst3", "1"),
        ]);
        let order: Vec<(&str, &str)> = list
            .iter()
            .map(|p| (p.name.as_str(), p.vendor.as_str()))
            .collect();
        assert_eq!(
            order,
            [("Chorus", "0 first"), ("Chorus", "A"), ("delay", "B")]
        );
    }

    #[test]
    fn only_the_failures_of_modules_are_listed_apart() {
        let p = plugin("Gain", "Acme", "g.vst3", "1");
        let failures = vec![
            (p.kind.clone(), "caiu".to_string()),
            ("vst3:broken.vst3".to_string(), "não abriu".to_string()),
        ];
        let modules = failed_modules(&failures);
        assert_eq!(
            modules,
            vec![("vst3:broken.vst3".to_string(), "não abriu".to_string())]
        );
    }
}
