# Documentação Oficial de Scripts e Extensões Acopláveis — Neoveg

Este documento técnico detalha a arquitetura do **Sistema de Extensões e Scripts (Scripting Engine & Dockable Extensions)** do **Neoveg**, projetado para espelhar a API clássica do **Neoveg** (`Neoveg.Scripting.Compat` / `Neoveg.Scripting`) e suportar tanto scripts em **C# (`.cs` via Roslyn)** e **Python (`.py`)** quanto **Extensões Compiladas (`.dll` / `DockableControl`)** como **VPFlow**, **Datamosh**, **Neoveg Toolkit** e **HOS (Happy Otter Scripts)**.

---

## 1. Estrutura de Pastas do Ecossistema de Scripts e Extensões

Na raiz do projeto e na distribuição portátil (`dist/Neoveg-Portable-win-x64/`), o Neoveg monitora automaticamente duas pastas:

```text
Neoveg/
├── scripts/                                  # Scripts executáveis em tempo real (.cs, .py, .dll)
│   ├── Batch_Split_and_Datamosh.cs           # Exemplo C# (Roslyn): Cortes automáticos + Datamosh Glitch
│   └── VPFlow_AutoCrossfade_Helper.py        # Exemplo Python: Curvas suaves S-Curve + Alinhamento VPFlow
├── extensions/                               # Extensões de Janela Acopláveis (Dockable Extensions)
│   ├── VPFlow_MotionCurve.ext.json           # Editor de Curvas Bezier e Velocidade (VPFlow 2.4)
│   ├── Datamosh_Glitch.ext.json        # Estúdio I-Frame Drop & P-Vector Glitch
│   ├── Neoveg_Toolkit.ext.json             # Automação em Lote (Ripple Delete Gaps, Beat Markers)
│   └── HOS_RenderProxyBridge.ext.json        # Happy Otter Scripts (Normalização True-Peak & Proxy)
└── desktop-host/
    ├── Scripting/
    │   ├── NeovegScriptingApi.cs             # Namespace Neoveg.Scripting + Bridge Neoveg.Scripting.Compat
    │   └── ScriptingEngineService.cs         # Motor Roslyn C# + Python + Carregador de Assembly .DLL
    └── Views/
        └── DockableExtensionWindow.cs        # Host de Extensões de Janela Acopláveis + Console IDE
```

---

## 2. Arquitetura da API `Neoveg.Scripting` (`Neoveg.Scripting.Compat`)

O namespace `Neoveg.Scripting` implementa a hierarquia completa de classes do Neoveg:

| Classe / Tipo | Descrição e Membros Principais |
| :--- | :--- |
| **`Neoveg`** (`myNeoveg`) | Objeto raiz injetado no script. Expõe `Project`, `Cursor`, `SelectionStart`, `SelectionLength`, `UpdateUI()`, `Log(msg)`, `BatchSplitAndDatamosh(...)` e `ApplyVPFlowAutoCrossfade(...)`. |
| **`Project`** | Representa o projeto ativo (`myNeoveg.Project`). Expõe `Tracks`, `Markers`, `Regions`, `Video` (`Width`, `Height`, `FrameRate`) e `Audio` (`SampleRate`, `MasterBusVolume`). |
| **`Track` / `VideoTrack` / `AudioTrack`** | Faixa da timeline (`Index`, `Name`, `Selected`, `Mute`, `Solo`, `IsVideo`, `IsAudio`, `Events`, `Effects`). |
| **`TrackEvent`** | Classe base de clipe/evento na timeline (`Start`, `Length`, `End`, `Selected`, `Mute`, `Locked`, `PlaybackRate`, `FadeIn`, `FadeOut`, `Takes`, `ActiveTake`, `Effects`, `Split(Timecode offset)`). |
| **`VideoEvent`** | Subclasse de `TrackEvent` para vídeo. Inclui `PanCrop`, `ApplyDatamoshGlitch(intensity, mode)` e `ApplyVPFlowBezierCurve(preset, zoom, angle)`. |
| **`AudioEvent`** | Subclasse de `TrackEvent` para áudio. Inclui `Gain`, `PitchShiftSemitones` e `Normalize(targetUnity)`. |
| **`Take`** | Representa um take de mídia (`Name`, `MediaPath`, `Offset`, `IsActive`). |
| **`Effect`** | Representa um plugin na cadeia de efeitos (`UniqueID`, `Description`, `PlugInName`, `Bypass`, `Parameters`). |
| **`Marker` / `Region`** | Marcadores (`Position`, `Label`) e regiões (`Position`, `Length`, `End`, `Label`) na régua SMPTE. |
| **`Timecode`** | Estrutura de tempo precisa (`FromSeconds`, `FromMilliseconds`, `FromFrames`, `FromString`, `ToMilliseconds()`, `ToSeconds()`, operadores `+`, `-`, `<`, `>`). |

---

## 3. Como Criar um Script em C# (`.cs`) com Compilação Dinâmica Roslyn

O Neoveg utiliza o pacote oficial **`Microsoft.CodeAnalysis.CSharp.Scripting` (Roslyn)** para compilar e executar scripts `.cs` em memória sem reiniciar o aplicativo.

Você pode escrever o script no formato clássico do Neoveg (`class EntryPoint` com `public void FromNeoveg(Neoveg myNeoveg)`) ou diretamente com instruções top-level usando a variável global `myNeoveg`:

```csharp
using System;
using System.Linq;
using Neoveg.Scripting;

public class EntryPoint
{
    public void FromNeoveg(Neoveg myNeoveg)
    {
        Timecode interval = Timecode.FromSeconds(0.85);

        foreach (Track track in myNeoveg.Project.Tracks)
        {
            if (!track.IsVideo) continue;

            foreach (TrackEvent ev in track.Events.ToList())
            {
                if (ev is VideoEvent videoEv && videoEv.Length > Timecode.FromSeconds(1.2))
                {
                    // Divide o evento via Rust FFI e aplica Datamosh Glitch na segunda metade
                    TrackEvent? rightHalf = videoEv.Split(interval);
                    if (rightHalf is VideoEvent glitched)
                    {
                        glitched.ApplyDatamoshGlitch(0.90, "P-Frame Motion Vector Bloom");
                        glitched.PlaybackRate = 1.15;
                        glitched.FadeIn.Length = Timecode.FromSeconds(0.12);
                        glitched.FadeIn.Curve = CurveType.Sharp;
                        myNeoveg.Project.Markers.Add(new Marker(glitched.Start, "Datamosh Cut"));
                    }
                }
            }
        }

        myNeoveg.UpdateUI();
    }
}
```

### Como Executar no Neoveg:
1. Salve o arquivo `.cs` na pasta `scripts/`.
2. No menu superior, acesse **`Ferramentas -> Scripts -> Recarregar Pasta de Scripts`**.
3. Clique no nome do seu script na lista dinâmica de **`Ferramentas -> Scripts`** ou use **`Executar Script...`** para selecioná-lo em qualquer pasta.

---

## 4. Como Criar um Script em Python (`.py`)

Os scripts Python são executados pelo motor [`python-backend/neoveg_scripting.py`](file:///g:/Meu%20Drive/vp-rev-eng/teste/teste2/python-backend/neoveg_scripting.py), que recebe o estado completo da timeline em JSON, expõe o objeto global `neoveg` (ou `myNeoveg`) e sincroniza todas as alterações de volta para a interface Avalonia UI em tempo real.

```python
# Exemplo: scripts/VPFlow_AutoCrossfade_Helper.py
overlap_seconds = 0.75

# Aplica sobreposição suave S-Curve entre eventos adjacentes e suaviza o Pan/Crop (Bezier 88%)
crossfades = neoveg.apply_vpflow_easing_and_crossfades(overlap_sec=overlap_seconds, curve="smooth")

# Normaliza ganhos de áudio para 0.0 dB (1.0)
neoveg.normalize_audio_gains(1.0)

# Adiciona um marcador na posição atual da agulha (Playhead)
neoveg.add_marker(neoveg.cursor, "VPFlow Sync", "#38bdf8")
neoveg.log(f"Aplicados {crossfades} auto-crossfades suaves.")
```

---

## 5. Como Criar Extensões de Janela Acopláveis (`.dll` / `DockableControl` / `ICustomCommandModule`)

Para criar painéis interativos acopláveis ao estilo **VPFlow**, **Datamosh**, **Neoveg Toolkit** ou **HOS**:

1. Referencie `Neoveg.dll` em uma biblioteca de classes .NET (`net10.0`).
2. Implemente a interface `ICustomCommandModule` ou herde de `DockableControl`:

```csharp
using System.Collections;
using Neoveg.Scripting;

public class MyCustomNeovegExtension : ICustomCommandModule
{
    private Neoveg? _neoveg;
    private readonly CustomCommand _cmd = new(CommandCategory.View, "Minha Extensão Customizada");

    public void InitializeModule(Neoveg neoveg)
    {
        _neoveg = neoveg;
        _cmd.Invoked += (_, _) =>
        {
            _neoveg?.ApplyVPFlowAutoCrossfade(0.80, CurveType.Smooth);
            _neoveg?.Log("Extensão .DLL executada com sucesso!");
        };
    }

    public ICollection GetCustomCommands() => new[] { _cmd };
}
```

3. Copie a `.dll` compilada ou o manifesto `.ext.json` para a pasta `extensions/`.
4. Acesse **`Exibir -> Extensões de Janela (View -> Window Extensions)`** para abrir a extensão em uma janela flutuante ou clicar em **"Acoplar no Painel Central [📌]"** para embuti-la diretamente no dock central do Neoveg.
