//! Audio effects panel: the effect chain of the selected track or of the master bus.
//!
//! The effects come from `neoveg_audio::dsp::effects` (adapted from FilmCraft); the chain itself is
//! stored in the project (`neoveg_engine::effects`). The panel never touches the project: it
//! returns [`FxAction`]s and the application turns each one into one undoable command.

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui::{self, Slider};
use neoveg_audio::clap_host::{self, ClapParamInfo, ClapPluginInfo};
use neoveg_audio::dsp::effects::{self, EffectInfo};
use neoveg_audio::dsp::{ParamSpec, Unit};
use neoveg_audio::fxmixer::{
    AutomationCurve, AutomationKey, BusSpec, DuckSpec, EffectSpec, FxPlan, ParamAutomationSpec,
    RouteSpec, SendSpec, TrackMixSpec,
};
use neoveg_audio::vst3_host::{self, Vst3PluginInfo};
use neoveg_common::fade::FadeCurve;
use neoveg_engine::effects::DuckParam;
use neoveg_engine::{
    AutomationTarget, EnvelopeShape, Timeline, TrackDucking, TrackEffect, TrackEnvelope, TrackId,
    TrackKind,
};

use crate::icons::{self, Icon};

/// What the user did in the auto-ducking section.
#[derive(Debug, Clone, PartialEq)]
pub enum DuckAction {
    Enable(bool),
    Realtime(bool),
    Controller(TrackId, bool),
    Param(DuckParam, f32),
}

/// The auto-ducking of the selected track and the tracks it can duck for.
#[derive(Debug, Clone, PartialEq)]
pub struct DuckView {
    pub ducking: Option<TrackDucking>,
    /// Every other audio track, as `(id, name)`.
    pub candidates: Vec<(TrackId, String)>,
}

/// The numbers of the auto-ducking section, with the descriptions the sliders are built from.
const DUCK_SPECS: [(DuckParam, ParamSpec); 3] = [
    (
        DuckParam::Sensitivity,
        ParamSpec::new("sensitivity", "Sensibilidade", 0.0, 10.0, 6.0, Unit::None),
    ),
    (
        DuckParam::ReduceDb,
        ParamSpec::new("reduce", "Redução", 3.0, 40.0, 15.0, Unit::Decibels),
    ),
    (
        DuckParam::FadeSeconds,
        ParamSpec::new("fade", "Transição", 0.05, 5.0, 0.8, Unit::Seconds),
    ),
];

/// Index the sliders of the auto-ducking section use for their drafts (no effect has it).
const DUCK_INDEX: usize = usize::MAX;

/// What the user did in the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum FxAction {
    Duck(DuckAction),
    Add(&'static str),
    AddClap(String),
    BrowseClapFile,
    BrowseVst3File,
    Remove(usize),
    Move {
        from: usize,
        to: usize,
    },
    Bypass(usize, bool),
    SetParam {
        index: usize,
        param: String,
        value: f32,
    },
    ResetParam {
        index: usize,
        param: String,
    },
    ToggleAutomate {
        index: usize,
        param: String,
        value: f32,
    },
}

/// A slider value that is being dragged: the project changes only when the drag ends.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    index: usize,
    param: String,
    value: f32,
}

#[derive(Debug, Default)]
pub struct FxPanelState {
    /// Edit the master bus instead of the selected track.
    pub master: bool,
    /// Edit the chain of this audio bus instead of the selected track's.
    pub bus: Option<neoveg_engine::BusId>,
    /// The fader and pan of a bus being dragged, so the sound follows before the project changes.
    pub bus_mix_draft: Option<(neoveg_engine::BusId, f32, f32)>,
    draft: Option<Draft>,
    clap_scan_rx: Option<mpsc::Receiver<usize>>,
    clap_file_rx: Option<mpsc::Receiver<Result<Vec<ClapPluginInfo>, String>>>,
    clap_status: Option<String>,
    vst3_scan_rx: Option<mpsc::Receiver<usize>>,
    vst3_file_rx: Option<mpsc::Receiver<Result<Vec<Vst3PluginInfo>, String>>>,
    vst3_status: Option<String>,
}

impl FxPanelState {
    /// While the user drags a parameter slider on an unbypassed effect in `chain`, returns
    /// `(active_fx_index, param_id, draft_value)` so live playback can glide to it immediately.
    pub fn active_effect_draft(&self, chain: &[TrackEffect]) -> Option<(usize, &str, f32)> {
        let draft = self.draft.as_ref()?;
        if draft.index == DUCK_INDEX {
            return None;
        }
        let target = chain.get(draft.index)?;
        if target.bypass {
            return None;
        }
        let active_index = chain[..draft.index].iter().filter(|e| !e.bypass).count();
        Some((active_index, draft.param.as_str(), draft.value))
    }

    /// Starts scanning standard system CLAP folders on a background worker thread.
    pub fn start_system_clap_scan(&mut self) {
        if self.clap_scan_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.clap_scan_rx = Some(rx);
        self.clap_status = Some("Procurando plug-ins CLAP nas pastas do sistema…".into());
        std::thread::Builder::new()
            .name("neoveg-clap-scan".into())
            .spawn(move || {
                let count = clap_host::scan_system_plugins().len();
                let _ = tx.send(count);
            })
            .ok();
        self.start_system_vst3_scan();
    }

    /// Starts scanning a single `.clap` bundle chosen by the user on a background worker thread.
    pub fn start_file_clap_scan(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.clap_file_rx = Some(rx);
        self.clap_status = Some(format!("Carregando {}…", path.display()));
        std::thread::Builder::new()
            .name("neoveg-clap-file".into())
            .spawn(move || {
                let res = clap_host::scan_clap_file(&path);
                let _ = tx.send(res);
            })
            .ok();
    }

    fn poll_clap_workers(&mut self, ctx: &egui::Context, actions: &mut Vec<FxAction>, full: bool) {
        if let Some(rx) = &self.clap_scan_rx {
            match rx.try_recv() {
                Ok(count) => {
                    self.clap_scan_rx = None;
                    self.clap_status = Some(if count == 0 {
                        "Nenhum plug-in CLAP encontrado nas pastas padrão.".into()
                    } else {
                        format!("{count} plug-in(s) CLAP disponíveis.")
                    });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.clap_scan_rx = None;
                }
            }
        }
        if let Some(rx) = &self.clap_file_rx {
            match rx.try_recv() {
                Ok(Ok(found)) => {
                    self.clap_file_rx = None;
                    if found.is_empty() {
                        self.clap_status =
                            Some("O arquivo .clap não contém plug-ins compatíveis.".into());
                    } else {
                        if !full {
                            actions.push(FxAction::AddClap(found[0].kind.clone()));
                        }
                        self.clap_status = Some(format!(
                            "Plug-in CLAP carregado: {} ({} encontrado(s)).",
                            found[0].name,
                            found.len()
                        ));
                    }
                }
                Ok(Err(err)) => {
                    self.clap_file_rx = None;
                    self.clap_status = Some(format!("Erro ao carregar CLAP: {err}"));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.clap_file_rx = None;
                }
            }
        }
    }

    /// Starts scanning the standard VST3 folders on a background worker thread.
    pub fn start_system_vst3_scan(&mut self) {
        if self.vst3_scan_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.vst3_scan_rx = Some(rx);
        self.vst3_status = Some("Procurando plug-ins VST3 nas pastas do sistema…".into());
        std::thread::Builder::new()
            .name("neoveg-vst3-scan".into())
            .spawn(move || {
                let count = vst3_host::scan_system_plugins().len();
                let _ = tx.send(count);
            })
            .ok();
    }

    /// Starts scanning one `.vst3` file chosen by the user on a background worker thread.
    pub fn start_file_vst3_scan(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.vst3_file_rx = Some(rx);
        self.vst3_status = Some(format!("Carregando {}…", path.display()));
        std::thread::Builder::new()
            .name("neoveg-vst3-file".into())
            .spawn(move || {
                let res = vst3_host::scan_vst3_file(&path);
                let _ = tx.send(res);
            })
            .ok();
    }

    fn poll_vst3_workers(&mut self, ctx: &egui::Context, actions: &mut Vec<FxAction>, full: bool) {
        if let Some(rx) = &self.vst3_scan_rx {
            match rx.try_recv() {
                Ok(count) => {
                    self.vst3_scan_rx = None;
                    self.vst3_status = Some(if count == 0 {
                        "Nenhum plug-in VST3 encontrado nas pastas padrão.".into()
                    } else {
                        format!("{count} plug-in(s) VST3 disponíveis.")
                    });
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.vst3_scan_rx = None;
                }
            }
        }
        if let Some(rx) = &self.vst3_file_rx {
            match rx.try_recv() {
                Ok(Ok(found)) => {
                    self.vst3_file_rx = None;
                    if found.is_empty() {
                        self.vst3_status =
                            Some("O arquivo .vst3 não contém efeitos compatíveis.".into());
                    } else {
                        if !full {
                            actions.push(FxAction::AddClap(found[0].kind.clone()));
                        }
                        self.vst3_status = Some(format!(
                            "Plug-in VST3 carregado: {} ({} encontrado(s)).",
                            found[0].name,
                            found.len()
                        ));
                    }
                }
                Ok(Err(err)) => {
                    self.vst3_file_rx = None;
                    self.vst3_status = Some(format!("Erro ao carregar VST3: {err}"));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.vst3_file_rx = None;
                }
            }
        }
    }
}

/// Human-readable pt-BR label for a parameter group prefix (`"b1"` → `"Banda 1"`).
pub fn group_label(prefix: &str) -> String {
    if let Some(num) = prefix
        .strip_prefix('b')
        .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
    {
        return format!("Banda {num}");
    }
    prefix.to_string()
}

// ---- pure helpers -------------------------------------------------------------------------------

/// The effects offered for a chain. Effects that delay the signal are fine anywhere: the mixer
/// compensates the delay so tracks stay aligned.
pub fn available_effects() -> Vec<&'static EffectInfo> {
    effects::effects().iter().collect()
}

/// The auto-ducking of `track_id` and the other audio tracks it can listen to.
pub fn duck_view(timeline: &Timeline, track_id: TrackId) -> Option<DuckView> {
    let track = timeline.track(track_id)?;
    Some(DuckView {
        ducking: track.ducking.clone(),
        candidates: timeline
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && t.id != track_id)
            .map(|t| (t.id, t.name.clone()))
            .collect(),
    })
}

/// Value of `spec` in `effect`: what the user stored (kept inside the range) or the default.
pub fn effective_value(effect: &TrackEffect, spec: &ParamSpec) -> f32 {
    effect
        .params
        .get(spec.id)
        .map_or(spec.default, |v| spec.sanitize(*v))
}

/// A hosted plug-in (CLAP or VST3) as the panel shows it: every value is the same kind of number
/// whatever the format (VST3 parameters are `0.0..=1.0`).
pub struct HostedInfo {
    pub name: String,
    pub format: &'static str,
    pub latency: u32,
    pub params: Vec<ClapParamInfo>,
}

/// `true` for the kind of a CLAP or VST3 plug-in.
pub fn is_hosted_kind(kind: &str) -> bool {
    clap_host::is_clap_kind(kind) || vst3_host::is_vst3_kind(kind)
}

/// What the scans know about the plug-in of `kind` (probing its file once if it was not scanned).
pub fn hosted_info(kind: &str) -> Option<HostedInfo> {
    if clap_host::is_clap_kind(kind) {
        return clap_host::plugin_info(kind).map(|p| HostedInfo {
            name: p.name,
            format: "CLAP",
            latency: p.latency,
            params: p.params,
        });
    }
    vst3_host::plugin_info(kind).map(|p| HostedInfo {
        name: p.name,
        format: "VST3",
        latency: p.latency,
        params: p
            .params
            .into_iter()
            .map(|q| ClapParamInfo {
                id: q.id,
                clap_id: q.vst_id,
                name: q.name,
                min: 0.0,
                max: 1.0,
                default: q.default,
                stepped: q.stepped,
                is_bypass: q.is_bypass,
            })
            .collect(),
    })
}

/// Effective sanitized value of a dynamic CLAP parameter in `effect`.
pub fn effective_clap_value(effect: &TrackEffect, param: &ClapParamInfo) -> f32 {
    let raw = effect
        .params
        .get(&param.id)
        .copied()
        .unwrap_or(param.default);
    if raw.is_nan() {
        return param.default;
    }
    let clamped = raw.clamp(param.min, param.max);
    if param.stepped {
        clamped.round()
    } else {
        clamped
    }
}

/// A parameter value as shown next to its slider.
pub fn format_value(spec: &ParamSpec, value: f32) -> String {
    match spec.unit {
        Unit::Decibels => format!("{value:.1} dB"),
        Unit::Hertz if value >= 1000.0 => format!("{:.2} kHz", value / 1000.0),
        Unit::Hertz => format!("{value:.0} Hz"),
        Unit::Milliseconds if value >= 100.0 => format!("{value:.0} ms"),
        Unit::Milliseconds => format!("{value:.1} ms"),
        Unit::Seconds => format!("{value:.2} s"),
        Unit::Percent => format!("{value:.0} %"),
        Unit::Ratio => format!("{value:.1}:1"),
        Unit::Q => format!("{value:.2}"),
        Unit::Semitones => format!("{value:+.1} st"),
        Unit::Bpm => format!("{value:.0} BPM"),
        Unit::Pan => format!("{value:+.0}"),
        Unit::Toggle => if value >= 0.5 { "ligado" } else { "desligado" }.into(),
        Unit::Choice => spec
            .choices
            .get(value.round().max(0.0) as usize)
            .copied()
            .unwrap_or("")
            .into(),
        Unit::None => format!("{value:.2}"),
    }
}

/// Splits parameters into groups by the part of their id before the first `.` (`b3.freq` belongs
/// to `b3`); parameters without a prefix form the first group, named `None`.
pub fn group_params(
    params: &'static [ParamSpec],
) -> Vec<(Option<&'static str>, Vec<&'static ParamSpec>)> {
    let mut groups: Vec<(Option<&'static str>, Vec<&'static ParamSpec>)> = Vec::new();
    for spec in params {
        let prefix = spec.id.split_once('.').map(|(p, _)| p);
        match groups.iter_mut().find(|(p, _)| *p == prefix) {
            Some((_, list)) => list.push(spec),
            None => groups.push((prefix, vec![spec])),
        }
    }
    groups
}

fn spec_chain(chain: &[TrackEffect]) -> Vec<EffectSpec> {
    chain
        .iter()
        .filter(|e| !e.bypass)
        .map(|e| EffectSpec {
            kind: e.kind.clone(),
            params: e.params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        })
        .collect()
}

fn to_automation_curve(env: &TrackEnvelope) -> Option<AutomationCurve> {
    if !env.enabled || env.points.is_empty() {
        return None;
    }
    let keys = env
        .points
        .iter()
        .map(|p| AutomationKey {
            time: p.time,
            value: p.value,
            curve: match p.shape {
                EnvelopeShape::Linear => Some(FadeCurve::Linear),
                EnvelopeShape::Fast => Some(FadeCurve::Fast),
                EnvelopeShape::Slow => Some(FadeCurve::Slow),
                EnvelopeShape::Smooth => Some(FadeCurve::Smooth),
                EnvelopeShape::Sharp => Some(FadeCurve::Sharp),
                EnvelopeShape::Hold => None,
            },
        })
        .collect();
    Some(AutomationCurve { keys })
}

fn collect_param_envelopes(
    track: Option<usize>,
    chain: &[TrackEffect],
    envelopes: &[TrackEnvelope],
    out: &mut Vec<ParamAutomationSpec>,
) {
    for env in envelopes {
        let AutomationTarget::EffectParam {
            effect_index,
            param_id,
        } = &env.target
        else {
            continue;
        };
        let Some(target_fx) = chain.get(*effect_index) else {
            continue;
        };
        if target_fx.bypass {
            continue;
        }
        let Some(curve) = to_automation_curve(env) else {
            continue;
        };
        let active_fx_index = chain[..*effect_index].iter().filter(|e| !e.bypass).count();
        out.push(ParamAutomationSpec {
            track,
            active_fx_index,
            param_id: param_id.clone(),
            curve,
        });
    }
}

/// What the mixer needs to play the project: every track's chain (indexed like the tracks of the
/// timeline) and the master's, without the bypassed effects, plus the per-track mixer settings,
/// automation envelopes and sidechain ducking.
pub fn fx_plan(timeline: &Timeline) -> FxPlan {
    let mut param_envelopes = Vec::new();
    for (ti, t) in timeline.tracks.iter().enumerate() {
        collect_param_envelopes(Some(ti), &t.effects, &t.envelopes, &mut param_envelopes);
    }
    collect_param_envelopes(
        None,
        &timeline.master_effects,
        &timeline.master_envelopes,
        &mut param_envelopes,
    );
    FxPlan {
        tracks: timeline
            .tracks
            .iter()
            .map(|t| spec_chain(&t.effects))
            .collect(),
        master: spec_chain(&timeline.master_effects),
        duck: timeline
            .tracks
            .iter()
            .map(|t| duck_spec(timeline, t))
            .collect(),
        mix: timeline
            .tracks
            .iter()
            .map(|t| {
                let m = t.mix();
                TrackMixSpec {
                    gain: m.gain(),
                    pan: m.pan,
                    invert: m.invert_phase,
                }
            })
            .collect(),
        pan_law: timeline.pan_law,
        track_vol_envelopes: timeline
            .tracks
            .iter()
            .map(|t| {
                t.envelopes
                    .iter()
                    .find(|e| e.target == AutomationTarget::Volume)
                    .and_then(to_automation_curve)
            })
            .collect(),
        track_pan_envelopes: timeline
            .tracks
            .iter()
            .map(|t| {
                t.envelopes
                    .iter()
                    .find(|e| e.target == AutomationTarget::Pan)
                    .and_then(to_automation_curve)
            })
            .collect(),
        param_envelopes,
        buses: timeline
            .buses
            .iter()
            .map(|b| BusSpec {
                chain: spec_chain(&b.effects),
                mix: TrackMixSpec {
                    // A muted bus is silent; its place in the routing stays
                    gain: if b.mix.muted { 0.0 } else { b.mix.gain() },
                    pan: b.mix.pan,
                    invert: b.mix.invert_phase,
                },
                output: bus_index(timeline, b.output),
            })
            .collect(),
        routes: timeline
            .tracks
            .iter()
            .map(|t| RouteSpec {
                output: bus_index(timeline, t.output),
                sends: t
                    .sends
                    .iter()
                    .filter_map(|s| {
                        let bus = timeline.buses.iter().position(|b| b.id == s.bus)?;
                        Some(SendSpec {
                            bus,
                            gain: send_gain(s.level_db),
                            post: s.post_volume,
                            link_pan: s.link_pan,
                        })
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The index of the bus `output` points at (`None` for the master, or for a bus that is gone).
fn bus_index(timeline: &Timeline, output: neoveg_engine::BusOutput) -> Option<usize> {
    match output {
        neoveg_engine::BusOutput::Master => None,
        neoveg_engine::BusOutput::Bus(id) => timeline.buses.iter().position(|b| b.id == id),
    }
}

/// Linear level of a send of `db` dB (silent at the bottom of the range).
pub fn send_gain(db: f32) -> f32 {
    if db <= neoveg_engine::buses::MIN_SEND_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// The ducking of `track` as the mixer wants it: controllers become track indices, and tracks that
/// no longer exist, are not audio or are the track itself are left out.
fn duck_spec(timeline: &Timeline, track: &neoveg_engine::Track) -> Option<DuckSpec> {
    let duck = track.ducking.as_ref()?;
    let mut controllers: Vec<usize> = duck
        .controllers
        .iter()
        .filter_map(|id| {
            timeline
                .tracks
                .iter()
                .position(|t| t.id == *id && t.kind == TrackKind::Audio && t.id != track.id)
        })
        .collect();
    controllers.sort_unstable();
    controllers.dedup();
    Some(DuckSpec {
        controllers,
        sensitivity: duck.sensitivity,
        reduce_db: duck.reduce_db,
        fade_s: duck.fade_s,
        realtime: duck.realtime,
    })
}

// ---- drawing ----------------------------------------------------------------------------------------

/// Draws the chain `chain` of the target called `title` and returns what the user did.
pub fn fx_ui(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    title: &str,
    chain: &[TrackEffect],
    duck: Option<&DuckView>,
) -> Vec<FxAction> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.selectable_value(&mut panel.master, false, "Faixa");
        ui.selectable_value(&mut panel.master, true, "Master");
        ui.label(title);
    });

    let full = chain.len() >= neoveg_engine::effects::MAX_EFFECTS;
    panel.poll_clap_workers(ui.ctx(), &mut actions, full);
    panel.poll_vst3_workers(ui.ctx(), &mut actions, full);
    let clap_plugins = clap_host::cached_plugins();

    ui.horizontal(|ui| {
        ui.add_enabled_ui(!full, |ui| {
            egui::ComboBox::from_id_salt("fx_add")
                .selected_text("Adicionar efeito…")
                .width(200.0)
                .show_ui(ui, |ui| {
                    for category in effects::Category::ALL {
                        ui.weak(category.label());
                        for info in available_effects()
                            .into_iter()
                            .filter(|e| e.category == category)
                        {
                            if ui.selectable_label(false, info.name).clicked() {
                                actions.push(FxAction::Add(info.id));
                            }
                        }
                    }
                    if !clap_plugins.is_empty() {
                        ui.separator();
                        ui.weak("Plug-ins CLAP");
                        for plugin in &clap_plugins {
                            let label = if plugin.vendor.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{} ({})", plugin.name, plugin.vendor)
                            };
                            if ui.selectable_label(false, label).clicked() {
                                actions.push(FxAction::AddClap(plugin.kind.clone()));
                            }
                        }
                    }
                    let vst3_plugins = vst3_host::cached_plugins();
                    if !vst3_plugins.is_empty() {
                        ui.separator();
                        ui.weak("Plug-ins VST3");
                        for plugin in &vst3_plugins {
                            let label = if plugin.vendor.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{} ({})", plugin.name, plugin.vendor)
                            };
                            if ui.selectable_label(false, label).clicked() {
                                actions.push(FxAction::AddClap(plugin.kind.clone()));
                            }
                        }
                    }
                });
        });

        let scanning = panel.clap_scan_rx.is_some() || panel.vst3_scan_rx.is_some();
        ui.add_enabled_ui(!scanning, |ui| {
            if icons::icon_button_sized(
                ui,
                Icon::Reset,
                "Procurar plug-ins CLAP e VST3 instalados nas pastas padrão do sistema",
                false,
                22.0,
            )
            .clicked()
            {
                panel.start_system_clap_scan();
            }
        });
        if icons::icon_button_sized(
            ui,
            Icon::Import,
            "Carregar arquivo de plug-in de áudio CLAP (.clap)…",
            false,
            22.0,
        )
        .clicked()
        {
            actions.push(FxAction::BrowseClapFile);
        }
        if icons::icon_button_sized(
            ui,
            Icon::Import,
            "Carregar arquivo de plug-in de áudio VST3 (.vst3)…",
            false,
            22.0,
        )
        .clicked()
        {
            actions.push(FxAction::BrowseVst3File);
        }
    });
    if let Some(status) = &panel.clap_status {
        ui.weak(status);
    }
    if let Some(status) = &panel.vst3_status {
        ui.weak(status);
    }
    if chain.is_empty() {
        ui.weak("Sem efeitos. Os efeitos rodam na ordem da lista.");
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (index, effect) in chain.iter().enumerate() {
            ui.separator();
            effect_ui(ui, panel, index, chain.len(), effect, &mut actions);
        }
        if let Some(view) = duck {
            ducking_ui(ui, panel, view, &mut actions);
        }
    });
    actions
}

/// The auto-ducking section: on / off, the tracks that make this one duck, and the amounts.
fn ducking_ui(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    view: &DuckView,
    actions: &mut Vec<FxAction>,
) {
    ui.separator();
    let mut on = view.ducking.is_some();
    if ui
        .checkbox(
            &mut on,
            "Auto-ducking: abaixar esta faixa quando outras tiverem som",
        )
        .changed()
    {
        actions.push(FxAction::Duck(DuckAction::Enable(on)));
    }
    let Some(duck) = &view.ducking else {
        return;
    };
    let mut realtime = duck.realtime;
    if ui
        .checkbox(
            &mut realtime,
            "Sidechain em tempo real (pós-FX e pós-fader)",
        )
        .changed()
    {
        actions.push(FxAction::Duck(DuckAction::Realtime(realtime)));
    }
    if duck.realtime {
        ui.weak(
            "O detector acompanha ao vivo o áudio pós-efeitos e pós-fader das faixas de controle.",
        );
    } else {
        ui.weak("A análise é feita ao iniciar a reprodução, e a faixa desce antes do som entrar.");
    }
    if view.candidates.is_empty() {
        ui.weak("Não há outras faixas de áudio.");
    } else {
        ui.label("Abaixar quando houver som em:");
        for (id, name) in &view.candidates {
            let mut selected = duck.controllers.contains(id);
            if ui.checkbox(&mut selected, name).changed() {
                actions.push(FxAction::Duck(DuckAction::Controller(*id, selected)));
            }
        }
    }
    if duck.controllers.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(241, 196, 15),
            "Marque ao menos uma faixa de controle.",
        );
    }
    for (param, spec) in &DUCK_SPECS {
        let current = match param {
            DuckParam::Sensitivity => duck.sensitivity,
            DuckParam::ReduceDb => duck.reduce_db,
            DuckParam::FadeSeconds => duck.fade_s,
        };
        ui.horizontal(|ui| {
            ui.add_sized([120.0, 18.0], egui::Label::new(spec.name).truncate());
            let mut committed = Vec::new();
            slider(ui, panel, DUCK_INDEX, spec, current, &mut committed);
            for action in committed {
                if let FxAction::SetParam { value, .. } = action {
                    actions.push(FxAction::Duck(DuckAction::Param(*param, value)));
                }
            }
        });
    }
}

fn effect_ui(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    count: usize,
    effect: &TrackEffect,
    actions: &mut Vec<FxAction>,
) {
    let is_clap = is_hosted_kind(&effect.kind);
    let clap_info = is_clap.then(|| hosted_info(&effect.kind)).flatten();
    let info = (!is_clap)
        .then(|| effects::effect_info(&effect.kind))
        .flatten();
    ui.horizontal(|ui| {
        if icons::icon_button_sized(
            ui,
            Icon::Bypass,
            "Ligar ou desligar o efeito (bypass)",
            !effect.bypass,
            22.0,
        )
        .clicked()
        {
            actions.push(FxAction::Bypass(index, !effect.bypass));
        }
        let name = if let Some(ci) = &clap_info {
            ci.name.as_str()
        } else {
            info.map_or(effect.kind.as_str(), |i| i.name)
        };
        ui.strong(name);
        if is_clap {
            ui.weak(clap_info.as_ref().map_or("Plug-in", |ci| ci.format));
            if let Some(ci) = &clap_info {
                if ci.latency > 0 {
                    ui.weak(format!("({} am.)", ci.latency));
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button_sized(ui, Icon::Remove, "Remover o efeito", false, 22.0).clicked()
            {
                actions.push(FxAction::Remove(index));
            }
            ui.add_enabled_ui(index + 1 < count, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveDown, "Mover para baixo", false, 22.0)
                    .clicked()
                {
                    actions.push(FxAction::Move {
                        from: index,
                        to: index + 1,
                    });
                }
            });
            ui.add_enabled_ui(index > 0, |ui| {
                if icons::icon_button_sized(ui, Icon::MoveUp, "Mover para cima", false, 22.0)
                    .clicked()
                {
                    actions.push(FxAction::Move {
                        from: index,
                        to: index - 1,
                    });
                }
            });
        });
    });
    if is_clap {
        let Some(ci) = clap_info else {
            ui.colored_label(
                egui::Color32::from_rgb(231, 76, 60),
                "Plug-in ausente neste sistema: ele é ignorado na reprodução.",
            );
            return;
        };
        if effect.bypass {
            ui.weak("Em bypass");
        }
        ui.add_enabled_ui(!effect.bypass, |ui| {
            if ci.params.is_empty() {
                ui.weak("Este plug-in não expõe parâmetros editáveis.");
            }
            for param in &ci.params {
                clap_param_row(ui, panel, index, effect, param, actions);
            }
        });
        return;
    }
    let Some(info) = info else {
        ui.colored_label(
            egui::Color32::from_rgb(231, 76, 60),
            "Efeito desconhecido: ele é ignorado na reprodução.",
        );
        return;
    };
    if effect.bypass {
        ui.weak("Em bypass");
    }
    ui.add_enabled_ui(!effect.bypass, |ui| {
        for (prefix, specs) in group_params(info.params) {
            match prefix {
                None => {
                    for spec in specs {
                        param_row(ui, panel, index, effect, spec, actions);
                    }
                }
                Some(prefix) => {
                    egui::CollapsingHeader::new(group_label(prefix))
                        .id_salt((index, &effect.kind, prefix))
                        .default_open(false)
                        .show(ui, |ui| {
                            for spec in specs {
                                param_row(ui, panel, index, effect, spec, actions);
                            }
                        });
                }
            }
        }
    });
}

fn param_row(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    effect: &TrackEffect,
    spec: &'static ParamSpec,
    actions: &mut Vec<FxAction>,
) {
    let current = effective_value(effect, spec);
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new(spec.name).truncate());
        let response = match spec.unit {
            Unit::Toggle => {
                let mut on = current >= 0.5;
                let response = ui.checkbox(&mut on, "");
                if response.changed() {
                    actions.push(FxAction::SetParam {
                        index,
                        param: spec.id.into(),
                        value: if on { 1.0 } else { 0.0 },
                    });
                }
                response
            }
            Unit::Choice => {
                let selected = current.round().max(0.0) as usize;
                let text = spec.choices.get(selected).copied().unwrap_or("");
                egui::ComboBox::from_id_salt((index, &effect.kind, spec.id))
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        for (i, choice) in spec.choices.iter().enumerate() {
                            if ui.selectable_label(i == selected, *choice).clicked() {
                                actions.push(FxAction::SetParam {
                                    index,
                                    param: spec.id.into(),
                                    value: i as f32,
                                });
                            }
                        }
                    })
                    .response
            }
            _ => slider(ui, panel, index, spec, current, actions),
        };
        response.context_menu(|ui| {
            if ui.button("Restaurar o padrão").clicked() {
                actions.push(FxAction::ResetParam {
                    index,
                    param: spec.id.into(),
                });
                ui.close_menu();
            }
            if !panel.master && ui.button("Automatizar parâmetro na faixa").clicked() {
                actions.push(FxAction::ToggleAutomate {
                    index,
                    param: spec.id.into(),
                    value: current,
                });
                ui.close_menu();
            }
        });
    });
}

fn slider(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    spec: &'static ParamSpec,
    current: f32,
    actions: &mut Vec<FxAction>,
) -> egui::Response {
    // While a drag is in progress the slider follows the draft; the project changes on release
    let dragging = panel
        .draft
        .as_ref()
        .filter(|d| d.index == index && d.param == spec.id);
    let mut value = dragging.map_or(current, |d| d.value);
    let response = ui.add(
        Slider::new(&mut value, spec.min..=spec.max)
            .logarithmic(spec.log_scale)
            .custom_formatter(|v, _| format_value(spec, v as f32)),
    );
    if response.changed() {
        panel.draft = Some(Draft {
            index,
            param: spec.id.into(),
            value,
        });
    }
    let finished = response.drag_stopped()
        || response.lost_focus()
        || (response.changed() && !response.dragged() && !response.has_focus());
    if finished {
        if let Some(draft) = panel
            .draft
            .take_if(|d| d.index == index && d.param == spec.id)
        {
            if (draft.value - current).abs() > f32::EPSILON {
                actions.push(FxAction::SetParam {
                    index,
                    param: draft.param,
                    value: draft.value,
                });
            }
        }
    }
    response
}

fn clap_param_row(
    ui: &mut egui::Ui,
    panel: &mut FxPanelState,
    index: usize,
    effect: &TrackEffect,
    param: &ClapParamInfo,
    actions: &mut Vec<FxAction>,
) {
    let current = effective_clap_value(effect, param);
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new(&param.name).truncate());
        let response =
            if param.stepped && (param.min - 0.0).abs() < 1e-6 && (param.max - 1.0).abs() < 1e-6 {
                let mut on = current >= 0.5;
                let resp = ui.checkbox(&mut on, "");
                if resp.changed() {
                    actions.push(FxAction::SetParam {
                        index,
                        param: param.id.clone(),
                        value: if on { 1.0 } else { 0.0 },
                    });
                }
                resp
            } else {
                let dragging = panel
                    .draft
                    .as_ref()
                    .filter(|d| d.index == index && d.param == param.id);
                let mut value = dragging.map_or(current, |d| d.value);
                let mut s = Slider::new(&mut value, param.min..=param.max);
                if param.stepped {
                    s = s.integer();
                }
                let resp = ui.add(s);
                if resp.changed() {
                    panel.draft = Some(Draft {
                        index,
                        param: param.id.clone(),
                        value: if param.stepped { value.round() } else { value },
                    });
                }
                let finished = resp.drag_stopped()
                    || resp.lost_focus()
                    || (resp.changed() && !resp.dragged() && !resp.has_focus());
                if finished {
                    if let Some(draft) = panel
                        .draft
                        .take_if(|d| d.index == index && d.param == param.id)
                    {
                        if (draft.value - current).abs() > f32::EPSILON {
                            actions.push(FxAction::SetParam {
                                index,
                                param: draft.param,
                                value: draft.value,
                            });
                        }
                    }
                }
                resp
            };
        response.context_menu(|ui| {
            if ui.button("Restaurar o padrão").clicked() {
                actions.push(FxAction::ResetParam {
                    index,
                    param: param.id.clone(),
                });
                ui.close_menu();
            }
            if !panel.master && ui.button("Automatizar parâmetro na faixa").clicked() {
                actions.push(FxAction::ToggleAutomate {
                    index,
                    param: param.id.clone(),
                    value: current,
                });
                ui.close_menu();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_engine::TrackKind;

    fn spec(id: &str) -> &'static ParamSpec {
        effects::effects()
            .iter()
            .flat_map(|e| e.params)
            .find(|p| p.id == id)
            .unwrap()
    }

    #[test]
    fn every_registered_effect_is_offered_including_the_ones_with_latency() {
        let offered = available_effects();
        assert_eq!(offered.len(), effects::effects().len());
        for category in effects::Category::ALL {
            assert!(
                offered.iter().any(|e| e.category == category),
                "category {category:?} should offer at least one effect"
            );
        }
        for info in &offered {
            for p in info.params {
                assert!(
                    !matches!(
                        p.name,
                        "Threshold"
                            | "Output Gain"
                            | "Make-up Gain"
                            | "Release"
                            | "Feedback"
                            | "Delay Time"
                            | "Room Size"
                    ),
                    "{}.{} still has English display name `{}`",
                    info.id,
                    p.id,
                    p.name
                );
            }
        }
        for id in [
            "limiter",
            "denoise",
            "fft_filter",
            "notch_filter",
            "scientific_filter",
            "chorus_flanger",
            "pitch_shifter",
            "stereo_width",
        ] {
            assert!(offered.iter().any(|e| e.id == id), "{id}");
        }
    }

    #[test]
    fn active_effect_draft_skips_bypassed_effects_and_ducking() {
        let mut bypassed = TrackEffect::new("reverb");
        bypassed.bypass = true;
        let chain = vec![
            TrackEffect::new("simple_eq"),
            bypassed,
            TrackEffect::new("compressor"),
        ];
        let mut panel = FxPanelState {
            master: false,
            draft: Some(Draft {
                index: 2,
                param: "ratio".into(),
                value: 6.0,
            }),
            ..Default::default()
        };
        assert_eq!(
            panel.active_effect_draft(&chain),
            Some((1, "ratio", 6.0)),
            "bypassed reverb at index 1 is skipped in the active mixer chain"
        );
        panel.draft.as_mut().unwrap().index = 1;
        assert_eq!(panel.active_effect_draft(&chain), None);
        panel.draft.as_mut().unwrap().index = DUCK_INDEX;
        assert_eq!(panel.active_effect_draft(&chain), None);
        assert_eq!(group_label("b3"), "Banda 3");
    }

    #[test]
    fn the_ducking_sliders_cover_the_ranges_the_engine_accepts() {
        for (param, spec) in &DUCK_SPECS {
            assert_eq!((spec.min, spec.max), param.range(), "{}", spec.id);
            assert!(spec.default >= spec.min && spec.default <= spec.max);
        }
        let defaults = TrackDucking::default();
        assert_eq!(DUCK_SPECS[0].1.default, defaults.sensitivity);
        assert_eq!(DUCK_SPECS[1].1.default, defaults.reduce_db);
        assert_eq!(DUCK_SPECS[2].1.default, defaults.fade_s);
    }

    #[test]
    fn the_ducking_view_lists_the_other_audio_tracks_only() {
        let mut tl = Timeline::new();
        for kind in [
            TrackKind::Audio,
            TrackKind::Video,
            TrackKind::Audio,
            TrackKind::Audio,
        ] {
            let track = tl.new_track(kind);
            tl.tracks.push(track);
        }
        let first = tl.tracks[0].id;
        let view = duck_view(&tl, first).unwrap();
        assert!(view.ducking.is_none());
        let names: Vec<&str> = view.candidates.iter().map(|c| c.1.as_str()).collect();
        assert_eq!(
            names,
            ["Audio 2", "Audio 3"],
            "no video track and not itself"
        );
        assert!(duck_view(&tl, 9999).is_none());
    }

    #[test]
    fn the_plan_turns_controllers_into_track_indices_and_drops_the_invalid_ones() {
        let mut tl = Timeline::new();
        for kind in [TrackKind::Audio, TrackKind::Audio, TrackKind::Video] {
            let track = tl.new_track(kind);
            tl.tracks.push(track);
        }
        let (music, voice, video) = (tl.tracks[0].id, tl.tracks[1].id, tl.tracks[2].id);
        tl.tracks[0].ducking = Some(TrackDucking {
            controllers: vec![voice, voice, video, music, 9999],
            reduce_db: 18.0,
            ..Default::default()
        });
        let plan = fx_plan(&tl);
        assert_eq!(plan.duck.len(), 3);
        assert!(plan.duck[1].is_none() && plan.duck[2].is_none());
        let spec = plan.duck[0].as_ref().unwrap();
        assert_eq!(spec.controllers, [1], "only the other audio track, once");
        assert_eq!(spec.reduce_db, 18.0);
        assert!(!plan.is_empty());

        // Ducking with nothing to listen to is as good as none
        tl.tracks[0].ducking = Some(TrackDucking::default());
        assert!(fx_plan(&tl).is_empty());
    }

    #[test]
    fn a_parameter_shows_its_stored_value_inside_the_range_or_its_default() {
        let ratio = spec("ratio");
        let mut e = TrackEffect::new("compressor");
        assert_eq!(effective_value(&e, ratio), ratio.default);
        e.params.insert("ratio".into(), 8.0);
        assert_eq!(effective_value(&e, ratio), 8.0);
        e.params.insert("ratio".into(), 1000.0);
        assert_eq!(
            effective_value(&e, ratio),
            ratio.max,
            "a file with a wild value is clamped"
        );
        e.params.insert("ratio".into(), f32::NAN);
        assert_eq!(effective_value(&e, ratio), ratio.default);
    }

    #[test]
    fn values_are_shown_with_their_units() {
        let by_unit = |unit: Unit| {
            effects::effects()
                .iter()
                .flat_map(|e| e.params)
                .find(|p| p.unit == unit)
                .unwrap()
        };
        assert_eq!(format_value(by_unit(Unit::Decibels), -20.0), "-20.0 dB");
        assert_eq!(format_value(by_unit(Unit::Hertz), 250.0), "250 Hz");
        assert_eq!(format_value(by_unit(Unit::Hertz), 5000.0), "5.00 kHz");
        assert_eq!(format_value(by_unit(Unit::Milliseconds), 10.0), "10.0 ms");
        assert_eq!(format_value(by_unit(Unit::Percent), 30.0), "30 %");
        assert_eq!(format_value(by_unit(Unit::Ratio), 4.0), "4.0:1");
        assert_eq!(format_value(spec("detector"), 1.0), "RMS");
        assert_eq!(format_value(spec("true_peak"), 1.0), "ligado");
    }

    #[test]
    fn eq_bands_are_grouped_by_their_prefix_and_plain_effects_form_one_group() {
        let eq = effects::effect_info("parametric_eq").unwrap();
        let groups = group_params(eq.params);
        assert!(groups.len() > 2);
        assert!(groups
            .iter()
            .any(|(p, list)| *p == Some("b1") && list.len() == 5));
        // The output gain has no prefix
        assert!(groups.iter().any(|(p, _)| p.is_none()));

        let comp = effects::effect_info("compressor").unwrap();
        let groups = group_params(comp.params);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, None);
        assert_eq!(groups[0].1.len(), comp.params.len());
    }

    #[test]
    fn the_plan_follows_the_track_order_and_leaves_out_bypassed_effects() {
        let mut tl = Timeline::new();
        for _ in 0..3 {
            let track = tl.new_track(TrackKind::Audio);
            tl.tracks.push(track);
        }
        let mut compressor = TrackEffect::new("compressor");
        compressor.params.insert("ratio".into(), 8.0);
        let mut bypassed = TrackEffect::new("reverb");
        bypassed.bypass = true;
        tl.tracks[1].effects = vec![compressor, bypassed, TrackEffect::new("simple_eq")];
        tl.master_effects = vec![TrackEffect::new("limiter")];

        let plan = fx_plan(&tl);
        assert_eq!(plan.tracks.len(), 3);
        assert!(plan.tracks[0].is_empty() && plan.tracks[2].is_empty());
        let kinds: Vec<&str> = plan.tracks[1].iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["compressor", "simple_eq"]);
        assert_eq!(plan.tracks[1][0].params, vec![("ratio".to_string(), 8.0)]);
        assert_eq!(plan.master.len(), 1);
        assert!(!plan.is_empty());
        assert!(fx_plan(&Timeline::new()).is_empty());
    }

    #[test]
    fn the_plan_carries_per_track_mixer_settings_and_pan_law() {
        let mut tl = Timeline::new();
        let a = tl.new_track(TrackKind::Audio);
        let b = tl.new_track(TrackKind::Audio);
        tl.tracks.push(a);
        tl.tracks.push(b);
        assert!(fx_plan(&tl).is_empty(), "neutral faders keep the plan dry");

        tl.tracks[1].volume_db = -6.0;
        tl.tracks[1].pan = -0.5;
        tl.tracks[1].invert_phase = true;
        tl.pan_law = neoveg_common::pan::PanLaw::ConstantPower;
        let plan = fx_plan(&tl);
        assert!(!plan.is_empty());
        assert!(plan.mix[0].is_neutral());
        assert!((plan.mix[1].gain - 0.501_187).abs() < 1e-3);
        assert_eq!((plan.mix[1].pan, plan.mix[1].invert), (-0.5, true));
        assert_eq!(plan.pan_law, neoveg_common::pan::PanLaw::ConstantPower);
    }

    #[test]
    fn clap_effect_values_clamp_and_round_when_stepped_and_plan_preserves_clap_kind() {
        let cont = ClapParamInfo {
            id: "1".into(),
            clap_id: 1,
            name: "Cutoff".into(),
            min: 20.0,
            max: 20_000.0,
            default: 1_000.0,
            stepped: false,
            is_bypass: false,
        };
        let step = ClapParamInfo {
            id: "2".into(),
            clap_id: 2,
            name: "Mode".into(),
            min: 0.0,
            max: 3.0,
            default: 1.0,
            stepped: true,
            is_bypass: false,
        };
        let mut fx = TrackEffect::new("clap:C:/CLAP/Test.clap::com.vendor.test");
        assert_eq!(effective_clap_value(&fx, &cont), 1_000.0);
        fx.params.insert("1".into(), 50_000.0);
        assert_eq!(effective_clap_value(&fx, &cont), 20_000.0);
        fx.params.insert("2".into(), 2.4);
        assert_eq!(effective_clap_value(&fx, &step), 2.0);

        let mut tl = Timeline::new();
        let tr = tl.new_track(TrackKind::Audio);
        tl.tracks.push(tr);
        tl.tracks[0].effects.push(fx);
        let plan = fx_plan(&tl);
        assert_eq!(
            plan.tracks[0][0].kind,
            "clap:C:/CLAP/Test.clap::com.vendor.test"
        );
    }

    #[test]
    fn the_plan_has_the_buses_the_routes_and_the_sends_of_the_project() {
        use neoveg_engine::{buses, BusOutput, EditCommand, History, TrackKind, TrackSend};
        let mut tl = Timeline::new();
        let mut h = History::new();
        let mut tracks = Vec::new();
        for _ in 0..3 {
            let t = tl.new_track(TrackKind::Audio);
            tracks.push(t.id);
            assert!(h.execute(&mut tl, EditCommand::AddTrack { track: t }));
        }
        for id in [500, 501] {
            let cmd = buses::add_bus(&tl, id).unwrap();
            assert!(h.execute(&mut tl, cmd));
        }
        // Bus A goes to B and is muted; B has an effect; track 0 goes to A with a send to B at -6 dB
        let mut a = tl.buses[0].clone();
        a.output = BusOutput::Bus(501);
        a.mix.muted = true;
        let cmd = buses::set_bus(&tl, a).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let mut b = tl.buses[1].clone();
        b.effects = vec![TrackEffect::new("gain")];
        let cmd = buses::set_bus(&tl, b).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let send = TrackSend {
            bus: 501,
            level_db: -6.0,
            post_volume: true,
            link_pan: true,
        };
        let cmd = buses::set_track_route(&tl, tracks[0], BusOutput::Bus(500), vec![send]).unwrap();
        assert!(h.execute(&mut tl, cmd));
        let plan = fx_plan(&tl);
        assert_eq!(plan.buses.len(), 2);
        assert_eq!(plan.buses[0].output, Some(1), "A goes to B");
        assert_eq!(plan.buses[0].mix.gain, 0.0, "a muted bus is silent");
        assert_eq!(plan.buses[1].output, None);
        assert_eq!(plan.buses[1].chain.len(), 1);
        assert_eq!(plan.routes.len(), 3);
        assert_eq!(plan.routes[0].output, Some(0));
        let s = plan.routes[0].sends[0];
        assert_eq!(s.bus, 1);
        assert!(
            (s.gain - 0.501_187).abs() < 1e-4,
            "-6 dB is about 0.501: {}",
            s.gain
        );
        assert!(s.post && s.link_pan);
        assert!(plan.routes[1].is_default() && plan.routes[2].is_default());
        assert!(!plan.is_empty());
        // A bus that is gone reads as the master, and the bottom of the range is silence
        tl.buses.remove(1);
        let broken = fx_plan(&tl);
        assert_eq!(broken.buses[0].output, None);
        assert!(broken.routes[0].sends.is_empty());
        assert_eq!(send_gain(-60.0), 0.0);
        assert!((send_gain(0.0) - 1.0).abs() < 1e-6);
        assert!(fx_plan(&Timeline::new()).is_empty());
    }
}
