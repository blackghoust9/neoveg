//! The child process of the VST3 host: `neoveg_vst3_host serve` opens VST3 plug-ins for the editor and
//! runs their audio, answering the frames of `neoveg_audio::vst3_wire` on its input and output (see
//! `neoveg_audio::vst3_isolated` for the editor's side and the reasons). A plug-in that crashes ends
//! this process and nothing else.
//!
//! Standard output belongs to the protocol: the plug-ins print there too sometimes, and the editor
//! skips what is not a frame. The exit code is 0 after a normal end and 2 for a command line this
//! does not understand.

use std::io::{BufReader, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::process::ExitCode;

use neoveg_audio::dsp::AudioEffect;
use neoveg_audio::vst3_host::{create_vst3_effect_in_process, scan_vst3_file};
use neoveg_audio::vst3_isolated::{scan_item_of, set_isolation};
use neoveg_audio::vst3_wire::{
    decode_block, encode_block, kind, read_frame, write_frame, Open, Ready, Scan,
};

/// What a frame is answered with: the kind and the payload.
type Answer = (u8, Vec<u8>);

fn error(text: impl Into<String>) -> Answer {
    (kind::ERROR, text.into().into_bytes())
}

fn json<T: serde::Serialize>(what: u8, value: &T) -> Answer {
    match serde_json::to_vec(value) {
        Ok(bytes) => (what, bytes),
        Err(e) => error(format!("não foi possível montar a resposta: {e}")),
    }
}

fn open(payload: &[u8], effect: &mut Option<Box<dyn AudioEffect>>) -> Answer {
    let Ok(request) = serde_json::from_slice::<Open>(payload) else {
        return error("pedido de abertura ilegível");
    };
    let kind_text =
        neoveg_audio::vst3_host::format_vst3_kind(Path::new(&request.path), &request.class_id);
    match create_vst3_effect_in_process(&kind_text, request.sample_rate) {
        Some(opened) => {
            let ready = Ready {
                latency: opened.latency() as u32,
                params: opened.dynamic_params().to_vec(),
            };
            *effect = Some(opened);
            json(kind::READY, &ready)
        }
        None => error("não foi possível abrir o plug-in"),
    }
}

fn process(payload: &[u8], effect: &mut Option<Box<dyn AudioEffect>>) -> Answer {
    let Some(effect) = effect.as_mut() else {
        return error("nenhum plug-in aberto");
    };
    let Some((changes, mut audio)) = decode_block(payload) else {
        return error("bloco de áudio ilegível");
    };
    for (key, value) in &changes {
        effect.set_param(key, *value);
    }
    let mut slices: Vec<&mut [f32]> = audio.iter_mut().map(Vec::as_mut_slice).collect();
    effect.process(&mut slices);
    (kind::PROCESSED, encode_block(&[], &slices))
}

fn scan(payload: &[u8]) -> Answer {
    let Ok(request) = serde_json::from_slice::<Scan>(payload) else {
        return error("pedido de varredura ilegível");
    };
    match scan_vst3_file(Path::new(&request.path)) {
        Ok(found) => {
            let items: Vec<_> = found.iter().map(scan_item_of).collect();
            json(kind::SCANNED, &items)
        }
        Err(e) => error(e),
    }
}

fn serve() -> ExitCode {
    // This process is the isolation: the plug-ins it opens must not start another one
    set_isolation(false);
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut output = std::io::stdout().lock();
    let mut effect: Option<Box<dyn AudioEffect>> = None;
    // The end of the input, or a frame that cannot be read, ends the process
    while let Ok(Some((what, payload))) = read_frame(&mut input) {
        let answer = match what {
            kind::QUIT => break,
            // A panic of Rust code becomes an error answer; a crash of the plug-in ends the process
            kind::OPEN => catch_unwind(AssertUnwindSafe(|| open(&payload, &mut effect)))
                .unwrap_or_else(|_| error("o plug-in causou um pânico ao abrir")),
            kind::PROCESS => catch_unwind(AssertUnwindSafe(|| process(&payload, &mut effect)))
                .unwrap_or_else(|_| error("o plug-in causou um pânico ao processar")),
            kind::RESET => {
                if let Some(effect) = effect.as_mut() {
                    effect.reset();
                }
                (kind::DONE, Vec::new())
            }
            kind::SCAN => catch_unwind(AssertUnwindSafe(|| scan(&payload)))
                .unwrap_or_else(|_| error("o plug-in causou um pânico na varredura")),
            other => error(format!("pedido desconhecido ({other})")),
        };
        if write_frame(&mut output, answer.0, &answer.1).is_err() {
            break;
        }
    }
    let _ = output.flush();
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("serve") => serve(),
        _ => {
            eprintln!("uso: neoveg_vst3_host serve");
            ExitCode::from(2)
        }
    }
}
