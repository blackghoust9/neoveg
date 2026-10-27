# Neoveg — Catálogo de Bibliotecas e SDKs de Terceiros (`third_party/`)

Este diretório centraliza os cabeçalhos C/C++, submódulos, conectores de renderização e motores de IA de código aberto integrados à arquitetura híbrida do **Neoveg** (**C# Avalonia UI + Rust `neoveg_core` + C++ `neoveg_cpp_engine` + Python Backend**).

---

## 1. Módulos Integrados e Licenciamento (SPDX)

| Módulo / Projeto | Repositório Oficial (GitHub) | Licença (SPDX) | Caminho Local | Camada de Integração no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **Whisper.cpp** | [`ggerganov/whisper.cpp`](https://github.com/ggerganov/whisper.cpp) | `MIT` | `third_party/whisper.cpp/` & `cpp-extensions/whisper.cpp/` | `neoveg_cpp_engine` + `SubtitleGeneratorService.cs` (Geração Automática de Eventos de Texto na Timeline) |
| **CorridorKey** | [`CorridorDigital/CorridorKey`](https://github.com/CorridorDigital/CorridorKey) | `MIT` / `Apache-2.0` | `third_party/corridorkey/` | `python-backend/corridor_key_green_screen.py` + `CorridorKey Lab` no Avalonia UI |
| **LADSPA Audio DSP** | [`ladspa/ladspa`](https://www.ladspa.org/) | `LGPL-2.1-or-later` | `third_party/ladspa/` | `python-backend/ladspa_audio_dsp.py` + `Audio Event FX` |
| **Audacity Waveform & DSP** | [`audacity/audacity`](https://github.com/audacity/audacity) | `GPL-3.0-or-later` (`GPLv3 / Audacity Contributors`) | `rust-core/src/audio_peaks.rs` & `desktop-host/AudioEngine/` | Amostragem multi-zoom de picos (`min`, `max`, `rms`), cache assíncrono `.sfk`, `AudioWaveformRenderer.cs` e `Neoveg.AudioEngine` (EQ Paramétrico 8 Bandas + VST3/LV2) |
| **nnnoiseless (RNNoise)** | [`jneem/nnnoiseless`](https://github.com/jneem/nnnoiseless) | `BSD-3-Clause` | `third_party/nnnoiseless/` (licença) | Dependência do Cargo de `neoveg_audio`: efeito **Redução de ruído por IA (RNNoise)** |
| **clack (`clack-host` / `clack-extensions`)** | [`prokopyl/clack`](https://github.com/prokopyl/clack) | `MIT OR Apache-2.0` | (dependência do Cargo, sem cópia) | `neoveg_audio::clap_host`: host de plug-ins de áudio CLAP (`.clap`) com automação e compensação de latência |
| **Candle (`candle-transformers`)** | [`huggingface/candle`](https://github.com/huggingface/candle) | `MIT OR Apache-2.0` | (dependência do Cargo, sem cópia) | `neoveg_audio::whisper`: transcrição automática de fala para legendas com Whisper em Rust puro (`.safetensors` / `.gguf`) |
| **roxmltree** | [`RazrFalcon/roxmltree`](https://github.com/RazrFalcon/roxmltree) | `MIT OR Apache-2.0` | (dependência do Cargo, sem cópia) | `neoveg_engine`: importar XML do Final Cut Pro 7 e FCPXML |
| **image (`image-rs`)** | [`image-rs/image`](https://github.com/image-rs/image) | `MIT OR Apache-2.0` | (dependência do Cargo, sem cópia) | `neoveg_video::still`: decodificação em Rust puro de imagens estáticas e sequências numeradas de quadros |
| **OpenTimelineIO** | [`AcademySoftwareFoundation/OpenTimelineIO`](https://github.com/AcademySoftwareFoundation/OpenTimelineIO) | `Apache-2.0` | `third_party/opentimelineio/` | Exportação/Importação `.otio` nativo e em `python-backend/` |
| **FilmCraft (`filmcraft-audio-dsp`)** | [`storytold/filmcraft`](https://github.com/storytold/filmcraft) | `MIT OR Apache-2.0` (usado sob a `MIT`) | `third_party/filmcraft/` (origem e licença) e `crates/neoveg_audio/src/dsp/` (código adaptado) | `neoveg_audio` (medidor de loudness BS.1770-4 / EBU R128 e efeitos de áudio por faixa e no master e análise de auto-ducking, dentro do callback de áudio) e painéis **Loudness** e **Efeitos de Áudio** em `neoveg_ui` |

---

## 2. Detalhamento das Licenças e Conformidade

0. **BSD-3-Clause (`nnnoiseless` / RNNoise)**:
   - Exige reproduzir o aviso de copyright e a isenção de garantia nas distribuições binárias (`third_party/nnnoiseless/COPYING` vai em todos os pacotes) e proíbe usar o nome da Xiph.Org Foundation para promover o produto.
1. **MIT License (`whisper.cpp`, `CorridorKey`, `FilmCraft`)**:
   - Permite uso irrestrito, modificação, linkagem estática ou dinâmica e redistribuição no pacote portátil `dist/Neoveg-Portable-*`, mantendo os avisos de copyright originais.
2. **BSD-3-Clause & Apache-2.0 (`OpenFX`, `OpenTimelineIO`)**:
   - Os cabeçalhos da especificação `ofxCore.h` e `ofxImageEffect.h` da Academy Software Foundation (ASWF) permitem implementação livre de hosts OpenFX (`OfxHost` / `OfxImageEffectSuiteV1`) em C++17 com exportação `extern "C"`.
3. **LGPL-2.1 / GPL-2.0 / GPL-3.0 (`VST3 SDK`, `Voukoder`, `Frei0r`, `LADSPA`)**:
   - Carregados dinamicamente via bibliotecas compartilhadas (`.dll`, `.so`, `.dylib`, `.ofx`, `.vst3`) e isolamento de processos (*Out-of-Process Shared Memory*), preservando a modularidade da arquitetura `Neoveg`.

---

## 3. Como Atualizar e Compilar (`scripts/update_third_party.py`)

Para sincronizar os repositórios do GitHub, validar os cabeçalhos C/C++, compilar a engine nativa (`neoveg_cpp_engine` + `neoveg_core` + `whisper.cpp`) e copiar os binários para `dist/`:

```powershell
# Sincronização completa + compilação C++/Rust + empacotamento em dist/
python scripts/update_third_party.py --build --stage-dist

# Execução em modo offline/CI (utiliza os cabeçalhos vendorizados em third_party/)
python scripts/update_third_party.py --offline --build --stage-dist
```
