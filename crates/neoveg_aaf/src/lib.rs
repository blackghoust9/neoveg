// Adapted from FilmCraft `crates/interchange` and `crates/time` (storytold/filmcraft, MIT OR Apache-2.0, ArtCraft Team and the FilmCraft contributors); see third_party/filmcraft/README.md.
//! AAF (Advanced Authoring Format) import and export, following the AAF Edit Protocol.
//!
//! Written from the public AMWA specifications (AAF Object Specification v1.1, the AAF Low-Level
//! Container Specification and the AAF Edit Protocol v1.0); the container is Microsoft's compound
//! file ([`neoveg_cfb`]). This crate reads and writes the format-neutral [`model::Document`]; the
//! engine maps a timeline to and from it.
//!
//! The exported file holds one top-level composition mob with a timecode slot, one timeline slot
//! per track (a sequence of fillers, source clips and transitions), clip gain as an operation
//! group, markers as comment markers, and for every media item a master mob with a file source mob
//! (a locator with the path) and a tape source mob that carries the start timecode.

#![allow(clippy::needless_range_loop, clippy::type_complexity)]

mod ids;
pub mod model;
mod read;
mod store;
mod support;
pub mod time;
mod write;

pub use support::{
    Error, Level, Report, ReportEntry, Result, file_stem, file_url_to_path, path_to_file_url,
};

use model::Document;

/// Writes `doc` as an AAF file: the first composition becomes the top-level composition mob.
/// `small_sectors` writes 512-byte sectors (compound file version 3) instead of 4096-byte ones.
pub fn write_aaf(doc: &Document, small_sectors: bool) -> Result<(Vec<u8>, Report)> {
    let mut report = Report::default();
    let version = if small_sectors {
        neoveg_cfb::Version::V3
    } else {
        neoveg_cfb::Version::V4
    };
    let bytes = write::write(doc, version, &mut report)?;
    Ok((bytes, report))
}

/// Reads an AAF file into the neutral model.
pub fn read_aaf(bytes: &[u8]) -> Result<(Document, Report)> {
    let mut report = Report::default();
    let doc = read::read(bytes, &mut report)?;
    Ok((doc, report))
}

/// Whether `bytes` look like an AAF file (a compound file whose root has the AAF root class or a
/// `Header-2` storage).
pub fn sniff(bytes: &[u8]) -> bool {
    if !neoveg_cfb::sniff(bytes) {
        return false;
    }
    match neoveg_cfb::CompoundFile::open(bytes) {
        Ok(cf) => cf.root().clsid == ids::ROOT || cf.find("Header-2").is_ok(),
        Err(_) => false,
    }
}
