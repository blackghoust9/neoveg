//! Loading a plug-in library and asking its plug-ins what they are: the part of the host that runs
//! **inside the child process** (see `neoveg_ofx_host`). Loading a library runs code that is not
//! ours and then calls it with pointers, so none of this may run in the program itself.
//!
//! The sequence is the one of the standard for the first moments of a plug-in: the library exports
//! how many plug-ins it holds and a table for each; the host gives each its host structure
//! (`setHost`), then calls its entry point with the actions *load*, *describe*, *describe in
//! context* (once for every context the plug-in said it supports) and *unload*.

use std::ffi::{c_char, c_void, CStr};
use std::path::Path;

use crate::abi::*;
use crate::report::{ParamReport, PluginReport};
use crate::store::{new_effect, new_set, set_of, Descriptor};
use crate::suites::fetch_suite;

/// The host structure plug-ins are given: its properties say what this host is and can do.
fn build_host() -> &'static OfxHost {
    let props = new_set();
    if let Ok(set) = unsafe { set_of(props) } {
        set.put_string(PROP_NAME, "com.neoveg.ofxhost");
        set.put_string(PROP_LABEL, "Neoveg");
        set.put_string(PROP_VERSION_LABEL, env!("CARGO_PKG_VERSION"));
        set.put_ints(PROP_VERSION, &[0, 1, 0]);
        set.put_ints(PROP_API_VERSION, &[1, 4]);
        // A host with no window and no pictures: it is in the background, and a plug-in that asks
        // for the rest gets a plain "no"
        set.put_ints("OfxImageEffectHostPropIsBackground", &[1]);
        for name in [
            "OfxImageEffectPropSupportsOverlays",
            "OfxImageEffectPropSupportsMultiResolution",
            "OfxImageEffectPropSupportsTiles",
            "OfxImageEffectPropTemporalClipAccess",
            "OfxImageEffectPropSupportsMultipleClipDepths",
            "OfxImageEffectPropSupportsMultipleClipPARs",
            "OfxImageEffectPropSetableFrameRate",
            "OfxImageEffectPropSetableFielding",
            "OfxParamHostPropSupportsCustomAnimation",
            "OfxParamHostPropSupportsStringAnimation",
            "OfxParamHostPropSupportsChoiceAnimation",
            "OfxParamHostPropSupportsBooleanAnimation",
            "OfxParamHostPropSupportsCustomInteract",
            "OfxParamHostPropSupportsParametricAnimation",
        ] {
            set.put_ints(name, &[0]);
        }
        set.put_ints("OfxParamHostPropMaxParameters", &[-1]);
        set.put_ints("OfxParamHostPropMaxPages", &[0]);
        set.put_strings(
            "OfxImageEffectPropSupportedComponents",
            &[
                "OfxImageComponentRGBA",
                "OfxImageComponentRGB",
                "OfxImageComponentAlpha",
            ],
        );
        set.put_strings(
            PROP_SUPPORTED_CONTEXTS,
            &[
                "OfxImageEffectContextFilter",
                "OfxImageEffectContextGeneral",
                "OfxImageEffectContextGenerator",
            ],
        );
        set.put_strings(
            "OfxImageEffectPropSupportedPixelDepths",
            &["OfxBitDepthByte", "OfxBitDepthShort", "OfxBitDepthFloat"],
        );
    }
    Box::leak(Box::new(OfxHost {
        host: props,
        fetch_suite,
    }))
}

/// Calls the entry point of a plug-in with an action.
unsafe fn call(
    entry: PluginEntryPoint,
    action: &CStr,
    handle: *const c_void,
    in_args: OfxPropertySetHandle,
) -> OfxStatus {
    unsafe { entry(action.as_ptr(), handle, in_args, std::ptr::null_mut()) }
}

fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Loads the library at `path` and describes every plug-in in it.
pub fn describe_library(path: &Path) -> Result<Vec<PluginReport>, String> {
    // SAFETY: loading a library runs its initialisers; that is why this is done in a child process
    let library = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("não foi possível carregar {}: {e}", path.display()))?;
    let count: libloading::Symbol<GetNumberOfPlugins> =
        unsafe { library.get(GET_NUMBER_OF_PLUGINS) }
            .map_err(|e| format!("a biblioteca não exporta OfxGetNumberOfPlugins: {e}"))?;
    let get: libloading::Symbol<GetPlugin> = unsafe { library.get(GET_PLUGIN) }
        .map_err(|e| format!("a biblioteca não exporta OfxGetPlugin: {e}"))?;
    let host = build_host();
    let total = unsafe { count() };
    let mut reports = Vec::new();
    for nth in 0..total.max(0) {
        let plugin = unsafe { get(nth) };
        if plugin.is_null() {
            reports.push(PluginReport {
                problem: Some(format!("o plug-in {nth} não existe")),
                ..PluginReport::default()
            });
            continue;
        }
        reports.push(unsafe { describe_plugin(&*plugin, host) });
    }
    // The library stays loaded until the process ends: unloading code a plug-in may still have
    // threads in is a fine way to crash
    std::mem::forget(library);
    Ok(reports)
}

/// Runs the first actions on one plug-in and collects what it said.
///
/// # Safety
/// `plugin` has to be a plug-in table a library gave (`OfxGetPlugin`).
unsafe fn describe_plugin(plugin: &OfxPlugin, host: &'static OfxHost) -> PluginReport {
    let mut report = PluginReport {
        identifier: text(plugin.plugin_identifier),
        api: text(plugin.plugin_api),
        version: [plugin.plugin_version_major, plugin.plugin_version_minor],
        ..PluginReport::default()
    };
    if report.api != IMAGE_EFFECT_PLUGIN_API {
        report.problem = Some(format!("API de plug-in sem suporte: {}", report.api));
        return report;
    }
    let Some(entry) = plugin.main_entry else {
        report.problem = Some("o plug-in não tem ponto de entrada".into());
        return report;
    };
    if let Some(set_host) = plugin.set_host {
        unsafe { set_host(host as *const OfxHost as *mut OfxHost) };
    }

    let none = std::ptr::null_mut();
    let loaded = unsafe { call(entry, ACTION_LOAD, std::ptr::null(), none) };
    if loaded != STAT_OK && loaded != STAT_REPLY_DEFAULT {
        report.problem = Some(format!("a ação de carregar respondeu {loaded}"));
        return report;
    }
    let effect = new_effect();
    let described = unsafe { call(entry, ACTION_DESCRIBE, effect.cast(), none) };
    if described != STAT_OK {
        report.problem = Some(format!("a ação de descrever respondeu {described}"));
        let _ = unsafe { call(entry, ACTION_UNLOAD, std::ptr::null(), none) };
        return report;
    }
    let contexts = unsafe { &*effect }.props.strings(PROP_SUPPORTED_CONTEXTS);
    let mut notes = Vec::new();
    for context in &contexts {
        let in_args = new_set();
        if let Ok(set) = unsafe { set_of(in_args) } {
            set.put_string(PROP_CONTEXT, context);
        }
        let status = unsafe { call(entry, ACTION_DESCRIBE_IN_CONTEXT, effect.cast(), in_args) };
        if status != STAT_OK && status != STAT_REPLY_DEFAULT {
            notes.push(format!("o contexto {context} respondeu {status}"));
        }
    }
    let descriptor: &Descriptor = unsafe { &*effect };
    report.label = descriptor
        .props
        .string(PROP_LABEL)
        .or_else(|| descriptor.props.string(PROP_SHORT_LABEL))
        .unwrap_or_else(|| report.identifier.clone());
    report.grouping = descriptor.props.string(PROP_GROUPING).unwrap_or_default();
    report.description = descriptor
        .props
        .string(PROP_PLUGIN_DESCRIPTION)
        .unwrap_or_default();
    report.contexts = contexts;
    for (name, kind, _) in &descriptor.params {
        if !report.params.iter().any(|p| p.name == *name) {
            report.params.push(ParamReport {
                name: name.clone(),
                kind: kind.clone(),
            });
        }
    }
    for (name, _) in &descriptor.clips {
        if !report.clips.contains(name) {
            report.clips.push(name.clone());
        }
    }
    report.problem = (!notes.is_empty()).then(|| notes.join("; "));
    let _ = unsafe { call(entry, ACTION_UNLOAD, std::ptr::null(), none) };
    report
}
