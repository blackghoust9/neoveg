//! A real, loadable VST3 library for the tests of `neoveg_audio::vst3_isolated`: the gain example
//! plug-in of the `vst3` crate (MIT OR Apache-2.0, Micah Johnston), the same file the in-process
//! tests of the host use, exported the way a `.vst3` library is (`GetPluginFactory`).
//!
//! Two environment variables make it misbehave, to see what the host does about it:
//! `NEOVEG_VST3_SAMPLE_LATENCY` is the latency in samples it reports, and
//! `NEOVEG_VST3_SAMPLE=crash_on_process` makes it abort when it is given audio.

#[path = "../../neoveg_audio/src/vst3_host/test_gain.rs"]
#[allow(clippy::all)]
mod test_gain;

use vst3::Steinberg::IPluginFactory;

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn InitDll() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleEntry(_library_handle: *mut std::ffi::c_void) -> bool {
    true
}

#[cfg(target_os = "linux")]
#[no_mangle]
extern "system" fn ModuleExit() -> bool {
    true
}

#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    test_gain::factory().into_raw()
}
