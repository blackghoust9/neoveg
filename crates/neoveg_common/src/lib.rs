//! Shared types for every Neoveg crate.

pub mod alerts;
pub mod channels;
pub mod chroma;
pub mod clock;
pub mod command_palette;
pub mod command_template;
pub mod commands;
pub mod composite;
pub mod effect_preset;
pub mod envelope;
pub mod fade;
pub mod fx_markers;
pub mod fx_mask;
pub mod grade;
pub mod i18n;
pub mod keymap;
pub mod keymap_sets;
pub mod logging;
pub mod loudness_target;
pub mod mask;
pub mod media_props;
pub mod pan;
pub mod pan_crop;
pub mod paths;
pub mod pitch_change;
pub mod preferences;
pub mod presets;
pub mod profiles;
pub mod project;
pub mod quick_export;
pub mod rename_pattern;
pub mod ruler_format;
pub mod sys_info;
pub mod theme;
pub mod time_input;
pub mod timecode;
pub mod title_style;
pub mod toolbar;
pub mod transcript;
pub mod window_layout;

/// Folder for the files that the tests write: inside the repository's `target/`, not in the system
/// temporary folder (on some Windows setups the test process cannot create folders there).
#[cfg(test)]
pub(crate) fn test_scratch_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("neoveg_test_tmp");
    std::fs::create_dir_all(&dir).expect("test scratch folder");
    dir
}
