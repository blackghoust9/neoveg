//! The things the host owns and a plug-in holds handles to: sets of properties, and the descriptor
//! of an effect (its properties, its parameters and its clips).
//!
//! A handle given to a plug-in is the address of a leaked box, and every address handed out is kept
//! in a list of live handles: a call that brings any other address is refused with a status instead
//! of being trusted. The plug-in is called from one thread (the host describes one plug-in at a
//! time), so the sets themselves are not locked.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};

use crate::abi::{
    OfxPropertySetHandle, OfxStatus, STAT_ERR_BAD_HANDLE, STAT_ERR_BAD_INDEX, STAT_ERR_UNKNOWN,
};

/// One value of a property (a property holds one or more, all of one kind).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i32),
    Double(f64),
    Str(CString),
    Ptr(usize),
}

/// A set of named properties.
#[derive(Debug, Default)]
pub struct PropertySet {
    values: HashMap<String, Vec<Value>>,
}

impl PropertySet {
    /// Puts `value` at `index` of `name`: an index that is there is replaced, the one after the last
    /// extends the property, any other is refused.
    pub fn set(&mut self, name: &str, index: usize, value: Value) -> Result<(), OfxStatus> {
        let list = self.values.entry(name.to_owned()).or_default();
        match index.cmp(&list.len()) {
            std::cmp::Ordering::Less => list[index] = value,
            std::cmp::Ordering::Equal => list.push(value),
            std::cmp::Ordering::Greater => return Err(STAT_ERR_BAD_INDEX),
        }
        Ok(())
    }

    /// The value at `index` of `name`: an unknown property and an index past the end are different
    /// answers, as in the standard.
    pub fn get(&self, name: &str, index: usize) -> Result<&Value, OfxStatus> {
        let list = self.values.get(name).ok_or(STAT_ERR_UNKNOWN)?;
        list.get(index).ok_or(STAT_ERR_BAD_INDEX)
    }

    /// How many values `name` holds (`Err` when the property is unknown).
    pub fn dimension(&self, name: &str) -> Result<usize, OfxStatus> {
        self.values.get(name).map(Vec::len).ok_or(STAT_ERR_UNKNOWN)
    }

    /// Forgets the values of `name`.
    pub fn reset(&mut self, name: &str) -> Result<(), OfxStatus> {
        self.values.remove(name).map(|_| ()).ok_or(STAT_ERR_UNKNOWN)
    }

    /// The first value of `name` as text, when it is a string.
    pub fn string(&self, name: &str) -> Option<String> {
        match self.get(name, 0) {
            Ok(Value::Str(text)) => Some(text.to_string_lossy().into_owned()),
            _ => None,
        }
    }

    /// Every value of `name` that is a string.
    pub fn strings(&self, name: &str) -> Vec<String> {
        self.values
            .get(name)
            .map(|list| {
                list.iter()
                    .filter_map(|v| match v {
                        Value::Str(text) => Some(text.to_string_lossy().into_owned()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Puts one string at index 0.
    pub fn put_string(&mut self, name: &str, text: &str) {
        let _ = self.set(name, 0, Value::Str(cstring(text)));
    }

    /// Puts a list of strings, replacing what was there.
    pub fn put_strings(&mut self, name: &str, texts: &[&str]) {
        self.values.insert(
            name.to_owned(),
            texts.iter().map(|t| Value::Str(cstring(t))).collect(),
        );
    }

    /// Puts a list of integers, replacing what was there.
    pub fn put_ints(&mut self, name: &str, numbers: &[i32]) {
        self.values.insert(
            name.to_owned(),
            numbers.iter().map(|n| Value::Int(*n)).collect(),
        );
    }
}

/// A C string from `text` (a character that cannot be in one is dropped).
pub fn cstring(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// What a plug-in describes: its properties first (so that the address of a descriptor is also the
/// address of its set of properties), then the parameters and clips it defined with the name, the
/// kind for a parameter and the handle of its own set of properties.
#[repr(C)]
#[derive(Debug, Default)]
pub struct Descriptor {
    pub props: PropertySet,
    pub params: Vec<(String, String, usize)>,
    pub clips: Vec<(String, usize)>,
}

fn live() -> &'static Mutex<HashSet<usize>> {
    static LIVE: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    LIVE.get_or_init(Mutex::default)
}

fn register(address: usize) {
    if let Ok(mut set) = live().lock() {
        set.insert(address);
    }
}

/// `true` when `address` was handed out by this host.
pub fn is_live(address: usize) -> bool {
    live().lock().is_ok_and(|set| set.contains(&address))
}

/// A new set of properties; the handle stays valid until the process ends.
pub fn new_set() -> OfxPropertySetHandle {
    let address = Box::into_raw(Box::<PropertySet>::default()) as usize;
    register(address);
    address as OfxPropertySetHandle
}

/// A new descriptor; the address is the one of its properties too.
pub fn new_descriptor() -> *mut Descriptor {
    let pointer = Box::into_raw(Box::<Descriptor>::default());
    register(pointer as usize);
    pointer
}

/// The set behind `handle`, or the status to answer when the handle is not one of ours.
///
/// # Safety
/// `handle` is only dereferenced when it is in the list of the addresses this host handed out, which
/// are all addresses of leaked boxes that are never freed; calls come from one thread.
pub unsafe fn set_of<'a>(handle: OfxPropertySetHandle) -> Result<&'a mut PropertySet, OfxStatus> {
    if handle.is_null() || !is_live(handle as usize) {
        return Err(STAT_ERR_BAD_HANDLE);
    }
    Ok(unsafe { &mut *(handle as *mut PropertySet) })
}

/// The descriptor behind `handle` (an effect handle or a set of parameters), or the status to answer.
///
/// # Safety
/// As [`set_of`]: the address has to be one that [`new_descriptor`] handed out, which the list of
/// live handles cannot tell from the address of a plain set; a plug-in that passes a set where an
/// effect is expected is a plug-in that breaks the standard, and the descriptor list is checked too.
pub unsafe fn descriptor_of<'a>(
    handle: *mut std::ffi::c_void,
) -> Result<&'a mut Descriptor, OfxStatus> {
    if handle.is_null() || !is_descriptor(handle as usize) {
        return Err(STAT_ERR_BAD_HANDLE);
    }
    Ok(unsafe { &mut *(handle as *mut Descriptor) })
}

fn descriptors() -> &'static Mutex<HashSet<usize>> {
    static DESCRIPTORS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    DESCRIPTORS.get_or_init(Mutex::default)
}

fn is_descriptor(address: usize) -> bool {
    descriptors().lock().is_ok_and(|set| set.contains(&address))
}

/// A new descriptor that [`descriptor_of`] accepts.
pub fn new_effect() -> *mut Descriptor {
    let pointer = new_descriptor();
    if let Ok(mut set) = descriptors().lock() {
        set.insert(pointer as usize);
    }
    pointer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_grows_one_value_at_a_time_and_answers_like_the_standard() {
        let mut set = PropertySet::default();
        set.set("OfxPropLabel", 0, Value::Str(cstring("Invert")))
            .unwrap();
        set.set("OfxPropLabel", 1, Value::Str(cstring("Second")))
            .unwrap();
        assert_eq!(set.dimension("OfxPropLabel"), Ok(2));
        assert_eq!(set.string("OfxPropLabel").as_deref(), Some("Invert"));
        assert_eq!(
            set.set("OfxPropLabel", 5, Value::Int(1)),
            Err(STAT_ERR_BAD_INDEX)
        );
        assert_eq!(set.get("Nope", 0), Err(STAT_ERR_UNKNOWN));
        assert_eq!(set.get("OfxPropLabel", 2), Err(STAT_ERR_BAD_INDEX));
        // An index that is there is replaced, not extended
        set.set("OfxPropLabel", 0, Value::Str(cstring("Other")))
            .unwrap();
        assert_eq!(set.dimension("OfxPropLabel"), Ok(2));
        assert_eq!(set.strings("OfxPropLabel"), vec!["Other", "Second"]);
        set.reset("OfxPropLabel").unwrap();
        assert_eq!(set.dimension("OfxPropLabel"), Err(STAT_ERR_UNKNOWN));
    }

    #[test]
    fn only_the_handles_the_host_made_are_trusted() {
        let handle = new_set();
        assert!(unsafe { set_of(handle) }.is_ok());
        assert_eq!(
            unsafe { set_of(std::ptr::null_mut()) }.err(),
            Some(STAT_ERR_BAD_HANDLE)
        );
        let stranger = 0x1234_usize as OfxPropertySetHandle;
        assert_eq!(unsafe { set_of(stranger) }.err(), Some(STAT_ERR_BAD_HANDLE));
        // A plain set is not an effect
        assert_eq!(
            unsafe { descriptor_of(handle.cast()) }.err(),
            Some(STAT_ERR_BAD_HANDLE)
        );
        let effect = new_effect();
        assert!(unsafe { descriptor_of(effect.cast()) }.is_ok());
        // The address of an effect is also the address of its properties
        assert!(unsafe { set_of(effect.cast()) }.is_ok());
    }

    #[test]
    fn a_text_with_a_nul_character_still_makes_a_string() {
        assert_eq!(cstring("a\0b").to_str().unwrap(), "ab");
    }
}
