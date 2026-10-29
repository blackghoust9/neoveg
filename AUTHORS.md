# AUTHORS & Third-Party Credits — Neoveg

Este documento cataloga os autores, mantenedores e licenças de todos os componentes, bibliotecas, SDKs nativos e modelos de Inteligência Artificial integrados ao ecossistema **Neoveg** (`C# Avalonia UI`, `Rust neoveg_core`, `C/C++ neoveg_cpp_engine` e `Python Backend`).

---

## 1. Arquitetura Principal do Projeto (`Neoveg`)

- **Neoveg Core Team & Contributors**
  - Arquitetura NLE Multiplataforma (`desktop-host/`, `rust-core/`, `cpp-extensions/`, `python-backend/`)
  - Licença do código-fonte próprio: **MIT License** (consulte o ficheiro [`LICENSE`](LICENSE)), respeitando os termos específicos de interoperabilidade dinâmica com bibliotecas sob **LGPL-2.1**, **GPL-2.0/3.0**, **Apache-2.0** e **BSD-3-Clause**.

---

## 1-A. Código de terceiros adaptado no workspace Rust (`crates/`)

| Origem | Autores / Organização | Licença (SPDX) | Onde está no Neoveg | Utilização |
| :--- | :--- | :--- | :--- | :--- |
| **FilmCraft — `filmcraft-text`** ([`storytold/filmcraft`](https://github.com/storytold/filmcraft), commit `8d925e9`) | ArtCraft Team and the FilmCraft contributors | `MIT OR Apache-2.0` (usado sob a `MIT`) | `crates/neoveg_text/` | Motor de texto: composição, fontes e rasterização dos títulos |
| **Inter, JetBrains Mono e Noto Serif** (fontes) | The Inter Project Authors; The JetBrains Mono Project Authors; The Noto Project Authors | `OFL-1.1` (textos em `crates/neoveg_text/fonts/OFL-*.txt`) | `crates/neoveg_text/fonts/` (arquivos sem alteração, embutidos no programa) | Fontes padrão dos títulos |
| **FilmCraft — `filmcraft-audio-dsp`** ([`storytold/filmcraft`](https://github.com/storytold/filmcraft), commit `8fda2f2`) | ArtCraft Team and the FilmCraft contributors | `MIT OR Apache-2.0` (usado sob a `MIT`) | `crates/neoveg_audio/src/dsp/` (`loudness.rs`, `biquad.rs`, `oversample.rs`, `smooth.rs`, `effect.rs`, `design.rs`, `fft.rs`, `ducking.rs`, `effects/`) `crates/neoveg_engine/src/captions.rs` (legendas) e, como fórmulas reescritas, os efeitos de cor de `crates/neoveg_video/src/vfx.rs` e `vfx_gpu.rs` | Medidor de loudness ITU-R BS.1770-4 / EBU R128 do painel **Loudness**, os efeitos de áudio das faixas e do master (equalizadores, filtros, compressor, gate, limitador, delay, reverb, DeHum e DeNoise) e a análise do auto-ducking |

| **nnnoiseless** ([`jneem/nnnoiseless`](https://github.com/jneem/nnnoiseless) 0.5.2), porte do **RNNoise** | Joe Neeman; RNNoise: Jean-Marc Valin, Mozilla Corporation, Xiph.Org Foundation | `BSD-3-Clause` | Dependência do Cargo de `neoveg_audio` (não copiada); texto em [`third_party/nnnoiseless/COPYING`](third_party/nnnoiseless/COPYING) | Efeito **Redução de ruído por IA (RNNoise)** |
| **vst3 (`vst3-rs`)** ([`coupler-rs/vst3-rs`](https://github.com/coupler-rs/vst3-rs) 0.3) | Micah Johnston and the vst3-rs contributors | `MIT OR Apache-2.0` | Dependência do Cargo de `neoveg_audio` (não copiada); o plug-in de ganho de exemplo está em `crates/neoveg_audio/src/vst3_host/test_gain.rs`, só para testes | Host de plug-ins de áudio **VST3** (`.vst3`) em `crates/neoveg_audio/src/vst3_host.rs` |
| **clack (`clack-host` / `clack-extensions`)** ([`prokopyl/clack`](https://github.com/prokopyl/clack) 0.2) | Prokopyl and the Clack contributors | `MIT OR Apache-2.0` | Dependência do Cargo de `neoveg_audio` (não copiada) | Host de plug-ins de áudio **CLAP** (`.clap`) em `crates/neoveg_audio/src/clap_host.rs`, com compensação de latência (PDC) e automação |
| **Candle (`candle-core` / `candle-nn` / `candle-transformers`)** ([`huggingface/candle`](https://github.com/huggingface/candle) 0.11) | Laurent Mazare, Hugging Face and the Candle contributors | `MIT OR Apache-2.0` | Dependência do Cargo de `neoveg_audio` (não copiada) | Transcrição automática de fala para legendas com **Whisper** em Rust puro (`crates/neoveg_audio/src/whisper.rs`, pesos `.safetensors` e `.gguf`) |
| **roxmltree** ([`RazrFalcon/roxmltree`](https://github.com/RazrFalcon/roxmltree)) | Yevhenii Reizner | `MIT OR Apache-2.0` | Dependência do Cargo de `neoveg_engine` (não copiada) | Leitura do XML do Final Cut Pro 7 e FCPXML |
| **image (`image-rs`)** ([`image-rs/image`](https://github.com/image-rs/image) 0.25) | The image-rs Developers | `MIT OR Apache-2.0` | Dependência do Cargo de `neoveg_video` (não copiada) | Importação e decodificação em Rust puro de imagens estáticas (PNG, JPEG, BMP, GIF, WebP, TIFF, TGA, ICO, PNM) e sequências numeradas de quadros (`crates/neoveg_video/src/still.rs`) |
| **Formato CMX 3600** (listas de edição `.edl`) | Especificação pública do formato (CMX Systems) | — (formato, não código) | `crates/neoveg_engine/src/edl.rs` (código próprio do Neoveg) | Importar e exportar listas de edição; o tratamento dos comentários `* FROM CLIP NAME:` e `* SOURCE FILE:` foi conferido contra a documentação do módulo `edl` do FilmCraft, sem copiar código |

- Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. O texto da licença MIT está em [`third_party/filmcraft/LICENSE-MIT`](third_party/filmcraft/LICENSE-MIT) e vai em todos os pacotes; a origem, a lista de arquivos e as alterações feitas estão em [`third_party/filmcraft/README.md`](third_party/filmcraft/README.md).
- A marca, o nome e os logotipos da ArtCraft **não** fazem parte da licença do FilmCraft e não são usados no Neoveg.
- Os créditos também aparecem na interface, em **Ajuda → Créditos e licenças…**.

---

## 1-B. Arte própria

- **Tela de carregamento** (`crates/neoveg_ui/src/splash.rs`): as formas 3D, o carretel de filme com a fita de quadros, o playhead e a animação do nome NEOVEG são trabalho original do Neoveg, desenhados em código com formas do egui, sem imagens nem fontes de terceiros, e liberados com o projeto sob a licença **MIT**. Nenhuma arte, ícone ou imagem de outro editor foi usada como base.

---

## 2. Camada Desktop & Interface Gráfica (`desktop-host/Neoveg.csproj` — C# / .NET)

| Pacote / Biblioteca | Versão | Autores / Organização | Licença (SPDX) | Utilização no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **`Avalonia`** | `11.2.3` | AvaloniaUI OÜ & Contributors | `MIT` | Motor principal de UI XAML multiplataforma (Windows, Linux, macOS) |
| **`Avalonia.Desktop`** | `11.2.3` | AvaloniaUI OÜ & Contributors | `MIT` | Backends de janela nativa Win32, X11/Wayland e Cocoa/macOS |
| **`Avalonia.Themes.Fluent`** | `11.2.3` | AvaloniaUI OÜ & Contributors | `MIT` | Base de estilos de controles para o `NeovegThemeManager` |
| **`Avalonia.Fonts.Inter`** | `11.2.3` | Rasmus Andersson & AvaloniaUI OÜ | `OFL-1.1` / `MIT` | Tipografia de alta legibilidade em timecodes, réguas e inspetores |
| **`Microsoft.CodeAnalysis.CSharp.Scripting`** | `4.11.0` | Microsoft Corporation & .NET Foundation | `MIT` | Compilador e motor dinâmico **Roslyn C#** (`Neoveg.Scripting` / `.cs` & `.dll`) |
| **`SkiaSharp` / `HarfBuzzSharp`** | `2.88.x` | Google Inc., Microsoft & Mono Project | `MIT` | Renderização 2D acelerada por GPU (Timeline, Gizmo `'F'` Pan/Crop, Scopes, `RenderStatusRuler`) |
| **`.NET Runtime (`net10.0`)`** | `10.0` | Microsoft Corporation & .NET Foundation | `MIT` | Runtime base, `NativeLibrary` P/Invoke e `MemoryMappedFile` IPC |

---

## 3. Motor NLE & DSP em Rust (`rust-core/Cargo.toml` — `neoveg_core`)

| Crate / Componente | Versão | Autores / Organização | Licença (SPDX) | Utilização no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **`neoveg-core` (`neoveg_core`)** | `2.0.0` | Neoveg Core Team | `MIT` | Biblioteca `cdylib`/`rlib` C-ABI (12 exportações `neoveg_*`: SMPTE, 5 curvas de Fade, Split, Auto-Crossfade, cache `.sfk`, RAM Pool e `neoveg_get_timeline_render_status`) |
| **`Rust Standard Library (`std`)`** | `Edition 2021` | The Rust Project Developers | `MIT OR Apache-2.0` | Primitivas FFI (`std::ffi`, `std::os::raw`), concorrência e alocação segura sem GC |

---

## 4. Backend de Mídia, Empacotamento e Automação (`python-backend/requirements.txt` — Python)

| Pacote Python | Versão Mínima | Autores / Mantenedores | Licença (SPDX) | Utilização no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **`pytest`** | `>=7.4.0` | Holger Krekel & Pytest-Dev Team | `MIT` | Suíte de testes automatizados e auditoria de conformidade ISO/IEC 29119 |
| **`pytest-asyncio`** | `>=0.21.0` | Tin Tvrtković & Contributors | `Apache-2.0` | Suporte a testes assíncronos no backend Python |
| **`pytest-cov`** | `>=5.0.0` | Marc Schlaich & Contributors | `MIT` | Coleta de métricas de cobertura de código (`coverage.xml`) |
| **`numpy`** | `>=1.24.0` | Travis Oliphant & NumPy Developers | `BSD-3-Clause` | Processamento matricial de áudio PCM, picos `.sfk`, LUTs 3D e máscaras Chroma Key |
| **`scipy`** | `>=1.10.0` | Travis Oliphant, Pearu Peterson & SciPy Developers | `BSD-3-Clause` | Processamento digital de sinais (DSP), filtros de áudio e interpolação científica |
| **`pillow` (PIL Fork)** | `>=10.0.0` | Jeffrey A. Clark (Alex) & Contributors | `HPND` (`MIT-CMU`) | Geração de ícones `.ico`/`.png`, quadros de teste e assets de temas históricos |
| **`pefile`** | `>=2024.8.26` | Ero Carrera | `MIT` | Inspeção de cabeçalhos PE32+/DLL e tabelas de exportação C-ABI no Windows |
| **`packaging`** | `>=24.0` | Donald Stufft & PyPA Contributors | `Apache-2.0 OR BSD-2-Clause` | Resolução de versões de pacotes e validação de ambiente multiplataforma |
| **`typing_extensions`** | `>=4.10.0` | Guido van Rossum, Jukka Lehtosalo & Python Typing Team | `PSF-2.0` | Anotações estáticas de tipos na ponte Python ↔ Rust ↔ C++ |
| **`requests`** | `>=2.31.0` | Kenneth Reitz & Python Software Foundation | `Apache-2.0` | Sincronização de pacotes, modelos e recursos remotos |
| **`OpenTimelineIO` (`otio`)** | `>=0.16.0` | Pixar Animation Studios & Academy Software Foundation | `Apache-2.0` | Intercâmbio universal de timelines (`.otio`) e adaptadores XML/EDL |

---

## 5. Motor C/C++ e SDKs da Indústria (`cpp-extensions/CMakeLists.txt` & `third_party/`)

| SDK / Biblioteca | Repositório / Origem | Autores / Organização | Licença (SPDX) | Utilização no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **`neoveg_cpp_engine`** | `cpp-extensions/` | Neoveg Core Team | `MIT` | Biblioteca C++17 compartilhada (`.dll`, `.so`, `.dylib`) com Shared Memory IPC (`NVSH`), matriz afim Pan/Crop e host de plugins |
| **`LADSPA Audio DSP (`ladspa.h`)`** | [`ladspa.org`](https://www.ladspa.org/) | Richard W.E. Furse, Paul Barton-Davis, Stefan Westerfeld | `LGPL-2.1-or-later` | Plugins de processamento dinâmico e equalização de áudio (`ladspa_audio_dsp.py`) |
| **`Audacity Waveform & Audio DSP Architecture`** | [`audacity/audacity`](https://github.com/audacity/audacity) | Audacity Team & Audacity Contributors | `GPL-3.0-or-later` (`GPLv3`) | Estudo arquitetural do algoritmo de amostragem multi-zoom de picos de forma de onda (`min`, `max`, `rms` em `rust-core/src/audio_peaks.rs` e `AudioWaveformRenderer.cs`) e processamento DSP (`Neoveg.AudioEngine`) |
| **`Kdenlive / Neoveg Presets & Color Scopes`** | [`KDE/kdenlive`](https://invent.kde.org/multimedia/kdenlive) | Jean-Baptiste Mardelle, Simon Andreas Eugster & KDE Community | `GPL-3.0-only` | Coeficientes CIE Rec.601/Rec.709 (`colorconstants.h`), LUTs `.cube`, perfis de proxy e miniaturas `.webp` de transições |

| **`Flat Color Icons`** | [`icons8/flat-color-icons`](https://github.com/icons8/flat-color-icons) | Icons8 | `MIT` | 27 ícones coloridos opcionais (BMP) em `crates/neoveg_ui/assets/icons_color/`, via Iconify |

---

## 6. FFmpeg, Whisper.cpp e Modelos de Inteligência Artificial

| Motor / Modelo | Repositório / Origem | Autores / Organização | Licença (SPDX) | Utilização no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **`FFmpeg` & `FFprobe`** | [`FFmpeg/FFmpeg`](https://ffmpeg.org/) (`gyan.dev` / `BtbN`) | Fabrice Bellard & The FFmpeg Developers | `LGPL-2.1-or-later` / `GPL-2.0-or-later` | Decodificação/codificação isolada em `runtimes/<rid>/ffmpeg/bin/`, validação `MediaRelinker` e pré-renderização seletiva |
| **`Whisper.cpp` (`whisper.h` / `ggml`)** | [`ggerganov/whisper.cpp`](https://github.com/ggerganov/whisper.cpp) | Georgi Gerganov & Contributors | `MIT` | Motor C/C++ de transcrição de voz e geração automática de eventos de texto na timeline (`SubtitleGeneratorService.cs`) |
| **`OpenAI Whisper Models (`ggml-base.bin`)`** | [`openai/whisper`](https://github.com/openai/whisper) | OpenAI | `MIT` | Pesos acústicos multilíngues (8 idiomas suportados no Neoveg) para legendagem automática |
| **`CorridorKey`** | [`CorridorDigital/CorridorKey`](https://github.com/CorridorDigital/CorridorKey) | Corridor Digital | `MIT` | Pipeline de recorte Chroma Key (Green Screen), supressão de *spill* e refino de bordas em 16-bit (`corridor_key_green_screen.py`) |
| **`SAM 2 (Segment Anything Model 2)`** | [`facebookresearch/sam2`](https://github.com/facebookresearch/sam2) | Meta Platforms, Inc. | `Apache-2.0` | Rotoscopia e mascaramento inteligente de objetos em vídeo (`automask`) |
| **`Vosk Speech Recognition`** | [`alphacep/vosk-api`](https://github.com/alphacep/vosk-api) | Alpha Cephei Inc. (Nickolay Shmyrev) | `Apache-2.0` | Motor alternativo leve de reconhecimento de fala offline para legendas |
