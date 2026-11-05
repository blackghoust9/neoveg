//! VST3 plug-ins in a process of their own.
//!
//! A VST3 plug-in is a native library written by someone else. Hosted in the editor's process
//! ([`crate::vst3_host`]) it can take the program down with the user's work in it, and loading it
//! is enough to run its code. Here the editor never loads one: `neoveg_vst3_host` (a second program
//! built from this crate) opens the plug-in and answers over its pipes ([`crate::vst3_wire`]), and
//! [`IsolatedEffect`] is the [`AudioEffect`] the effect chain runs in its place.
//!
//! **Latency.** The proxy asks for each block and waits for the answer, so the transport adds no
//! delay of its own: the latency of the effect is the one the plug-in reports, and the
//! [`BlockMixer`](crate::fxmixer::BlockMixer) compensates it like that of any other effect (the
//! sum along a track, a bus and the master chain; the other tracks are delayed to match).
//!
//! **When the plug-in fails.** If the child dies, does not answer in time or answers with rubbish,
//! the proxy marks the plug-in as crashed (see [`crashes`]), stops asking it and lets the audio
//! through delayed by the same latency, so the tracks stay aligned and the render goes on. A
//! plug-in that crashes while it is scanned or opened is the same: an error for that plug-in only.

use std::collections::{HashMap, VecDeque};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::dsp::{AudioEffect, ParamSpec};
use crate::vst3_host::{parse_vst3_kind, Vst3ParamInfo, Vst3PluginInfo};
use crate::vst3_wire::{
    decode_block, encode_block, kind, read_frame, write_frame, Open, Ready, Scan, ScanItem,
};

/// Whether the effects chain runs VST3 plug-ins in a child process (it does by default).
static ISOLATE: AtomicBool = AtomicBool::new(true);

/// Turns the isolation on or off (a preference of the editor).
pub fn set_isolation(on: bool) {
    ISOLATE.store(on, Ordering::Relaxed);
}

/// `true` when plug-ins run in a child process.
pub fn isolation_enabled() -> bool {
    ISOLATE.load(Ordering::Relaxed)
}

/// The child program: `NEOVEG_VST3_HOST` when it is set, otherwise the one beside the running program.
pub fn host_exe() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NEOVEG_VST3_HOST") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let exe = std::env::current_exe().ok()?;
    let name = format!("neoveg_vst3_host{}", std::env::consts::EXE_SUFFIX);
    exe.parent()
        .map(|dir| dir.join(name))
        .filter(|path| path.is_file())
}

// ---- the plug-ins that failed ---------------------------------------------------------------------------

fn crashed() -> &'static Mutex<HashMap<String, String>> {
    static CRASHED: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CRASHED.get_or_init(Mutex::default)
}

/// Notes that the plug-in of `kind` failed, and why.
pub fn record_crash(kind: &str, reason: &str) {
    tracing::warn!("[WARN] o plug-in VST3 `{kind}` falhou no processo isolado: {reason}");
    if let Ok(mut map) = crashed().lock() {
        map.insert(kind.to_owned(), reason.to_owned());
    }
}

/// What failed so far: the kind of each plug-in and the reason, in the order of the kinds.
pub fn crashes() -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> = crashed()
        .lock()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    list.sort();
    list
}

/// The reason the plug-in of `kind` failed, if it did.
pub fn crash_of(kind: &str) -> Option<String> {
    crashed().lock().ok()?.get(kind).cloned()
}

/// Forgets the failure of `kind` (the user wants to try it again).
pub fn forget_crash(kind: &str) {
    if let Ok(mut map) = crashed().lock() {
        map.remove(kind);
    }
}

// ---- the link to the child process --------------------------------------------------------------------------

/// A running child process and the two pipes to it. The output of the child is read on a thread of
/// its own, so that waiting for an answer can have a time limit.
struct Link {
    child: Child,
    input: ChildStdin,
    answers: Receiver<Result<(u8, Vec<u8>), String>>,
}

impl Link {
    fn start(host: &Path) -> Result<Link, String> {
        let mut child = Command::new(host)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("não foi possível iniciar {}: {e}", host.display()))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| "o processo do plug-in não tem entrada".to_string())?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| "o processo do plug-in não tem saída".to_string())?;
        let (tx, answers) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                match read_frame(&mut reader) {
                    Ok(Some(frame)) => {
                        if tx.send(Ok(frame)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = tx.send(Err("o processo do plug-in terminou".into()));
                        break;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("resposta ilegível do plug-in: {e}")));
                        break;
                    }
                }
            }
        });
        Ok(Link {
            child,
            input,
            answers,
        })
    }

    /// Sends a frame and waits at most `limit` for the answer.
    fn ask(&mut self, what: u8, payload: &[u8], limit: Duration) -> Result<(u8, Vec<u8>), String> {
        write_frame(&mut self.input, what, payload)
            .map_err(|e| format!("falha ao falar com o plug-in: {e}"))?;
        match self.answers.recv_timeout(limit) {
            Ok(Ok((kind::ERROR, text))) => Err(String::from_utf8_lossy(&text).into_owned()),
            Ok(Ok(frame)) => Ok(frame),
            Ok(Err(reason)) => Err(reason),
            Err(RecvTimeoutError::Timeout) => Err(format!(
                "o plug-in não respondeu em {:.1} s",
                limit.as_secs_f32()
            )),
            Err(RecvTimeoutError::Disconnected) => Err("o processo do plug-in terminou".into()),
        }
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // Ask politely, then do not wait for a plug-in that does not listen
        let _ = write_frame(&mut self.input, kind::QUIT, &[]);
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.kill();
    }
}

/// A scan result in the form that crosses the pipe.
pub fn scan_item_of(info: &Vst3PluginInfo) -> ScanItem {
    ScanItem {
        kind: info.kind.clone(),
        class_id: info.class_id.clone(),
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        version: info.version.clone(),
        path: info.path.to_string_lossy().into_owned(),
        audio_inputs: info.audio_inputs,
        audio_outputs: info.audio_outputs,
        params: info
            .params
            .iter()
            .map(|p| crate::vst3_wire::ScanParam {
                id: p.id.clone(),
                vst_id: p.vst_id,
                name: p.name.clone(),
                default: p.default,
                stepped: p.stepped,
                is_bypass: p.is_bypass,
            })
            .collect(),
        latency: info.latency,
    }
}

// ---- scanning in the child ------------------------------------------------------------------------------------

/// How long a scan of one module may take before the child is killed.
pub const SCAN_LIMIT: Duration = Duration::from_secs(30);

fn info_of(item: ScanItem) -> Vst3PluginInfo {
    Vst3PluginInfo {
        kind: item.kind,
        class_id: item.class_id,
        name: item.name,
        vendor: item.vendor,
        version: item.version,
        path: PathBuf::from(item.path),
        audio_inputs: item.audio_inputs,
        audio_outputs: item.audio_outputs,
        params: item
            .params
            .into_iter()
            .map(|p| Vst3ParamInfo {
                id: p.id,
                vst_id: p.vst_id,
                name: p.name,
                default: p.default,
                stepped: p.stepped,
                is_bypass: p.is_bypass,
            })
            .collect(),
        latency: item.latency,
    }
}

/// Scans the module at `path` (a `.vst3` file or bundle) in a child process: the effect classes it
/// holds, or the reason it could not be read (including that it crashed or hung).
pub fn scan_isolated(
    path: &Path,
    host: &Path,
    limit: Duration,
) -> Result<Vec<Vst3PluginInfo>, String> {
    let mut link = Link::start(host)?;
    let request = serde_json::to_vec(&Scan {
        path: path.to_string_lossy().into_owned(),
    })
    .map_err(|e| e.to_string())?;
    let result = link.ask(kind::SCAN, &request, limit);
    link.kill();
    match result? {
        (kind::SCANNED, payload) => serde_json::from_slice::<Vec<ScanItem>>(&payload)
            .map(|items| items.into_iter().map(info_of).collect())
            .map_err(|e| format!("resposta do plug-in ilegível: {e}")),
        (other, _) => Err(format!("resposta inesperada do plug-in ({other})")),
    }
}

// ---- the effect ---------------------------------------------------------------------------------------------------

/// Frames sent in one request: a long block is cut so that a frame stays small.
const REQUEST_FRAMES: usize = 8192;

/// How long the child may take on a block: the first one includes the plug-in waking up.
const FIRST_BLOCK_LIMIT: Duration = Duration::from_secs(8);
const BLOCK_LIMIT: Duration = Duration::from_millis(2000);

/// A VST3 plug-in that runs in a child process, as an [`AudioEffect`].
pub struct IsolatedEffect {
    kind: String,
    link: Option<Link>,
    latency: usize,
    dynamic_pairs: Vec<(String, f32)>,
    /// Parameter changes not sent yet, with the next block.
    pending: Vec<(String, f32)>,
    /// The plug-in must forget its state before the next block.
    reset_pending: bool,
    blocks: u64,
    /// The delay that stands in for the plug-in once it has failed (the latency it reported).
    stand_in: Vec<VecDeque<f32>>,
}

impl IsolatedEffect {
    /// Starts a child with `host`, opens the plug-in of `kind` in it at `sample_rate`.
    pub fn open(kind: &str, host: &Path, sample_rate: f32) -> Result<IsolatedEffect, String> {
        let (path, class_id) = parse_vst3_kind(kind)
            .ok_or_else(|| format!("tipo de plug-in VST3 inválido: {kind}"))?;
        let mut link = Link::start(host)?;
        let request = serde_json::to_vec(&Open {
            path: path.to_owned(),
            class_id: class_id.to_owned(),
            sample_rate,
        })
        .map_err(|e| e.to_string())?;
        let ready = match link.ask(kind::OPEN, &request, Duration::from_secs(20))? {
            (kind::READY, payload) => serde_json::from_slice::<Ready>(&payload)
                .map_err(|e| format!("resposta do plug-in ilegível: {e}"))?,
            (other, _) => return Err(format!("resposta inesperada do plug-in ({other})")),
        };
        Ok(IsolatedEffect {
            kind: kind.to_owned(),
            link: Some(link),
            latency: ready.latency as usize,
            dynamic_pairs: ready.params,
            pending: Vec::new(),
            reset_pending: false,
            blocks: 0,
            stand_in: Vec::new(),
        })
    }

    /// `true` once the plug-in has failed and the audio only passes through.
    pub fn has_failed(&self) -> bool {
        self.link.is_none()
    }

    /// Kills the child (for tests, and for the editor when the user closes the plug-in).
    pub fn kill_child(&mut self) {
        if let Some(link) = self.link.as_mut() {
            link.kill();
        }
    }

    /// The plug-in failed: it is not asked again, and the failure is noted.
    fn fail(&mut self, reason: &str, channels: usize) {
        record_crash(&self.kind, reason);
        self.link = None;
        // The audio goes on delayed like the plug-in would have, so the tracks stay aligned
        self.stand_in = (0..channels)
            .map(|_| std::iter::repeat_n(0.0, self.latency).collect())
            .collect();
    }

    /// Lets the audio through, delayed by the latency of the plug-in.
    fn pass_through(&mut self, channels: &mut [&mut [f32]]) {
        if self.stand_in.len() < channels.len() {
            self.stand_in.resize_with(channels.len(), || {
                std::iter::repeat_n(0.0, self.latency).collect()
            });
        }
        for (channel, line) in channels.iter_mut().zip(self.stand_in.iter_mut()) {
            for sample in channel.iter_mut() {
                line.push_back(*sample);
                *sample = line.pop_front().unwrap_or(0.0);
            }
        }
    }

    fn process_chunk(&mut self, channels: &mut [&mut [f32]]) -> Result<(), String> {
        let link = self
            .link
            .as_mut()
            .ok_or_else(|| "o plug-in já falhou".to_string())?;
        let limit = if self.blocks == 0 {
            FIRST_BLOCK_LIMIT
        } else {
            BLOCK_LIMIT
        };
        if std::mem::take(&mut self.reset_pending) {
            link.ask(kind::RESET, &[], limit)?;
        }
        let changes = std::mem::take(&mut self.pending);
        let payload = encode_block(&changes, channels);
        let (what, answer) = link.ask(kind::PROCESS, &payload, limit)?;
        if what != kind::PROCESSED {
            return Err(format!("resposta inesperada do plug-in ({what})"));
        }
        let (_, audio) =
            decode_block(&answer).ok_or_else(|| "bloco de áudio ilegível".to_string())?;
        if audio.len() != channels.len()
            || audio
                .iter()
                .zip(channels.iter())
                .any(|(a, c)| a.len() != c.len())
        {
            return Err("o plug-in devolveu um bloco de outro tamanho".into());
        }
        for (channel, processed) in channels.iter_mut().zip(&audio) {
            channel.copy_from_slice(processed);
        }
        self.blocks += 1;
        Ok(())
    }
}

impl AudioEffect for IsolatedEffect {
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
        let Some(slot) = self.dynamic_pairs.iter_mut().find(|(k, _)| k == id) else {
            return false;
        };
        let value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        slot.1 = value;
        match self.pending.iter_mut().find(|(k, _)| k == id) {
            Some(change) => change.1 = value,
            None => self.pending.push((id.to_owned(), value)),
        }
        true
    }

    fn param(&self, id: &str) -> Option<f32> {
        self.dynamic_pairs
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
    }

    fn reset(&mut self) {
        self.reset_pending = true;
        // Every parameter is sent again, as the hosted effect does
        self.pending = self.dynamic_pairs.clone();
        for line in &mut self.stand_in {
            line.iter_mut().for_each(|s| *s = 0.0);
        }
    }

    fn process(&mut self, channels: &mut [&mut [f32]]) {
        let total = channels.iter().map(|c| c.len()).min().unwrap_or(0);
        if total == 0 || channels.is_empty() {
            return;
        }
        if self.link.is_none() {
            self.pass_through(channels);
            return;
        }
        let mut offset = 0;
        while offset < total {
            let n = (total - offset).min(REQUEST_FRAMES);
            let mut part: Vec<&mut [f32]> = channels
                .iter_mut()
                .map(|c| &mut c[offset..offset + n])
                .collect();
            if let Err(reason) = self.process_chunk(&mut part) {
                let count = part.len();
                self.fail(&reason, count);
                // This part and the rest go through the stand-in
                let mut rest: Vec<&mut [f32]> =
                    channels.iter_mut().map(|c| &mut c[offset..total]).collect();
                self.pass_through(&mut rest);
                return;
            }
            offset += n;
        }
    }

    fn latency(&self) -> usize {
        self.latency
    }
}

/// The effect for `kind` in a child process, or `None` (the reason is logged and noted in
/// [`crashes`]) when the child cannot start or the plug-in cannot be opened.
pub fn create_isolated_effect(
    kind: &str,
    sample_rate: f32,
    host: &Path,
) -> Option<Box<dyn AudioEffect>> {
    match IsolatedEffect::open(kind, host, sample_rate.max(8_000.0)) {
        Ok(effect) => Some(Box::new(effect)),
        Err(reason) => {
            record_crash(kind, &reason);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_isolation_is_on_until_it_is_turned_off() {
        // Other tests may flip it: only the round trip is checked
        let before = isolation_enabled();
        set_isolation(false);
        assert!(!isolation_enabled());
        set_isolation(true);
        assert!(isolation_enabled());
        set_isolation(before);
    }

    #[test]
    fn a_failure_is_noted_asked_about_and_forgotten() {
        let kind = "vst3:test-only.vst3::00000000000000000000000000000001";
        assert_eq!(crash_of(kind), None);
        record_crash(kind, "o processo terminou");
        assert_eq!(crash_of(kind).as_deref(), Some("o processo terminou"));
        assert!(crashes().iter().any(|(k, _)| k == kind));
        forget_crash(kind);
        assert_eq!(crash_of(kind), None);
    }

    #[test]
    fn a_plugin_that_cannot_start_is_an_error_with_a_note_not_a_panic() {
        let kind = "vst3:no-such-file.vst3::00000000000000000000000000000002";
        let missing_host = Path::new("no_such_neoveg_vst3_host");
        assert!(create_isolated_effect(kind, 48_000.0, missing_host).is_none());
        assert!(crash_of(kind).is_some_and(|r| r.contains("não foi possível iniciar")));
        forget_crash(kind);
        assert!(scan_isolated(Path::new("x.vst3"), missing_host, Duration::from_secs(1)).is_err());
    }
}
