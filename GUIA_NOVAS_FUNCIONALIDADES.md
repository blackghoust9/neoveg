# Guia Prático de Novas Funcionalidades — Neoveg (C# Avalonia UI + Rust FFI + Python FFmpeg)

Este documento técnico apresenta o passo a passo completo para desenvolvedores que desejam estender o **Neoveg** adicionando **novos efeitos de vídeo OFX / Frei0r / 3D LUT**, **plugins de áudio VST3 / LADSPA**, **atalhos de teclado customizados** e **novos temas visuais**, mantendo 100% de conformidade com a arquitetura modular multiplataforma (**Windows**, **Linux** e **macOS**).

---

## 1. Visão Geral da Arquitetura Tri-Camada

O Neoveg divide responsabilidades em três camadas desacopladas de alto desempenho:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ CAMADA 1: C# (.NET 8 + Avalonia UI 11 + SkiaSharp) — desktop-host/           │
│ • Janela Principal (MainWindow.cs), Multi-Monitor Pop-outs, NeovegThemeManager│
│ • VideoPreviewControl.cs (Pan/Crop F Gizmo, Safe Areas, Split-Screen Bypass) │
│ • MultiTrackTimelineControl.cs (Régua SMPTE, Waveform .sfk, 5 Fade Curves)   │
└───────────────────┬──────────────────────────────────────┬───────────────────┘
                    │ P/Invoke C-ABI (NativeLibrary)       │ Subprocesso Isolado JSON
                    ▼                                      ▼
┌────────────────────────────────────────┐ ┌───────────────────────────────────┐
│ CAMADA 2: Rust FFI Core — rust-core/   │ │ CAMADA 3: Python — python-backend/│
│ • neoveg_core (.dll / .so / .dylib)    │ │ • ffmpeg_manager.py (FFmpeg/probe)│
│ • SMPTE Timecode, Cortes Sub-Quadro (S)│ │ • rust_bridge.py (Filtergraph)    │
│ • Auto-Crossfades, 5 Curvas de Fade    │ │ • plugins_engine.py (OFX/VST3)    │
│ • Cache Binário .sfk (RIFF/SFPK) & DSP │ │ • neoveg_integration.py (LUTs/OTIO│
│ • Pool de Ring-Buffer SIMD 64-byte     │ │ • qa_audit_suite.py (Testes QA)   │
└────────────────────────────────────────┘ └───────────────────────────────────┘
```

| Diretório / Arquivo | Papel na Arquitetura |
| :--- | :--- |
| [`desktop-host/Views/MainWindow.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs) | Host principal Avalonia UI, docks, menus, barra de ferramentas, atalhos de teclado (`OnNeovegKeyDown`) e catálogo de efeitos. |
| [`desktop-host/Services/NeovegThemeManager.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Services/NeovegThemeManager.cs) | Gerenciador de temas históricos (Classic Industrial 1–3, Classic NLE 4–13, Modern NLE 14–22, Contemporary NLE 2026). |
| [`desktop-host/Interop/RustCoreInterop.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Interop/RustCoreInterop.cs) | Ponte FFI dinâmica (`NativeLibrary.TryLoad`) que carrega `neoveg_core.dll` / `.so` / `.dylib`. |
| [`rust-core/src/lib.rs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core/src/lib.rs) | Motor nativo em Rust com funções exportadas em C-ABI (`#[no_mangle] pub unsafe extern "C" fn`). |
| [`python-backend/rust_bridge.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/rust_bridge.py) | Compilador do grafo `filter_complex` do FFmpeg, gerador de cache `.sfk` e ponte Python↔Rust. |
| [`python-backend/plugins_engine.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/plugins_engine.py) | Registro de plugins OFX, Frei0r, LADSPA, VST3 e CorridorKey AI. |

---

## 2. Como Adicionar um Novo Efeito de Vídeo (OFX / Frei0r / 3D LUT)

No Neoveg original (`neoveg210k.dll`), efeitos de vídeo seguem o padrão **OpenFX (`OfxImageEffectPlugin`)** com ações `kOfxActionDescribe`, `kOfxImageEffectActionDescribeInContext` e `kOfxImageEffectActionRender`. No Neoveg, todo efeito de vídeo possui:
1. **Preview em Tempo Real (60fps)** no [`VideoPreviewControl.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Controls/VideoPreviewControl.cs).
2. **Tradução para Grafo de Renderização FFmpeg** no [`rust_bridge.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/rust_bridge.py).
3. **Card no Catálogo da Aba `Video FX`** no [`MainWindow.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs).

### Passo 2.1 — Registrar o Plugin OFX no Backend Python (`python-backend/plugins_engine.py`)

Abra [`python-backend/plugins_engine.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/plugins_engine.py) e adicione os metadados e parâmetros do novo plugin OFX (por exemplo, `ofx.film_halation`):

```python
# Em python-backend/plugins_engine.py
OFX_VIDEO_PLUGINS["ofx.film_halation"] = {
    "id": "ofx.film_halation",
    "badge": "OFX",
    "name": "OFX Film Halation & Chromatic Glow",
    "category": "Stylize / Film Emulation",
    "parameters": {
        "intensity": {"type": "double", "default": 0.45, "min": 0.0, "max": 1.0},
        "radius": {"type": "double", "default": 12.0, "min": 1.0, "max": 50.0},
        "redShift": {"type": "double", "default": 0.20, "min": 0.0, "max": 1.0},
    },
}
```

### Passo 2.2 — Mapear o Efeito no Compilador `filter_complex` (`python-backend/rust_bridge.py`)

Em [`python-backend/rust_bridge.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/rust_bridge.py), dentro de `compile_project_to_ffmpeg()`, adicione a regra de tradução para o filtro nativo do FFmpeg / Frei0r / LibPlacebo:

```python
# Em python-backend/rust_bridge.py -> dentro do loop `for fx in all_clip_fx:`
elif fx_type == "ofx.film_halation":
    intensity = float(params.get("intensity", 0.45))
    radius = float(params.get("radius", 12.0))
    # Aplica difusão de altas-luzes vermelhas estilo película 35mm
    v_filters.append(
        f"split[fh_a][fh_b];"
        f"[fh_b]gblur=sigma={radius:.1f},colorbalance=rh={intensity:.2f}[fh_glow];"
        f"[fh_a][fh_glow]blend=all_mode=screen:all_opacity={intensity:.2f}"
    )
```

### Passo 2.3 — Renderizar o Preview em Tempo Real no Skia (`desktop-host/Controls/VideoPreviewControl.cs`)

Em [`desktop-host/Controls/VideoPreviewControl.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Controls/VideoPreviewControl.cs), dentro de `RenderClipLayer()`, adicione o efeito visual para que o editor veja o resultado instantaneamente a 60fps (respeitando também o botão **Split FX Bypass**):

```csharp
// Em desktop-host/Controls/VideoPreviewControl.cs -> dentro de RenderClipLayer()
bool hasHalation = clip.Fx.Any(f => f.Enabled && f.Type == "ofx.film_halation");
if (hasHalation)
{
    var fxRect = Project != null && Project.SplitScreenBypass
        ? new Rect(stageRect.X + stageRect.Width * 0.5, dst.Y, stageRect.Width * 0.5, dst.Height)
        : dst;
    context.FillRectangle(new SolidColorBrush(Color.FromArgb(48, 239, 68, 68)), fxRect);
    DrawText(context, "OFX: FILM HALATION 35MM", new Point(stageRect.X + 14, stageRect.Bottom - 40), 9.5, Color.Parse("#f87171"), true);
}
```

### Passo 2.4 — Exibir o Card na Aba `Video FX` (`desktop-host/Views/MainWindow.cs`)

Em [`desktop-host/Views/MainWindow.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs), adicione uma linha ao array `fxCatalog` dentro de `BuildLeftMediaAndFxDock()`:

```csharp
// Em desktop-host/Views/MainWindow.cs -> BuildLeftMediaAndFxDock()
var fxCatalog = new (string Type, string Badge, string Name, string Desc)[]
{
    // ... efeitos existentes ...
    ("ofx.film_halation", "OFX", "OFX Film Halation 35mm", "Difusão óptica vermelha em altas-luzes estilo película Kodak Vision3")
};
```

---

## 3. Como Adicionar um Novo Plugin de Áudio (VST3 / LADSPA + DSP em Rust)

Os plugins de áudio no Neoveg combinam **processamento DSP em tempo real em Rust** (`rust-core/src/lib.rs`) para medidores Peak/RMS e preview, com **filtros de masterização LADSPA / VST3 / FFmpeg** durante o render final.

### Passo 3.1 — Criar a Função DSP C-ABI no Motor Rust (`rust-core/src/lib.rs`)

Abra [`rust-core/src/lib.rs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/rust-core/src/lib.rs) e exporte uma função `extern "C"` com `#[no_mangle]`:

```rust
/// Processa um buffer de áudio PCM 16-bit aplicando saturação harmônica suave (Tape Warmth VST3/DSP).
#[no_mangle]
pub unsafe extern "C" fn neoveg_dsp_tape_warmth(
    samples_ptr: *mut i16,
    sample_count: i32,
    drive: f64,
) -> i32 {
    if samples_ptr.is_null() || sample_count <= 0 {
        return -1;
    }
    let samples = std::slice::from_raw_parts_mut(samples_ptr, sample_count as usize);
    let k = drive.clamp(0.1, 5.0);
    for s in samples.iter_mut() {
        let x = (*s as f64) / 32768.0;
        let y = (x * k).tanh() / k.tanh();
        *s = (y * 32767.0).clamp(-32768.0, 32767.0) as i16;
    }
    0
}
```

### Passo 3.2 — Vincular a Função na Ponte FFI em C# (`desktop-host/Interop/RustCoreInterop.cs`)

Em [`desktop-host/Interop/RustCoreInterop.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Interop/RustCoreInterop.cs):

1. Declare o delegate não-gerenciado:
```csharp
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
private delegate int NeovegDspTapeWarmthFn(short* samplesPtr, int sampleCount, double drive);
private NeovegDspTapeWarmthFn? _fnTapeWarmth;
```

2. Faça o bind em `BindExportedFunctions(IntPtr handle)`:
```csharp
if (NativeLibrary.TryGetExport(handle, "neoveg_dsp_tape_warmth", out IntPtr pTape))
    _fnTapeWarmth = Marshal.GetDelegateForFunctionPointer<NeovegDspTapeWarmthFn>(pTape);
```

### Passo 3.3 — Mapear o Plugin de Áudio no Grafo FFmpeg (`python-backend/rust_bridge.py`)

Em [`python-backend/rust_bridge.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/rust_bridge.py), dentro da seção de efeitos de áudio de `compile_project_to_ffmpeg()`:

```python
elif fx_type == "vst3.tape_warmth":
    # Saturação harmônica de fita + limitador true-peak
    a_filters.append("acompressor=threshold=-14dB:ratio=2.5:makeup=2dB,alimiter=limit=0.95")
```

### Passo 3.4 — Adicionar ao Catálogo `Audio FX` na UI (`desktop-host/Views/MainWindow.cs`)

Em [`desktop-host/Views/MainWindow.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs), adicione o plugin à lista `ladspaCatalog` na aba **Audio FX**:

```csharp
var ladspaCatalog = new (string Type, string Name, string Desc)[]
{
    ("ladspa.sc4_1882", "LADSPA SC4 Stereo Compressor (1882)", "Compressor dinâmico RMS/Peak broadcast"),
    ("ladspa.fast_lookahead_limiter_1913", "LADSPA Lookahead Limiter (1913)", "Brickwall True-Peak Limiter (-1.0 dBTP)"),
    ("ladspa.mbeq_1197", "LADSPA 15-Band Graphic EQ (1197)", "Equalizador harmônico de 15 bandas"),
    ("ladspa.plate_1423", "LADSPA Neoveg Plate Reverb (1423)", "Reverberação acústica de placa metálica"),
    ("vst3.tape_warmth", "VST3 Analog Tape Warmth Saturator", "Saturação harmônica tangente hiperbólica via Rust DSP")
};
```

---

## 4. Como Adicionar Atalhos de Teclado Customizados

O Neoveg segue o mapa de atalhos clássicos do Neoveg documentado em [`assets/neoveg_presets/neoveg_shortcuts.xml`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/assets/neoveg_presets/neoveg_shortcuts.xml) e processado em [`MainWindow.OnNeovegKeyDown`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs#L1480-L1550).

### Atalhos Atuais Integrados por Padrão

| Tecla | Ação no Neoveg | Motor Responsável |
| :--- | :--- | :--- |
| `Space` | Play / Pause da Timeline | Avalonia DispatcherTimer + Skia |
| `S` | Dividir evento no Playhead com precisão sub-quadro (`1e-6`s) | Rust FFI (`neoveg_split_clip`) |
| `Z` / `Del` / `Back` | Remover evento selecionado e recalcular crossfades | C# + Rust FFI (`neoveg_compute_crossfade`) |
| `J` / `L` | Retroceder `-1.0s` / Avançar `+1.0s` na linha do tempo | Rust FFI (`neoveg_format_smpte`) |
| `I` / `O` | Marcar pontos de entrada (`LoopStart`) e saída (`LoopEnd`) | Avalonia Timeline Ruler |
| `M` | Inserir marcador numerado na posição exata da agulha | Avalonia Timeline Marker |
| `Alt + Scroll` | Aumentar / diminuir a altura das faixas de vídeo e áudio | `MultiTrackTimelineControl` |

### Passo a Passo para Criar um Novo Atalho (Exemplo: `Ctrl+G` para Agrupar Faixas V+A)

1. **Registre no XML de Atalhos ([`assets/neoveg_presets/neoveg_shortcuts.xml`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/assets/neoveg_presets/neoveg_shortcuts.xml))**:
```xml
<Action name="neoveg_group_av_tracks" shortcut="Ctrl+G" description="Agrupar Faixas de Vídeo e Áudio (Grupo V+A)" />
```

2. **Adicione o Tratamento em [`MainWindow.OnNeovegKeyDown`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Views/MainWindow.cs)**:
```csharp
else if (e.Key == Key.G && e.KeyModifiers.HasFlag(KeyModifiers.Control))
{
    e.Handled = true;
    GroupVideoAndAudioTracks();
}
```

---

## 5. Como Adicionar ou Customizar um Tema Histórico (`NeovegThemeManager`)

O sistema de temas visuais do Neoveg é gerenciado por [`desktop-host/Services/NeovegThemeManager.cs`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/desktop-host/Services/NeovegThemeManager.cs) e alimentado pelo catálogo JSON [`assets/neoveg_history_themes/theme_catalog.json`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/assets/neoveg_history_themes/theme_catalog.json), gerado pelo script [`python-backend/generate_history_theme_assets.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/generate_history_theme_assets.py).

As 4 eras históricas oficiais contam com renderização específica de bordas e gradientes:
- **Era 1 — Classic Industrial (Neoveg 1–3)**: `EraNumber = 1` ativa bordas chanfradas 3D (`BorderThickness = 2` e chanfro claro/escuro nos clipes da timeline) e cinza industrial metálico com detalhes em vermelho (`#CC291B`).
- **Era 2 — Classic NLE Classic / Silver (Neoveg 4–13)**: `EraNumber = 2` ativa o clássico degradê flamejante dourado/laranja (`LinearGradientBrush`) nas barras de cabeçalho dos docks e no topo dos eventos da timeline.
- **Era 3 — Modern NLE Modern Dark (Neoveg 14–22)**: `EraNumber = 3` ativa o modo flat design compacto (`BorderThickness = 1`, `MenuPadding = 4,1`) em cinza escuro (`#232326`) e azul ciano (`#0078D7`).
- **Era 4 — Contemporary NLE / Atual (2026)**: `EraNumber = 4` ativa o tema escuro profundo (`#0E1015`) com azul elétrico (`#00A3FF` / `#38BDF8`).

Para adicionar um novo tema ao catálogo:
1. Adicione a definição de paleta em [`python-backend/generate_history_theme_assets.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/generate_history_theme_assets.py) ou [`python-backend/neoveg_integration.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/neoveg_integration.py).
2. Execute `.\.venv\Scripts\python.exe python-backend/generate_history_theme_assets.py` para regerar `theme_catalog.json` e os ativos SVG/PNG.

---

## 6. Fluxo de Build Portátil (~300MB) e Auditoria Automatizada de Qualidade

Sempre que adicionar uma nova funcionalidade em C#, Rust ou Python, execute a esteira completa de validação:

```powershell
# 1. Compilar o motor Rust FFI (ou sintetizar binários C-ABI nativos) e empacotar a versão portátil (~300MB)
.\.venv\Scripts\python.exe build_portable.py --targets windows

# 2. Executar a suíte de testes de arquitetura (7 testes de integração C# + Rust + Python + Neoveg)
.\.venv\Scripts\python.exe python-backend/test_neoveg_engine.py

# 3. Executar a auditoria completa de qualidade com mídias reais (SD 480p, HD 1080p, 4K UHD, Vertical 9:16, WAV 24-bit/96kHz, VFR e .sfk)
.\.venv\Scripts\python.exe python-backend/qa_audit_suite.py
```
