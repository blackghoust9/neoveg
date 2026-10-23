using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.ProjectInterchange;

namespace Neoveg.Services;

/// <summary>
/// Cross-platform C# service that controls Python + Isolated FFmpeg/FFprobe (`ffmpeg_manager.py`),
/// Frei0r/LADSPA/CorridorKey plugins (`plugins_engine.py`), and `.veg` project persistence
/// across Windows, Linux, and macOS.
/// </summary>
public sealed class FFmpegPythonService
{
    public string RootDir { get; }
    public string PythonExecutable { get; }
    public string FfmpegManagerScript { get; }
    public string ProjectsDir { get; }
    public string MediaDir { get; }
    public string RendersDir { get; }

    public FFmpegPythonService(string rootDir)
    {
        RootDir = rootDir;
        ProjectsDir = Path.Combine(rootDir, "projects");
        MediaDir = Path.Combine(rootDir, "media");
        RendersDir = Path.Combine(rootDir, "renders");
        FfmpegManagerScript = Path.Combine(rootDir, "python-backend", "ffmpeg_manager.py");

        Directory.CreateDirectory(ProjectsDir);
        Directory.CreateDirectory(MediaDir);
        Directory.CreateDirectory(RendersDir);

        PythonExecutable = ResolvePythonExecutable(rootDir);
    }

    private static string ResolvePythonExecutable(string rootDir)
    {
        string rid = RustCoreInterop.DetectPlatformRid();
        bool isWin = RuntimeInformation.IsOSPlatform(OSPlatform.Windows);
        string exeName = isWin ? "python.exe" : "python3";

        var candidates = new[]
        {
            Path.Combine(rootDir, "runtimes", rid, "python", exeName),
            Path.Combine(rootDir, "runtimes", rid, "python", "bin", exeName),
            isWin ? "python" : "python3",
            "python"
        };

        foreach (string c in candidates)
        {
            if (Path.IsPathRooted(c) && File.Exists(c))
                return c;
        }
        return isWin ? "python" : "python3";
    }

    public async Task<string> RunPythonScriptJsonAsync(string args, int timeoutMs = 45000)
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = PythonExecutable,
                Arguments = $"\"{FfmpegManagerScript}\" {args}",
                WorkingDirectory = RootDir,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8,
                StandardErrorEncoding = Encoding.UTF8
            };

            using var proc = Process.Start(psi);
            if (proc == null) return "{\"ok\":false,\"error\":\"Failed to start python\"}";

            var outTask = proc.StandardOutput.ReadToEndAsync();
            var errTask = proc.StandardError.ReadToEndAsync();
            await proc.WaitForExitAsync().WaitAsync(TimeSpan.FromMilliseconds(timeoutMs));
            string stdout = await outTask;
            return string.IsNullOrWhiteSpace(stdout) ? "{\"ok\":false}" : stdout.Trim();
        }
        catch (Exception ex)
        {
            return JsonSerializer.Serialize(new { ok = false, error = ex.Message });
        }
    }

    public async Task<string> ProvisionAndGetStatusBannerAsync()
    {
        try
        {
            string json = await RunPythonScriptJsonAsync("status", 5000);
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            bool ok = root.TryGetProperty("ok", out var okEl) && okEl.GetBoolean();
            string rid = root.TryGetProperty("rid", out var rEl) ? rEl.GetString() ?? "" : "";
            bool bundled = root.TryGetProperty("is_bundled_portable", out var bEl) && bEl.GetBoolean();
            return ok
                ? $"FFmpeg/ffprobe Isolado ({rid}{(bundled ? " Portable" : "")}) Ativo"
                : $"FFmpeg/ffprobe ({rid}) Pronto";
        }
        catch
        {
            return "Python Isolated FFmpeg Ready";
        }
    }

    public async Task<MediaAssetItem?> ProbeAndImportMediaAsync(string filePath, RustCoreInterop rustInterop)
    {
        if (!File.Exists(filePath)) return null;
        string ext = Path.GetExtension(filePath).ToLowerInvariant();
        bool isAudio = ext is ".wav" or ".mp3" or ".ogg" or ".flac" or ".m4a";

        var item = new MediaAssetItem
        {
            Id = $"media_{Guid.NewGuid():N}"[..10],
            Name = Path.GetFileNameWithoutExtension(filePath),
            FileName = Path.GetFileName(filePath),
            FilePath = filePath,
            Type = isAudio ? "audio" : "video",
            Duration = 5.0,
            Width = isAudio ? 0 : 1920,
            Height = isAudio ? 0 : 1080,
            Fps = 29.97,
            Codec = isAudio ? "PCM/WAV" : "H264",
            Color = isAudio ? "#0891b2" : "#2563eb"
        };

        try
        {
            string probeJson = await RunPythonScriptJsonAsync($"probe \"{filePath}\"", 10000);
            using var doc = JsonDocument.Parse(probeJson);
            var root = doc.RootElement;
            if (root.TryGetProperty("ok", out var okEl) && okEl.GetBoolean())
            {
                if (root.TryGetProperty("duration", out var dEl)) item.Duration = Math.Max(0.5, dEl.GetDouble());
                if (root.TryGetProperty("width", out var wEl)) item.Width = wEl.GetInt32();
                if (root.TryGetProperty("height", out var hEl)) item.Height = hEl.GetInt32();
                if (root.TryGetProperty("fps", out var fEl)) item.Fps = fEl.GetDouble();
                if (root.TryGetProperty("codec", out var cEl)) item.Codec = cEl.GetString() ?? item.Codec;
                if (root.TryGetProperty("sampleRate", out var sEl)) item.SampleRate = sEl.GetInt32();
            }
        }
        catch
        {
            // Keep defaults if probe times out
        }

        if (isAudio && ext == ".wav")
        {
            var dsp = rustInterop.AnalyzeWavFileDsp(filePath, 2048);
            item.Peaks = dsp.Peaks;
            item.PeakDb = dsp.PeakDb;
            item.RmsDb = dsp.RmsDb;
            if (dsp.DurationSec > 0.2) item.Duration = dsp.DurationSec;
        }

        return item;
    }

    public List<MediaAssetItem> LoadWorkspaceMediaBin(RustCoreInterop rustInterop, bool includeDemoAssets = false)
    {
        var list = new List<MediaAssetItem>();
        if (!includeDemoAssets)
        {
            return list;
        }
        if (Directory.Exists(MediaDir))
        {
            foreach (string file in Directory.GetFiles(MediaDir))
            {
                string ext = Path.GetExtension(file).ToLowerInvariant();
                if (ext is not (".mp4" or ".mov" or ".webm" or ".wav" or ".mp3")) continue;
                bool isAud = ext is ".wav" or ".mp3";
                var asset = new MediaAssetItem
                {
                    Id = $"med_{Path.GetFileNameWithoutExtension(file)}",
                    Name = Path.GetFileName(file),
                    FileName = Path.GetFileName(file),
                    FilePath = file,
                    Type = isAud ? "audio" : "video",
                    Duration = 5.0,
                    Width = isAud ? 0 : 1280,
                    Height = isAud ? 0 : 720,
                    Fps = 29.97,
                    Codec = isAud ? "PCM 16-bit" : "H264 MP4",
                    Color = isAud ? "#0891b2" : "#2563eb"
                };
                if (ext == ".wav")
                {
                    var dsp = rustInterop.AnalyzeWavFileDsp(file, 2048);
                    asset.Peaks = dsp.Peaks;
                    asset.PeakDb = dsp.PeakDb;
                    asset.RmsDb = dsp.RmsDb;
                    asset.Duration = dsp.DurationSec;
                }
                list.Add(asset);
            }
        }

        if (list.Count == 0)
        {
            list.Add(new MediaAssetItem
            {
                Id = "demo_smpte",
                Name = "vegas_smpte_bars.mp4",
                FileName = "vegas_smpte_bars.mp4",
                Type = "video",
                GeneratorType = "smpteBars",
                Duration = 5.0,
                Width = 1280,
                Height = 720,
                Fps = 29.97,
                Codec = "H264",
                Color = "#2563eb"
            });
            list.Add(new MediaAssetItem
            {
                Id = "demo_synth",
                Name = "vegas_synth_pad.wav",
                FileName = "vegas_synth_pad.wav",
                Type = "audio",
                Duration = 5.0,
                SampleRate = 48000,
                PeakDb = -1.2,
                RmsDb = -12.4,
                Codec = "PCM WAV",
                Color = "#0891b2"
            });
        }
        return list;
    }

    public List<(string FileName, VegasProject Project)> LoadSavedProjects()
    {
        var result = new List<(string, VegasProject)>();
        string demoNeovegPath = Path.Combine(ProjectsDir, "Neoveg_Broadcast_Demo.neoveg");
        if (!File.Exists(demoNeovegPath))
        {
            SaveProjectToDisk(VegasProject.CreateBroadcastDemo(), "Neoveg_Broadcast_Demo.neoveg");
        }

        if (Directory.Exists(ProjectsDir))
        {
            foreach (string file in Directory.GetFiles(ProjectsDir, "*.neoveg"))
            {
                try
                {
                    var proj = NeovegProjectInterchangeEngine.LoadNativeNeovegContainer(file);
                    if (proj != null)
                    {
                        result.Add((Path.GetFileName(file), proj));
                    }
                }
                catch
                {
                    // Skip malformed file
                }
            }

            foreach (string file in Directory.GetFiles(ProjectsDir, "*.veg"))
            {
                try
                {
                    string json = File.ReadAllText(file, Encoding.UTF8);
                    var proj = JsonSerializer.Deserialize<VegasProject>(json);
                    if (proj != null)
                    {
                        result.Add((Path.GetFileName(file), proj));
                    }
                }
                catch
                {
                    // Skip malformed legacy file
                }
            }
        }

        if (result.Count == 0)
        {
            result.Add(("Neoveg_Broadcast_Demo.neoveg", VegasProject.CreateBroadcastDemo()));
        }
        return result;
    }

    public string SaveProjectToDisk(VegasProject project, string? fileName = null)
    {
        string safeName = string.IsNullOrWhiteSpace(fileName) ? project.Name : fileName;
        if (safeName.EndsWith(".veg", StringComparison.OrdinalIgnoreCase))
            safeName = Path.GetFileNameWithoutExtension(safeName) + ".neoveg";
        else if (!safeName.EndsWith(".neoveg", StringComparison.OrdinalIgnoreCase))
            safeName += ".neoveg";

        string fullPath = Path.IsPathRooted(safeName) ? safeName : Path.Combine(ProjectsDir, safeName);
        return NeovegProjectInterchangeEngine.SaveNativeNeovegContainer(project, fullPath, RootDir);
    }

    public async Task<(bool Ok, string OutputPath, string CommandPreview)> RenderProjectViaPythonFFmpegAsync(
        VegasProject project,
        string outputFileName = "Neoveg_Master_Cut.mp4")
    {
        string tempVeg = Path.Combine(ProjectsDir, "_temp_render_job.veg");
        File.WriteAllText(tempVeg, JsonSerializer.Serialize(project), Encoding.UTF8);

        string rawJson = await RunPythonScriptJsonAsync($"render \"{tempVeg}\" \"{outputFileName}\"", 120000);
        try
        {
            using var doc = JsonDocument.Parse(rawJson);
            var root = doc.RootElement;
            bool ok = root.TryGetProperty("ok", out var okEl) && okEl.GetBoolean();
            string outPath = root.TryGetProperty("output_path", out var pEl) ? pEl.GetString() ?? "" : "";
            string cmdStr = root.TryGetProperty("command_str", out var cEl) ? cEl.GetString() ?? "" : "";
            return (ok, outPath, cmdStr);
        }
        catch (Exception ex)
        {
            return (false, "", ex.Message);
        }
    }
}
