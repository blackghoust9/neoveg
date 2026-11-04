//! VST3 audio plug-in host built on the `vst3` bindings crate (MIT OR Apache-2.0, bindings
//! generated from the public headers, no SDK needed to build).
//!
//! A VST3 plug-in is a shared library (`.vst3`, a single file or a bundle folder) that exports
//! `GetPluginFactory`. The factory lists *classes*: an **audio module** (`IComponent` +
//! `IAudioProcessor`, the DSP) and, unless the plug-in is a single object, a separate **edit
//! controller** (`IEditController`, the parameters). [`Vst3Instance`] creates both, connects them,
//! sets a stereo (or mono) layout on the main buses and runs the processor.
//!
//! [`Vst3Effect`] is the [`AudioEffect`] the effect chain runs: every buffer and every object that
//! the processor reads (the parameter-change lists) is built before the audio stream starts, so
//! `process` allocates nothing and takes no lock. Parameters are the controller's normalised values
//! (`0.0..=1.0`) and reach the processor as parameter changes at the start of the next block.
//!
//! Limits of this first version, also listed in `docs/research/ui_audit.md`:
//! * the plug-in runs in the process of the editor (no isolation yet, so a crashing plug-in takes
//!   the program with it);
//! * effects only (a main audio input is required), no MIDI events, no editor window, no state
//!   save and restore, no `IMessage` between the component and its controller;
//! * Windows and Linux; on macOS the bundle entry needs a `CFBundleRef`, so the scan reports it.
//!
//! The kind stored in `TrackEffect::kind` is `"vst3:<path>::<class id>"`, with the class id as 32
//! hexadecimal digits.

use std::cell::Cell;
use std::ffi::{c_char, c_void, OsStr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{Class, ComPtr, ComRef, ComWrapper, Interface};

use crate::dsp::{AudioEffect, ParamSpec};
use crate::fxmixer::MAX_BLOCK;

/// Prefix of `TrackEffect::kind` for hosted VST3 plug-ins (`"vst3:<path>::<class id>"`).
pub const VST3_KIND_PREFIX: &str = "vst3:";

/// `true` when `kind` names a hosted VST3 plug-in rather than a built-in effect or a CLAP one.
#[inline]
pub fn is_vst3_kind(kind: &str) -> bool {
    kind.starts_with(VST3_KIND_PREFIX)
}

/// The `TrackEffect::kind` of the class `class_id` of the module at `path`.
pub fn format_vst3_kind(path: &Path, class_id: &str) -> String {
    format!("{VST3_KIND_PREFIX}{}::{class_id}", path.to_string_lossy())
}

/// Splits `"vst3:<path>::<class id>"` into its two parts (the last `::` splits, so a Windows drive
/// colon in the path is safe).
pub fn parse_vst3_kind(kind: &str) -> Option<(&str, &str)> {
    let rest = kind.strip_prefix(VST3_KIND_PREFIX)?;
    let (path, class_id) = rest.rsplit_once("::")?;
    if path.is_empty() || class_id.len() != 32 || !class_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        None
    } else {
        Some((path, class_id))
    }
}

/// A controllable parameter of a hosted plug-in.
#[derive(Debug, Clone, PartialEq)]
pub struct Vst3ParamInfo {
    /// Key stored in `TrackEffect::params` (the decimal parameter id).
    pub id: String,
    /// The plug-in's own parameter id.
    pub vst_id: u32,
    pub name: String,
    /// Normalised default (the range of every VST3 parameter is `0.0..=1.0`).
    pub default: f32,
    /// `true` for a parameter with a few discrete steps (a switch or a choice).
    pub stepped: bool,
    pub is_bypass: bool,
}

/// What a scan learns about one effect class of a `.vst3` module.
#[derive(Debug, Clone, PartialEq)]
pub struct Vst3PluginInfo {
    /// `"vst3:<path>::<class id>"`.
    pub kind: String,
    /// The class id as 32 hexadecimal digits.
    pub class_id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: PathBuf,
    /// Channels of the main audio input and output.
    pub audio_inputs: u32,
    pub audio_outputs: u32,
    pub params: Vec<Vst3ParamInfo>,
    /// Latency in samples reported while probing at 48 kHz.
    pub latency: u32,
}

// ---- text helpers ------------------------------------------------------------------------------------

fn c_text(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn wide_text(chars: &[TChar]) -> String {
    let units: Vec<u16> = chars.iter().take_while(|c| **c != 0).copied().collect();
    String::from_utf16_lossy(&units)
}

fn write_wide(src: &str, dst: &mut [TChar]) {
    let mut len = 0;
    for (unit, slot) in src.encode_utf16().zip(dst.iter_mut()) {
        *slot = unit as TChar;
        len += 1;
    }
    if len < dst.len() {
        dst[len] = 0;
    } else if let Some(last) = dst.last_mut() {
        *last = 0;
    }
}

fn class_id_text(cid: &TUID) -> String {
    cid.iter().map(|b| format!("{:02X}", *b as u8)).collect()
}

fn class_id_from_text(text: &str) -> Option<TUID> {
    if text.len() != 32 {
        return None;
    }
    let mut tuid = [0 as c_char; 16];
    for (i, slot) in tuid.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()? as c_char;
    }
    Some(tuid)
}

fn iid_of<I: Interface>() -> TUID {
    let mut tuid = [0 as c_char; 16];
    for (slot, byte) in tuid.iter_mut().zip(I::IID.iter()) {
        *slot = *byte as c_char;
    }
    tuid
}

// ---- host objects the plug-in talks to ---------------------------------------------------------------

/// `IHostApplication`: the plug-in may ask the name of the host.
struct HostApp;

impl Class for HostApp {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        write_wide("Neoveg", &mut *name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        _cid: *mut TUID,
        _iid: *mut TUID,
        _obj: *mut *mut c_void,
    ) -> tresult {
        // No `IMessage` and no `IAttributeList` yet
        kNotImplemented
    }
}

/// `IComponentHandler`: the controller reports edits made in its own window; there is none yet, so
/// everything is accepted and ignored.
struct Handler;

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, _id: ParamID, _value: ParamValue) -> tresult {
        kResultOk
    }

    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, _flags: int32) -> tresult {
        kResultOk
    }
}

/// One entry of a parameter-change list: a parameter and its new normalised value, at sample 0.
struct ParamQueue {
    id: Cell<u32>,
    value: Cell<f64>,
    /// An output queue swallows what the plug-in writes.
    sink: bool,
}

impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.get()
    }

    unsafe fn getPointCount(&self) -> int32 {
        i32::from(!self.sink)
    }

    unsafe fn getPoint(
        &self,
        index: int32,
        sample_offset: *mut int32,
        value: *mut ParamValue,
    ) -> tresult {
        if index == 0 && !self.sink {
            *sample_offset = 0;
            *value = self.value.get();
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn addPoint(
        &self,
        _sample_offset: int32,
        _value: ParamValue,
        index: *mut int32,
    ) -> tresult {
        if !index.is_null() {
            *index = 0;
        }
        if self.sink {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

/// The most parameter changes sent to the processor in one block; the rest wait for the next one.
const MAX_CHANGES: usize = 32;

/// `IParameterChanges` over a fixed set of queues built before the stream starts.
struct ParamChanges {
    queues: Vec<ComWrapper<ParamQueue>>,
    count: Cell<i32>,
    sink: bool,
}

impl ParamChanges {
    fn new(sink: bool) -> ComWrapper<ParamChanges> {
        let queues = (0..if sink { 1 } else { MAX_CHANGES })
            .map(|_| {
                ComWrapper::new(ParamQueue {
                    id: Cell::new(0),
                    value: Cell::new(0.0),
                    sink,
                })
            })
            .collect();
        ComWrapper::new(ParamChanges {
            queues,
            count: Cell::new(0),
            sink,
        })
    }
}

impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.count.get()
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        if index < 0 || index >= self.count.get() {
            return std::ptr::null_mut();
        }
        self.queues
            .get(index as usize)
            .and_then(|q| q.as_com_ref::<IParamValueQueue>())
            .map_or(std::ptr::null_mut(), |q| q.as_ptr())
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        if !self.sink {
            return std::ptr::null_mut();
        }
        if !index.is_null() {
            *index = 0;
        }
        self.queues[0]
            .as_com_ref::<IParamValueQueue>()
            .map_or(std::ptr::null_mut(), |q| {
                if !id.is_null() {
                    self.queues[0].id.set(*id);
                }
                q.as_ptr()
            })
    }
}

// ---- the module (the shared library) -------------------------------------------------------------------

type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;
type EntryFn = unsafe extern "system" fn() -> bool;

/// A loaded `.vst3` library and its factory. The library stays loaded while any plug-in made from
/// it lives.
pub struct Vst3Module {
    factory: ComPtr<IPluginFactory>,
    exit: Option<EntryFn>,
    // Declared last: the library must outlive every COM pointer above
    _library: Option<libloading::Library>,
}

// The factory is thread safe by the VST3 rules (`IPluginFactory` is called from any thread)
unsafe impl Send for Vst3Module {}
unsafe impl Sync for Vst3Module {}

impl Drop for Vst3Module {
    fn drop(&mut self) {
        if let Some(exit) = self.exit {
            unsafe {
                exit();
            }
        }
    }
}

/// The file inside a `.vst3` that holds the code: the file itself, or the binary of the bundle.
fn module_binary(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| "nome de pacote VST3 invalido".to_string())?;
    let (folder, file) = if cfg!(target_os = "windows") {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64-win"
        } else {
            "x86_64-win"
        };
        (arch, format!("{stem}.vst3"))
    } else if cfg!(target_os = "linux") {
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64-linux"
        } else {
            "x86_64-linux"
        };
        (arch, format!("{stem}.so"))
    } else {
        return Err("VST3 ainda nao e suportado neste sistema (precisa de um CFBundleRef)".into());
    };
    let binary = path.join("Contents").join(folder).join(file);
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(format!("sem binario VST3 em `{}`", binary.display()))
    }
}

impl Vst3Module {
    /// Loads the library of `path` (a `.vst3` file or bundle) and gets its factory.
    pub fn load(path: &Path) -> Result<Arc<Vst3Module>, String> {
        let binary = module_binary(path)?;
        unsafe {
            let library = libloading::Library::new(&binary)
                .map_err(|e| format!("falha ao carregar `{}`: {e}", binary.display()))?;
            // The entry and exit points are optional on Windows and required on Linux
            let (enter, exit): (&[u8], &[u8]) = if cfg!(target_os = "windows") {
                (b"InitDll\0", b"ExitDll\0")
            } else {
                (b"ModuleEntry\0", b"ModuleExit\0")
            };
            if cfg!(target_os = "windows") {
                if let Ok(init) = library.get::<EntryFn>(enter) {
                    if !init() {
                        return Err("o plug-in recusou a inicializacao (InitDll)".into());
                    }
                }
            } else if let Ok(init) =
                library.get::<unsafe extern "system" fn(*mut c_void) -> bool>(enter)
            {
                // Linux passes the handle of the library; the dynamic loader keeps it alive
                if !init(std::ptr::null_mut()) {
                    return Err("o plug-in recusou a inicializacao (ModuleEntry)".into());
                }
            }
            let exit_fn = library.get::<EntryFn>(exit).ok().map(|f| *f);
            let get_factory = library
                .get::<GetFactory>(b"GetPluginFactory\0")
                .map_err(|_| "o arquivo nao exporta GetPluginFactory".to_string())?;
            let factory = ComPtr::from_raw(get_factory())
                .ok_or_else(|| "GetPluginFactory devolveu nulo".to_string())?;
            Ok(Arc::new(Vst3Module {
                factory,
                exit: exit_fn,
                _library: Some(library),
            }))
        }
    }

    /// A module over a factory that already exists in memory (tests, built-in plug-ins).
    pub fn from_factory(factory: ComPtr<IPluginFactory>) -> Arc<Vst3Module> {
        Arc::new(Vst3Module {
            factory,
            exit: None,
            _library: None,
        })
    }

    /// The audio effect classes of the module: `(class id, name, vendor, version)`.
    fn effect_classes(&self) -> Vec<(TUID, String, String, String)> {
        let mut found = Vec::new();
        unsafe {
            let count = self.factory.countClasses();
            let factory2 = self.factory.cast::<IPluginFactory2>();
            for index in 0..count {
                let mut info: PClassInfo2 = std::mem::zeroed();
                let (cid, category, name, sub, vendor, version) = if let Some(f2) = &factory2 {
                    if f2.getClassInfo2(index, &mut info) != kResultOk {
                        continue;
                    }
                    (
                        info.cid,
                        c_text(&info.category),
                        c_text(&info.name),
                        c_text(&info.subCategories),
                        c_text(&info.vendor),
                        c_text(&info.version),
                    )
                } else {
                    let mut plain: PClassInfo = std::mem::zeroed();
                    if self.factory.getClassInfo(index, &mut plain) != kResultOk {
                        continue;
                    }
                    (
                        plain.cid,
                        c_text(&plain.category),
                        c_text(&plain.name),
                        String::new(),
                        String::new(),
                        String::new(),
                    )
                };
                if category != "Audio Module Class" || sub.contains("Instrument") {
                    continue;
                }
                found.push((cid, name, vendor, version));
            }
        }
        found
    }
}

// ---- one plug-in instance ------------------------------------------------------------------------------

/// The component, its processor and its controller, set up for a stereo or mono effect.
pub struct Vst3Instance {
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: Option<ComPtr<IEditController>>,
    /// The controller is a different object from the component (so both are initialised, connected
    /// and terminated separately).
    separate_controller: bool,
    connections: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    _host: ComWrapper<HostApp>,
    _handler: ComWrapper<Handler>,
    in_channels: usize,
    out_channels: usize,
    processing: bool,
    // Declared last: the library must outlive every COM pointer above
    _module: Arc<Vst3Module>,
}

unsafe impl Send for Vst3Instance {}

fn channels_of(arrangement: SpeakerArrangement) -> usize {
    arrangement.count_ones() as usize
}

impl Vst3Instance {
    /// Creates the class `class_id`, connects its controller and activates it at `sample_rate`.
    pub fn open(
        module: &Arc<Vst3Module>,
        class_id: &TUID,
        sample_rate: f64,
    ) -> Result<Vst3Instance, String> {
        unsafe {
            let host = ComWrapper::new(HostApp);
            let host_unknown = host
                .to_com_ptr::<FUnknown>()
                .ok_or("falha ao criar o contexto do host")?;
            let handler = ComWrapper::new(Handler);

            let component: ComPtr<IComponent> = create::<IComponent>(&module.factory, class_id)?;
            if component.initialize(host_unknown.as_ptr()) != kResultOk {
                return Err("o plug-in recusou initialize".into());
            }
            let processor = component
                .cast::<IAudioProcessor>()
                .ok_or("o componente nao e um processador de audio")?;

            // Controller: the component itself, or the class it names
            let mut separate_controller = false;
            let mut controller = component.cast::<IEditController>();
            if controller.is_none() {
                let mut cid: TUID = [0; 16];
                if component.getControllerClassId(&mut cid) == kResultOk {
                    if let Ok(c) = create::<IEditController>(&module.factory, &cid) {
                        if c.initialize(host_unknown.as_ptr()) == kResultOk {
                            separate_controller = true;
                            controller = Some(c);
                        }
                    }
                }
            }
            let mut connections = None;
            if let Some(c) = &controller {
                if let Some(handler_ptr) = handler.to_com_ptr::<IComponentHandler>() {
                    c.setComponentHandler(handler_ptr.as_ptr());
                }
                if separate_controller {
                    if let (Some(a), Some(b)) = (
                        component.cast::<IConnectionPoint>(),
                        c.cast::<IConnectionPoint>(),
                    ) {
                        a.connect(b.as_ptr());
                        b.connect(a.as_ptr());
                        connections = Some((a, b));
                    }
                }
            }

            // Main buses only: stereo in and out when the plug-in accepts it
            let inputs = component.getBusCount(
                MediaTypes_::kAudio as MediaType,
                BusDirections_::kInput as BusDirection,
            );
            let outputs = component.getBusCount(
                MediaTypes_::kAudio as MediaType,
                BusDirections_::kOutput as BusDirection,
            );
            if inputs < 1 || outputs < 1 {
                return Err("o plug-in nao tem entrada e saida de audio (nao e um efeito)".into());
            }
            let mut in_arr = vec![SpeakerArr::kStereo; inputs as usize];
            let mut out_arr = vec![SpeakerArr::kStereo; outputs as usize];
            for (list, dir) in [
                (&mut in_arr, BusDirections_::kInput),
                (&mut out_arr, BusDirections_::kOutput),
            ] {
                for (i, slot) in list.iter_mut().enumerate() {
                    let mut current: SpeakerArrangement = SpeakerArr::kStereo;
                    if processor.getBusArrangement(dir as BusDirection, i as i32, &mut current)
                        == kResultOk
                    {
                        *slot = current;
                    }
                }
            }
            // Ask for stereo on the main buses; when it is refused the plug-in's own layout stays
            let mut want_in = in_arr.clone();
            let mut want_out = out_arr.clone();
            want_in[0] = SpeakerArr::kStereo;
            want_out[0] = SpeakerArr::kStereo;
            if processor.setBusArrangements(
                want_in.as_mut_ptr(),
                inputs,
                want_out.as_mut_ptr(),
                outputs,
            ) == kResultTrue
            {
                in_arr = want_in;
                out_arr = want_out;
            }
            for (i, dir, count) in [
                (0, BusDirections_::kInput, inputs),
                (0, BusDirections_::kOutput, outputs),
            ] {
                for bus in 0..count {
                    component.activateBus(
                        MediaTypes_::kAudio as MediaType,
                        dir as BusDirection,
                        bus,
                        u8::from(bus == i),
                    );
                }
            }
            let (in_channels, out_channels) = (channels_of(in_arr[0]), channels_of(out_arr[0]));
            if !(1..=2).contains(&in_channels) || !(1..=2).contains(&out_channels) {
                return Err(format!(
                    "disposicao de canais nao suportada ({in_channels} entradas, {out_channels} saidas)"
                ));
            }

            if processor.canProcessSampleSize(SymbolicSampleSizes_::kSample32) != kResultOk {
                return Err("o plug-in nao processa 32 bits".into());
            }
            let mut setup = ProcessSetup {
                processMode: ProcessModes_::kRealtime,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32,
                maxSamplesPerBlock: MAX_BLOCK as i32,
                sampleRate: sample_rate,
            };
            if processor.setupProcessing(&mut setup) != kResultOk {
                return Err("o plug-in recusou setupProcessing".into());
            }
            if component.setActive(1) != kResultOk {
                return Err("o plug-in recusou setActive".into());
            }
            let processing = processor.setProcessing(1) == kResultOk;

            Ok(Vst3Instance {
                component,
                processor,
                controller,
                separate_controller,
                connections,
                _host: host,
                _handler: handler,
                in_channels,
                out_channels,
                processing,
                _module: Arc::clone(module),
            })
        }
    }

    /// The parameters the controller shows (hidden and read-only ones left out).
    pub fn parameters(&self) -> Vec<Vst3ParamInfo> {
        let Some(controller) = &self.controller else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            for index in 0..controller.getParameterCount() {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) != kResultOk {
                    continue;
                }
                let flags = info.flags;
                if flags & ParameterInfo_::ParameterFlags_::kIsHidden != 0
                    || flags & ParameterInfo_::ParameterFlags_::kIsReadOnly != 0
                {
                    continue;
                }
                let title = wide_text(&info.title);
                out.push(Vst3ParamInfo {
                    id: info.id.to_string(),
                    vst_id: info.id,
                    name: if title.is_empty() {
                        wide_text(&info.shortTitle)
                    } else {
                        title
                    },
                    default: info.defaultNormalizedValue.clamp(0.0, 1.0) as f32,
                    stepped: info.stepCount > 0,
                    is_bypass: flags & ParameterInfo_::ParameterFlags_::kIsBypass != 0,
                });
            }
        }
        out
    }

    /// Latency in samples the processor reports now.
    pub fn latency(&self) -> u32 {
        unsafe { self.processor.getLatencySamples() }
    }

    pub fn in_channels(&self) -> usize {
        self.in_channels
    }

    pub fn out_channels(&self) -> usize {
        self.out_channels
    }
}

impl Drop for Vst3Instance {
    fn drop(&mut self) {
        unsafe {
            if self.processing {
                self.processor.setProcessing(0);
            }
            self.component.setActive(0);
            if let Some((a, b)) = &self.connections {
                a.disconnect(b.as_ptr());
                b.disconnect(a.as_ptr());
            }
            if self.separate_controller {
                if let Some(c) = &self.controller {
                    c.terminate();
                }
            }
            self.component.terminate();
        }
    }
}

/// Creates the object of class `cid` as interface `I`.
unsafe fn create<I: Interface>(
    factory: &ComPtr<IPluginFactory>,
    cid: &TUID,
) -> Result<ComPtr<I>, String> {
    let iid = iid_of::<I>();
    let mut object: *mut c_void = std::ptr::null_mut();
    let result = factory.createInstance(
        cid.as_ptr() as FIDString,
        iid.as_ptr() as FIDString,
        &mut object,
    );
    if result != kResultOk {
        return Err(format!("createInstance falhou ({result})"));
    }
    ComPtr::from_raw(object as *mut I).ok_or_else(|| "createInstance devolveu nulo".to_string())
}

// ---- discovery ---------------------------------------------------------------------------------------

static REGISTRY: OnceLock<RwLock<Vec<Vst3PluginInfo>>> = OnceLock::new();

fn registry() -> &'static RwLock<Vec<Vst3PluginInfo>> {
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// What the scans found so far, without touching the disk.
pub fn cached_plugins() -> Vec<Vst3PluginInfo> {
    registry().read().map(|g| g.clone()).unwrap_or_default()
}

/// The info of `kind`, probing its module once when it has not been scanned (a saved project that
/// is opened before the first scan).
pub fn plugin_info(kind: &str) -> Option<Vst3PluginInfo> {
    if let Ok(guard) = registry().read() {
        if let Some(found) = guard.iter().find(|p| p.kind == kind) {
            return Some(found.clone());
        }
    }
    let (path, _) = parse_vst3_kind(kind)?;
    scan_vst3_file(Path::new(path)).ok()?;
    registry()
        .read()
        .ok()?
        .iter()
        .find(|p| p.kind == kind)
        .cloned()
}

fn merge_into_registry(found: &[Vst3PluginInfo]) {
    if let Ok(mut guard) = registry().write() {
        for info in found {
            match guard.iter_mut().find(|p| p.kind == info.kind) {
                Some(slot) => *slot = info.clone(),
                None => guard.push(info.clone()),
            }
        }
    }
}

/// Probes the effect classes of a loaded module (each one is created, read and released).
pub fn probe_module(module: &Arc<Vst3Module>, path: &Path) -> Vec<Vst3PluginInfo> {
    let mut found = Vec::new();
    for (cid, name, vendor, version) in module.effect_classes() {
        match Vst3Instance::open(module, &cid, 48_000.0) {
            Ok(instance) => {
                let class_id = class_id_text(&cid);
                found.push(Vst3PluginInfo {
                    kind: format_vst3_kind(path, &class_id),
                    class_id,
                    name,
                    vendor,
                    version,
                    path: path.to_path_buf(),
                    audio_inputs: instance.in_channels() as u32,
                    audio_outputs: instance.out_channels() as u32,
                    params: instance.parameters(),
                    latency: instance.latency(),
                });
            }
            Err(err) => tracing::warn!(
                "[WARN] classe VST3 `{name}` de `{}` ignorada: {err}",
                path.display()
            ),
        }
    }
    found
}

/// Loads `path` (a `.vst3` file or bundle) and records its effects.
pub fn scan_vst3_file(path: &Path) -> Result<Vec<Vst3PluginInfo>, String> {
    let module = Vst3Module::load(path)?;
    let found = probe_module(&module, path);
    merge_into_registry(&found);
    Ok(found)
}

/// The folders the platform and `VST3_PATH` keep plug-ins in.
pub fn default_vst3_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(extra) = std::env::var_os("VST3_PATH") {
        dirs.extend(std::env::split_paths(&extra));
    }
    if cfg!(target_os = "windows") {
        if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
            dirs.push(PathBuf::from(common).join("VST3"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(
                PathBuf::from(local)
                    .join("Programs")
                    .join("Common")
                    .join("VST3"),
            );
        }
    } else if cfg!(target_os = "linux") {
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".vst3"));
        }
        dirs.push(PathBuf::from("/usr/lib/vst3"));
        dirs.push(PathBuf::from("/usr/local/lib/vst3"));
    }
    dirs
}

fn collect_modules(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|e| e.eq_ignore_ascii_case("vst3"))
        {
            out.push(path);
        } else if path.is_dir() && depth < 2 {
            collect_modules(&path, depth + 1, out);
        }
    }
}

/// Walks [`default_vst3_dirs`], scans every module and returns what is known. Run it on a worker
/// thread, not the UI thread: it loads libraries.
pub fn scan_system_plugins() -> Vec<Vst3PluginInfo> {
    let mut modules = Vec::new();
    for dir in default_vst3_dirs() {
        collect_modules(&dir, 0, &mut modules);
    }
    for module in modules {
        if let Err(err) = scan_vst3_file(&module) {
            tracing::warn!("[WARN] falha ao inspecionar `{}`: {err}", module.display());
        }
    }
    cached_plugins()
}

// ---- the effect ----------------------------------------------------------------------------------------

struct ParamSlot {
    id_str: String,
    vst_id: u32,
    current: f32,
}

/// A hosted VST3 plug-in as an [`AudioEffect`].
pub struct Vst3Effect {
    instance: Vst3Instance,
    slots: Vec<ParamSlot>,
    dynamic_pairs: Vec<(String, f32)>,
    in_bufs: [Vec<f32>; 2],
    out_bufs: [Vec<f32>; 2],
    input_changes: ComWrapper<ParamChanges>,
    output_changes: ComWrapper<ParamChanges>,
    /// Slots whose value changed since the last block.
    pending: Vec<usize>,
    latency: usize,
}

unsafe impl Send for Vst3Effect {}

impl Vst3Effect {
    pub fn new(instance: Vst3Instance, params: &[Vst3ParamInfo]) -> Self {
        let slots: Vec<ParamSlot> = params
            .iter()
            .map(|p| ParamSlot {
                id_str: p.id.clone(),
                vst_id: p.vst_id,
                current: p.default,
            })
            .collect();
        let dynamic_pairs = params.iter().map(|p| (p.id.clone(), p.default)).collect();
        let latency = instance.latency() as usize;
        Self {
            instance,
            slots,
            dynamic_pairs,
            in_bufs: [vec![0.0; MAX_BLOCK], vec![0.0; MAX_BLOCK]],
            out_bufs: [vec![0.0; MAX_BLOCK], vec![0.0; MAX_BLOCK]],
            input_changes: ParamChanges::new(false),
            output_changes: ParamChanges::new(true),
            pending: Vec::with_capacity(MAX_CHANGES),
            latency,
        }
    }
}

/// Instantiates and activates the hosted plug-in of `kind` as an [`AudioEffect`].
pub fn create_vst3_effect(kind: &str, sample_rate: f32) -> Option<Box<dyn AudioEffect>> {
    let (path, class_text) = parse_vst3_kind(kind)?;
    let cid = class_id_from_text(class_text)?;
    let module = match Vst3Module::load(Path::new(path)) {
        Ok(m) => m,
        Err(err) => {
            tracing::warn!("[WARN] falha ao carregar o plug-in VST3 `{kind}`: {err}");
            return None;
        }
    };
    effect_from_module(&module, &cid, sample_rate)
}

/// The effect for class `cid` of an already loaded module.
pub fn effect_from_module(
    module: &Arc<Vst3Module>,
    cid: &TUID,
    sample_rate: f32,
) -> Option<Box<dyn AudioEffect>> {
    match Vst3Instance::open(module, cid, f64::from(sample_rate.max(8_000.0))) {
        Ok(instance) => {
            let params = instance.parameters();
            Some(Box::new(Vst3Effect::new(instance, &params)))
        }
        Err(err) => {
            tracing::warn!("[WARN] falha ao ativar o plug-in VST3: {err}");
            None
        }
    }
}

impl AudioEffect for Vst3Effect {
    fn id(&self) -> &'static str {
        "vst3"
    }

    fn params(&self) -> &'static [ParamSpec] {
        &[]
    }

    fn dynamic_params(&self) -> &[(String, f32)] {
        &self.dynamic_pairs
    }

    fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(idx) = self.slots.iter().position(|s| s.id_str == id) else {
            return false;
        };
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.slots[idx].current = value;
        if self.pending.len() < MAX_CHANGES && !self.pending.contains(&idx) {
            self.pending.push(idx);
        }
        true
    }

    fn param(&self, id: &str) -> Option<f32> {
        self.slots
            .iter()
            .find(|s| s.id_str == id)
            .map(|s| s.current)
    }

    fn reset(&mut self) {
        unsafe {
            if self.instance.processing {
                self.instance.processor.setProcessing(0);
                self.instance.processor.setProcessing(1);
            }
        }
        self.pending.clear();
        for idx in 0..self.slots.len().min(MAX_CHANGES) {
            self.pending.push(idx);
        }
    }

    fn process(&mut self, channels: &mut [&mut [f32]]) {
        if channels.is_empty() {
            return;
        }
        let total = channels.iter().map(|c| c.len()).min().unwrap_or(0);
        if total == 0 {
            return;
        }
        let (in_ch, out_ch) = (self.instance.in_channels, self.instance.out_channels);
        let mut offset = 0;
        while offset < total {
            let n = (total - offset).min(MAX_BLOCK);

            // The input: the mix for a mono plug-in, both channels (or the one twice) for stereo
            if in_ch == 1 {
                if channels.len() >= 2 {
                    for i in 0..n {
                        self.in_bufs[0][i] =
                            0.5 * (channels[0][offset + i] + channels[1][offset + i]);
                    }
                } else {
                    self.in_bufs[0][..n].copy_from_slice(&channels[0][offset..offset + n]);
                }
            } else {
                self.in_bufs[0][..n].copy_from_slice(&channels[0][offset..offset + n]);
                let second = if channels.len() >= 2 { 1 } else { 0 };
                self.in_bufs[1][..n].copy_from_slice(&channels[second][offset..offset + n]);
            }
            for buf in &mut self.out_bufs {
                buf[..n].fill(0.0);
            }

            // The parameters that changed since the last block
            let changes = &*self.input_changes;
            let mut count = 0;
            for &idx in &self.pending {
                if let Some(queue) = changes.queues.get(count) {
                    queue.id.set(self.slots[idx].vst_id);
                    queue.value.set(f64::from(self.slots[idx].current));
                    count += 1;
                }
            }
            changes.count.set(count as i32);
            self.pending.clear();

            let mut in_ptrs = [self.in_bufs[0].as_mut_ptr(), self.in_bufs[1].as_mut_ptr()];
            let mut out_ptrs = [self.out_bufs[0].as_mut_ptr(), self.out_bufs[1].as_mut_ptr()];
            let mut input = AudioBusBuffers {
                numChannels: in_ch as i32,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: in_ptrs.as_mut_ptr(),
                },
            };
            let mut output = AudioBusBuffers {
                numChannels: out_ch as i32,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 {
                    channelBuffers32: out_ptrs.as_mut_ptr(),
                },
            };
            let in_changes = self.input_changes.as_com_ref::<IParameterChanges>();
            let out_changes = self.output_changes.as_com_ref::<IParameterChanges>();
            let mut data = ProcessData {
                processMode: ProcessModes_::kRealtime,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32,
                numSamples: n as i32,
                numInputs: 1,
                numOutputs: 1,
                inputs: &mut input,
                outputs: &mut output,
                inputParameterChanges: in_changes.map_or(std::ptr::null_mut(), |c| c.as_ptr()),
                outputParameterChanges: out_changes.map_or(std::ptr::null_mut(), |c| c.as_ptr()),
                inputEvents: std::ptr::null_mut(),
                outputEvents: std::ptr::null_mut(),
                processContext: std::ptr::null_mut(),
            };
            let ok = unsafe { self.instance.processor.process(&mut data) } == kResultOk;
            self.input_changes.count.set(0);

            if ok {
                if out_ch == 1 {
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

// Re-exported so the unused-import lint stays quiet when the module has no ComRef user left
#[allow(dead_code)]
type _KeepComRef<'a> = ComRef<'a, IComponent>;

#[cfg(test)]
#[allow(clippy::all)]
mod test_gain;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vst3_kind_round_trips_and_rejects_what_is_not_one() {
        let path = Path::new(r"C:\Program Files\Common Files\VST3\Gain.vst3");
        let id = "6E33225254224A00AA69301AF318797D";
        let kind = format_vst3_kind(path, id);
        assert!(is_vst3_kind(&kind));
        assert_eq!(parse_vst3_kind(&kind), Some((path.to_str().unwrap(), id)));
        assert!(parse_vst3_kind("vst3:x.vst3::short").is_none());
        assert!(parse_vst3_kind("clap:x.clap::6E33225254224A00AA69301AF318797D").is_none());
        assert!(!is_vst3_kind("clap:a::b"));
        let cid = class_id_from_text(id).unwrap();
        assert_eq!(class_id_text(&cid), id);
    }

    #[test]
    fn the_gain_plug_in_is_found_probed_and_runs_as_an_effect() {
        let module = Vst3Module::from_factory(test_gain::factory());
        let found = probe_module(&module, Path::new("gain.vst3"));
        assert_eq!(found.len(), 1, "one audio module class");
        let info = &found[0];
        assert_eq!((info.audio_inputs, info.audio_outputs), (2, 2));
        assert_eq!(info.params.len(), 1);
        assert_eq!(info.params[0].name, "Gain");
        assert_eq!(info.params[0].default, 1.0);
        assert_eq!(info.latency, 0);

        let cid = class_id_from_text(&info.class_id).unwrap();
        let mut effect = effect_from_module(&module, &cid, 48_000.0).expect("effect");
        assert_eq!(effect.id(), "vst3");
        assert_eq!(effect.dynamic_params().len(), 1);
        let key = effect.dynamic_params()[0].0.clone();

        let mut left = vec![0.5f32; 100];
        let mut right = vec![-0.25f32; 100];
        effect.process(&mut [&mut left, &mut right]);
        assert!(
            (left[10] - 0.5).abs() < 1e-6 && (right[10] + 0.25).abs() < 1e-6,
            "gain 1 passes"
        );

        // Half gain arrives as a parameter change before the next block
        assert!(effect.set_param(&key, 0.5));
        assert_eq!(effect.param(&key), Some(0.5));
        let mut left = vec![0.5f32; 100];
        let mut right = vec![-0.25f32; 100];
        effect.process(&mut [&mut left, &mut right]);
        assert!((left[0] - 0.25).abs() < 1e-6 && (right[99] + 0.125).abs() < 1e-6);
        // And it stays
        let mut left = vec![0.5f32; 100];
        let mut right = vec![0.5f32; 100];
        effect.process(&mut [&mut left, &mut right]);
        assert!((left[50] - 0.25).abs() < 1e-6);

        // A mono signal is used on both channels; an unknown id is refused; zero length is fine
        let mut only = vec![0.8f32; 64];
        effect.process(&mut [&mut only]);
        assert!((only[0] - 0.4).abs() < 1e-6);
        assert!(!effect.set_param("nope", 0.3));
        effect.process(&mut [&mut [], &mut []]);
        // Blocks longer than the internal block are cut and joined again
        let mut long_l = vec![1.0f32; MAX_BLOCK * 2 + 17];
        let mut long_r = vec![1.0f32; MAX_BLOCK * 2 + 17];
        effect.process(&mut [&mut long_l, &mut long_r]);
        assert!(long_l.iter().chain(&long_r).all(|v| (v - 0.5).abs() < 1e-6));
        effect.reset();
        assert!(effect.latency() == 0);
    }

    #[test]
    fn a_file_that_is_not_a_plug_in_is_an_error_not_a_crash() {
        let dir = std::env::temp_dir().join(format!("neoveg_vst3_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("fake.vst3");
        std::fs::write(&fake, b"not a library").unwrap();
        assert!(Vst3Module::load(&fake).is_err());
        assert!(Vst3Module::load(&dir.join("missing.vst3")).is_err());
        assert!(create_vst3_effect(
            &format_vst3_kind(&fake, "6E33225254224A00AA69301AF318797D"),
            48_000.0
        )
        .is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
