//! CLAP audio plug-in host built on `clack-host` and `clack-extensions`.
//!
//! CLAP (`CLever Audio Plug-in`) separates `[main-thread]` lifecycle/metadata operations from
//! `[audio-thread]` real-time processing (`PluginInstance` is `!Send`, while
//! `StoppedPluginAudioProcessor` / `StartedPluginAudioProcessor` are `Send`).
//!
//! [`ClapManager`] runs a dedicated background main-thread loop that owns all loaded
//! [`PluginEntry`] handles and active [`PluginInstance`] objects. When [`create_clap_effect`] is
//! called outside the audio callback (during [`crate::fxmixer::BlockMixer`] construction), the
//! manager instantiates and activates the plug-in, queries its parameters, audio ports and latency,
//! and hands the `Send` audio processor to [`ClapEffect`]. Inside `ClapEffect::process`, all audio
//! and event buffers are pre-allocated — zero allocations, zero locks and zero I/O occur on the
//! real-time audio callback. On [`Drop`] (after the stream stops), `ClapEffect` stops processing
//! and returns the processor to `ClapManager` for clean deactivation on its main thread.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, OnceLock, RwLock};

use clack_extensions::audio_ports::{
    AudioPortInfoBuffer, AudioPortRescanFlags, HostAudioPorts, HostAudioPortsImpl, PluginAudioPorts,
};
use clack_extensions::latency::{HostLatency, HostLatencyImpl, PluginLatency};
use clack_extensions::params::{
    HostParams, HostParamsImplMainThread, HostParamsImplShared, ParamClearFlags, ParamInfoBuffer,
    ParamInfoFlags, ParamRescanFlags, PluginParams,
};
use clack_host::events::event_types::ParamValueEvent;
use clack_host::prelude::*;

use crate::dsp::{AudioEffect, ParamSpec};
use crate::fxmixer::MAX_BLOCK;

/// Prefix used in `TrackEffect::kind` for hosted CLAP plug-ins (`"clap:<path>::<plugin_id>"`).
pub const CLAP_KIND_PREFIX: &str = "clap:";

/// Returns `true` when `kind` identifies a hosted CLAP plug-in rather than a built-in DSP effect.
#[inline]
pub fn is_clap_kind(kind: &str) -> bool {
    kind.starts_with(CLAP_KIND_PREFIX)
}

/// Formats a `TrackEffect::kind` identifier for `(bundle_path, plugin_id)`.
pub fn format_clap_kind(path: &Path, plugin_id: &str) -> String {
    format!("{CLAP_KIND_PREFIX}{}::{plugin_id}", path.to_string_lossy())
}

/// Splits `"clap:<bundle_path>::<plugin_id>"` back into `(bundle_path, plugin_id)`.
///
/// Uses `rsplit_once("::")` so Windows drive colons (`C:\...`) in the path are preserved intact.
pub fn parse_clap_kind(kind: &str) -> Option<(&str, &str)> {
    let rest = kind.strip_prefix(CLAP_KIND_PREFIX)?;
    let (path, plugin_id) = rest.rsplit_once("::")?;
    if path.is_empty() || plugin_id.is_empty() {
        None
    } else {
        Some((path, plugin_id))
    }
}

/// Metadata for a single automatable/controllable parameter exposed by a CLAP plug-in.
#[derive(Debug, Clone, PartialEq)]
pub struct ClapParamInfo {
    /// Stable string key stored in `TrackEffect::params` (decimal `clap_id`).
    pub id: String,
    /// Raw 32-bit CLAP parameter identifier.
    pub clap_id: u32,
    /// Human-readable parameter name (prefixed with module path if provided by the plug-in).
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub stepped: bool,
    pub is_bypass: bool,
}

/// Metadata for a discovered CLAP plug-in inside a `.clap` bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct ClapPluginInfo {
    /// Full `TrackEffect::kind` key (`"clap:<path>::<plugin_id>"`).
    pub kind: String,
    /// Stable CLAP reverse-DNS identifier (e.g. `"org.surge-synth-team.surge-xt"`).
    pub plugin_id: String,
    /// Display name reported by the plug-in descriptor.
    pub name: String,
    /// Vendor/author reported by the plug-in descriptor.
    pub vendor: String,
    /// Version string reported by the plug-in descriptor.
    pub version: String,
    /// Filesystem path to the `.clap` bundle.
    pub path: PathBuf,
    /// Main input channel count (or `0` for pure generators).
    pub audio_inputs: u32,
    /// Main output channel count.
    pub audio_outputs: u32,
    /// Exposed controllable parameters.
    pub params: Vec<ClapParamInfo>,
    /// Reported processing latency in samples at 48 kHz probe time.
    pub latency: u32,
}

static REGISTRY: OnceLock<RwLock<Vec<ClapPluginInfo>>> = OnceLock::new();

fn registry() -> &'static RwLock<Vec<ClapPluginInfo>> {
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Returns a snapshot of all CLAP plug-ins discovered so far without touching the disk.
pub fn cached_plugins() -> Vec<ClapPluginInfo> {
    registry().read().map(|g| g.clone()).unwrap_or_default()
}

/// Looks up metadata for `kind` (`"clap:<path>::<plugin_id>"`), probing its `.clap` bundle once if
/// it has not been scanned yet (e.g. when reopening a saved `.neoveg` project).
pub fn plugin_info(kind: &str) -> Option<ClapPluginInfo> {
    if let Ok(guard) = registry().read() {
        if let Some(found) = guard.iter().find(|p| p.kind == kind) {
            return Some(found.clone());
        }
    }
    let (path_str, _) = parse_clap_kind(kind)?;
    let _ = scan_clap_file(Path::new(path_str)).ok()?;
    registry()
        .read()
        .ok()
        .and_then(|g| g.iter().find(|p| p.kind == kind).cloned())
}

/// Standard per-OS CLAP search directories plus any paths listed in `CLAP_PATH`.
pub fn default_clap_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(env_paths) = std::env::var_os("CLAP_PATH") {
        for p in std::env::split_paths(&env_paths) {
            if !p.as_os_str().is_empty() && !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
            let p = PathBuf::from(common).join("CLAP");
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(local)
                .join("Programs")
                .join("Common")
                .join("CLAP");
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let sys = PathBuf::from("/Library/Audio/Plug-Ins/CLAP");
        if !dirs.contains(&sys) {
            dirs.push(sys);
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user = PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP");
            if !dirs.contains(&user) {
                dirs.push(user);
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let user = PathBuf::from(home).join(".clap");
            if !dirs.contains(&user) {
                dirs.push(user);
            }
        }
        for sys in ["/usr/lib/clap", "/usr/local/lib/clap", "/usr/lib64/clap"] {
            let p = PathBuf::from(sys);
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }

    dirs
}

fn collect_clap_bundles(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_clap = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("clap"));
        if is_clap {
            if !out.contains(&path) {
                out.push(path);
            }
            continue;
        }
        if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            collect_clap_bundles(&path, depth + 1, out);
        }
    }
}

fn merge_into_registry(discovered: &[ClapPluginInfo]) {
    if let Ok(mut guard) = registry().write() {
        for info in discovered {
            if let Some(existing) = guard.iter_mut().find(|p| p.kind == info.kind) {
                *existing = info.clone();
            } else {
                guard.push(info.clone());
            }
        }
        guard.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.kind.cmp(&b.kind))
        });
    }
}

/// Scans a single `.clap` file/bundle on the CLAP manager thread, caches the discovered plug-ins
/// and returns them.
pub fn scan_clap_file(path: &Path) -> Result<Vec<ClapPluginInfo>, String> {
    let (tx, rx) = mpsc::channel();
    manager_sender()
        .send(ManagerCmd::ScanBundle {
            path: path.to_path_buf(),
            reply: tx,
        })
        .map_err(|_| "Host CLAP indisponivel".to_string())?;
    let found = rx
        .recv()
        .map_err(|_| "Falha ao receber resposta do host CLAP".to_string())??;
    merge_into_registry(&found);
    Ok(found)
}

/// Walks all [`default_clap_dirs`], scans every `.clap` bundle found (skipping invalid bundles
/// with a warning) and returns the updated registry. Run on a worker thread, not the UI thread.
pub fn scan_system_plugins() -> Vec<ClapPluginInfo> {
    let mut bundles = Vec::new();
    for dir in default_clap_dirs() {
        collect_clap_bundles(&dir, 0, &mut bundles);
    }
    for bundle in bundles {
        if let Err(err) = scan_clap_file(&bundle) {
            tracing::warn!(
                "[WARN] falha ao inspecionar pacote CLAP `{}`: {}",
                bundle.display(),
                err
            );
        }
    }
    cached_plugins()
}

/// Instantiates and activates a hosted CLAP plug-in as a real-time [`AudioEffect`].
pub fn create_clap_effect(kind: &str, sample_rate: f32) -> Option<Box<dyn AudioEffect>> {
    let (path_str, plugin_id) = parse_clap_kind(kind)?;
    let (tx, rx) = mpsc::channel();
    manager_sender()
        .send(ManagerCmd::Activate {
            path: PathBuf::from(path_str),
            plugin_id: plugin_id.to_string(),
            sample_rate: f64::from(sample_rate.max(8_000.0)),
            max_frames: MAX_BLOCK as u32,
            reply: tx,
        })
        .ok()?;
    match rx.recv().ok()? {
        Ok(activated) => Some(Box::new(ClapEffect::new(
            activated,
            manager_sender().clone(),
        ))),
        Err(err) => {
            tracing::warn!("[WARN] falha ao ativar plug-in CLAP `{kind}`: {err}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Host Handlers (`clack-host`)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NeovegHostShared {
    params_ext: OnceLock<Option<PluginParams>>,
    latency_ext: OnceLock<Option<PluginLatency>>,
    ports_ext: OnceLock<Option<PluginAudioPorts>>,
    restart_requested: AtomicBool,
    process_requested: AtomicBool,
    callback_requested: AtomicBool,
    flush_requested: AtomicBool,
}

impl<'a> SharedHandler<'a> for NeovegHostShared {
    fn initializing(&self, instance: InitializingPluginHandle<'a>) {
        let _ = self.params_ext.set(instance.get_extension());
        let _ = self.latency_ext.set(instance.get_extension());
        let _ = self.ports_ext.set(instance.get_extension());
    }

    fn request_restart(&self) {
        self.restart_requested.store(true, Ordering::Relaxed);
    }

    fn request_process(&self) {
        self.process_requested.store(true, Ordering::Relaxed);
    }

    fn request_callback(&self) {
        self.callback_requested.store(true, Ordering::Relaxed);
    }
}

impl HostParamsImplShared for NeovegHostShared {
    fn request_flush(&self) {
        self.flush_requested.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct NeovegHostMainThread {
    params_rescan: Cell<bool>,
    latency_changed: Cell<bool>,
}

impl<'a> MainThreadHandler<'a> for NeovegHostMainThread {}

impl HostParamsImplMainThread for NeovegHostMainThread {
    fn rescan(&self, _flags: ParamRescanFlags) {
        self.params_rescan.set(true);
    }

    fn clear(&self, _param_id: ClapId, _flags: ParamClearFlags) {}
}

impl HostLatencyImpl for NeovegHostMainThread {
    fn changed(&self) {
        self.latency_changed.set(true);
    }
}

impl HostAudioPortsImpl for NeovegHostMainThread {
    fn is_rescan_flag_supported(&self, _flag: AudioPortRescanFlags) -> bool {
        false
    }

    fn rescan(&self, _flags: AudioPortRescanFlags) {}
}

struct NeovegClapHost;

impl HostHandlers for NeovegClapHost {
    type Shared<'a> = NeovegHostShared;
    type MainThread<'a> = NeovegHostMainThread;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder
            .register::<HostParams>()
            .register::<HostLatency>()
            .register::<HostAudioPorts>();
    }
}

// ---------------------------------------------------------------------------
// Dedicated CLAP Main-Thread Manager
// ---------------------------------------------------------------------------

struct ActivatedClap {
    instance_id: u64,
    processor: StoppedPluginAudioProcessor<NeovegClapHost>,
    params: Vec<ClapParamInfo>,
    latency: usize,
    in_port_channels: Vec<usize>,
    out_port_channels: Vec<usize>,
}

enum ManagerCmd {
    ScanBundle {
        path: PathBuf,
        reply: mpsc::Sender<Result<Vec<ClapPluginInfo>, String>>,
    },
    Activate {
        path: PathBuf,
        plugin_id: String,
        sample_rate: f64,
        max_frames: u32,
        reply: mpsc::Sender<Result<ActivatedClap, String>>,
    },
    Deactivate {
        instance_id: u64,
        processor: StoppedPluginAudioProcessor<NeovegClapHost>,
    },
}

static MANAGER_TX: OnceLock<mpsc::Sender<ManagerCmd>> = OnceLock::new();

fn manager_sender() -> &'static mpsc::Sender<ManagerCmd> {
    MANAGER_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("neoveg-clap-host".to_string())
            .spawn(move || run_clap_manager(rx))
            .expect("failed to spawn neoveg-clap-host thread");
        tx
    })
}

fn host_info() -> HostInfo {
    HostInfo::new(
        "Neoveg",
        "Neoveg Contributors",
        "https://github.com/paulomatos/neoveg",
        env!("CARGO_PKG_VERSION"),
    )
    .expect("valid static CLAP HostInfo strings")
}

fn load_or_get_entry(
    entries: &mut HashMap<PathBuf, PluginEntry>,
    path: &Path,
) -> Result<PluginEntry, String> {
    if let Some(existing) = entries.get(path) {
        return Ok(existing.clone());
    }

    #[cfg(test)]
    if path == Path::new("__builtin_test__") {
        let entry = PluginEntry::load_from_clack::<tests::TestClapEntry>(c"__builtin_test__")
            .map_err(|e| format!("falha ao carregar entrada CLAP de teste: {e}"))?;
        entries.insert(path.to_path_buf(), entry.clone());
        return Ok(entry);
    }

    if !path.exists() {
        return Err(format!("arquivo CLAP nao encontrado: {}", path.display()));
    }

    // SAFETY: `PluginEntry::load` dynamically loads a user-installed `.clap` shared library and
    // invokes its `clap_entry` initialization routine on our dedicated CLAP main thread. Per the
    // CLAP specification and `clack-host`'s safety contract, dynamic library loading cannot verify
    // foreign binary compliance at compile time; we keep the returned `PluginEntry` alive in
    // `entries` for the lifetime of the manager thread so code pages are never unmapped while any
    // `PluginInstance` or `PluginAudioProcessor` derived from it exists.
    let entry = unsafe { PluginEntry::load(path) }
        .map_err(|e| format!("falha ao carregar `{}`: {e}", path.display()))?;
    entries.insert(path.to_path_buf(), entry.clone());
    Ok(entry)
}

fn inspect_instance(
    instance: &mut PluginInstance<NeovegClapHost>,
) -> (Vec<ClapParamInfo>, u32, Vec<usize>, Vec<usize>) {
    let (params_ext, latency_ext, ports_ext) = instance.access_shared_handler(|s| {
        (
            s.params_ext.get().copied().flatten(),
            s.latency_ext.get().copied().flatten(),
            s.ports_ext.get().copied().flatten(),
        )
    });

    let handle = instance.plugin_handle();
    let mut param_infos = Vec::new();
    if let Some(ext) = params_ext {
        let count = ext.count(&handle);
        let mut buf = ParamInfoBuffer::new();
        for idx in 0..count {
            let Some(info) = ext.get_info(&handle, idx, &mut buf) else {
                continue;
            };
            if info
                .flags
                .intersects(ParamInfoFlags::IS_HIDDEN | ParamInfoFlags::IS_READONLY)
            {
                continue;
            }
            let raw_name = String::from_utf8_lossy(info.name).trim().to_string();
            let module = String::from_utf8_lossy(info.module).trim().to_string();
            let display_name = if module.is_empty() {
                if raw_name.is_empty() {
                    format!("Param {}", info.id.get())
                } else {
                    raw_name
                }
            } else if raw_name.is_empty() {
                module
            } else {
                format!("{module} / {raw_name}")
            };
            let min = (info.min_value as f32).clamp(-1.0e9, 1.0e9);
            let max = (info.max_value as f32).clamp(min, 1.0e9);
            let current = ext
                .get_value(&handle, info.id)
                .unwrap_or(info.default_value);
            let default = (current as f32).clamp(min, max);
            param_infos.push(ClapParamInfo {
                id: info.id.get().to_string(),
                clap_id: info.id.get(),
                name: display_name,
                min,
                max,
                default,
                stepped: info.flags.contains(ParamInfoFlags::IS_STEPPED),
                is_bypass: info.flags.contains(ParamInfoFlags::IS_BYPASS),
            });
        }
    }

    let latency = latency_ext.map(|ext| ext.get(&handle)).unwrap_or(0);

    let (in_ports, out_ports) = if let Some(ext) = ports_ext {
        let mut buf = AudioPortInfoBuffer::new();
        let in_count = ext.count(&handle, true);
        let mut ins = Vec::with_capacity(in_count as usize);
        for i in 0..in_count {
            let ch = ext
                .get(&handle, i, true, &mut buf)
                .map(|p| p.channel_count.max(1) as usize)
                .unwrap_or(2);
            ins.push(ch);
        }
        let out_count = ext.count(&handle, false);
        let mut outs = Vec::with_capacity(out_count as usize);
        for i in 0..out_count {
            let ch = ext
                .get(&handle, i, false, &mut buf)
                .map(|p| p.channel_count.max(1) as usize)
                .unwrap_or(2);
            outs.push(ch);
        }
        (ins, outs)
    } else {
        (vec![2], vec![2])
    };

    (param_infos, latency, in_ports, out_ports)
}

fn scan_bundle_on_manager(
    entries: &mut HashMap<PathBuf, PluginEntry>,
    host: &HostInfo,
    path: &Path,
) -> Result<Vec<ClapPluginInfo>, String> {
    let entry = load_or_get_entry(entries, path)?;
    let factory = entry
        .get_plugin_factory()
        .ok_or_else(|| format!("pacote `{}` nao expoe PluginFactory CLAP", path.display()))?;

    let mut out = Vec::new();
    for desc in factory.plugin_descriptors() {
        let Some(id_cstr) = desc.id() else {
            continue;
        };
        let Ok(plugin_id) = id_cstr.to_str() else {
            continue;
        };
        if plugin_id.is_empty() {
            continue;
        }
        let name = desc
            .name()
            .and_then(|s| s.to_str().ok())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(plugin_id)
            .to_string();
        let vendor = desc
            .vendor()
            .and_then(|s| s.to_str().ok())
            .unwrap_or("")
            .to_string();
        let version = desc
            .version()
            .and_then(|s| s.to_str().ok())
            .unwrap_or("")
            .to_string();

        let mut instance = match PluginInstance::<NeovegClapHost>::new(
            |_| NeovegHostShared::default(),
            |_| NeovegHostMainThread::default(),
            &entry,
            id_cstr,
            host,
        ) {
            Ok(inst) => inst,
            Err(err) => {
                tracing::warn!(
                    "[WARN] falha ao instanciar `{plugin_id}` em `{}`: {err}",
                    path.display()
                );
                continue;
            }
        };

        let (params, mut latency, in_ports, out_ports) = inspect_instance(&mut instance);
        let probe_cfg = PluginAudioConfiguration {
            sample_rate: 48_000.0,
            min_frames_count: 1,
            max_frames_count: MAX_BLOCK as u32,
        };
        if let Ok(proc) = instance.activate(|_, _| (), probe_cfg) {
            let latency_ext =
                instance.access_shared_handler(|s| s.latency_ext.get().copied().flatten());
            if let Some(ext) = latency_ext {
                let handle = instance.plugin_handle();
                latency = ext.get(&handle);
            }
            instance.deactivate(proc);
        }

        let audio_inputs = in_ports.first().copied().unwrap_or(0) as u32;
        let audio_outputs = out_ports.first().copied().unwrap_or(0) as u32;
        out.push(ClapPluginInfo {
            kind: format_clap_kind(path, plugin_id),
            plugin_id: plugin_id.to_string(),
            name,
            vendor,
            version,
            path: path.to_path_buf(),
            audio_inputs,
            audio_outputs,
            params,
            latency,
        });
    }
    Ok(out)
}

fn run_clap_manager(rx: mpsc::Receiver<ManagerCmd>) {
    let host = host_info();
    let mut entries: HashMap<PathBuf, PluginEntry> = HashMap::new();
    let mut instances: HashMap<u64, PluginInstance<NeovegClapHost>> = HashMap::new();
    let mut next_id: u64 = 1;

    while let Ok(cmd) = rx.recv() {
        for instance in instances.values_mut() {
            let requested = instance
                .access_shared_handler(|s| s.callback_requested.swap(false, Ordering::Relaxed));
            if requested {
                instance.call_on_main_thread_callback();
            }
        }

        match cmd {
            ManagerCmd::ScanBundle { path, reply } => {
                let res = scan_bundle_on_manager(&mut entries, &host, &path);
                let _ = reply.send(res);
            }
            ManagerCmd::Activate {
                path,
                plugin_id,
                sample_rate,
                max_frames,
                reply,
            } => {
                let res = (|| -> Result<ActivatedClap, String> {
                    let entry = load_or_get_entry(&mut entries, &path)?;
                    let c_id = CString::new(plugin_id.as_bytes())
                        .map_err(|_| "plugin_id CLAP contem byte nulo".to_string())?;
                    let mut instance = PluginInstance::<NeovegClapHost>::new(
                        |_| NeovegHostShared::default(),
                        |_| NeovegHostMainThread::default(),
                        &entry,
                        c_id.as_c_str(),
                        &host,
                    )
                    .map_err(|e| format!("falha ao instanciar `{plugin_id}`: {e}"))?;

                    let (params, pre_latency, in_port_channels, out_port_channels) =
                        inspect_instance(&mut instance);
                    let cfg = PluginAudioConfiguration {
                        sample_rate,
                        min_frames_count: 1,
                        max_frames_count: max_frames.max(1),
                    };
                    let processor = instance
                        .activate(|_, _| (), cfg)
                        .map_err(|e| format!("falha ao ativar `{plugin_id}`: {e}"))?;

                    let latency_ext =
                        instance.access_shared_handler(|s| s.latency_ext.get().copied().flatten());
                    let post_latency = latency_ext
                        .map(|ext| {
                            let handle = instance.plugin_handle();
                            ext.get(&handle)
                        })
                        .unwrap_or(pre_latency);

                    let instance_id = next_id;
                    next_id = next_id.wrapping_add(1);
                    instances.insert(instance_id, instance);

                    Ok(ActivatedClap {
                        instance_id,
                        processor,
                        params,
                        latency: post_latency as usize,
                        in_port_channels,
                        out_port_channels,
                    })
                })();
                let _ = reply.send(res);
            }
            ManagerCmd::Deactivate {
                instance_id,
                processor,
            } => {
                if let Some(mut instance) = instances.remove(&instance_id) {
                    instance.deactivate(processor);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Zero-Allocation Audio-Thread Adapter (`AudioEffect`)
// ---------------------------------------------------------------------------

struct ClapParamSlot {
    id_str: String,
    clap_id: ClapId,
    min: f32,
    max: f32,
    default: f32,
    stepped: bool,
    current: f32,
}

impl ClapParamSlot {
    #[inline]
    fn sanitize(&self, v: f32) -> f32 {
        if v.is_nan() {
            return self.default;
        }
        let clamped = v.clamp(self.min, self.max);
        if self.stepped {
            clamped.round()
        } else {
            clamped
        }
    }
}

struct InputChanIter<'a> {
    chans: std::slice::IterMut<'a, Vec<f32>>,
    frames: usize,
}

impl<'a> Iterator for InputChanIter<'a> {
    type Item = InputChannel<'a, f32>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.chans.next()?;
        Some(InputChannel::variable(&mut buf[..self.frames]))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chans.size_hint()
    }
}

impl ExactSizeIterator for InputChanIter<'_> {}

struct InputPortIter<'a> {
    counts: &'a [usize],
    channels: &'a mut [Vec<f32>],
    frames: usize,
}

impl<'a> Iterator for InputPortIter<'a> {
    type Item = AudioPortBuffer<InputChanIter<'a>, core::array::IntoIter<InputChannel<'a, f64>, 0>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (&count, rest) = self.counts.split_first()?;
        self.counts = rest;
        let all = std::mem::take(&mut self.channels);
        let (head, tail) = all.split_at_mut(count);
        self.channels = tail;
        Some(AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::F32(InputChanIter {
                chans: head.iter_mut(),
                frames: self.frames,
            }),
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.counts.len(), Some(self.counts.len()))
    }
}

impl ExactSizeIterator for InputPortIter<'_> {}

struct OutputChanIter<'a> {
    chans: std::slice::IterMut<'a, Vec<f32>>,
    frames: usize,
}

impl<'a> Iterator for OutputChanIter<'a> {
    type Item = &'a mut [f32];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.chans.next()?;
        Some(&mut buf[..self.frames])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chans.size_hint()
    }
}

impl ExactSizeIterator for OutputChanIter<'_> {}

struct OutputPortIter<'a> {
    counts: &'a [usize],
    channels: &'a mut [Vec<f32>],
    frames: usize,
}

impl<'a> Iterator for OutputPortIter<'a> {
    type Item = AudioPortBuffer<OutputChanIter<'a>, core::array::IntoIter<&'a mut [f64], 0>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (&count, rest) = self.counts.split_first()?;
        self.counts = rest;
        let all = std::mem::take(&mut self.channels);
        let (head, tail) = all.split_at_mut(count);
        self.channels = tail;
        Some(AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::F32(OutputChanIter {
                chans: head.iter_mut(),
                frames: self.frames,
            }),
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.counts.len(), Some(self.counts.len()))
    }
}

impl ExactSizeIterator for OutputPortIter<'_> {}

struct ClapEffect {
    instance_id: u64,
    processor: Option<PluginAudioProcessor<NeovegClapHost>>,
    manager_tx: mpsc::Sender<ManagerCmd>,
    param_slots: Vec<ClapParamSlot>,
    dynamic_pairs: Vec<(String, f32)>,
    pending_events: Vec<ParamValueEvent>,
    out_events: EventBuffer,
    in_port_channels: Vec<usize>,
    out_port_channels: Vec<usize>,
    in_bufs: Vec<Vec<f32>>,
    out_bufs: Vec<Vec<f32>>,
    in_ports: AudioPorts,
    out_ports: AudioPorts,
    latency: usize,
}

impl ClapEffect {
    fn new(activated: ActivatedClap, manager_tx: mpsc::Sender<ManagerCmd>) -> Self {
        let mut param_slots = Vec::with_capacity(activated.params.len());
        let mut dynamic_pairs = Vec::with_capacity(activated.params.len());
        for p in activated.params {
            let Some(clap_id) = ClapId::from_raw(p.clap_id) else {
                continue;
            };
            dynamic_pairs.push((p.id.clone(), p.default));
            param_slots.push(ClapParamSlot {
                id_str: p.id,
                clap_id,
                min: p.min,
                max: p.max,
                default: p.default,
                stepped: p.stepped,
                current: p.default,
            });
        }

        let total_in: usize = activated.in_port_channels.iter().sum();
        let total_out: usize = activated.out_port_channels.iter().sum();
        let in_bufs = (0..total_in).map(|_| vec![0.0_f32; MAX_BLOCK]).collect();
        let out_bufs = (0..total_out).map(|_| vec![0.0_f32; MAX_BLOCK]).collect();
        let in_ports = AudioPorts::with_capacity(total_in, activated.in_port_channels.len());
        let out_ports = AudioPorts::with_capacity(total_out, activated.out_port_channels.len());
        let event_cap = param_slots.len().max(64);

        Self {
            instance_id: activated.instance_id,
            processor: Some(PluginAudioProcessor::from(activated.processor)),
            manager_tx,
            param_slots,
            dynamic_pairs,
            pending_events: Vec::with_capacity(event_cap),
            out_events: EventBuffer::with_capacity(64),
            in_port_channels: activated.in_port_channels,
            out_port_channels: activated.out_port_channels,
            in_bufs,
            out_bufs,
            in_ports,
            out_ports,
            latency: activated.latency,
        }
    }

    #[inline]
    fn enqueue_param_event(&mut self, clap_id: ClapId, value: f32) {
        for ev in &mut self.pending_events {
            if ev.param_id() == Some(clap_id) {
                ev.set_value(f64::from(value));
                return;
            }
        }
        if self.pending_events.len() < self.pending_events.capacity() {
            self.pending_events.push(ParamValueEvent::new(
                0,
                clap_id,
                Pckn::match_all(),
                f64::from(value),
            ));
        }
    }
}

impl Drop for ClapEffect {
    fn drop(&mut self) {
        let Some(mut proc) = self.processor.take() else {
            return;
        };
        proc.ensure_processing_stopped();
        if let PluginAudioProcessor::Stopped(stopped) = proc {
            let _ = self.manager_tx.send(ManagerCmd::Deactivate {
                instance_id: self.instance_id,
                processor: stopped,
            });
        }
    }
}

impl AudioEffect for ClapEffect {
    fn id(&self) -> &'static str {
        "clap"
    }

    fn params(&self) -> &'static [ParamSpec] {
        &[]
    }

    fn dynamic_params(&self) -> &[(String, f32)] {
        &self.dynamic_pairs
    }

    fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(idx) = self.param_slots.iter().position(|s| s.id_str == id) else {
            return false;
        };
        let sanitized = self.param_slots[idx].sanitize(value);
        let clap_id = self.param_slots[idx].clap_id;
        self.param_slots[idx].current = sanitized;
        self.enqueue_param_event(clap_id, sanitized);
        true
    }

    fn param(&self, id: &str) -> Option<f32> {
        self.param_slots
            .iter()
            .find(|s| s.id_str == id)
            .map(|s| s.current)
    }

    fn reset(&mut self) {
        if let Some(proc) = &mut self.processor {
            proc.reset();
        }
        self.pending_events.clear();
        for idx in 0..self.param_slots.len() {
            let clap_id = self.param_slots[idx].clap_id;
            let val = self.param_slots[idx].current;
            if self.pending_events.len() < self.pending_events.capacity() {
                self.pending_events.push(ParamValueEvent::new(
                    0,
                    clap_id,
                    Pckn::match_all(),
                    f64::from(val),
                ));
            }
        }
    }

    fn process(&mut self, channels: &mut [&mut [f32]]) {
        if channels.is_empty() {
            return;
        }
        let total_frames = channels.iter().map(|c| c.len()).min().unwrap_or(0);
        if total_frames == 0 {
            return;
        }
        let Some(proc) = &mut self.processor else {
            return;
        };
        let Ok(started) = proc.ensure_processing_started() else {
            return;
        };

        let main_in_ch = self.in_port_channels.first().copied().unwrap_or(0);
        let main_out_ch = self.out_port_channels.first().copied().unwrap_or(0);

        let mut offset = 0;
        while offset < total_frames {
            let n = (total_frames - offset).min(MAX_BLOCK);

            for buf in &mut self.in_bufs {
                buf[..n].fill(0.0);
            }
            if main_in_ch == 1 {
                if channels.len() >= 2 {
                    for i in 0..n {
                        self.in_bufs[0][i] =
                            0.5 * (channels[0][offset + i] + channels[1][offset + i]);
                    }
                } else {
                    self.in_bufs[0][..n].copy_from_slice(&channels[0][offset..offset + n]);
                }
            } else if main_in_ch >= 2 {
                self.in_bufs[0][..n].copy_from_slice(&channels[0][offset..offset + n]);
                if channels.len() >= 2 {
                    self.in_bufs[1][..n].copy_from_slice(&channels[1][offset..offset + n]);
                } else {
                    self.in_bufs[1][..n].copy_from_slice(&channels[0][offset..offset + n]);
                }
            }

            for buf in &mut self.out_bufs {
                buf[..n].fill(0.0);
            }

            self.out_events.clear();
            let input_events = InputEvents::from_buffer(&self.pending_events);
            let mut output_events = OutputEvents::from_buffer(&mut self.out_events);

            let input_audio = self.in_ports.with_input_buffers(InputPortIter {
                counts: &self.in_port_channels,
                channels: &mut self.in_bufs,
                frames: n,
            });
            let mut output_audio = self.out_ports.with_output_buffers(OutputPortIter {
                counts: &self.out_port_channels,
                channels: &mut self.out_bufs,
                frames: n,
            });

            let ok = started
                .process(
                    &input_audio,
                    &mut output_audio,
                    &input_events,
                    &mut output_events,
                    None,
                    None,
                )
                .is_ok();
            self.pending_events.clear();

            if ok && main_out_ch >= 1 {
                if main_out_ch == 1 {
                    for ch in channels.iter_mut() {
                        ch[offset..offset + n].copy_from_slice(&self.out_bufs[0][..n]);
                    }
                } else {
                    channels[0][offset..offset + n].copy_from_slice(&self.out_bufs[0][..n]);
                    if channels.len() >= 2 {
                        channels[1][offset..offset + n].copy_from_slice(&self.out_bufs[1][..n]);
                    }
                }
            }

            offset += n;
        }
    }

    fn latency(&self) -> usize {
        self.latency
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::fmt::Write as _;
    use std::sync::atomic::AtomicU32;

    use clack_extensions::audio_ports::{
        AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPortsImpl,
    };
    use clack_extensions::latency::PluginLatencyImpl;
    use clack_extensions::params::{
        ParamDisplayWriter, ParamInfo, ParamInfoWriter, PluginAudioProcessorParams,
        PluginMainThreadParams,
    };
    use clack_plugin::events::spaces::CoreEventSpace;
    use clack_plugin::plugin::features::{AUDIO_EFFECT, STEREO};
    use clack_plugin::prelude::{
        Audio, DefaultPluginFactory, Events, HostAudioProcessorHandle, HostMainThreadHandle,
        HostSharedHandle, Plugin, PluginAudioProcessor as ClackPluginProcessorTrait,
        PluginDescriptor, PluginError, PluginExtensions, PluginMainThread, PluginShared, Process,
        SinglePluginEntry,
    };

    const GAIN_PARAM_ID: u32 = 1;
    const TEST_LATENCY: usize = 16;

    pub struct TestGainDelayPlugin;

    pub struct TestShared {
        gain_bits: AtomicU32,
    }

    impl Default for TestShared {
        fn default() -> Self {
            Self {
                gain_bits: AtomicU32::new(1.0_f32.to_bits()),
            }
        }
    }

    impl TestShared {
        fn gain(&self) -> f32 {
            f32::from_bits(self.gain_bits.load(Ordering::Relaxed))
        }

        fn set_gain(&self, g: f32) {
            self.gain_bits
                .store(g.clamp(0.0, 4.0).to_bits(), Ordering::Relaxed);
        }

        fn apply_events(&self, events: &InputEvents) {
            for ev in events {
                if let Some(CoreEventSpace::ParamValue(pv)) = ev.as_core_event() {
                    if pv.param_id() == ClapId::from_raw(GAIN_PARAM_ID) {
                        self.set_gain(pv.value() as f32);
                    }
                }
            }
        }
    }

    impl<'a> PluginShared<'a> for TestShared {}

    pub struct TestMainThread<'a> {
        shared: &'a TestShared,
    }

    impl<'a> PluginMainThread<'a, TestShared> for TestMainThread<'a> {}

    impl PluginAudioPortsImpl for TestMainThread<'_> {
        fn count(&self, _is_input: bool) -> u32 {
            1
        }

        fn get(&self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
            if index == 0 {
                writer.set(&AudioPortInfo {
                    id: ClapId::new(0),
                    name: if is_input { b"Main In" } else { b"Main Out" },
                    channel_count: 2,
                    flags: AudioPortFlags::IS_MAIN,
                    port_type: Some(AudioPortType::STEREO),
                    in_place_pair: None,
                });
            }
        }
    }

    impl PluginLatencyImpl for TestMainThread<'_> {
        fn get(&self) -> u32 {
            TEST_LATENCY as u32
        }
    }

    impl PluginMainThreadParams for TestMainThread<'_> {
        fn count(&self) -> u32 {
            1
        }

        fn get_info(&self, param_index: u32, info: &mut ParamInfoWriter) {
            if param_index == 0 {
                info.set(&ParamInfo {
                    id: ClapId::new(GAIN_PARAM_ID),
                    flags: ParamInfoFlags::IS_AUTOMATABLE,
                    cookie: clack_host::utils::Cookie::empty(),
                    name: b"Gain",
                    module: b"Amp",
                    min_value: 0.0,
                    max_value: 4.0,
                    default_value: 1.0,
                });
            }
        }

        fn get_value(&self, param_id: ClapId) -> Option<f64> {
            (param_id.get() == GAIN_PARAM_ID).then(|| f64::from(self.shared.gain()))
        }

        fn value_to_text(
            &self,
            param_id: ClapId,
            value: f64,
            writer: &mut ParamDisplayWriter,
        ) -> core::fmt::Result {
            if param_id.get() != GAIN_PARAM_ID {
                return Err(core::fmt::Error);
            }
            write!(writer, "{value:.2}x")
        }

        fn text_to_value(&self, param_id: ClapId, text: &CStr) -> Option<f64> {
            (param_id.get() == GAIN_PARAM_ID)
                .then(|| text.to_str().ok()?.parse::<f64>().ok())
                .flatten()
        }

        fn flush(
            &self,
            input_parameter_changes: &InputEvents,
            _output_parameter_changes: &mut OutputEvents,
        ) {
            self.shared.apply_events(input_parameter_changes);
        }
    }

    pub struct TestProcessor<'a> {
        shared: &'a TestShared,
        delay_l: [f32; TEST_LATENCY],
        delay_r: [f32; TEST_LATENCY],
        pos: usize,
    }

    impl PluginAudioProcessorParams for TestProcessor<'_> {
        fn flush(
            &mut self,
            input_parameter_changes: &InputEvents,
            _output_parameter_changes: &mut OutputEvents,
        ) {
            self.shared.apply_events(input_parameter_changes);
        }
    }

    impl<'a> ClackPluginProcessorTrait<'a, TestShared, TestMainThread<'a>> for TestProcessor<'a> {
        fn activate(
            _host: HostAudioProcessorHandle<'a>,
            _main_thread: &TestMainThread<'a>,
            shared: &'a TestShared,
            _audio_config: PluginAudioConfiguration,
        ) -> Result<Self, PluginError> {
            Ok(Self {
                shared,
                delay_l: [0.0; TEST_LATENCY],
                delay_r: [0.0; TEST_LATENCY],
                pos: 0,
            })
        }

        fn reset(&mut self) {
            self.delay_l = [0.0; TEST_LATENCY];
            self.delay_r = [0.0; TEST_LATENCY];
            self.pos = 0;
        }

        fn process(
            &mut self,
            _process: Process,
            mut audio: Audio,
            events: Events,
        ) -> Result<ProcessStatus, PluginError> {
            use clack_plugin::process::audio::ChannelPair;

            self.shared.apply_events(events.input);
            let gain = self.shared.gain();

            let mut port = audio
                .port_pair(0)
                .ok_or(PluginError::Message("missing port pair"))?;
            let channels = port
                .channels()?
                .into_f32()
                .ok_or(PluginError::Message("expected f32"))?;

            let mut iter = channels.into_iter();
            let Some(ChannelPair::InputOutput(in_l, out_l)) = iter.next() else {
                return Err(PluginError::Message("expected stereo L I/O"));
            };
            let Some(ChannelPair::InputOutput(in_r, out_r)) = iter.next() else {
                return Err(PluginError::Message("expected stereo R I/O"));
            };

            let frames = in_l.len().min(out_l.len()).min(in_r.len()).min(out_r.len());
            for i in 0..frames {
                out_l[i] = self.delay_l[self.pos];
                out_r[i] = self.delay_r[self.pos];
                self.delay_l[self.pos] = in_l[i] * gain;
                self.delay_r[self.pos] = in_r[i] * gain;
                self.pos = (self.pos + 1) % TEST_LATENCY;
            }

            Ok(ProcessStatus::Continue)
        }
    }

    impl Plugin for TestGainDelayPlugin {
        type AudioProcessor<'a> = TestProcessor<'a>;
        type Shared<'a> = TestShared;
        type MainThread<'a> = TestMainThread<'a>;

        fn declare_extensions(
            builder: &mut PluginExtensions<Self>,
            _shared: Option<&Self::Shared<'_>>,
        ) {
            builder
                .register::<PluginAudioPorts>()
                .register::<PluginLatency>()
                .register::<PluginParams>();
        }
    }

    impl DefaultPluginFactory for TestGainDelayPlugin {
        fn get_descriptor() -> PluginDescriptor {
            PluginDescriptor::new("com.neoveg.test-gain-delay", "Neoveg Test CLAP Gain")
                .with_vendor("Neoveg")
                .with_version("1.0.0")
                .with_features([AUDIO_EFFECT, STEREO])
        }

        fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
            Ok(TestShared::default())
        }

        fn new_main_thread<'a>(
            _host: HostMainThreadHandle<'a>,
            shared: &'a Self::Shared<'a>,
        ) -> Result<Self::MainThread<'a>, PluginError> {
            Ok(TestMainThread { shared })
        }
    }

    pub type TestClapEntry = SinglePluginEntry<TestGainDelayPlugin>;

    pub const BUILTIN_TEST_KIND: &str = "clap:__builtin_test__::com.neoveg.test-gain-delay";

    #[test]
    fn parse_and_format_clap_kind_preserves_windows_drive_colons() {
        let win_path = Path::new(r"C:\Program Files\Common Files\CLAP\Surge XT.clap");
        let kind = format_clap_kind(win_path, "org.surge-synth-team.surge-xt");
        assert!(is_clap_kind(&kind));
        let (parsed_path, parsed_id) = parse_clap_kind(&kind).expect("must parse");
        assert_eq!(
            parsed_path,
            r"C:\Program Files\Common Files\CLAP\Surge XT.clap"
        );
        assert_eq!(parsed_id, "org.surge-synth-team.surge-xt");
        assert!(parse_clap_kind("compressor").is_none());
        assert!(parse_clap_kind("clap:missing_delim").is_none());
    }

    #[test]
    fn scan_builtin_clap_plugin_discovers_metadata_params_and_latency() {
        let found = scan_clap_file(Path::new("__builtin_test__")).expect("scan test CLAP");
        assert_eq!(found.len(), 1);
        let info = &found[0];
        assert_eq!(info.kind, BUILTIN_TEST_KIND);
        assert_eq!(info.name, "Neoveg Test CLAP Gain");
        assert_eq!(info.vendor, "Neoveg");
        assert_eq!(info.audio_inputs, 2);
        assert_eq!(info.audio_outputs, 2);
        assert_eq!(info.latency, TEST_LATENCY as u32);
        assert_eq!(info.params.len(), 1);
        assert_eq!(info.params[0].id, "1");
        assert_eq!(info.params[0].name, "Amp / Gain");
        assert_eq!(info.params[0].min, 0.0);
        assert_eq!(info.params[0].max, 4.0);
        assert_eq!(info.params[0].default, 1.0);
        assert!(plugin_info(BUILTIN_TEST_KIND).is_some());
    }

    #[test]
    fn clap_effect_processes_stereo_audio_applies_params_and_deactivates_cleanly() {
        let mut fx = create_clap_effect(BUILTIN_TEST_KIND, 48_000.0).expect("instantiate CLAP");
        assert_eq!(fx.latency(), TEST_LATENCY);
        assert_eq!(fx.dynamic_params(), &[("1".to_string(), 1.0)]);
        assert!(fx.set_param("1", 2.5));
        assert!((fx.param("1").unwrap() - 2.5).abs() < 1e-6);

        let mut left = vec![0.4_f32; 64];
        let mut right = vec![-0.2_f32; 64];
        fx.process(&mut [&mut left, &mut right]);

        // First TEST_LATENCY frames are the delay line priming zeros; afterwards 2.5x gain applies.
        for i in 0..TEST_LATENCY {
            assert!(left[i].abs() < 1e-6);
            assert!(right[i].abs() < 1e-6);
        }
        for i in TEST_LATENCY..64 {
            assert!((left[i] - 1.0).abs() < 1e-5, "left[{i}] = {}", left[i]);
            assert!(
                (right[i] - (-0.5)).abs() < 1e-5,
                "right[{i}] = {}",
                right[i]
            );
        }

        // Reset clears the delay line while preserving target parameter values.
        fx.reset();
        let mut l2 = vec![0.2_f32; 32];
        let mut r2 = vec![0.2_f32; 32];
        fx.process(&mut [&mut l2, &mut r2]);
        assert!(l2[0].abs() < 1e-6);
        assert!((l2[TEST_LATENCY] - 0.5).abs() < 1e-5);
    }
}
