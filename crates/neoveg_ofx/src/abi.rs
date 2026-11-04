//! The C interface of the OpenFX image effect plug-in standard that a host needs to load a plug-in,
//! written from the public specification (OpenFX 1.4, BSD licensed): the structures a plug-in
//! library exports, the host structure it is given, and the three suites a plug-in calls while it
//! describes itself (properties, image effects and parameters).
//!
//! Only what the prototype uses is here. Every name keeps the spelling of the standard so the two
//! can be read side by side; the strings of the standard are `&CStr` constants.

use std::ffi::{c_char, c_double, c_int, c_uint, c_void, CStr};

/// What every call of the standard answers: `STAT_OK` or one of the codes below.
pub type OfxStatus = c_int;

pub const STAT_OK: OfxStatus = 0;
pub const STAT_FAILED: OfxStatus = 1;
pub const STAT_ERR_FATAL: OfxStatus = 2;
pub const STAT_ERR_UNKNOWN: OfxStatus = 3;
pub const STAT_ERR_MISSING_HOST_FEATURE: OfxStatus = 4;
pub const STAT_ERR_UNSUPPORTED: OfxStatus = 5;
pub const STAT_ERR_BAD_HANDLE: OfxStatus = 9;
pub const STAT_ERR_BAD_INDEX: OfxStatus = 10;
pub const STAT_ERR_VALUE: OfxStatus = 11;
pub const STAT_REPLY_DEFAULT: OfxStatus = 14;

/// A set of named properties; the handle is opaque to the plug-in.
#[repr(C)]
pub struct OfxPropertySetStruct {
    _private: [u8; 0],
}
pub type OfxPropertySetHandle = *mut OfxPropertySetStruct;

/// A time in frames.
pub type OfxTime = c_double;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OfxRectD {
    pub x1: c_double,
    pub y1: c_double,
    pub x2: c_double,
    pub y2: c_double,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OfxRangeD {
    pub min: c_double,
    pub max: c_double,
}

/// How a plug-in asks the host for one of its suites.
pub type FetchSuite = unsafe extern "C" fn(
    host: OfxPropertySetHandle,
    suite_name: *const c_char,
    suite_version: c_int,
) -> *const c_void;

/// What the host gives a plug-in (`setHost`): its properties and the way to fetch suites.
#[repr(C)]
pub struct OfxHost {
    pub host: OfxPropertySetHandle,
    pub fetch_suite: FetchSuite,
}

/// The one function a plug-in answers every action with.
pub type PluginEntryPoint = unsafe extern "C" fn(
    action: *const c_char,
    handle: *const c_void,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus;

/// What a plug-in library exports for each plug-in it holds (`OfxGetPlugin`).
#[repr(C)]
pub struct OfxPlugin {
    pub plugin_api: *const c_char,
    pub api_version: c_int,
    pub plugin_identifier: *const c_char,
    pub plugin_version_major: c_uint,
    pub plugin_version_minor: c_uint,
    pub set_host: Option<unsafe extern "C" fn(host: *mut OfxHost)>,
    pub main_entry: Option<PluginEntryPoint>,
}

/// `int OfxGetNumberOfPlugins(void)`, exported by every plug-in library.
pub type GetNumberOfPlugins = unsafe extern "C" fn() -> c_int;
/// `OfxPlugin *OfxGetPlugin(int nth)`, exported by every plug-in library.
pub type GetPlugin = unsafe extern "C" fn(nth: c_int) -> *mut OfxPlugin;

pub const GET_NUMBER_OF_PLUGINS: &[u8] = b"OfxGetNumberOfPlugins\0";
pub const GET_PLUGIN: &[u8] = b"OfxGetPlugin\0";

// Actions
pub const ACTION_LOAD: &CStr = c"OfxActionLoad";
pub const ACTION_DESCRIBE: &CStr = c"OfxActionDescribe";
pub const ACTION_UNLOAD: &CStr = c"OfxActionUnload";
pub const ACTION_DESCRIBE_IN_CONTEXT: &CStr = c"OfxImageEffectActionDescribeInContext";

// Names of the suites and of the plug-in API
pub const SUITE_PROPERTY: &str = "OfxPropertySuite";
pub const SUITE_IMAGE_EFFECT: &str = "OfxImageEffectSuite";
pub const SUITE_PARAMETER: &str = "OfxParameterSuite";
pub const IMAGE_EFFECT_PLUGIN_API: &str = "OfxImageEffectPluginAPI";

// Property names a host reads or fills
pub const PROP_NAME: &str = "OfxPropName";
pub const PROP_LABEL: &str = "OfxPropLabel";
pub const PROP_SHORT_LABEL: &str = "OfxPropShortLabel";
pub const PROP_VERSION: &str = "OfxPropVersion";
pub const PROP_VERSION_LABEL: &str = "OfxPropVersionLabel";
pub const PROP_API_VERSION: &str = "OfxPropAPIVersion";
pub const PROP_PLUGIN_DESCRIPTION: &str = "OfxPropPluginDescription";
pub const PROP_GROUPING: &str = "OfxImageEffectPluginPropGrouping";
pub const PROP_SUPPORTED_CONTEXTS: &str = "OfxImageEffectPropSupportedContexts";
pub const PROP_CONTEXT: &str = "OfxImageEffectPropContext";

/// The property suite (`OfxPropertySuite`, version 1): reading and writing the properties of a set.
#[repr(C)]
pub struct OfxPropertySuiteV1 {
    pub prop_set_pointer:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_void) -> OfxStatus,
    pub prop_set_string: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *const c_char,
    ) -> OfxStatus,
    pub prop_set_double:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, c_double) -> OfxStatus,
    pub prop_set_int:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, c_int) -> OfxStatus,
    pub prop_set_pointer_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut *mut c_void,
    ) -> OfxStatus,
    pub prop_set_string_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *const *const c_char,
    ) -> OfxStatus,
    pub prop_set_double_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *const c_double,
    ) -> OfxStatus,
    pub prop_set_int_n:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const c_int) -> OfxStatus,
    pub prop_get_pointer: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut *mut c_void,
    ) -> OfxStatus,
    pub prop_get_string: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut *const c_char,
    ) -> OfxStatus,
    pub prop_get_double: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut c_double,
    ) -> OfxStatus,
    pub prop_get_int:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_int) -> OfxStatus,
    pub prop_get_pointer_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut *mut c_void,
    ) -> OfxStatus,
    pub prop_get_string_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut *const c_char,
    ) -> OfxStatus,
    pub prop_get_double_n: unsafe extern "C" fn(
        OfxPropertySetHandle,
        *const c_char,
        c_int,
        *mut c_double,
    ) -> OfxStatus,
    pub prop_get_int_n:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_int) -> OfxStatus,
    pub prop_reset: unsafe extern "C" fn(OfxPropertySetHandle, *const c_char) -> OfxStatus,
    pub prop_get_dimension:
        unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, *mut c_int) -> OfxStatus,
}

/// Handles of an effect, a clip, a parameter and a set of parameters: all point at things the host
/// owns, and none of them is looked into by the plug-in.
pub type OfxImageEffectHandle = *mut c_void;
pub type OfxImageClipHandle = *mut c_void;
pub type OfxImageMemoryHandle = *mut c_void;
pub type OfxParamSetHandle = *mut c_void;
pub type OfxParamHandle = *mut c_void;

/// The image effect suite (`OfxImageEffectSuite`, version 1).
#[repr(C)]
pub struct OfxImageEffectSuiteV1 {
    pub get_property_set:
        unsafe extern "C" fn(OfxImageEffectHandle, *mut OfxPropertySetHandle) -> OfxStatus,
    pub get_param_set:
        unsafe extern "C" fn(OfxImageEffectHandle, *mut OfxParamSetHandle) -> OfxStatus,
    pub clip_define: unsafe extern "C" fn(
        OfxImageEffectHandle,
        *const c_char,
        *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub clip_get_handle: unsafe extern "C" fn(
        OfxImageEffectHandle,
        *const c_char,
        *mut OfxImageClipHandle,
        *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub clip_get_property_set:
        unsafe extern "C" fn(OfxImageClipHandle, *mut OfxPropertySetHandle) -> OfxStatus,
    pub clip_get_image: unsafe extern "C" fn(
        OfxImageClipHandle,
        OfxTime,
        *const OfxRectD,
        *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub clip_release_image: unsafe extern "C" fn(OfxPropertySetHandle) -> OfxStatus,
    pub clip_get_region_of_definition:
        unsafe extern "C" fn(OfxImageClipHandle, OfxTime, *mut OfxRectD) -> OfxStatus,
    pub abort: unsafe extern "C" fn(OfxImageEffectHandle) -> c_int,
    pub image_memory_alloc:
        unsafe extern "C" fn(OfxImageEffectHandle, usize, *mut OfxImageMemoryHandle) -> OfxStatus,
    pub image_memory_free: unsafe extern "C" fn(OfxImageMemoryHandle) -> OfxStatus,
    pub image_memory_lock:
        unsafe extern "C" fn(OfxImageMemoryHandle, *mut *mut c_void) -> OfxStatus,
    pub image_memory_unlock: unsafe extern "C" fn(OfxImageMemoryHandle) -> OfxStatus,
}

/// The parameter suite (`OfxParameterSuite`, version 1). The calls with a variable list of
/// arguments in the standard are given the arguments of the first one only: this host never calls
/// them, and the way the entries are filled in ignores whatever follows.
#[repr(C)]
pub struct OfxParameterSuiteV1 {
    pub param_define: unsafe extern "C" fn(
        OfxParamSetHandle,
        *const c_char,
        *const c_char,
        *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub param_get_handle: unsafe extern "C" fn(
        OfxParamSetHandle,
        *const c_char,
        *mut OfxParamHandle,
        *mut OfxPropertySetHandle,
    ) -> OfxStatus,
    pub param_set_get_property_set:
        unsafe extern "C" fn(OfxParamSetHandle, *mut OfxPropertySetHandle) -> OfxStatus,
    pub param_get_property_set:
        unsafe extern "C" fn(OfxParamHandle, *mut OfxPropertySetHandle) -> OfxStatus,
    pub param_get_value: unsafe extern "C" fn(OfxParamHandle) -> OfxStatus,
    pub param_get_value_at_time: unsafe extern "C" fn(OfxParamHandle, OfxTime) -> OfxStatus,
    pub param_get_derivative: unsafe extern "C" fn(OfxParamHandle, OfxTime) -> OfxStatus,
    pub param_get_integral: unsafe extern "C" fn(OfxParamHandle, OfxTime, OfxTime) -> OfxStatus,
    pub param_set_value: unsafe extern "C" fn(OfxParamHandle) -> OfxStatus,
    pub param_set_value_at_time: unsafe extern "C" fn(OfxParamHandle, OfxTime) -> OfxStatus,
    pub param_get_num_keys: unsafe extern "C" fn(OfxParamHandle, *mut c_uint) -> OfxStatus,
    pub param_get_key_time: unsafe extern "C" fn(OfxParamHandle, c_uint, *mut OfxTime) -> OfxStatus,
    pub param_get_key_index:
        unsafe extern "C" fn(OfxParamHandle, OfxTime, c_int, *mut c_int) -> OfxStatus,
    pub param_delete_key: unsafe extern "C" fn(OfxParamHandle, OfxTime) -> OfxStatus,
    pub param_delete_all_keys: unsafe extern "C" fn(OfxParamHandle) -> OfxStatus,
    pub param_copy: unsafe extern "C" fn(
        OfxParamHandle,
        OfxParamHandle,
        OfxTime,
        *const OfxRangeD,
    ) -> OfxStatus,
    pub param_edit_begin: unsafe extern "C" fn(OfxParamSetHandle, *const c_char) -> OfxStatus,
    pub param_edit_end: unsafe extern "C" fn(OfxParamSetHandle) -> OfxStatus,
}
