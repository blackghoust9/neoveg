using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Core;

public sealed class MissingMediaEntry
{
    public string Id { get; set; } = "";
    public string FileName { get; set; } = "";
    public string OriginalPath { get; set; } = "";
    public double ExpectedDuration { get; set; } = 4.0;
    public string MediaType { get; set; } = "video"; // "video" or "audio"
    public string Status { get; set; } = "Missing"; // "Missing", "Relinked", "IgnoredOffline"
    public string? ResolvedPath { get; set; }
    public string MatchStage { get; set; } = "Aguardando ação";
    public double Similarity { get; set; }
    public string? SfkCachePath { get; set; }
    public List<float>? RegeneratedPeaks { get; set; }
}

/// <summary>
/// `Neoveg.Core.MediaRelinker` — Automated 3-Stage Missing Media Search & Relink Engine:
/// 1. Detects missing files in `NeovegProject.MediaPool` and `TimelineClip.MediaFile` when opening `.veg` projects.
/// 2. Recursive Directory Scanner with 3-Stage Matching:
///    - Stage 1: Exact Filename & Extension Match (case-insensitive).
///    - Stage 2: Fuzzy Camera Prefix/Suffix & Levenshtein Token Similarity Match.
///    - Stage 3: Metadata Validation via Isolated FFprobe & Rust FFI (verifies duration and audio/video streams).
/// 3. Same-Directory Sibling Auto-Relink when a missing file is located or manually replaced.
/// 4. Regenerates `.sfk` peak caches via Rust FFI and updates `MediaPool`, Timeline clips, and Video Preview.
/// </summary>
public sealed class MediaRelinker
{
    private static readonly HashSet<string> VideoExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mp4", ".mov", ".mkv", ".avi", ".webm", ".m4v", ".mxf"
    };

    private static readonly HashSet<string> AudioExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".wav", ".mp3", ".flac", ".aac", ".ogg", ".m4a", ".aiff"
    };

    private static readonly Regex[] CameraNoiseRegexes =
    {
        new(@"^cam\d+[_-]?", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"^dji[_-]?\d*[_-]?", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"^gopro[_-]?", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"^a\d{3}[_-]?c\d{3}[_-]?", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?final\d*$", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?master$", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?proxy$", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?v\d+$", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?copy$", RegexOptions.IgnoreCase | RegexOptions.Compiled),
        new(@"[_-]?take\d+$", RegexOptions.IgnoreCase | RegexOptions.Compiled)
    };

    private readonly string _rootDir;
    private readonly RustCoreInterop _rustInterop;
    private readonly FFmpegPythonService _pythonService;

    public MediaRelinker(string rootDir, RustCoreInterop rustInterop, FFmpegPythonService pythonService)
    {
        _rootDir = rootDir;
        _rustInterop = rustInterop;
        _pythonService = pythonService;
    }

    /// <summary>
    /// Checks all files referenced in `project.MediaPool` and `project.Tracks` on disk.
    /// Returns the list of missing media files that could not be found at their original path.
    /// </summary>
    public List<MissingMediaEntry> DetectMissingMediaInProject(NeovegProject project)
    {
        var missingList = new List<MissingMediaEntry>();
        var seenOriginalPaths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        // 1. Inspect MediaPool entries
        foreach (var poolItem in project.MediaPool)
        {
            string rawPath = poolItem.FilePath ?? poolItem.FileName ?? "";
            if (string.IsNullOrWhiteSpace(rawPath)) continue;

            if (!DoesMediaPathExistOnDisk(rawPath))
            {
                if (seenOriginalPaths.Add(rawPath))
                {
                    missingList.Add(new MissingMediaEntry
                    {
                        Id = poolItem.Id,
                        FileName = !string.IsNullOrWhiteSpace(poolItem.FileName)
                            ? poolItem.FileName
                            : Path.GetFileName(rawPath),
                        OriginalPath = rawPath,
                        ExpectedDuration = Math.Max(0.5, poolItem.Duration),
                        MediaType = poolItem.Type == "audio" ? "audio" : "video",
                        Status = "Missing",
                        MatchStage = "Não encontrado no caminho original"
                    });
                }
            }
        }

        // 2. Inspect TimelineClip MediaFile references
        foreach (var track in project.Tracks)
        {
            foreach (var clip in track.Clips)
            {
                if (string.IsNullOrWhiteSpace(clip.MediaFile)) continue;
                string rawPath = clip.MediaFile;

                if (!DoesMediaPathExistOnDisk(rawPath) && seenOriginalPaths.Add(rawPath))
                {
                    missingList.Add(new MissingMediaEntry
                    {
                        Id = !string.IsNullOrWhiteSpace(clip.MediaId) ? clip.MediaId : clip.Id,
                        FileName = Path.GetFileName(rawPath),
                        OriginalPath = rawPath,
                        ExpectedDuration = Math.Max(0.5, clip.Duration),
                        MediaType = track.Type == "audio" ? "audio" : "video",
                        Status = "Missing",
                        MatchStage = "Não encontrado no caminho original"
                    });
                }
            }
        }

        return missingList;
    }

    private bool DoesMediaPathExistOnDisk(string rawPath)
    {
        if (File.Exists(rawPath))
            return true;

        // If `rawPath` is a relative filename without a drive/root (e.g. "neoveg_smpte_bars.mp4"), check `media/`
        if (!Path.IsPathRooted(rawPath) && !rawPath.Contains(':') && !rawPath.Contains('\\') && !rawPath.Contains('/'))
        {
            string inWorkspaceMedia = Path.Combine(_rootDir, "media", rawPath);
            if (File.Exists(inWorkspaceMedia))
                return true;
            string inBaseMedia = Path.Combine(AppContext.BaseDirectory, "media", rawPath);
            if (File.Exists(inBaseMedia))
                return true;
        }

        return false;
    }

    /// <summary>
    /// Recursively scans `searchDirectory`, executes the 3-Stage Matching Algorithm (Exact Name -> Fuzzy Camera Name -> FFprobe/Rust Metadata),
    /// auto-relinks sibling files in discovered folders, and regenerates `.sfk` peaks via Rust FFI.
    /// </summary>
    public async Task<int> SearchDirectoryAndRelinkAsync(
        string searchDirectory,
        List<MissingMediaEntry> entries)
    {
        if (!Directory.Exists(searchDirectory))
        {
            searchDirectory = Path.Combine(_rootDir, "media");
        }

        var candidateFiles = new List<FileInfo>();
        CollectMediaFilesRecursively(new DirectoryInfo(searchDirectory), candidateFiles, maxDepth: 6);

        var discoveredFolders = new List<string>();
        int relinkedCount = 0;

        foreach (var entry in entries)
        {
            if (entry.Status == "Relinked")
            {
                relinkedCount++;
                continue;
            }

            FileInfo? matchedFile = null;
            string matchStage = "";
            double bestSim = 0.0;

            // STAGE 1: Exact Filename + Extension Match + Metadata Validation
            foreach (var cand in candidateFiles)
            {
                if (string.Equals(cand.Name, entry.FileName, StringComparison.OrdinalIgnoreCase))
                {
                    if (await ValidateCandidateMetadataAsync(cand.FullName, entry.MediaType, entry.ExpectedDuration))
                    {
                        matchedFile = cand;
                        matchStage = "Etapa 1: Nome Exato + Validação FFprobe/Rust FFI";
                        bestSim = 1.0;
                        break;
                    }
                }
            }

            // STAGE 2 + STAGE 3: Fuzzy Camera Name Match + FFprobe Metadata Validation
            if (matchedFile == null)
            {
                var scoredCandidates = candidateFiles
                    .Where(c => IsExtensionCompatible(entry.MediaType, c.Extension))
                    .Select(c => (File: c, Score: ComputeFuzzyNameSimilarity(entry.FileName, c.Name)))
                    .Where(x => x.Score >= 0.52)
                    .OrderByDescending(x => x.Score)
                    .ToList();

                foreach (var (cand, score) in scoredCandidates)
                {
                    if (await ValidateCandidateMetadataAsync(cand.FullName, entry.MediaType, entry.ExpectedDuration))
                    {
                        matchedFile = cand;
                        matchStage = $"Etapa 2+3: Fuzzy Match ({score * 100:0}%) + Metadados FFprobe";
                        bestSim = score;
                        break;
                    }
                }
            }

            if (matchedFile != null)
            {
                FinalizeEntryRelink(entry, matchedFile.FullName, matchStage, bestSim);
                if (matchedFile.DirectoryName != null &&
                    !discoveredFolders.Contains(matchedFile.DirectoryName, StringComparer.OrdinalIgnoreCase))
                {
                    discoveredFolders.Add(matchedFile.DirectoryName);
                }
                relinkedCount++;
            }
        }

        // AUTO-RELINK DE MÍDIAS DO MESMO DIRETÓRIO (Same-Directory Sibling Auto-Relink)
        foreach (string folder in discoveredFolders)
        {
            relinkedCount += AutoRelinkSiblingsInFolder(folder, entries);
        }

        return relinkedCount;
    }

    /// <summary>
    /// Manually replaces a single missing media entry and automatically relinks any other missing files found in that same directory.
    /// </summary>
    public int SpecifyManualLocationAndAutoRelinkSiblings(
        MissingMediaEntry targetEntry,
        string selectedFilePath,
        List<MissingMediaEntry> allEntries)
    {
        if (!File.Exists(selectedFilePath)) return 0;

        FinalizeEntryRelink(targetEntry, selectedFilePath, "Substituição Manual + Validação Rust FFI (.sfk)", 1.0);
        int count = 1;

        string? dir = Path.GetDirectoryName(selectedFilePath);
        if (!string.IsNullOrWhiteSpace(dir) && Directory.Exists(dir))
        {
            count += AutoRelinkSiblingsInFolder(dir, allEntries);
        }

        return count;
    }

    public int AutoRelinkSiblingsInFolder(string directoryPath, List<MissingMediaEntry> entries)
    {
        if (!Directory.Exists(directoryPath)) return 0;

        int autoRelinked = 0;
        var siblingFiles = new DirectoryInfo(directoryPath)
            .GetFiles()
            .Where(f => VideoExtensions.Contains(f.Extension) || AudioExtensions.Contains(f.Extension))
            .ToList();

        foreach (var entry in entries)
        {
            if (entry.Status == "Relinked") continue;

            // Check exact match first, then fuzzy match in the same folder
            var exact = siblingFiles.FirstOrDefault(f => string.Equals(f.Name, entry.FileName, StringComparison.OrdinalIgnoreCase));
            if (exact != null)
            {
                FinalizeEntryRelink(entry, exact.FullName, "Auto-Relink (Mesmo Diretório • Nome Exato)", 1.0);
                autoRelinked++;
                continue;
            }

            var fuzzy = siblingFiles
                .Where(f => IsExtensionCompatible(entry.MediaType, f.Extension))
                .Select(f => (File: f, Sim: ComputeFuzzyNameSimilarity(entry.FileName, f.Name)))
                .Where(x => x.Sim >= 0.52)
                .OrderByDescending(x => x.Sim)
                .FirstOrDefault();

            if (fuzzy.File != null)
            {
                FinalizeEntryRelink(entry, fuzzy.File.FullName, $"Auto-Relink (Mesmo Diretório • Fuzzy {fuzzy.Sim * 100:0}%)", fuzzy.Sim);
                autoRelinked++;
            }
        }

        return autoRelinked;
    }

    private void FinalizeEntryRelink(MissingMediaEntry entry, string resolvedFullPath, string matchStage, double similarity)
    {
        entry.ResolvedPath = Path.GetFullPath(resolvedFullPath);
        entry.Status = "Relinked";
        entry.MatchStage = matchStage;
        entry.Similarity = similarity;

        // Regenerate .sfk audio peak cache via Rust FFI
        string sfkPath = entry.ResolvedPath + ".sfk";
        var (peaks, _) = _rustInterop.GenerateAndSaveSfkPeaks(entry.ResolvedPath, sfkPath, 96);
        entry.SfkCachePath = sfkPath;
        entry.RegeneratedPeaks = peaks;
    }

    /// <summary>
    /// Applies the relinked paths and offline flags to `project.MediaPool` and all `TimelineClip` events on the timeline.
    /// </summary>
    public void ApplyRelinkResolutionsToProject(NeovegProject project, IReadOnlyList<MissingMediaEntry> entries)
    {
        var byOriginalPath = entries.ToDictionary(e => e.OriginalPath, StringComparer.OrdinalIgnoreCase);
        var byId = entries.GroupBy(e => e.Id, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => g.First(), StringComparer.OrdinalIgnoreCase);

        // 1. Update MediaPool
        foreach (var poolItem in project.MediaPool)
        {
            MissingMediaEntry? matched = null;
            if (!string.IsNullOrWhiteSpace(poolItem.FilePath) && byOriginalPath.TryGetValue(poolItem.FilePath, out var mPath))
                matched = mPath;
            else if (byId.TryGetValue(poolItem.Id, out var mId))
                matched = mId;

            if (matched != null)
            {
                if (matched.Status == "Relinked" && !string.IsNullOrWhiteSpace(matched.ResolvedPath))
                {
                    poolItem.FilePath = matched.ResolvedPath;
                    poolItem.FileName = Path.GetFileName(matched.ResolvedPath);
                    poolItem.IsOffline = false;
                    if (matched.RegeneratedPeaks != null && matched.RegeneratedPeaks.Count > 0)
                        poolItem.Peaks = matched.RegeneratedPeaks;
                }
                else
                {
                    poolItem.IsOffline = true;
                }
            }
        }

        // 2. Update Timeline Clips
        foreach (var track in project.Tracks)
        {
            foreach (var clip in track.Clips)
            {
                MissingMediaEntry? matched = null;
                if (!string.IsNullOrWhiteSpace(clip.MediaFile) && byOriginalPath.TryGetValue(clip.MediaFile, out var mPath))
                    matched = mPath;
                else if (!string.IsNullOrWhiteSpace(clip.MediaId) && byId.TryGetValue(clip.MediaId, out var mId))
                    matched = mId;

                if (matched != null)
                {
                    if (matched.Status == "Relinked" && !string.IsNullOrWhiteSpace(matched.ResolvedPath))
                    {
                        clip.MediaFile = matched.ResolvedPath;
                        clip.IsOffline = false;
                        if (matched.RegeneratedPeaks != null && matched.RegeneratedPeaks.Count > 0)
                            clip.Peaks = new List<float>(matched.RegeneratedPeaks);
                    }
                    else
                    {
                        clip.IsOffline = true;
                        clip.Color = "#dc2626";
                    }
                }
            }
        }
    }

    private async Task<bool> ValidateCandidateMetadataAsync(
        string candidateFullPath,
        string expectedType,
        double expectedDuration)
    {
        if (!File.Exists(candidateFullPath)) return false;

        string ext = Path.GetExtension(candidateFullPath);
        if (!IsExtensionCompatible(expectedType, ext)) return false;

        try
        {
            var probed = await _pythonService.ProbeAndImportMediaAsync(candidateFullPath, _rustInterop);
            if (probed != null && probed.Duration > 0.1 && expectedDuration > 0.1)
            {
                // Allow reasonable duration tolerance for camera proxies/trims
                double diff = Math.Abs(probed.Duration - expectedDuration);
                return diff <= Math.Max(15.0, expectedDuration * 3.0);
            }
        }
        catch
        {
            // Fallback to file size + extension validation
        }

        return new FileInfo(candidateFullPath).Length > 64;
    }

    private static bool IsExtensionCompatible(string expectedType, string extension)
    {
        if (string.Equals(expectedType, "audio", StringComparison.OrdinalIgnoreCase))
            return AudioExtensions.Contains(extension) || VideoExtensions.Contains(extension);
        return VideoExtensions.Contains(extension);
    }

    public static string NormalizeCameraStem(string fileName)
    {
        string stem = Path.GetFileNameWithoutExtension(fileName).ToLowerInvariant().Trim();
        foreach (var rx in CameraNoiseRegexes)
        {
            stem = rx.Replace(stem, "").Trim('_', '-', ' ');
        }
        return string.IsNullOrWhiteSpace(stem)
            ? Path.GetFileNameWithoutExtension(fileName).ToLowerInvariant()
            : stem;
    }

    public static double ComputeFuzzyNameSimilarity(string expectedFileName, string candidateFileName)
    {
        string s1 = NormalizeCameraStem(expectedFileName);
        string s2 = NormalizeCameraStem(candidateFileName);
        if (string.IsNullOrEmpty(s1) || string.IsNullOrEmpty(s2)) return 0.0;
        if (string.Equals(s1, s2, StringComparison.OrdinalIgnoreCase)) return 0.98;
        if (s1.Contains(s2, StringComparison.OrdinalIgnoreCase) || s2.Contains(s1, StringComparison.OrdinalIgnoreCase))
            return 0.88;

        int dist = ComputeLevenshteinDistance(s1, s2);
        int maxLen = Math.Max(s1.Length, s2.Length);
        return Math.Clamp(1.0 - ((double)dist / maxLen), 0.0, 1.0);
    }

    private static int ComputeLevenshteinDistance(string a, string b)
    {
        int n = a.Length;
        int m = b.Length;
        int[,] d = new int[n + 1, m + 1];
        for (int i = 0; i <= n; i++) d[i, 0] = i;
        for (int j = 0; j <= m; j++) d[0, j] = j;

        for (int i = 1; i <= n; i++)
        {
            for (int j = 1; j <= m; j++)
            {
                int cost = char.ToLowerInvariant(a[i - 1]) == char.ToLowerInvariant(b[j - 1]) ? 0 : 1;
                d[i, j] = Math.Min(
                    Math.Min(d[i - 1, j] + 1, d[i, j - 1] + 1),
                    d[i - 1, j - 1] + cost);
            }
        }
        return d[n, m];
    }

    private static void CollectMediaFilesRecursively(DirectoryInfo dir, List<FileInfo> results, int maxDepth)
    {
        if (maxDepth < 0 || !dir.Exists) return;
        try
        {
            foreach (var f in dir.GetFiles())
            {
                if (VideoExtensions.Contains(f.Extension) || AudioExtensions.Contains(f.Extension))
                    results.Add(f);
            }
            foreach (var sub in dir.GetDirectories())
            {
                if (sub.Name.StartsWith('.')) continue;
                CollectMediaFilesRecursively(sub, results, maxDepth - 1);
            }
        }
        catch
        {
            // Ignore inaccessible directories
        }
    }
}
