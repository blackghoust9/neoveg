//! A tiny OpenFX image effect plug-in for the tests of `neoveg_ofx`: it describes itself (a label, a
//! group, one context, one parameter, two clips) and draws nothing. The environment variable
//! `NEOVEG_OFX_SAMPLE` makes it misbehave when it is loaded, to see what the host does about it:
//!
//! * `abort_on_load`: the process aborts, as a plug-in that crashes would,
//! * `hang_on_load`: it never answers,
//! * `print_noise`: it prints a line before the answer, as plug-ins that log do,
//! * `refuse_describe`: it answers the *describe* action with a failure.

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicPtr, Ordering};

use neoveg_ofx::abi::*;

static HOST: AtomicPtr<OfxHost> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C" fn set_host(host: *mut OfxHost) {
    HOST.store(host, Ordering::SeqCst);
}

/// A suite of the host.
unsafe fn suite<T>(name: &CStr) -> Option<&'static T> {
    let host = HOST.load(Ordering::SeqCst);
    if host.is_null() {
        return None;
    }
    let found = unsafe { ((*host).fetch_suite)((*host).host, name.as_ptr(), 1) };
    if found.is_null() {
        None
    } else {
        Some(unsafe { &*(found as *const T) })
    }
}

unsafe fn describe(effect: *const c_void) -> OfxStatus {
    let (Some(props), Some(effects)) = (
        unsafe { suite::<OfxPropertySuiteV1>(c"OfxPropertySuite") },
        unsafe { suite::<OfxImageEffectSuiteV1>(c"OfxImageEffectSuite") },
    ) else {
        return STAT_ERR_MISSING_HOST_FEATURE;
    };
    if std::env::var("NEOVEG_OFX_SAMPLE").as_deref() == Ok("refuse_describe") {
        return STAT_FAILED;
    }
    let mut set: OfxPropertySetHandle = std::ptr::null_mut();
    let status = unsafe { (effects.get_property_set)(effect as OfxImageEffectHandle, &mut set) };
    if status != STAT_OK {
        return status;
    }
    unsafe {
        (props.prop_set_string)(
            set,
            c"OfxPropLabel".as_ptr(),
            0,
            c"Neoveg Sample Invert".as_ptr(),
        );
        (props.prop_set_string)(
            set,
            c"OfxImageEffectPluginPropGrouping".as_ptr(),
            0,
            c"Neoveg/Test".as_ptr(),
        );
        (props.prop_set_string)(
            set,
            c"OfxPropPluginDescription".as_ptr(),
            0,
            c"A plug-in that only describes itself".as_ptr(),
        );
        (props.prop_set_string)(
            set,
            c"OfxImageEffectPropSupportedContexts".as_ptr(),
            0,
            c"OfxImageEffectContextFilter".as_ptr(),
        );
    }
    STAT_OK
}

unsafe fn describe_in_context(effect: *const c_void) -> OfxStatus {
    let (Some(props), Some(effects), Some(params)) = (
        unsafe { suite::<OfxPropertySuiteV1>(c"OfxPropertySuite") },
        unsafe { suite::<OfxImageEffectSuiteV1>(c"OfxImageEffectSuite") },
        unsafe { suite::<OfxParameterSuiteV1>(c"OfxParameterSuite") },
    ) else {
        return STAT_ERR_MISSING_HOST_FEATURE;
    };
    let effect = effect as OfxImageEffectHandle;
    let mut clip: OfxPropertySetHandle = std::ptr::null_mut();
    unsafe {
        (effects.clip_define)(effect, c"Source".as_ptr(), &mut clip);
        (effects.clip_define)(effect, c"Output".as_ptr(), &mut clip);
    }
    let mut set: OfxParamSetHandle = std::ptr::null_mut();
    let status = unsafe { (effects.get_param_set)(effect, &mut set) };
    if status != STAT_OK {
        return status;
    }
    let mut amount: OfxPropertySetHandle = std::ptr::null_mut();
    let status = unsafe {
        (params.param_define)(
            set,
            c"OfxParamTypeDouble".as_ptr(),
            c"amount".as_ptr(),
            &mut amount,
        )
    };
    if status == STAT_OK {
        unsafe { (props.prop_set_string)(amount, c"OfxPropLabel".as_ptr(), 0, c"Amount".as_ptr()) };
    }
    status
}

unsafe extern "C" fn main_entry(
    action: *const c_char,
    handle: *const c_void,
    _in_args: OfxPropertySetHandle,
    _out_args: OfxPropertySetHandle,
) -> OfxStatus {
    let action = unsafe { CStr::from_ptr(action) }.to_str().unwrap_or("");
    match action {
        "OfxActionLoad" => {
            match std::env::var("NEOVEG_OFX_SAMPLE").as_deref() {
                Ok("abort_on_load") => std::process::abort(),
                Ok("hang_on_load") => loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                },
                Ok("print_noise") => println!("a line the plug-in printed on its own"),
                _ => {}
            }
            STAT_OK
        }
        "OfxActionDescribe" => unsafe { describe(handle) },
        "OfxImageEffectActionDescribeInContext" => unsafe { describe_in_context(handle) },
        "OfxActionUnload" => STAT_OK,
        _ => STAT_REPLY_DEFAULT,
    }
}

/// The table of the plug-in, in a cell the host may read.
struct Table(UnsafeCell<OfxPlugin>);

// SAFETY: the table is never written after it is built.
unsafe impl Sync for Table {}

static PLUGIN: Table = Table(UnsafeCell::new(OfxPlugin {
    plugin_api: c"OfxImageEffectPluginAPI".as_ptr(),
    api_version: 1,
    plugin_identifier: c"com.neoveg.sample.invert".as_ptr(),
    plugin_version_major: 1,
    plugin_version_minor: 2,
    set_host: Some(set_host),
    main_entry: Some(main_entry),
}));

#[no_mangle]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    1
}

#[no_mangle]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *mut OfxPlugin {
    if nth == 0 {
        PLUGIN.0.get()
    } else {
        std::ptr::null_mut()
    }
}
