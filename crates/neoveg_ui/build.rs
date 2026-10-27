//! Build script of the application crate.
//!
//! * Writes the date of the build as `NEOVEG_BUILD_DATE` (`dd/mm/aaaa`) for the startup screen.
//!   `SOURCE_DATE_EPOCH` makes it reproducible.
//! * On Windows, embeds the "Nv" icon (`assets/app_icon/app_icon.ico`) and the version information in
//!   the executable with `winres`, so the file shows the program's icon in Explorer, the task bar
//!   and the alt-tab list. A failure to compile the resource (no Windows SDK) is a warning, not an
//!   error: the program still builds, only without the embedded icon.

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rustc-env=NEOVEG_BUILD_DATE={}", build_date());
    embed_windows_resources();
}

/// `dd/mm/aaaa` for the day of the build (UTC).
fn build_date() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        })
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{day:02}/{month:02}/{year:04}")
}

/// Calendar date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(windows)]
fn embed_windows_resources() {
    let icon = "../../assets/app_icon/app_icon.ico";
    println!("cargo:rerun-if-changed={icon}");
    let mut resource = winres::WindowsResource::new();
    resource.set_icon(icon);
    resource.set("ProductName", "Neoveg");
    resource.set(
        "FileDescription",
        "Neoveg - editor nao linear de video e audio",
    );
    resource.set("LegalCopyright", "Neoveg. MIT OR Apache-2.0");
    if let Err(error) = resource.compile() {
        println!("cargo:warning=nao foi possivel embutir o icone no executavel: {error}");
    }
}

#[cfg(not(windows))]
fn embed_windows_resources() {}
