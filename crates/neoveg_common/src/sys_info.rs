//! What the program can tell about the machine it runs on: operating system, processor, installed
//! memory and the user name, for the startup screen. The graphics adapter is not here: it is what
//! `wgpu` reports once the window exists, so the interface adds it.
//!
//! Collecting takes a moment (the system is asked about its processors and memory), so callers do it
//! off the interface thread. The formatting is in small pure functions that are tested.

use sysinfo::System;

/// The facts shown on the startup screen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemInfo {
    /// Operating system and architecture, e.g. `Windows 11 Pro 64-bit`.
    pub os: String,
    /// Processor model, e.g. `Intel Core i9-13900K`.
    pub cpu: String,
    /// Installed memory, e.g. `64 GB`.
    pub memory: String,
    /// The user name of the operating system session.
    pub user: String,
}

impl SystemInfo {
    /// Asks the system. Anything it cannot tell is shown as "Desconhecido" (never a panic).
    pub fn collect() -> Self {
        let mut system = System::new();
        system.refresh_memory();
        system.refresh_cpu_all();
        let os_name = System::long_os_version()
            .or_else(|| match (System::name(), System::os_version()) {
                (Some(name), Some(version)) => Some(format!("{name} {version}")),
                (Some(name), None) => Some(name),
                _ => None,
            })
            .unwrap_or_default();
        let cpu = system
            .cpus()
            .first()
            .map(|c| clean_cpu_name(c.brand()))
            .unwrap_or_default();
        Self {
            os: format_os(&os_name, std::env::consts::ARCH),
            cpu: if cpu.is_empty() {
                "Desconhecido".into()
            } else {
                cpu
            },
            memory: format_memory(system.total_memory()),
            user: non_empty(whoami::username(), "Desconhecido"),
        }
    }
}

/// Installed memory in bytes, `None` when the system does not say.
pub fn total_memory_bytes() -> Option<u64> {
    let mut system = System::new();
    system.refresh_memory();
    Some(system.total_memory()).filter(|&bytes| bytes > 0)
}

/// How much the picture caches may hold on a machine with `total` bytes of memory: the full budget
/// from 16 GiB up, half of it from 8 GiB, a quarter below (and when the memory is unknown, half).
pub fn scale_cache_budget(full: usize, total: Option<u64>) -> usize {
    const GIB: u64 = 1 << 30;
    match total {
        Some(t) if t >= 16 * GIB => full,
        Some(t) if t >= 8 * GIB => full / 2,
        Some(_) => full / 4,
        None => full / 2,
    }
}

/// How many video decoders may stay open at once: 6 from 16 GiB up, 4 from 8 GiB, 2 below (and 4
/// when the memory is unknown). Each one holds its own frame pool, which is large for 4K.
pub fn max_open_decoders(total: Option<u64>) -> usize {
    const GIB: u64 = 1 << 30;
    match total {
        Some(t) if t >= 16 * GIB => 6,
        Some(t) if t >= 8 * GIB => 4,
        Some(_) => 2,
        None => 4,
    }
}

fn non_empty(text: String, fallback: &str) -> String {
    if text.trim().is_empty() {
        fallback.to_owned()
    } else {
        text.trim().to_owned()
    }
}

/// The operating system with its word size: `Windows 11 Pro` and `x86_64` give `Windows 11 Pro
/// 64-bit`.
pub fn format_os(name: &str, arch: &str) -> String {
    let bits = match arch {
        "x86_64" | "aarch64" | "arm64" | "riscv64" | "powerpc64" | "s390x" | "loongarch64" => {
            "64-bit"
        }
        "x86" | "arm" | "riscv32" => "32-bit",
        _ => "",
    };
    let name = name.trim();
    match (name.is_empty(), bits.is_empty()) {
        (true, _) => "Desconhecido".to_owned(),
        (false, true) => name.to_owned(),
        (false, false) => format!("{name} {bits}"),
    }
}

/// The processor name without the legal marks and the clock: `Intel(R) Core(TM) i9-13900K CPU @
/// 3.00GHz` gives `Intel Core i9-13900K`.
pub fn clean_cpu_name(brand: &str) -> String {
    let mut name = brand
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace("(tm)", "");
    if let Some(at) = name.find(" @ ") {
        name.truncate(at);
    }
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|w| !matches!(*w, "CPU" | "Processor" | "processor"))
        .collect();
    words.join(" ")
}

/// Installed memory in whole gigabytes (binary), `64 GB`; with a fraction below 8 GB (`7.8 GB`) so a
/// small machine is not rounded up to something it is not. Reported memory is a little under the
/// installed size, so a value within 3% under a whole number is shown as that number.
pub fn format_memory(bytes: u64) -> String {
    if bytes == 0 {
        return "Desconhecida".to_owned();
    }
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let rounded = gib.round();
    if rounded >= 8.0 || (gib - rounded).abs() < 0.03 * rounded.max(1.0) {
        format!("{} GB", rounded as u64)
    } else {
        format!("{gib:.1} GB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn the_picture_caches_shrink_on_small_machines() {
        let full = 96 * 1024 * 1024;
        assert_eq!(scale_cache_budget(full, Some(32 * GIB)), full);
        assert_eq!(scale_cache_budget(full, Some(16 * GIB)), full);
        assert_eq!(scale_cache_budget(full, Some(8 * GIB)), full / 2);
        assert_eq!(scale_cache_budget(full, Some(4 * GIB)), full / 4);
        assert_eq!(scale_cache_budget(full, None), full / 2);
    }

    #[test]
    fn small_machines_keep_fewer_decoders_open() {
        assert_eq!(max_open_decoders(Some(32 * GIB)), 6);
        assert_eq!(max_open_decoders(Some(8 * GIB)), 4);
        assert_eq!(max_open_decoders(Some(4 * GIB)), 2);
        assert_eq!(max_open_decoders(None), 4);
    }

    #[test]
    fn the_operating_system_gets_its_word_size() {
        assert_eq!(
            format_os("Windows 11 Pro", "x86_64"),
            "Windows 11 Pro 64-bit"
        );
        assert_eq!(format_os("Ubuntu 24.04", "aarch64"), "Ubuntu 24.04 64-bit");
        assert_eq!(format_os("Debian", "x86"), "Debian 32-bit");
        assert_eq!(format_os("Plan 9", "mips"), "Plan 9");
        assert_eq!(format_os("  ", "x86_64"), "Desconhecido");
    }

    #[test]
    fn processor_names_lose_the_legal_marks_and_the_clock() {
        assert_eq!(
            clean_cpu_name("Intel(R) Core(TM) i9-13900K CPU @ 3.00GHz"),
            "Intel Core i9-13900K"
        );
        assert_eq!(
            clean_cpu_name("AMD Ryzen 9 7950X 16-Core Processor"),
            "AMD Ryzen 9 7950X 16-Core"
        );
        assert_eq!(clean_cpu_name("Apple M2 Max"), "Apple M2 Max");
        assert_eq!(clean_cpu_name(""), "");
    }

    #[test]
    fn memory_is_shown_in_whole_gigabytes_close_to_what_is_installed() {
        assert_eq!(format_memory(64 * GIB), "64 GB");
        // The system reports a little less than installed
        assert_eq!(format_memory(16 * GIB - 200 * 1024 * 1024), "16 GB");
        assert_eq!(format_memory(8 * GIB), "8 GB");
        assert_eq!(format_memory(6 * GIB), "6 GB");
        assert_eq!(
            format_memory(7 * GIB / 2),
            "3.5 GB",
            "a small machine keeps its fraction"
        );
        assert_eq!(format_memory(0), "Desconhecida");
    }

    #[test]
    fn collecting_gives_a_filled_in_record_on_any_machine() {
        let info = SystemInfo::collect();
        for (name, value) in [
            ("os", &info.os),
            ("cpu", &info.cpu),
            ("memory", &info.memory),
            ("user", &info.user),
        ] {
            assert!(!value.trim().is_empty(), "{name} is empty");
        }
        assert!(info.memory.ends_with("GB") || info.memory == "Desconhecida");
    }
}
