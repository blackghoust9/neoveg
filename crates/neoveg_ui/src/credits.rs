//! "Créditos e licenças" window: who wrote the code and libraries Neoveg is built on.
//!
//! The list mirrors `AUTHORS.md` and `third_party/README.md`; a test keeps the FilmCraft entry
//! (adapted source code, so its licence text must ship with Neoveg) in sync with them.

use eframe::egui;

pub struct Credit {
    pub name: &'static str,
    /// What Neoveg uses it for.
    pub used_for: &'static str,
    pub authors: &'static str,
    /// SPDX expression.
    pub license: &'static str,
    pub url: &'static str,
}

pub const CREDITS: &[Credit] = &[
    Credit {
        name: "Neoveg",
        used_for: "Editor de vídeo e áudio: timeline, motor de edição, mixer e interface",
        authors: "Neoveg Contributors",
        license: "MIT",
        url: "https://github.com/blackghoust9/neoveg",
    },
    Credit {
        name: "FilmCraft (filmcraft-audio-dsp)",
        used_for: "Medidor de loudness ITU-R BS.1770-4 / EBU R128 (loudness momentâneo, de curto \
                   prazo e integrado, faixa de loudness e pico verdadeiro) e efeitos de áudio das \
                   faixas e do master (equalizadores, compressor, gate, limitador, delay e reverb); \
                   código adaptado em neoveg_audio::dsp",
        authors: "ArtCraft Team and the FilmCraft contributors",
        license: "MIT OR Apache-2.0 (usado sob a MIT)",
        url: "https://github.com/storytold/filmcraft",
    },
    Credit {
        name: "egui / eframe",
        used_for: "Interface gráfica",
        authors: "Emil Ernerfeldt and contributors",
        license: "MIT OR Apache-2.0",
        url: "https://github.com/emilk/egui",
    },
    Credit {
        name: "egui_dock",
        used_for: "Painéis acopláveis",
        authors: "egui_dock contributors",
        license: "MIT",
        url: "https://github.com/Adanos020/egui_dock",
    },
    Credit {
        name: "wgpu",
        used_for: "Vídeo e interface na GPU (WebGPU)",
        authors: "wgpu contributors",
        license: "MIT OR Apache-2.0",
        url: "https://github.com/gfx-rs/wgpu",
    },
    Credit {
        name: "cpal",
        used_for: "Saída de áudio no hardware",
        authors: "RustAudio contributors",
        license: "Apache-2.0",
        url: "https://github.com/RustAudio/cpal",
    },
    Credit {
        name: "Symphonia",
        used_for: "Decodificação de áudio",
        authors: "Symphonia contributors",
        license: "MPL-2.0",
        url: "https://github.com/pdeljanov/Symphonia",
    },
    Credit {
        name: "FFmpeg (opcional)",
        used_for: "Decodificação de vídeo (recurso `ffmpeg`, biblioteca compartilhada)",
        authors: "FFmpeg developers",
        license: "LGPL-2.1-or-later",
        url: "https://ffmpeg.org",
    },
    Credit {
        name: "serde / serde_json",
        used_for: "Arquivos de projeto e preferências",
        authors: "Erick Tryzelaar, David Tolnay and contributors",
        license: "MIT OR Apache-2.0",
        url: "https://serde.rs",
    },
    Credit {
        name: "zip, rfd, tracing",
        used_for: "Contêiner .neoveg, diálogos de arquivo e logs",
        authors: "zip-rs, rfd and Tokio contributors",
        license: "MIT",
        url: "https://crates.io",
    },
];

/// Contents of the window.
pub fn credits_ui(ui: &mut egui::Ui) {
    ui.label(
        "O Neoveg é distribuído sob a licença MIT. Ele usa e adapta o trabalho de terceiros \
         listado abaixo; as licenças completas estão em AUTHORS.md e em third_party/.",
    );
    ui.add_space(6.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for credit in CREDITS {
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong(credit.name);
                ui.label(format!("· {}", credit.license));
            });
            ui.label(credit.used_for);
            ui.horizontal(|ui| {
                ui.weak(credit.authors);
                ui.hyperlink_to("página do projeto", credit.url);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_file(path: &str) -> String {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn every_credit_has_every_field() {
        assert!(!CREDITS.is_empty());
        for c in CREDITS {
            for field in [c.name, c.used_for, c.authors, c.license, c.url] {
                assert!(!field.trim().is_empty(), "{} has an empty field", c.name);
            }
            assert!(c.url.starts_with("https://"), "{}", c.name);
        }
    }

    #[test]
    fn the_adapted_filmcraft_code_is_credited_with_its_licence_and_author() {
        let filmcraft = CREDITS
            .iter()
            .find(|c| c.name.contains("FilmCraft"))
            .expect("FilmCraft credit");
        assert!(filmcraft.license.contains("MIT"));
        assert!(filmcraft.authors.contains("FilmCraft contributors"));

        // The licence text shipped with the sources and with the packages
        let licence = repo_file("third_party/filmcraft/LICENSE-MIT");
        assert!(licence.contains("ArtCraft Team and the FilmCraft contributors"));
        assert!(licence.contains("Permission is hereby granted"));
        for doc in ["AUTHORS.md", "third_party/README.md"] {
            assert!(
                repo_file(doc).contains("filmcraft"),
                "{doc} does not mention filmcraft"
            );
        }
    }

    #[test]
    fn every_adapted_source_file_carries_the_attribution_header() {
        for file in [
            "biquad.rs",
            "loudness.rs",
            "oversample.rs",
            "smooth.rs",
            "effect.rs",
            "effects/mod.rs",
            "effects/eq.rs",
            "effects/dynamics.rs",
            "effects/time.rs",
        ] {
            let text = repo_file(&format!("crates/neoveg_audio/src/dsp/{file}"));
            assert!(
                text.contains("Adapted from `filmcraft-audio-dsp`")
                    && text.contains("ArtCraft Team and the FilmCraft contributors"),
                "{file} lost its attribution header"
            );
        }
    }
}
