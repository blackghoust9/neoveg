# Neoveg — Catálogo de Bibliotecas e SDKs de Terceiros (`third_party/`)

Este diretório centraliza os cabeçalhos C/C++, submódulos, conectores de renderização e motores de IA de código aberto integrados à arquitetura híbrida do **Neoveg** (**C# Avalonia UI + Rust `neoveg_core` + C++ `neoveg_cpp_engine` + Python Backend**).

---

## 1. Módulos Integrados e Licenciamento (SPDX)

| Módulo / Projeto | Repositório Oficial (GitHub) | Licença (SPDX) | Caminho Local | Camada de Integração no Neoveg |
| :--- | :--- | :--- | :--- | :--- |
| **OpenFX Core (`ofxCore`)** | [`AcademySoftwareFoundation/openfx`](https://github.com/AcademySoftwareFoundation/openfx) | `BSD-3-Clause` / `Apache-2.0` | `third_party/openfx/include/` | `neoveg_cpp_engine` (`ofxCore.h`, `ofxImageEffect.h`) ↔ `CppCoreInterop.cs` & `neoveg_core` |
| **VST3 SDK** | [`steinbergmedia/vst3sdk`](https://github.com/steinbergmedia/vst3sdk) | `GPL-3.0-or-later` | `third_party/vst3sdk/pluginterfaces/` | `neoveg_cpp_engine` (`ivstaudioprocessor.h`) ↔ DSP de Áudio 32-bit Float (`CppCoreInterop.cs`) |
| **Whisper.cpp** | [`ggerganov/whisper.cpp`](https://github.com/ggerganov/whisper.cpp) | `MIT` | `third_party/whisper.cpp/` & `cpp-extensions/whisper.cpp/` | `neoveg_cpp_engine` + `SubtitleGeneratorService.cs` (Geração Automática de Eventos de Texto na Timeline) |
| **Voukoder Connector** | [`Vouk/voukoder`](https://github.com/Vouk/voukoder) | `GPL-2.0-only` / `LGPL-2.1-or-later` | `third_party/voukoder/include/` | `neoveg_cpp_engine` (`voukoder_bridge.h`) + Perfis NVENC/AMF/QSV/ProRes/FFV1 |
| **CorridorKey** | [`CorridorDigital/CorridorKey`](https://github.com/CorridorDigital/CorridorKey) | `MIT` / `Apache-2.0` | `third_party/corridorkey/` | `python-backend/corridor_key_green_screen.py` + `CorridorKey Lab` no Avalonia UI |
| **Frei0r Video FX** | [`dyne/frei0r`](https://github.com/dyne/frei0r) | `GPL-2.0-or-later` | `third_party/frei0r/` | `python-backend/frei0r_openfx_bridge.py` + `Video FX Chain` |
| **LADSPA Audio DSP** | [`ladspa/ladspa`](https://www.ladspa.org/) | `LGPL-2.1-or-later` | `third_party/ladspa/` | `python-backend/ladspa_audio_dsp.py` + `Audio Event FX` |
| **Audacity Waveform & DSP** | [`audacity/audacity`](https://github.com/audacity/audacity) | `GPL-3.0-or-later` (`GPLv3 / Audacity Contributors`) | `rust-core/src/audio_peaks.rs` & `desktop-host/AudioEngine/` | Amostragem multi-zoom de picos (`min`, `max`, `rms`), cache assíncrono `.sfk`, `AudioWaveformRenderer.cs` e `Neoveg.AudioEngine` (EQ Paramétrico 8 Bandas + VST3/LV2) |
| **OpenTimelineIO** | [`AcademySoftwareFoundation/OpenTimelineIO`](https://github.com/AcademySoftwareFoundation/OpenTimelineIO) | `Apache-2.0` | `third_party/opentimelineio/` | Exportação/Importação `.otio` nativo e em `python-backend/` |

---

## 2. Detalhamento das Licenças e Conformidade

1. **MIT License (`whisper.cpp`, `CorridorKey`)**:
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
