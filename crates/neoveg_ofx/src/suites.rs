//! The suites this host gives a plug-in: properties, the part of the image effect suite a plug-in
//! needs to describe itself (its properties, its parameters, its clips) and the same for
//! parameters. Everything that needs a picture or a value in time answers "unsupported": this is a
//! host that asks plug-ins what they are, not one that runs them.

use std::ffi::{c_char, c_double, c_int, c_uint, c_void, CStr};

use crate::abi::*;
use crate::store::{cstring, descriptor_of, new_set, set_of, Value};

/// The name behind a C string, `None` for a null pointer.
unsafe fn name_of(text: *const c_char) -> Option<String> {
    if text.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// A status from the result of a call.
fn status(result: Result<(), OfxStatus>) -> OfxStatus {
    result.err().unwrap_or(STAT_OK)
}

fn index_of(index: c_int) -> Result<usize, OfxStatus> {
    usize::try_from(index).map_err(|_| STAT_ERR_BAD_INDEX)
}

fn count_of(count: c_int) -> Result<usize, OfxStatus> {
    usize::try_from(count).map_err(|_| STAT_ERR_VALUE)
}

unsafe fn put(
    handle: OfxPropertySetHandle,
    property: *const c_char,
    index: c_int,
    value: Value,
) -> OfxStatus {
    status((|| {
        let name = unsafe { name_of(property) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        unsafe { set_of(handle) }?.set(&name, index_of(index)?, value)
    })())
}

unsafe fn fetch<R>(
    handle: OfxPropertySetHandle,
    property: *const c_char,
    index: c_int,
    read: impl FnOnce(&Value) -> Result<R, OfxStatus>,
) -> Result<R, OfxStatus> {
    let name = unsafe { name_of(property) }.ok_or(STAT_ERR_BAD_HANDLE)?;
    let set = unsafe { set_of(handle) }?;
    read(set.get(&name, index_of(index)?)?)
}

unsafe extern "C" fn prop_set_pointer(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    v: *mut c_void,
) -> OfxStatus {
    unsafe { put(h, p, i, Value::Ptr(v as usize)) }
}

unsafe extern "C" fn prop_set_string(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    v: *const c_char,
) -> OfxStatus {
    let text = unsafe { name_of(v) }.unwrap_or_default();
    unsafe { put(h, p, i, Value::Str(cstring(&text))) }
}

unsafe extern "C" fn prop_set_double(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    v: c_double,
) -> OfxStatus {
    unsafe { put(h, p, i, Value::Double(v)) }
}

unsafe extern "C" fn prop_set_int(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    v: c_int,
) -> OfxStatus {
    unsafe { put(h, p, i, Value::Int(v)) }
}

unsafe extern "C" fn prop_set_pointer_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    v: *mut *mut c_void,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let value = unsafe { *v.add(k) } as usize;
        let status = unsafe { put(h, p, k as c_int, Value::Ptr(value)) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_set_string_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    v: *const *const c_char,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let text = unsafe { name_of(*v.add(k)) }.unwrap_or_default();
        let status = unsafe { put(h, p, k as c_int, Value::Str(cstring(&text))) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_set_double_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    v: *const c_double,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { put(h, p, k as c_int, Value::Double(*v.add(k))) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_set_int_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    v: *const c_int,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { put(h, p, k as c_int, Value::Int(*v.add(k))) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_get_pointer(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    out: *mut *mut c_void,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe {
        fetch(h, p, i, |v| match v {
            Value::Ptr(address) => {
                *out = *address as *mut c_void;
                Ok(())
            }
            _ => Err(STAT_ERR_VALUE),
        })
    })
}

unsafe extern "C" fn prop_get_string(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    out: *mut *const c_char,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe {
        fetch(h, p, i, |v| match v {
            Value::Str(text) => {
                // The text stays where it is as long as the property does
                *out = text.as_ptr();
                Ok(())
            }
            _ => Err(STAT_ERR_VALUE),
        })
    })
}

unsafe extern "C" fn prop_get_double(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    out: *mut c_double,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe {
        fetch(h, p, i, |v| match v {
            Value::Double(x) => {
                *out = *x;
                Ok(())
            }
            Value::Int(x) => {
                *out = f64::from(*x);
                Ok(())
            }
            _ => Err(STAT_ERR_VALUE),
        })
    })
}

unsafe extern "C" fn prop_get_int(
    h: OfxPropertySetHandle,
    p: *const c_char,
    i: c_int,
    out: *mut c_int,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe {
        fetch(h, p, i, |v| match v {
            Value::Int(x) => {
                *out = *x;
                Ok(())
            }
            _ => Err(STAT_ERR_VALUE),
        })
    })
}

unsafe extern "C" fn prop_get_pointer_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    out: *mut *mut c_void,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { prop_get_pointer(h, p, k as c_int, out.add(k)) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_get_string_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    out: *mut *const c_char,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { prop_get_string(h, p, k as c_int, out.add(k)) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_get_double_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    out: *mut c_double,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { prop_get_double(h, p, k as c_int, out.add(k)) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_get_int_n(
    h: OfxPropertySetHandle,
    p: *const c_char,
    n: c_int,
    out: *mut c_int,
) -> OfxStatus {
    let Ok(n) = count_of(n) else {
        return STAT_ERR_VALUE;
    };
    for k in 0..n {
        let status = unsafe { prop_get_int(h, p, k as c_int, out.add(k)) };
        if status != STAT_OK {
            return status;
        }
    }
    STAT_OK
}

unsafe extern "C" fn prop_reset(h: OfxPropertySetHandle, p: *const c_char) -> OfxStatus {
    status((|| {
        let name = unsafe { name_of(p) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        unsafe { set_of(h) }?.reset(&name)
    })())
}

unsafe extern "C" fn prop_get_dimension(
    h: OfxPropertySetHandle,
    p: *const c_char,
    out: *mut c_int,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status((|| {
        let name = unsafe { name_of(p) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        let count = unsafe { set_of(h) }?.dimension(&name)?;
        unsafe { *out = count as c_int };
        Ok(())
    })())
}

pub static PROPERTY_SUITE: OfxPropertySuiteV1 = OfxPropertySuiteV1 {
    prop_set_pointer,
    prop_set_string,
    prop_set_double,
    prop_set_int,
    prop_set_pointer_n,
    prop_set_string_n,
    prop_set_double_n,
    prop_set_int_n,
    prop_get_pointer,
    prop_get_string,
    prop_get_double,
    prop_get_int,
    prop_get_pointer_n,
    prop_get_string_n,
    prop_get_double_n,
    prop_get_int_n,
    prop_reset,
    prop_get_dimension,
};

// --- The image effect suite -------------------------------------------------------------------

unsafe extern "C" fn effect_get_property_set(
    effect: OfxImageEffectHandle,
    out: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe { descriptor_of(effect) }.map(|d| {
        // The address of a descriptor is the address of its properties
        unsafe { *out = d as *mut _ as OfxPropertySetHandle };
    }))
}

unsafe extern "C" fn effect_get_param_set(
    effect: OfxImageEffectHandle,
    out: *mut OfxParamSetHandle,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe { descriptor_of(effect) }.map(|d| {
        unsafe { *out = d as *mut _ as OfxParamSetHandle };
    }))
}

unsafe extern "C" fn effect_clip_define(
    effect: OfxImageEffectHandle,
    name: *const c_char,
    out: *mut OfxPropertySetHandle,
) -> OfxStatus {
    status((|| {
        let name = unsafe { name_of(name) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        let descriptor = unsafe { descriptor_of(effect) }?;
        let props = new_set();
        descriptor.clips.push((name, props as usize));
        if !out.is_null() {
            unsafe { *out = props };
        }
        Ok(())
    })())
}

unsafe extern "C" fn effect_clip_get_handle(
    _effect: OfxImageEffectHandle,
    _name: *const c_char,
    _clip: *mut OfxImageClipHandle,
    _props: *mut OfxPropertySetHandle,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_clip_get_property_set(
    _clip: OfxImageClipHandle,
    _props: *mut OfxPropertySetHandle,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_clip_get_image(
    _clip: OfxImageClipHandle,
    _time: OfxTime,
    _region: *const OfxRectD,
    _image: *mut OfxPropertySetHandle,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_clip_release_image(_image: OfxPropertySetHandle) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_clip_region_of_definition(
    _clip: OfxImageClipHandle,
    _time: OfxTime,
    _bounds: *mut OfxRectD,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_abort(_effect: OfxImageEffectHandle) -> c_int {
    0
}

unsafe extern "C" fn effect_memory_alloc(
    _effect: OfxImageEffectHandle,
    _bytes: usize,
    _memory: *mut OfxImageMemoryHandle,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_memory_free(_memory: OfxImageMemoryHandle) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_memory_lock(
    _memory: OfxImageMemoryHandle,
    _pointer: *mut *mut c_void,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn effect_memory_unlock(_memory: OfxImageMemoryHandle) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

pub static IMAGE_EFFECT_SUITE: OfxImageEffectSuiteV1 = OfxImageEffectSuiteV1 {
    get_property_set: effect_get_property_set,
    get_param_set: effect_get_param_set,
    clip_define: effect_clip_define,
    clip_get_handle: effect_clip_get_handle,
    clip_get_property_set: effect_clip_get_property_set,
    clip_get_image: effect_clip_get_image,
    clip_release_image: effect_clip_release_image,
    clip_get_region_of_definition: effect_clip_region_of_definition,
    abort: effect_abort,
    image_memory_alloc: effect_memory_alloc,
    image_memory_free: effect_memory_free,
    image_memory_lock: effect_memory_lock,
    image_memory_unlock: effect_memory_unlock,
};

// --- The parameter suite ----------------------------------------------------------------------

unsafe extern "C" fn param_define(
    param_set: OfxParamSetHandle,
    kind: *const c_char,
    name: *const c_char,
    out: *mut OfxPropertySetHandle,
) -> OfxStatus {
    status((|| {
        let kind = unsafe { name_of(kind) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        let name = unsafe { name_of(name) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        let descriptor = unsafe { descriptor_of(param_set) }?;
        let props = new_set();
        descriptor.params.push((name, kind, props as usize));
        if !out.is_null() {
            unsafe { *out = props };
        }
        Ok(())
    })())
}

unsafe extern "C" fn param_get_handle(
    param_set: OfxParamSetHandle,
    name: *const c_char,
    param: *mut OfxParamHandle,
    props: *mut OfxPropertySetHandle,
) -> OfxStatus {
    status((|| {
        let name = unsafe { name_of(name) }.ok_or(STAT_ERR_BAD_HANDLE)?;
        let descriptor = unsafe { descriptor_of(param_set) }?;
        let (_, _, address) = descriptor
            .params
            .iter()
            .find(|(n, _, _)| *n == name)
            .ok_or(STAT_ERR_UNKNOWN)?;
        // A parameter handle is the address of its set of properties
        if !param.is_null() {
            unsafe { *param = *address as OfxParamHandle };
        }
        if !props.is_null() {
            unsafe { *props = *address as OfxPropertySetHandle };
        }
        Ok(())
    })())
}

unsafe extern "C" fn param_set_get_property_set(
    param_set: OfxParamSetHandle,
    out: *mut OfxPropertySetHandle,
) -> OfxStatus {
    unsafe { effect_get_property_set(param_set, out) }
}

unsafe extern "C" fn param_get_property_set(
    param: OfxParamHandle,
    out: *mut OfxPropertySetHandle,
) -> OfxStatus {
    if out.is_null() {
        return STAT_ERR_BAD_HANDLE;
    }
    status(unsafe { set_of(param as OfxPropertySetHandle) }.map(|_| {
        unsafe { *out = param as OfxPropertySetHandle };
    }))
}

unsafe extern "C" fn param_unsupported_1(_param: OfxParamHandle) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn param_unsupported_time(_param: OfxParamHandle, _time: OfxTime) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn param_unsupported_integral(
    _param: OfxParamHandle,
    _from: OfxTime,
    _to: OfxTime,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn param_num_keys(_param: OfxParamHandle, keys: *mut c_uint) -> OfxStatus {
    if !keys.is_null() {
        unsafe { *keys = 0 };
    }
    STAT_OK
}

unsafe extern "C" fn param_key_time(
    _param: OfxParamHandle,
    _nth: c_uint,
    _time: *mut OfxTime,
) -> OfxStatus {
    STAT_ERR_BAD_INDEX
}

unsafe extern "C" fn param_key_index(
    _param: OfxParamHandle,
    _time: OfxTime,
    _direction: c_int,
    _index: *mut c_int,
) -> OfxStatus {
    STAT_ERR_BAD_INDEX
}

unsafe extern "C" fn param_copy(
    _to: OfxParamHandle,
    _from: OfxParamHandle,
    _offset: OfxTime,
    _range: *const OfxRangeD,
) -> OfxStatus {
    STAT_ERR_UNSUPPORTED
}

unsafe extern "C" fn param_edit_begin(_set: OfxParamSetHandle, _name: *const c_char) -> OfxStatus {
    STAT_OK
}

unsafe extern "C" fn param_edit_end(_set: OfxParamSetHandle) -> OfxStatus {
    STAT_OK
}

pub static PARAMETER_SUITE: OfxParameterSuiteV1 = OfxParameterSuiteV1 {
    param_define,
    param_get_handle,
    param_set_get_property_set,
    param_get_property_set,
    param_get_value: param_unsupported_1,
    param_get_value_at_time: param_unsupported_time,
    param_get_derivative: param_unsupported_time,
    param_get_integral: param_unsupported_integral,
    param_set_value: param_unsupported_1,
    param_set_value_at_time: param_unsupported_time,
    param_get_num_keys: param_num_keys,
    param_get_key_time: param_key_time,
    param_get_key_index: param_key_index,
    param_delete_key: param_unsupported_time,
    param_delete_all_keys: param_unsupported_1,
    param_copy,
    param_edit_begin,
    param_edit_end,
};

/// `fetchSuite` of the host: the suites above at version 1, a null pointer (the standard's way of
/// saying "this host does not have it") for anything else.
///
/// # Safety
/// `name` is null or a C string, as the standard requires of a plug-in.
pub unsafe extern "C" fn fetch_suite(
    _host: OfxPropertySetHandle,
    name: *const c_char,
    version: c_int,
) -> *const c_void {
    let Some(name) = (unsafe { name_of(name) }) else {
        return std::ptr::null();
    };
    suite_named(&name, version)
}

/// The suite called `name` at `version`, or null.
pub fn suite_named(name: &str, version: c_int) -> *const c_void {
    if version != 1 {
        return std::ptr::null();
    }
    match name {
        SUITE_PROPERTY => &PROPERTY_SUITE as *const _ as *const c_void,
        SUITE_IMAGE_EFFECT => &IMAGE_EFFECT_SUITE as *const _ as *const c_void,
        SUITE_PARAMETER => &PARAMETER_SUITE as *const _ as *const c_void,
        _ => std::ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{new_effect, PropertySet};

    #[test]
    fn the_host_has_the_three_suites_at_version_one_and_nothing_else() {
        for name in [SUITE_PROPERTY, SUITE_IMAGE_EFFECT, SUITE_PARAMETER] {
            assert!(!suite_named(name, 1).is_null(), "{name}");
            assert!(suite_named(name, 2).is_null(), "{name} v2");
        }
        assert!(suite_named("OfxMemorySuite", 1).is_null());
    }

    /// What a plug-in does while it describes itself, through the same function pointers it gets.
    #[test]
    fn a_plugin_can_set_and_read_its_properties_define_a_parameter_and_a_clip() {
        let effect = new_effect();
        let mut props: OfxPropertySetHandle = std::ptr::null_mut();
        unsafe {
            assert_eq!(
                (IMAGE_EFFECT_SUITE.get_property_set)(effect.cast(), &mut props),
                STAT_OK
            );
            let label = c"Invert";
            assert_eq!(
                (PROPERTY_SUITE.prop_set_string)(
                    props,
                    c"OfxPropLabel".as_ptr(),
                    0,
                    label.as_ptr()
                ),
                STAT_OK
            );
            let mut read: *const c_char = std::ptr::null();
            assert_eq!(
                (PROPERTY_SUITE.prop_get_string)(props, c"OfxPropLabel".as_ptr(), 0, &mut read),
                STAT_OK
            );
            assert_eq!(CStr::from_ptr(read).to_str().unwrap(), "Invert");
            // An integer read from a string is a value error, a missing property an unknown one
            let mut number = 0;
            assert_eq!(
                (PROPERTY_SUITE.prop_get_int)(props, c"OfxPropLabel".as_ptr(), 0, &mut number),
                STAT_ERR_VALUE
            );
            assert_eq!(
                (PROPERTY_SUITE.prop_get_int)(props, c"Nope".as_ptr(), 0, &mut number),
                STAT_ERR_UNKNOWN
            );
            let mut count = 0;
            assert_eq!(
                (PROPERTY_SUITE.prop_get_dimension)(props, c"OfxPropLabel".as_ptr(), &mut count),
                STAT_OK
            );
            assert_eq!(count, 1);

            let mut param_props: OfxPropertySetHandle = std::ptr::null_mut();
            let mut params: OfxParamSetHandle = std::ptr::null_mut();
            assert_eq!(
                (IMAGE_EFFECT_SUITE.get_param_set)(effect.cast(), &mut params),
                STAT_OK
            );
            assert_eq!(
                (PARAMETER_SUITE.param_define)(
                    params,
                    c"OfxParamTypeDouble".as_ptr(),
                    c"amount".as_ptr(),
                    &mut param_props
                ),
                STAT_OK
            );
            assert_eq!(
                (IMAGE_EFFECT_SUITE.clip_define)(
                    effect.cast(),
                    c"Source".as_ptr(),
                    &mut param_props
                ),
                STAT_OK
            );
            // A stranger's handle is refused, not followed
            assert_eq!(
                (PROPERTY_SUITE.prop_get_int)(
                    0x10 as OfxPropertySetHandle,
                    c"x".as_ptr(),
                    0,
                    &mut number
                ),
                STAT_ERR_BAD_HANDLE
            );
        }
        let descriptor = unsafe { &*effect };
        assert_eq!(descriptor.params.len(), 1);
        assert_eq!(descriptor.params[0].0, "amount");
        assert_eq!(descriptor.params[0].1, "OfxParamTypeDouble");
        assert_eq!(descriptor.clips.len(), 1);
        assert_eq!(
            descriptor.props.string("OfxPropLabel").as_deref(),
            Some("Invert")
        );
        let _ = PropertySet::default();
    }

    #[test]
    fn lists_of_values_go_in_and_out_in_one_call() {
        let handle = new_set();
        unsafe {
            let texts = [c"a".as_ptr(), c"b".as_ptr(), c"c".as_ptr()];
            assert_eq!(
                (PROPERTY_SUITE.prop_set_string_n)(handle, c"list".as_ptr(), 3, texts.as_ptr()),
                STAT_OK
            );
            let mut out = [std::ptr::null(); 3];
            assert_eq!(
                (PROPERTY_SUITE.prop_get_string_n)(handle, c"list".as_ptr(), 3, out.as_mut_ptr()),
                STAT_OK
            );
            let read: Vec<&str> = out
                .iter()
                .map(|p| CStr::from_ptr(*p).to_str().unwrap())
                .collect();
            assert_eq!(read, ["a", "b", "c"]);
            // More than there are is an index error, a negative count a value error
            let mut more = [std::ptr::null(); 4];
            assert_eq!(
                (PROPERTY_SUITE.prop_get_string_n)(handle, c"list".as_ptr(), 4, more.as_mut_ptr()),
                STAT_ERR_BAD_INDEX
            );
            assert_eq!(
                (PROPERTY_SUITE.prop_get_string_n)(handle, c"list".as_ptr(), -1, more.as_mut_ptr()),
                STAT_ERR_VALUE
            );
        }
    }
}
