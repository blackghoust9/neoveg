# Neoveg — Editor Não-Linear de Vídeo e Áudio Multiplataforma (`Neoveg.exe`)

O **Neoveg** é um editor não-linear de vídeo e áudio (NLE) multiplataforma (**Windows**, **Linux** e **macOS**) construído com **C# Avalonia UI 11**, **Rust FFI (`neoveg_core`)**, **C++17 (`neoveg_cpp_engine`)** e **Python FFmpeg Isolado**.

---

## Como Executar

No Windows, execute diretamente o binário portátil:
- **[`dist/Neoveg-Portable-win-x64/Neoveg.exe`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/dist/Neoveg-Portable-win-x64/Neoveg.exe)** ou **[`Neoveg.exe`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/Neoveg.exe)**

Ou pelo terminal:
```powershell
.\dist\Neoveg-Portable-win-x64\Neoveg.exe
```

Para compilar todos os pacotes portáteis (`Windows`, `Linux AppDir`, `macOS .app`) e sincronizar as bibliotecas de terceiros:
```powershell
python scripts/update_third_party.py --offline --build --stage-dist
python build_portable.py --targets all --framework-dependent
```

---

## Arquitetura Híbrida do Neoveg

| Camada | Linguagem | Diretório / Alvo | Responsabilidades |
| :--- | :--- | :--- | :--- |
| **Interface Desktop Multiplataforma** | **C# / Avalonia UI 11** | [`desktop-host/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host) (`Neoveg`) | Dockable Workspace, Timeline Multi-Faixa com Cabeçalho de Eventos (`Event Header`), Barra de Pré-Renderização Seletiva ([`RenderStatusRuler.axaml`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Assets/RenderStatusRuler.axaml)), `Event Pan/Crop` (Gizmo `'F'`), `MissingMediaWindow`, Internacionalização (`i18n` em 8 idiomas) e `Neoveg.Scripting` (Roslyn C#). |
| **Core NLE & DSP Engine** | **Rust (C-ABI)** | [`rust-core/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core) (`neoveg_core`) | Cálculo de Timecode SMPTE, 5 curvas de Fade, Split sub-quadro, Auto-Crossfade, análise PCM e cache `.sfk`, métricas de RAM Pool e [`neoveg_get_timeline_render_status`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core/src/lib.rs). |
| **Extensões C/C++ & SDKs Nativos** | **C++17 / CMake** | [`cpp-extensions/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/cpp-extensions) (`neoveg_cpp_engine`) | Host **OpenFX 1.4 (`ofxCore.h`)**, **Steinberg VST3 (`ivstaudioprocessor.h`)**, **Voukoder Connector**, **Whisper.cpp (`SubtitleGeneratorService.cs`)** e **Shared Memory Out-of-Process (`NVSH`)**. |
| **Motor de Mídia, IA & Automação** | **Python 3.10+** | [`python-backend/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend) (`neoveg`) | Gerenciador isolado de `FFmpeg`/`ffprobe`, varredura de 3 estágios de mídias ausentes ([`media_relinker.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/media_relinker.py)), `CorridorKey` Green Screen, `Frei0r`, `LADSPA` e `OpenTimelineIO`. |

---

## Créditos, Licenciamento e Guia para Forks / Contribuições

O código-fonte original do **Neoveg** é distribuído sob a licença **[`MIT`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/LICENSE)**. O projeto integra-se a bibliotecas, cabeçalhos e ferramentas de código aberto amplamente consolidadas na indústria de áudio e vídeo.

- **Mapeamento Completo de Autores e Licenças:** Consulte o ficheiro **[`AUTHORS.md`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/AUTHORS.md)** na raiz do projeto (e **[`third_party/README.md`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/third_party/README.md)**) para verificar os créditos detalhados de todas as dependências utilizadas em:
  - [`desktop-host/Neoveg.csproj`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Neoveg.csproj) (`Avalonia UI`, `SkiaSharp`, `Microsoft.CodeAnalysis.CSharp.Scripting`, `Inter Font`)
  - [`rust-core/Cargo.toml`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core/Cargo.toml) (`neoveg_core` & Rust Standard Library)
  - [`python-backend/requirements.txt`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/requirements.txt) (`NumPy`, `Pillow`, `pefile`, `packaging`, `typing_extensions`, `requests`, `OpenTimelineIO`)
  - [`cpp-extensions/CMakeLists.txt`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/cpp-extensions/CMakeLists.txt) & [`third_party/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/third_party) (`OpenFX`, `VST3 SDK`, `Voukoder`, `Whisper.cpp`, `CorridorKey`, `Frei0r`, `LADSPA`, `FFmpeg`)

### Orientações para Criar Forks, Modificar e Contribuir

1. **Preservação de Créditos e Avisos Legais:**
   - Ao criar um *fork* ou redistribuir pacotes compilados (`dist/Neoveg-Portable-*`), mantenha sempre os ficheiros [`AUTHORS.md`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/AUTHORS.md), [`LICENSE`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/LICENSE) e [`third_party/README.md`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/third_party/README.md) junto aos binários (o script [`build_portable.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/build_portable.py) realiza essa cópia automaticamente para Windows, Linux `AppDir` e macOS `Contents/Resources/`).
2. **Conformidade com Componentes Dinâmicos (LGPL / GPL):**
   - Módulos sob licenças `LGPL-2.1` ou `GPL-2.0/3.0` (`FFmpeg`, `VST3 SDK`, `Voukoder`, `Frei0r`, `LADSPA`) são carregados dinamicamente ou executados fora do processo (*Out-of-Process*). Caso modifique diretamente o código-fonte de algum desses módulos em [`third_party/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/third_party), disponibilize as alterações sob a respectiva licença original do componente.
3. **Adição de Novas Dependências:**
   - Sempre que adicionar um novo pacote NuGet (`.csproj`), crate Rust (`Cargo.toml`), pacote Python (`requirements.txt`) ou SDK C/C++ (`CMakeLists.txt` / `scripts/update_third_party.py`), registe o autor, repositório e identificador SPDX correspondente em [`AUTHORS.md`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/AUTHORS.md) e execute a suíte de validação:
   ```powershell
   python python-backend/test_vegas_engine.py
   ```
