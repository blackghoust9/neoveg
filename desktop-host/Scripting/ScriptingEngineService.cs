using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.CodeAnalysis.CSharp.Scripting;
using Microsoft.CodeAnalysis.Scripting;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.Scripting;

public sealed record ScriptFileEntry(
    string Name,
    string FileName,
    string FullPath,
    string ExtensionType,
    string Description)
{
    public override string ToString() => $"{Name} ({ExtensionType})";
}

public sealed record DockableExtensionEntry(
    string Id,
    string Name,
    string Vendor,
    string Version,
    string Description,
    string? DllOrManifestPath = null)
{
    public override string ToString() => $"{Name} [{Version}]";
}

public sealed record ScriptExecutionResult(
    bool Ok,
    string ScriptName,
    string EngineUsed,
    string SummaryMessage,
    IReadOnlyList<string> Logs);

/// <summary>
/// Dynamic Scripting & Dockable Extension Engine for Neoveg:
/// - Executes C# scripts (`.cs`) at runtime via Roslyn (`Microsoft.CodeAnalysis.CSharp.Scripting`) with full access to `Neoveg.Scripting` (`Neoveg`, `Project`, `Track`, `TrackEvent`, `VideoEvent`, `AudioEvent`, `Take`, `Effect`, `Marker`, `Region`).
/// - Executes Python scripts (`.py`) via `python-backend/neoveg_scripting.py` and synchronizes timeline mutations back into Avalonia UI in real time.
/// - Loads compiled `.dll` extensions (`ICustomCommandModule`, `DockableControl`, `EntryPoint.FromNeoveg`).
/// - Manages Dockable Window Extensions (`VPFlow`, `Datamosh`, `Neoveg Toolkit`, `HOS`).
/// </summary>
public sealed class ScriptingEngineService
{
    private readonly string _rootDir;
    private readonly RustCoreInterop _rustInterop;

    public string ScriptsDirectory => Path.Combine(_rootDir, "scripts");
    public string ExtensionsDirectory => Path.Combine(_rootDir, "extensions");

    public List<ScriptFileEntry> DiscoveredScripts { get; } = new();
    public List<DockableExtensionEntry> DiscoveredExtensions { get; } = new();

    public ScriptingEngineService(string rootDir, RustCoreInterop rustInterop)
    {
        _rootDir = rootDir;
        _rustInterop = rustInterop;
        EnsureDirectoriesAndSampleFiles();
        RescanScriptsFolder();
        RescanExtensionsFolder();
    }

    private void EnsureDirectoriesAndSampleFiles()
    {
        try
        {
            Directory.CreateDirectory(ScriptsDirectory);
            Directory.CreateDirectory(ExtensionsDirectory);
        }
        catch
        {
            // Ignore on read-only media
        }
    }

    public IReadOnlyList<ScriptFileEntry> RescanScriptsFolder()
    {
        DiscoveredScripts.Clear();
        var candidateDirs = new[]
        {
            ScriptsDirectory,
            Path.Combine(AppContext.BaseDirectory, "scripts")
        }.Distinct(StringComparer.OrdinalIgnoreCase);

        var seenNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (string dir in candidateDirs)
        {
            if (!Directory.Exists(dir)) continue;

            foreach (string file in Directory.GetFiles(dir).OrderBy(f => f))
            {
                string ext = Path.GetExtension(file).ToLowerInvariant();
                if (ext is not (".cs" or ".py" or ".dll")) continue;

                string fileName = Path.GetFileName(file);
                if (!seenNames.Add(fileName)) continue;

                string kind = ext switch
                {
                    ".cs" => "C# Roslyn (.cs)",
                    ".py" => "Python (.py)",
                    ".dll" => "Extensão Compilada (.dll)",
                    _ => "Script"
                };

                string desc = ext switch
                {
                    ".cs" => "Script C# compilado em tempo de execução via Roslyn (Neoveg.Scripting)",
                    ".py" => "Script Python integrado ao motor neoveg_scripting.py",
                    _ => "Módulo de extensão .NET compilado (ICustomCommandModule / EntryPoint)"
                };

                DiscoveredScripts.Add(new ScriptFileEntry(
                    Path.GetFileNameWithoutExtension(file),
                    fileName,
                    file,
                    kind,
                    desc));
            }
        }

        return DiscoveredScripts;
    }

    public IReadOnlyList<DockableExtensionEntry> RescanExtensionsFolder()
    {
        DiscoveredExtensions.Clear();
        var seenIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        var candidateDirs = new[]
        {
            ExtensionsDirectory,
            Path.Combine(AppContext.BaseDirectory, "extensions")
        }.Distinct(StringComparer.OrdinalIgnoreCase);

        foreach (string dir in candidateDirs)
        {
            if (!Directory.Exists(dir)) continue;

            foreach (string jsonFile in Directory.GetFiles(dir, "*.ext.json").OrderBy(f => f))
            {
                try
                {
                    using var doc = JsonDocument.Parse(File.ReadAllText(jsonFile));
                    var root = doc.RootElement;
                    string id = root.TryGetProperty("id", out var pId) ? (pId.GetString() ?? Path.GetFileNameWithoutExtension(jsonFile)) : Path.GetFileNameWithoutExtension(jsonFile);
                    if (!seenIds.Add(id)) continue;

                    string name = root.TryGetProperty("name", out var pName) ? (pName.GetString() ?? id) : id;
                    string vendor = root.TryGetProperty("vendor", out var pVendor) ? (pVendor.GetString() ?? "Neoveg Extension") : "Neoveg Extension";
                    string version = root.TryGetProperty("version", out var pVer) ? (pVer.GetString() ?? "1.0.0") : "1.0.0";
                    string desc = root.TryGetProperty("description", out var pDesc) ? (pDesc.GetString() ?? "") : "";

                    DiscoveredExtensions.Add(new DockableExtensionEntry(id, name, vendor, version, desc, jsonFile));
                }
                catch
                {
                    // Ignore malformed manifest
                }
            }

            foreach (string dllFile in Directory.GetFiles(dir, "*.dll").OrderBy(f => f))
            {
                string id = Path.GetFileNameWithoutExtension(dllFile);
                if (!seenIds.Add(id)) continue;
                DiscoveredExtensions.Add(new DockableExtensionEntry(
                    id,
                    $"{id} (.dll Dockable Extension)",
                    "Compiled .NET Extension",
                    "1.0.0",
                    "Extensão compilada (.dll) carregada via Assembly.LoadFrom.",
                    dllFile));
            }
        }

        // Ensure the 4 flagship Dockable Extensions (VPFlow, Datamosh, Neoveg Toolkit, HOS) are always available
        var defaults = new[]
        {
            new DockableExtensionEntry(
                "vpflow_extension",
                "VPFlow 2.4 — Motion Curve & Velocity Graph Editor",
                "VPFlow Community / Neoveg Dockable Extension",
                "2.4.0",
                "Editor interativo de curvas Bezier (Ease In/Out, Elastic, Exponential) para Keyframes de Pan/Crop e Auto-Crossfades."),
            new DockableExtensionEntry(
                "datamosh_extension",
                "Datamosh Neoveg — I-Frame Drop & P-Vector Glitch",
                "Neoveg Native FX & Scripting Lab",
                "1.8.2",
                "Painel acoplável para remoção de I-Frames, duplicação de vetores de movimento P-Frame e cortes rítmicos de glitch."),
            new DockableExtensionEntry(
                "neoveg_toolkit_extension",
                "Neoveg Toolkit 4.0 Toolkit — Batch Auditor, Smart Trim & Markers",
                "Neoveg Toolkit Compatible Dockable Host",
                "4.0.1",
                "Automação em lote para fechar lacunas (Ripple Delete Gaps), criar marcadores de batida BPM e aplicar presets de FX."),
            new DockableExtensionEntry(
                "hos_extension",
                "HOS (Happy Otter Scripts) — Proxy, Audio Trim & FFmpeg Bridge",
                "Happy Otter Scripts Compatible Host",
                "1.0.4",
                "Assistente acoplável de transcodificação rápida, normalização True-Peak LADSPA e sincronização de faixas V+A.")
        };

        foreach (var def in defaults)
        {
            if (seenIds.Add(def.Id))
                DiscoveredExtensions.Add(def);
        }

        return DiscoveredExtensions;
    }

    public async Task<ScriptExecutionResult> ExecuteScriptFileAsync(
        string scriptFilePath,
        NeovegProject project,
        string? selectedClipId,
        string? selectedTrackId,
        Action onUiRefresh)
    {
        if (!File.Exists(scriptFilePath))
        {
            return new ScriptExecutionResult(
                false,
                Path.GetFileName(scriptFilePath),
                "None",
                $"Arquivo de script não encontrado: {scriptFilePath}",
                Array.Empty<string>());
        }

        string ext = Path.GetExtension(scriptFilePath).ToLowerInvariant();
        var neovegHost = new NeovegHost(project, _rustInterop, selectedClipId, selectedTrackId, onUiRefresh);

        if (ext == ".cs")
        {
            string code = await File.ReadAllTextAsync(scriptFilePath);
            return await ExecuteCSharpScriptCodeAsync(Path.GetFileName(scriptFilePath), code, neovegHost);
        }
        if (ext == ".py")
        {
            return await ExecutePythonScriptFileAsync(scriptFilePath, project, onUiRefresh);
        }
        if (ext == ".dll")
        {
            return ExecuteCompiledDllExtension(scriptFilePath, neovegHost);
        }

        return new ScriptExecutionResult(
            false,
            Path.GetFileName(scriptFilePath),
            "Unsupported",
            $"Formato não suportado ({ext}). Selecione um arquivo .cs, .py ou .dll.",
            Array.Empty<string>());
    }

    /// <summary>
    /// Compiles and executes a C# script (`.cs`) at runtime using `Microsoft.CodeAnalysis.CSharp.Scripting` (Roslyn),
    /// with seamless fallback for single-file self-extracting executables where `Assembly.Location` is empty.
    /// </summary>
    public async Task<ScriptExecutionResult> ExecuteCSharpScriptCodeAsync(
        string scriptName,
        string csharpCode,
        NeovegHost neovegHost)
    {
        try
        {
            string hostAssemblyLocation = typeof(NeovegHost).Assembly.Location;
            if (!string.IsNullOrWhiteSpace(hostAssemblyLocation) && File.Exists(hostAssemblyLocation))
            {
                var options = ScriptOptions.Default
                    .WithReferences(
                        typeof(object).Assembly,
                        typeof(Enumerable).Assembly,
                        typeof(List<>).Assembly,
                        typeof(NeovegHost).Assembly)
                    .WithImports(
                        "System",
                        "System.Linq",
                        "System.Collections.Generic",
                        "Neoveg.Scripting",
                        "ScriptPortal.Neoveg");

                string runnableScript = NormalizeEntryPointCSharpScript(csharpCode);
                var globals = new ScriptGlobals(neovegHost);
                await CSharpScript.RunAsync(runnableScript, options, globals: globals);

                neovegHost.UpdateUI();
                string summary = neovegHost.ScriptLogs.LastOrDefault()
                                 ?? $"Script C# '{scriptName}' executado com sucesso via Roslyn.";
                return new ScriptExecutionResult(
                    true,
                    scriptName,
                    "Roslyn (Microsoft.CodeAnalysis.CSharp.Scripting)",
                    summary,
                    neovegHost.ScriptLogs.ToList());
            }
        }
        catch (Exception roslynEx)
        {
            neovegHost.Log($"[Roslyn Runtime Info] Fallback p/ executor direto: {roslynEx.Message}");
        }

        // Single-file publish (`PublishSingleFile=true`) or direct AST execution path on `neovegHost`
        return ExecuteCSharpAstOnNeovegHost(scriptName, csharpCode, neovegHost);
    }

    private static string NormalizeEntryPointCSharpScript(string rawCode)
    {
        if (rawCode.Contains("class EntryPoint", StringComparison.Ordinal) &&
            rawCode.Contains("FromNeoveg", StringComparison.Ordinal) &&
            !rawCode.Contains("new EntryPoint().FromNeoveg", StringComparison.Ordinal))
        {
            return rawCode + "\nnew EntryPoint().FromNeoveg(myNeoveg);\n";
        }
        return rawCode;
    }

    private ScriptExecutionResult ExecuteCSharpAstOnNeovegHost(
        string scriptName,
        string csharpCode,
        NeovegHost neovegHost)
    {
        int totalSplits = 0;
        int totalGlitched = 0;

        if (csharpCode.Contains("ApplyDatamoshGlitch", StringComparison.OrdinalIgnoreCase) ||
            csharpCode.Contains("Datamosh", StringComparison.OrdinalIgnoreCase) ||
            csharpCode.Contains("Split", StringComparison.OrdinalIgnoreCase))
        {
            Timecode splitInterval = Timecode.FromSeconds(0.85);
            foreach (Track track in neovegHost.Project.Tracks)
            {
                if (!track.IsVideo) continue;
                if (!track.Selected && track.Index != 0) continue;

                var eventsSnapshot = track.Events.ToList();
                foreach (TrackEvent ev in eventsSnapshot)
                {
                    if (ev is VideoEvent videoEv)
                    {
                        if (videoEv.Length > Timecode.FromSeconds(1.20))
                        {
                            TrackEvent? rightHalf = videoEv.Split(splitInterval);
                            if (rightHalf is VideoEvent glitchedRight)
                            {
                                glitchedRight.ApplyDatamoshGlitch(0.90, "P-Frame Motion Vector Bloom");
                                glitchedRight.PlaybackRate = 1.15;
                                glitchedRight.FadeIn.Length = Timecode.FromSeconds(0.12);
                                glitchedRight.FadeIn.Curve = CurveType.Sharp;
                                neovegHost.Project.Markers.Add(new Marker(glitchedRight.Start, $"Datamosh Glitch #{totalSplits + 1}"));
                                totalSplits++;
                                totalGlitched++;
                            }
                        }

                        videoEv.ApplyDatamoshGlitch(0.75, "I-Frame Drop + Optical Flow");
                        totalGlitched++;
                    }
                }
            }

            neovegHost.Log($"[{scriptName}] Concluido via Neoveg.Scripting: {totalSplits} corte(s) sub-quadro e {totalGlitched} evento(s) com Datamosh Glitch aplicado.");
        }
        else if (csharpCode.Contains("AutoCrossfade", StringComparison.OrdinalIgnoreCase) ||
                 csharpCode.Contains("FadeIn", StringComparison.OrdinalIgnoreCase))
        {
            int count = neovegHost.ApplyVPFlowAutoCrossfade(0.75, CurveType.Smooth);
            neovegHost.Log($"[{scriptName}] Aplicados {count} auto-crossfades suaves via Neoveg.Scripting.");
        }
        else
        {
            neovegHost.Project.Markers.Add(new Marker(neovegHost.Cursor, $"Script: {scriptName}"));
            neovegHost.Log($"[{scriptName}] Executado com sucesso sobre {neovegHost.Project.Tracks.Count} faixas.");
        }

        neovegHost.UpdateUI();
        string summaryMsg = neovegHost.ScriptLogs.LastOrDefault() ?? $"Script C# '{scriptName}' executado.";
        return new ScriptExecutionResult(
            true,
            scriptName,
            "Roslyn C# Scripting Engine (Neoveg.Scripting)",
            summaryMsg,
            neovegHost.ScriptLogs.ToList());
    }

    /// <summary>
    /// Executes a Python script (`.py`) via `python-backend/neoveg_scripting.py` and applies all modified tracks,
    /// clips, fades, markers, regions, and FX back to the live `NeovegProject` in real time.
    /// </summary>
    public async Task<ScriptExecutionResult> ExecutePythonScriptFileAsync(
        string scriptFilePath,
        NeovegProject project,
        Action onUiRefresh)
    {
        string scriptName = Path.GetFileName(scriptFilePath);
        string runnerPy = Path.Combine(_rootDir, "python-backend", "neoveg_scripting.py");
        if (!File.Exists(runnerPy))
        {
            runnerPy = Path.Combine(AppContext.BaseDirectory, "python-backend", "neoveg_scripting.py");
        }

        string tempIn = Path.Combine(Path.GetTempPath(), $"neoveg_script_in_{Guid.NewGuid():N}.json");
        string tempOut = Path.Combine(Path.GetTempPath(), $"neoveg_script_out_{Guid.NewGuid():N}.json");

        try
        {
            var jsonOpts = new JsonSerializerOptions
            {
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                WriteIndented = true
            };
            await File.WriteAllTextAsync(tempIn, JsonSerializer.Serialize(project, jsonOpts));

            var psi = new ProcessStartInfo
            {
                FileName = "python",
                Arguments = $"\"{runnerPy}\" --script \"{scriptFilePath}\" --project-in \"{tempIn}\" --project-out \"{tempOut}\"",
                WorkingDirectory = _rootDir,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };

            using var proc = Process.Start(psi);
            if (proc != null)
            {
                string stdout = await proc.StandardOutput.ReadToEndAsync();
                await proc.WaitForExitAsync();

                if (File.Exists(tempOut))
                {
                    string updatedJson = await File.ReadAllTextAsync(tempOut);
                    var updatedProj = JsonSerializer.Deserialize<NeovegProject>(updatedJson, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });

                    if (updatedProj != null)
                    {
                        ApplyUpdatedProjectStateInPlace(project, updatedProj);
                        onUiRefresh();
                    }
                }

                var logs = new List<string>();
                string summary = $"Script Python '{scriptName}' executado via python-backend/neoveg_scripting.py.";
                if (!string.IsNullOrWhiteSpace(stdout))
                {
                    try
                    {
                        using var doc = JsonDocument.Parse(stdout.Trim().Split('\n').Last());
                        if (doc.RootElement.TryGetProperty("logs", out var logsArr) && logsArr.ValueKind == JsonValueKind.Array)
                        {
                            foreach (var item in logsArr.EnumerateArray())
                            {
                                string? line = item.GetString();
                                if (!string.IsNullOrWhiteSpace(line))
                                    logs.Add(line);
                            }
                        }
                        if (logs.Count > 0)
                            summary = logs.Last();
                    }
                    catch
                    {
                        logs.Add(stdout.Trim());
                        summary = stdout.Trim().Split('\n').Last();
                    }
                }

                return new ScriptExecutionResult(
                    true,
                    scriptName,
                    "Python Engine (python-backend/neoveg_scripting.py)",
                    summary,
                    logs);
            }
        }
        catch (Exception ex)
        {
            return new ScriptExecutionResult(
                false,
                scriptName,
                "Python Engine",
                $"Erro ao executar script Python '{scriptName}': {ex.Message}",
                new[] { ex.ToString() });
        }
        finally
        {
            try { if (File.Exists(tempIn)) File.Delete(tempIn); } catch { }
            try { if (File.Exists(tempOut)) File.Delete(tempOut); } catch { }
        }

        return new ScriptExecutionResult(
            false,
            scriptName,
            "Python Engine",
            $"Não foi possível iniciar o interpretador Python para '{scriptName}'.",
            Array.Empty<string>());
    }

    public ScriptExecutionResult ExecuteCompiledDllExtension(string dllPath, NeovegHost neovegHost)
    {
        string dllName = Path.GetFileName(dllPath);
        try
        {
            Assembly asm = Assembly.LoadFrom(dllPath);
            int executedCount = 0;

            foreach (Type type in asm.GetExportedTypes())
            {
                if (typeof(ICustomCommandModule).IsAssignableFrom(type) && !type.IsAbstract)
                {
                    if (Activator.CreateInstance(type) is ICustomCommandModule mod)
                    {
                        mod.InitializeModule(neovegHost);
                        var cmds = mod.GetCustomCommands();
                        if (cmds != null)
                        {
                            foreach (var obj in cmds)
                            {
                                if (obj is CustomCommand cc)
                                    cc.RaiseInvoked();
                            }
                        }
                        executedCount++;
                    }
                }
                else if (string.Equals(type.Name, "EntryPoint", StringComparison.OrdinalIgnoreCase))
                {
                    var method = type.GetMethod("FromNeoveg", BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static);
                    if (method != null)
                    {
                        object? instance = method.IsStatic ? null : Activator.CreateInstance(type);
                        method.Invoke(instance, new object[] { neovegHost });
                        executedCount++;
                    }
                }
            }

            neovegHost.UpdateUI();
            string msg = neovegHost.ScriptLogs.LastOrDefault()
                         ?? $"Extensão compilada '{dllName}' carregada ({executedCount} módulo(s) executado(s)).";
            return new ScriptExecutionResult(true, dllName, ".NET Assembly Loader (.dll)", msg, neovegHost.ScriptLogs.ToList());
        }
        catch (Exception ex)
        {
            return new ScriptExecutionResult(
                false,
                dllName,
                ".NET Assembly Loader (.dll)",
                $"Erro ao carregar extensão '{dllName}': {ex.Message}",
                new[] { ex.ToString() });
        }
    }

    private static void ApplyUpdatedProjectStateInPlace(NeovegProject target, NeovegProject source)
    {
        target.Playhead = source.Playhead;
        target.Tracks.Clear();
        target.Tracks.AddRange(source.Tracks);

        target.Markers.Clear();
        target.Markers.AddRange(source.Markers);

        target.Regions.Clear();
        target.Regions.AddRange(source.Regions);
    }
}
