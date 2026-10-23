# OpenNeoveg 21 — Aplicativo Desktop Nativo (`NeovegLegacy.exe`)

Um editor não-linear de vídeo e áudio (NLE) inspirado no **Neoveg 21**, empacotado como um **executável nativo de Windows (`NeovegLegacy.exe`)**, combinando **Rust**, **Python** e **TypeScript / JavaScript** dentro de uma janela desktop dedicada (sem abrir abas de navegador).

---

## 🖥️ Como Abrir o Programa Executável

Basta dar **dois cliques** diretamente em:
- **[`NeovegLegacy.exe`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/NeovegLegacy.exe)** (na raiz da pasta `teste2`)

Ou executar no terminal:
```powershell
.\NeovegLegacy.exe
```

O executável nativo (`WinExe`):
1. Abre uma **janela desktop nativa do Windows** com barra de título escura (*Windows 11 Immersive Dark Mode*), ícone próprio ([`NeovegLegacy.ico`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/NeovegLegacy.ico)) e **Menu Nativo** (`Arquivo`, `Editar`, `Inserir`, `Ferramentas`, `Exibir`).
2. Inicializa automaticamente em segundo plano (de forma 100% invisível, sem janela de prompt de comando) o motor [`python-backend/server.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/server.py), a ponte com o [`rust-core`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core) e o pipeline `FFmpeg`, encerrando o processo automaticamente quando você fecha o programa.
3. Oferece **diálogos nativos do Windows (`OpenFileDialog` / `SaveFileDialog`)** para:
   - **Abrir Projeto (`.veg`)** (`Ctrl+O`)
   - **Salvar Projeto (`.veg`)** (`Ctrl+S`)
   - **Importar Mídia Nativa (`.mp4`, `.wav`, `.mp3`, `.mov`, `.webm`)** (`Ctrl+I`)

---

## 🏗️ Arquitetura do Projeto

| Camada | Linguagem | Diretório / Arquivo | Responsabilidades |
| :--- | :--- | :--- | :--- |
| **Executável Desktop Nativo** | **C# / Win32 / WebView2** | [`NeovegLegacy.exe`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/NeovegLegacy.exe) / [`desktop-host/Program.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Program.cs) | Janela nativa WinForms (`NeovegStudioWindow`), barra de menus escura nativa, diálogos de arquivo nativos do Windows (`.veg`, `.mp4`, `.wav`), superfície nativa `WebView2` e gerenciamento automático de ciclo de vida do processo de backend. |
| **Core NLE & DSP Engine** | **Rust** | [`rust-core/src/lib.rs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core/src/lib.rs) | Cálculo de Timecode SMPTE Drop-Frame (`00:00:00;00`), avaliação das **5 curvas de fade do Neoveg** (`Linear`, `Fast`, `Slow`, `Smooth`, `Sharp`), detector de **Auto-Crossfades por sobreposição**, extrator de picos PCM/dBFS de arquivos WAV e compilador `ffmpeg -filter_complex`. |
| **Media Engine, Render & Scripting API** | **Python** | [`python-backend/server.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/server.py) | Servidor de mídia com *Byte-Range*, inspeção `ffprobe`, ponte Rust ([`rust_bridge.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/rust_bridge.py)), renderização real multi-pista via `FFmpeg` e **API de Scripting em Python estilo Neoveg** ([`neoveg_scripting.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/neoveg_scripting.py)). |
| **Studio NLE Surface & Compositor 60fps** | **TypeScript / JS** | [`frontend/src/`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/frontend/src) | Compositor multi-track 60fps ([`compositor.ts`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/frontend/src/engine/compositor.ts)), mixer de áudio e medidores dB ([`audioEngine.ts`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/frontend/src/engine/audioEngine.ts)) e timeline interativa ([`timeline.ts`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/frontend/src/ui/timeline.ts)). |
