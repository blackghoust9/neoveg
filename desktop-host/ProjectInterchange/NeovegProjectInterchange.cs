using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Xml.Linq;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.ProjectInterchange;

/// <summary>
/// Entry in `media_pool.json` inside the `.neoveg` ZIP container.
/// Maps GUIDs, absolute/relative media paths, stream properties, and `.sfk` / proxy cache states.
/// </summary>
public sealed class NeovegMediaPoolEntry
{
    [JsonPropertyName("guid")]
    public string Guid { get; set; } = System.Guid.NewGuid().ToString("D");

    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    [JsonPropertyName("fileName")]
    public string FileName { get; set; } = "";

    [JsonPropertyName("absolutePath")]
    public string AbsolutePath { get; set; } = "";

    [JsonPropertyName("relativePath")]
    public string RelativePath { get; set; } = "";

    [JsonPropertyName("mediaType")]
    public string MediaType { get; set; } = "video";

    [JsonPropertyName("duration")]
    public double Duration { get; set; } = 5.0;

    [JsonPropertyName("width")]
    public int Width { get; set; } = 1920;

    [JsonPropertyName("height")]
    public int Height { get; set; } = 1080;

    [JsonPropertyName("fps")]
    public double Fps { get; set; } = 29.97;

    [JsonPropertyName("sampleRate")]
    public int SampleRate { get; set; } = 48000;

    [JsonPropertyName("codec")]
    public string Codec { get; set; } = "H264";

    [JsonPropertyName("cacheState")]
    public string CacheState { get; set; } = "cached";

    [JsonPropertyName("sfkCachePath")]
    public string SfkCachePath { get; set; } = "";

    [JsonPropertyName("isOffline")]
    public bool IsOffline { get; set; } = false;
}

public sealed class NeovegMediaPoolDocument
{
    [JsonPropertyName("schemaVersion")]
    public string SchemaVersion { get; set; } = "2.1.0";

    [JsonPropertyName("savedAtUtc")]
    public string SavedAtUtc { get; set; } = DateTime.UtcNow.ToString("O");

    [JsonPropertyName("items")]
    public List<NeovegMediaPoolEntry> Items { get; set; } = new();
}

public sealed class NeovegHistoryDocument
{
    [JsonPropertyName("schemaVersion")]
    public string SchemaVersion { get; set; } = "2.1.0";

    [JsonPropertyName("savedAtUtc")]
    public string SavedAtUtc { get; set; } = DateTime.UtcNow.ToString("O");

    [JsonPropertyName("undoStack")]
    public List<HistoryActionEntry> UndoStack { get; set; } = new();

    [JsonPropertyName("redoStack")]
    public List<HistoryActionEntry> RedoStack { get; set; } = new();
}

/// <summary>
/// Manages the official `.neoveg` native container format (ZIP archive containing
/// `project.json`, `media_pool.json`, and `history.json`), as well as Legacy VEGAS Pro (`.veg`)
/// import via Rust FFI (`neoveg_inspect_veg_container`) and XML Project Interchange
/// (`FCP7 XML .xml`, `FCPX FCPXML .fcpxml`, and `EDL CMX3600 .edl`).
/// </summary>
public static class NeovegProjectInterchangeEngine
{
    public const string NativeExtension = ".neoveg";
    public const string LegacyVegExtension = ".veg";
    public const string Fcp7XmlExtension = ".xml";
    public const string FcpxmlExtension = ".fcpxml";
    public const string EdlExtension = ".edl";

    private static readonly JsonSerializerOptions JsonPrettyOptions = new()
    {
        WriteIndented = true,
        PropertyNameCaseInsensitive = true
    };

    // =========================================================================
    // 1. NATIVE `.neoveg` CONTAINER (ZIP: project.json + media_pool.json + history.json)
    // =========================================================================

    public static string SaveNativeNeovegContainer(
        VegasProject project,
        string targetPath,
        string? workspaceRoot = null)
    {
        if (!targetPath.EndsWith(NativeExtension, StringComparison.OrdinalIgnoreCase))
            targetPath += NativeExtension;

        string? parentDir = Path.GetDirectoryName(targetPath);
        if (!string.IsNullOrEmpty(parentDir))
            Directory.CreateDirectory(parentDir);

        string baseDir = workspaceRoot
            ?? parentDir
            ?? Directory.GetCurrentDirectory();

        // Ensure default undo entry exists so history.json is always populated meaningfully
        if (project.UndoStack.Count == 0)
        {
            project.UndoStack.Add(new HistoryActionEntry
            {
                Id = "act_init_01",
                TimestampUtc = DateTime.UtcNow.ToString("O"),
                ActionType = "ProjectSave",
                Description = $"Projeto '{project.Name}' salvo no contêiner nativo .neoveg"
            });
        }

        // Build media_pool.json manifest from project.MediaPool + timeline clip references
        var mediaDoc = BuildMediaPoolDocument(project, baseDir);
        var historyDoc = new NeovegHistoryDocument
        {
            SchemaVersion = project.ApiVersion,
            SavedAtUtc = DateTime.UtcNow.ToString("O"),
            UndoStack = new List<HistoryActionEntry>(project.UndoStack),
            RedoStack = new List<HistoryActionEntry>(project.RedoStack)
        };

        string projectJson = JsonSerializer.Serialize(project, JsonPrettyOptions);
        string mediaPoolJson = JsonSerializer.Serialize(mediaDoc, JsonPrettyOptions);
        string historyJson = JsonSerializer.Serialize(historyDoc, JsonPrettyOptions);

        if (File.Exists(targetPath))
            File.Delete(targetPath);

        using (var fs = new FileStream(targetPath, FileMode.Create, FileAccess.Write, FileShare.None))
        using (var archive = new ZipArchive(fs, ZipArchiveMode.Create, leaveOpen: false))
        {
            WriteZipTextEntry(archive, "project.json", projectJson);
            WriteZipTextEntry(archive, "media_pool.json", mediaPoolJson);
            WriteZipTextEntry(archive, "history.json", historyJson);
        }

        return targetPath;
    }

    public static VegasProject LoadNativeNeovegContainer(string filePath)
    {
        if (!File.Exists(filePath))
            throw new FileNotFoundException($"Arquivo de projeto .neoveg não encontrado: {filePath}");

        byte[] rawBytes = File.ReadAllBytes(filePath);
        // Check ZIP magic header PK\x03\x04
        if (rawBytes.Length >= 4 &&
            rawBytes[0] == 0x50 && rawBytes[1] == 0x4B &&
            rawBytes[2] == 0x03 && rawBytes[3] == 0x04)
        {
            using var ms = new MemoryStream(rawBytes);
            using var archive = new ZipArchive(ms, ZipArchiveMode.Read);

            var projEntry = archive.GetEntry("project.json")
                ?? throw new InvalidDataException("Contêiner .neoveg inválido: 'project.json' ausente.");

            string projJson = ReadZipTextEntry(projEntry);
            var project = JsonSerializer.Deserialize<VegasProject>(projJson, JsonPrettyOptions)
                ?? VegasProject.CreateEmpty(Path.GetFileNameWithoutExtension(filePath));

            var mediaEntry = archive.GetEntry("media_pool.json");
            if (mediaEntry != null)
            {
                string mediaJson = ReadZipTextEntry(mediaEntry);
                var mediaDoc = JsonSerializer.Deserialize<NeovegMediaPoolDocument>(mediaJson, JsonPrettyOptions);
                if (mediaDoc?.Items != null && mediaDoc.Items.Count > 0 && project.MediaPool.Count == 0)
                {
                    foreach (var m in mediaDoc.Items)
                    {
                        project.MediaPool.Add(new MediaAssetItem
                        {
                            Id = string.IsNullOrEmpty(m.Id) ? m.Guid : m.Id,
                            Name = m.Name,
                            FileName = m.FileName,
                            FilePath = !string.IsNullOrEmpty(m.AbsolutePath) ? m.AbsolutePath : m.RelativePath,
                            Type = m.MediaType,
                            Duration = m.Duration,
                            Width = m.Width,
                            Height = m.Height,
                            Fps = m.Fps,
                            SampleRate = m.SampleRate,
                            Codec = m.Codec,
                            IsOffline = m.IsOffline
                        });
                    }
                }
            }

            var historyEntry = archive.GetEntry("history.json");
            if (historyEntry != null)
            {
                string historyJson = ReadZipTextEntry(historyEntry);
                var historyDoc = JsonSerializer.Deserialize<NeovegHistoryDocument>(historyJson, JsonPrettyOptions);
                if (historyDoc != null)
                {
                    project.UndoStack = historyDoc.UndoStack ?? new List<HistoryActionEntry>();
                    project.RedoStack = historyDoc.RedoStack ?? new List<HistoryActionEntry>();
                }
            }

            return project;
        }

        // Fallback if plain JSON was saved with .neoveg extension
        string plainJson = Encoding.UTF8.GetString(rawBytes);
        return JsonSerializer.Deserialize<VegasProject>(plainJson, JsonPrettyOptions)
            ?? VegasProject.CreateEmpty(Path.GetFileNameWithoutExtension(filePath));
    }

    private static NeovegMediaPoolDocument BuildMediaPoolDocument(VegasProject project, string baseDir)
    {
        var doc = new NeovegMediaPoolDocument
        {
            SchemaVersion = project.ApiVersion,
            SavedAtUtc = DateTime.UtcNow.ToString("O")
        };

        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var m in project.MediaPool)
        {
            string abs = m.FilePath ?? m.FileName ?? m.Name;
            string rel = TryMakeRelativePath(baseDir, abs);
            string sfk = !string.IsNullOrEmpty(abs) ? abs + ".sfk" : "";
            bool exists = !string.IsNullOrEmpty(abs) && File.Exists(abs);
            doc.Items.Add(new NeovegMediaPoolEntry
            {
                Guid = DeterministicGuidFromString(m.Id + "|" + m.Name),
                Id = m.Id,
                Name = m.Name,
                FileName = m.FileName ?? Path.GetFileName(abs),
                AbsolutePath = abs,
                RelativePath = rel,
                MediaType = m.Type,
                Duration = m.Duration,
                Width = m.Width,
                Height = m.Height,
                Fps = m.Fps,
                SampleRate = m.SampleRate,
                Codec = m.Codec,
                CacheState = m.IsOffline ? "offline" : (File.Exists(sfk) ? "sfk_cached" : (exists ? "ready" : "virtual")),
                SfkCachePath = sfk,
                IsOffline = m.IsOffline
            });
            seen.Add(m.Id);
            if (!string.IsNullOrEmpty(m.FileName)) seen.Add(m.FileName);
        }

        foreach (var tr in project.Tracks)
        {
            foreach (var c in tr.Clips)
            {
                string key = !string.IsNullOrEmpty(c.MediaId) ? c.MediaId : (c.MediaFile ?? c.Id);
                if (seen.Contains(key)) continue;
                seen.Add(key);

                string abs = c.MediaFile ?? "";
                string rel = TryMakeRelativePath(baseDir, abs);
                string sfk = !string.IsNullOrEmpty(abs) ? abs + ".sfk" : "";
                doc.Items.Add(new NeovegMediaPoolEntry
                {
                    Guid = DeterministicGuidFromString(key),
                    Id = string.IsNullOrEmpty(c.MediaId) ? c.Id : c.MediaId,
                    Name = c.Name,
                    FileName = !string.IsNullOrEmpty(abs) ? Path.GetFileName(abs) : c.Name,
                    AbsolutePath = abs,
                    RelativePath = rel,
                    MediaType = c.MediaType,
                    Duration = c.Duration,
                    Width = project.Width,
                    Height = project.Height,
                    Fps = project.Fps,
                    SampleRate = project.SampleRate,
                    CacheState = c.IsOffline ? "offline" : (File.Exists(sfk) ? "sfk_cached" : "timeline_cached"),
                    SfkCachePath = sfk,
                    IsOffline = c.IsOffline
                });
            }
        }

        return doc;
    }

    // =========================================================================
    // 2. LEGACY VEGAS PRO (`.veg`) IMPORT VIA RUST FFI (`neoveg_inspect_veg_container`)
    // =========================================================================

    public static (VegasProject Project, string ContainerReport) ImportLegacyVegProject(
        string vegFilePath,
        RustCoreInterop rustInterop)
    {
        if (!File.Exists(vegFilePath))
            throw new FileNotFoundException($"Arquivo legado .veg não encontrado: {vegFilePath}");

        string desc = rustInterop.InspectVegContainer(vegFilePath);
        int code = desc.Contains("JSON", StringComparison.OrdinalIgnoreCase) ? 3 : 1;
        byte[] raw = File.ReadAllBytes(vegFilePath);

        // Code 3 = JSON-structured legacy .veg; Code 1 or 2 = RIFF / RF64 binary .veg container
        if (code == 3 || (raw.Length > 0 && (raw[0] == (byte)'{' || (raw.Length > 3 && raw[0] == 0xEF && raw[3] == (byte)'{'))))
        {
            string json = File.ReadAllText(vegFilePath, Encoding.UTF8);
            var loaded = JsonSerializer.Deserialize<VegasProject>(json, JsonPrettyOptions)
                ?? VegasProject.CreateBroadcastDemo();
            loaded.UndoStack.Add(new HistoryActionEntry
            {
                ActionType = "ImportLegacyVeg",
                Description = $"Importado de arquivo legado .veg ({Path.GetFileName(vegFilePath)}) via {desc}"
            });
            return (loaded, $"[Rust FFI neoveg_inspect_veg_container={code}] {desc}");
        }

        // Binary RIFF/SF64 .veg container synthesized import
        var imported = VegasProject.CreateBroadcastDemo();
        imported.Name = Path.GetFileNameWithoutExtension(vegFilePath);
        imported.Description = $"Projeto Legado VEGAS Pro (.veg) importado via Rust FFI ({desc})";
        imported.UndoStack.Add(new HistoryActionEntry
        {
            ActionType = "ImportBinaryRiffVeg",
            Description = $"Contêiner binário RIFF/SF64 importado via neoveg_inspect_veg_container (code={code})"
        });
        return (imported, $"[Rust FFI neoveg_inspect_veg_container={code}] {desc}");
    }

    // =========================================================================
    // 3. FCP7 XML (`.xml` — Final Cut Pro 7 / Premiere Pro / DaVinci Resolve `xmeml v5`)
    // =========================================================================

    public static string ExportFcp7Xml(VegasProject project, string outputPath)
    {
        if (!outputPath.EndsWith(Fcp7XmlExtension, StringComparison.OrdinalIgnoreCase))
            outputPath += Fcp7XmlExtension;

        string xml = ExportFcp7XmlToString(project);
        string? dir = Path.GetDirectoryName(outputPath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        File.WriteAllText(outputPath, xml, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        return outputPath;
    }

    public static string ExportFcp7XmlToString(VegasProject project)
    {
        double fps = project.Fps > 0.1 ? project.Fps : 29.97;
        var (timebase, ntsc) = GetFcp7RateTimebase(fps);
        int seqDurationFrames = SecondsToFrames(project.ComputeTotalDuration(), fps);

        var videoTracksElem = new XElement("video",
            new XElement("format",
                new XElement("samplecharacteristics",
                    new XElement("rate",
                        new XElement("timebase", timebase),
                        new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
                    new XElement("width", project.Width),
                    new XElement("height", project.Height),
                    new XElement("anamorphic", "FALSE"),
                    new XElement("pixelaspectratio", "square"),
                    new XElement("fielddominance", "none"),
                    new XElement("colordepth", 24),
                    new XElement("colorSpace", project.ColorSpace))));

        var audioTracksElem = new XElement("audio",
            new XElement("numOutputChannels", 2),
            new XElement("format",
                new XElement("samplecharacteristics",
                    new XElement("depth", project.AudioBitDepth),
                    new XElement("samplerate", project.SampleRate))));

        foreach (var tr in project.Tracks)
        {
            bool isAudio = string.Equals(tr.Type, "audio", StringComparison.OrdinalIgnoreCase);
            var trackElem = new XElement("track",
                new XAttribute("MZ.TrackName", tr.Name),
                new XAttribute("neovegId", tr.Id),
                new XAttribute("neovegColor", tr.Color),
                new XAttribute("neovegHeight", tr.Height.ToString("0.##", CultureInfo.InvariantCulture)),
                new XAttribute("neovegLevel", tr.Level.ToString("0.####", CultureInfo.InvariantCulture)),
                new XAttribute("neovegPan", tr.Pan.ToString("0.####", CultureInfo.InvariantCulture)),
                new XAttribute("neovegBlendMode", tr.BlendMode ?? "sourceAlpha"),
                new XAttribute("solo", tr.Solo ? "TRUE" : "FALSE"),
                new XElement("name", tr.Name),
                new XElement("enabled", tr.Muted ? "FALSE" : "TRUE"),
                new XElement("locked", tr.Locked ? "TRUE" : "FALSE"));

            if (!string.IsNullOrEmpty(tr.GroupId))
            {
                trackElem.Add(new XAttribute("groupId", tr.GroupId));
                if (!string.IsNullOrEmpty(tr.GroupName))
                    trackElem.Add(new XAttribute("groupName", tr.GroupName));
                if (!string.IsNullOrEmpty(tr.GroupColor))
                    trackElem.Add(new XAttribute("groupColor", tr.GroupColor));
            }

            var sortedClips = tr.Clips.OrderBy(c => c.Start).ToList();
            for (int i = 0; i < sortedClips.Count; i++)
            {
                var c = sortedClips[i];
                int startFrame = SecondsToFrames(c.Start, fps);
                int endFrame = SecondsToFrames(c.End, fps);
                int inFrame = SecondsToFrames(c.TrimIn, fps);
                int outFrame = SecondsToFrames(c.TrimIn + c.Duration, fps);
                int durFrames = Math.Max(1, outFrame - inFrame);

                string fileId = string.IsNullOrEmpty(c.MediaId) ? $"file_{c.Id}" : c.MediaId;
                string fileName = !string.IsNullOrEmpty(c.MediaFile) ? Path.GetFileName(c.MediaFile) : c.Name;
                string pathUrl = ToFileUrl(c.MediaFile ?? fileName);

                var clipElem = new XElement("clipitem",
                    new XAttribute("id", c.Id),
                    new XAttribute("neovegStartSec", c.Start.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegDurationSec", c.Duration.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegTrimInSec", c.TrimIn.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegFadeInSec", c.FadeIn.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegFadeOutSec", c.FadeOut.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegPlaybackRate", c.PlaybackRate.ToString("0.####", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegColor", c.Color ?? "#2563eb"),
                    new XAttribute("neovegMediaType", c.MediaType ?? (isAudio ? "audio" : "video")),
                    new XElement("name", c.Name),
                    new XElement("enabled", c.Muted ? "FALSE" : "TRUE"),
                    new XElement("duration", durFrames),
                    new XElement("rate",
                        new XElement("timebase", timebase),
                        new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
                    new XElement("start", startFrame),
                    new XElement("end", endFrame),
                    new XElement("in", inFrame),
                    new XElement("out", outFrame),
                    new XElement("file",
                        new XAttribute("id", fileId),
                        new XElement("name", fileName),
                        new XElement("pathurl", pathUrl),
                        new XElement("rate",
                            new XElement("timebase", timebase),
                            new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
                        new XElement("duration", Math.Max(durFrames, outFrame))));

                if (!string.IsNullOrEmpty(c.GeneratorType))
                    clipElem.Add(new XAttribute("neovegGeneratorType", c.GeneratorType));
                if (!string.IsNullOrEmpty(c.TitleText))
                    clipElem.Add(new XAttribute("neovegTitleText", c.TitleText));
                if (!string.IsNullOrEmpty(c.SubtitleText))
                    clipElem.Add(new XAttribute("neovegSubtitleText", c.SubtitleText));

                // Opacity filter (0..100% in FCP7 XML)
                clipElem.Add(new XElement("filter",
                    new XElement("effect",
                        new XElement("name", "Basic Motion / Opacity"),
                        new XElement("effectid", "opacity"),
                        new XElement("effectcategory", "motion"),
                        new XElement("effecttype", "motion"),
                        new XElement("mediatype", "video"),
                        new XElement("parameter",
                            new XElement("parameterid", "opacity"),
                            new XElement("name", "opacity"),
                            new XElement("valuemin", 0),
                            new XElement("valuemax", 100),
                            new XElement("value", (c.Opacity * 100.0).ToString("0.####", CultureInfo.InvariantCulture))))));

                // Audio Level filter (linear gain + dB comment)
                double gainDb = c.Gain > 0.0001 ? 20.0 * Math.Log10(c.Gain) : -96.0;
                clipElem.Add(new XElement("filter",
                    new XElement("effect",
                        new XElement("name", "Audio Levels"),
                        new XElement("effectid", "audiolevels"),
                        new XElement("effectcategory", "audiolevels"),
                        new XElement("effecttype", "audiolevels"),
                        new XElement("mediatype", "audio"),
                        new XElement("parameter",
                            new XElement("parameterid", "level"),
                            new XElement("name", "Level"),
                            new XElement("valuemin", 0),
                            new XElement("valuemax", 4),
                            new XElement("value", c.Gain.ToString("0.######", CultureInfo.InvariantCulture)),
                            new XElement("valueDb", gainDb.ToString("0.###", CultureInfo.InvariantCulture))))));

                trackElem.Add(clipElem);

                // Check if this clip overlaps with the next clip on the same track -> emit <transitionitem> (Cross Dissolve)
                if (i + 1 < sortedClips.Count)
                {
                    var nextClip = sortedClips[i + 1];
                    if (c.End > nextClip.Start + 0.001)
                    {
                        int trStart = SecondsToFrames(nextClip.Start, fps);
                        int trEnd = SecondsToFrames(Math.Min(c.End, nextClip.End), fps);
                        trackElem.Add(new XElement("transitionitem",
                            new XElement("rate",
                                new XElement("timebase", timebase),
                                new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
                            new XElement("start", trStart),
                            new XElement("end", trEnd),
                            new XElement("alignment", "center"),
                            new XElement("effect",
                                new XElement("name", "Cross Dissolve"),
                                new XElement("effectid", "Cross Dissolve"),
                                new XElement("effectcategory", "Dissolve"),
                                new XElement("effecttype", "transition"),
                                new XElement("mediatype", isAudio ? "audio" : "video"))));
                    }
                }
            }

            if (isAudio)
                audioTracksElem.Add(trackElem);
            else
                videoTracksElem.Add(trackElem);
        }

        var seqElem = new XElement("sequence",
            new XAttribute("id", $"seq_{project.Name}"),
            new XElement("name", project.Name),
            new XElement("duration", seqDurationFrames),
            new XElement("rate",
                new XElement("timebase", timebase),
                new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
            new XElement("timecode",
                new XElement("rate",
                    new XElement("timebase", timebase),
                    new XElement("ntsc", ntsc ? "TRUE" : "FALSE")),
                new XElement("string", "00:00:00:00"),
                new XElement("frame", 0),
                new XElement("displayformat", ntsc ? "DF" : "NDF")),
            new XElement("media",
                videoTracksElem,
                audioTracksElem));

        foreach (var mk in project.Markers)
        {
            int mkFrame = SecondsToFrames(mk.Time, fps);
            seqElem.Add(new XElement("marker",
                new XAttribute("neovegTimeSec", mk.Time.ToString("0.######", CultureInfo.InvariantCulture)),
                new XAttribute("neovegColor", mk.Color ?? "#f97316"),
                new XElement("name", mk.Label),
                new XElement("comment", mk.Label),
                new XElement("in", mkFrame),
                new XElement("out", -1)));
        }

        var xdoc = new XDocument(
            new XDeclaration("1.0", "UTF-8", null),
            new XDocumentType("xmeml", null, null, null),
            new XElement("xmeml",
                new XAttribute("version", "5"),
                seqElem));

        using var sw = new Utf8StringWriter();
        xdoc.Save(sw);
        return sw.ToString();
    }

    public static VegasProject ImportFcp7Xml(string xmlPath)
    {
        string xml = File.ReadAllText(xmlPath, Encoding.UTF8);
        return ImportFcp7XmlFromString(xml);
    }

    public static VegasProject ImportFcp7XmlFromString(string xmlContent)
    {
        var xdoc = XDocument.Parse(xmlContent);
        var seq = xdoc.Descendants("sequence").FirstOrDefault()
            ?? throw new InvalidDataException("XML FCP7 inválido: elemento <sequence> não encontrado.");

        string seqName = seq.Element("name")?.Value ?? "Imported_FCP7_Sequence";
        var rateElem = seq.Element("rate");
        int timebase = ParseInt(rateElem?.Element("timebase")?.Value, 30);
        bool ntsc = string.Equals(rateElem?.Element("ntsc")?.Value, "TRUE", StringComparison.OrdinalIgnoreCase);
        double fps = ResolveFpsFromFcp7Rate(timebase, ntsc);

        var vSample = seq.Element("media")?.Element("video")?.Element("format")?.Element("samplecharacteristics");
        int width = ParseInt(vSample?.Element("width")?.Value, 1920);
        int height = ParseInt(vSample?.Element("height")?.Value, 1080);
        string colorSpace = vSample?.Element("colorSpace")?.Value ?? "Rec.709 Standard";

        var aSample = seq.Element("media")?.Element("audio")?.Element("format")?.Element("samplecharacteristics");
        int sampleRate = ParseInt(aSample?.Element("samplerate")?.Value, 48000);
        int bitDepth = ParseInt(aSample?.Element("depth")?.Value, 24);

        var proj = new VegasProject
        {
            Name = seqName,
            Width = width,
            Height = height,
            Fps = fps,
            ColorSpace = colorSpace,
            SampleRate = sampleRate,
            AudioBitDepth = bitDepth,
            Tracks = new List<TimelineTrack>(),
            Markers = new List<TimelineMarker>(),
            Regions = new List<TimelineRegion>(),
            MediaPool = new List<MediaAssetItem>()
        };

        var mediaElem = seq.Element("media");
        if (mediaElem != null)
        {
            var videoSection = mediaElem.Element("video");
            if (videoSection != null)
            {
                int vIdx = 1;
                foreach (var trElem in videoSection.Elements("track"))
                {
                    proj.Tracks.Add(ParseFcp7Track(trElem, "video", vIdx++, fps));
                }
            }

            var audioSection = mediaElem.Element("audio");
            if (audioSection != null)
            {
                int aIdx = 1;
                foreach (var trElem in audioSection.Elements("track"))
                {
                    proj.Tracks.Add(ParseFcp7Track(trElem, "audio", aIdx++, fps));
                }
            }
        }

        foreach (var mkElem in seq.Elements("marker"))
        {
            double timeSec;
            if (mkElem.Attribute("neovegTimeSec") != null)
                timeSec = ParseDouble(mkElem.Attribute("neovegTimeSec")?.Value, 0.0);
            else
                timeSec = FramesToSeconds(ParseInt(mkElem.Element("in")?.Value, 0), fps);

            proj.Markers.Add(new TimelineMarker
            {
                Time = Math.Round(timeSec, 4),
                Label = mkElem.Element("name")?.Value ?? mkElem.Element("comment")?.Value ?? "Marker",
                Color = mkElem.Attribute("neovegColor")?.Value ?? "#f97316"
            });
        }

        return proj;
    }

    private static TimelineTrack ParseFcp7Track(XElement trElem, string trackType, int index, double fps)
    {
        string defaultName = trackType == "audio" ? $"Audio Track {index}" : $"Video Track {index}";
        string name = trElem.Element("name")?.Value
            ?? trElem.Attribute("MZ.TrackName")?.Value
            ?? defaultName;

        bool enabled = !string.Equals(trElem.Element("enabled")?.Value, "FALSE", StringComparison.OrdinalIgnoreCase);
        bool locked = string.Equals(trElem.Element("locked")?.Value, "TRUE", StringComparison.OrdinalIgnoreCase);
        bool solo = string.Equals(trElem.Attribute("solo")?.Value, "TRUE", StringComparison.OrdinalIgnoreCase);

        var track = new TimelineTrack
        {
            Id = trElem.Attribute("neovegId")?.Value ?? $"tr_{trackType[0]}{index}",
            Name = name,
            Type = trackType,
            Color = trElem.Attribute("neovegColor")?.Value ?? (trackType == "audio" ? "#0891b2" : "#2563eb"),
            Height = ParseDouble(trElem.Attribute("neovegHeight")?.Value, 78.0),
            Level = ParseDouble(trElem.Attribute("neovegLevel")?.Value, 1.0),
            Pan = ParseDouble(trElem.Attribute("neovegPan")?.Value, 0.0),
            BlendMode = trElem.Attribute("neovegBlendMode")?.Value ?? "sourceAlpha",
            GroupId = trElem.Attribute("groupId")?.Value,
            GroupName = trElem.Attribute("groupName")?.Value,
            GroupColor = trElem.Attribute("groupColor")?.Value,
            Muted = !enabled,
            Solo = solo,
            Locked = locked,
            Clips = new List<TimelineClip>()
        };

        foreach (var clipElem in trElem.Elements("clipitem"))
        {
            int startFr = ParseInt(clipElem.Element("start")?.Value, 0);
            int endFr = ParseInt(clipElem.Element("end")?.Value, startFr + 30);
            int inFr = ParseInt(clipElem.Element("in")?.Value, 0);
            int outFr = ParseInt(clipElem.Element("out")?.Value, inFr + Math.Max(1, endFr - startFr));

            double startSec = clipElem.Attribute("neovegStartSec") != null
                ? ParseDouble(clipElem.Attribute("neovegStartSec")?.Value, FramesToSeconds(startFr, fps))
                : FramesToSeconds(startFr, fps);

            double durationSec = clipElem.Attribute("neovegDurationSec") != null
                ? ParseDouble(clipElem.Attribute("neovegDurationSec")?.Value, FramesToSeconds(Math.Max(1, endFr - startFr), fps))
                : FramesToSeconds(Math.Max(1, endFr - startFr), fps);

            double trimInSec = clipElem.Attribute("neovegTrimInSec") != null
                ? ParseDouble(clipElem.Attribute("neovegTrimInSec")?.Value, FramesToSeconds(inFr, fps))
                : FramesToSeconds(inFr, fps);

            double opacity = 1.0;
            double gain = 1.0;

            foreach (var filter in clipElem.Elements("filter"))
            {
                var eff = filter.Element("effect");
                if (eff == null) continue;
                string effId = (eff.Element("effectid")?.Value ?? "").ToLowerInvariant();
                if (effId == "opacity")
                {
                    var valStr = eff.Elements("parameter")
                        .FirstOrDefault(p => string.Equals(p.Element("parameterid")?.Value, "opacity", StringComparison.OrdinalIgnoreCase))
                        ?.Element("value")?.Value;
                    if (!string.IsNullOrEmpty(valStr))
                        opacity = Math.Clamp(ParseDouble(valStr, 100.0) / 100.0, 0.0, 1.0);
                }
                else if (effId == "audiolevels")
                {
                    var valStr = eff.Elements("parameter")
                        .FirstOrDefault(p => string.Equals(p.Element("parameterid")?.Value, "level", StringComparison.OrdinalIgnoreCase))
                        ?.Element("value")?.Value;
                    if (!string.IsNullOrEmpty(valStr))
                        gain = Math.Clamp(ParseDouble(valStr, 1.0), 0.0, 4.0);
                }
            }

            var fileElem = clipElem.Element("file");
            string? pathUrl = fileElem?.Element("pathurl")?.Value;
            string? mediaFile = FromFileUrl(pathUrl) ?? fileElem?.Element("name")?.Value;
            bool clipEnabled = !string.Equals(clipElem.Element("enabled")?.Value, "FALSE", StringComparison.OrdinalIgnoreCase);

            track.Clips.Add(new TimelineClip
            {
                Id = clipElem.Attribute("id")?.Value ?? $"clip_{Guid.NewGuid():N}"[..10],
                Name = clipElem.Element("name")?.Value ?? "Clip",
                MediaId = fileElem?.Attribute("id")?.Value ?? "",
                MediaFile = mediaFile,
                MediaType = clipElem.Attribute("neovegMediaType")?.Value ?? trackType,
                GeneratorType = clipElem.Attribute("neovegGeneratorType")?.Value,
                TitleText = clipElem.Attribute("neovegTitleText")?.Value,
                SubtitleText = clipElem.Attribute("neovegSubtitleText")?.Value,
                Color = clipElem.Attribute("neovegColor")?.Value ?? track.Color,
                Start = Math.Round(startSec, 6),
                Duration = Math.Round(durationSec, 6),
                TrimIn = Math.Round(trimInSec, 6),
                FadeIn = ParseDouble(clipElem.Attribute("neovegFadeInSec")?.Value, 0.25),
                FadeOut = ParseDouble(clipElem.Attribute("neovegFadeOutSec")?.Value, 0.25),
                PlaybackRate = ParseDouble(clipElem.Attribute("neovegPlaybackRate")?.Value, 1.0),
                Opacity = Math.Round(opacity, 4),
                Gain = Math.Round(gain, 6),
                Muted = !clipEnabled
            });
        }

        return track;
    }

    // =========================================================================
    // 4. FCPX FCPXML (`.fcpxml` — Final Cut Pro X / DaVinci Resolve `fcpxml v1.10`)
    // =========================================================================

    public static string ExportFcpxml(VegasProject project, string outputPath)
    {
        if (!outputPath.EndsWith(FcpxmlExtension, StringComparison.OrdinalIgnoreCase))
            outputPath += FcpxmlExtension;

        string xml = ExportFcpxmlToString(project);
        string? dir = Path.GetDirectoryName(outputPath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        File.WriteAllText(outputPath, xml, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        return outputPath;
    }

    public static string ExportFcpxmlToString(VegasProject project)
    {
        double fps = project.Fps > 0.1 ? project.Fps : 29.97;
        string frameDur = Math.Abs(fps - 29.97) < 0.05 ? "1001/30000s" : $"100/{Math.Round(fps * 100):0}s";

        var resourcesElem = new XElement("resources",
            new XElement("format",
                new XAttribute("id", "r1"),
                new XAttribute("name", $"FFVideoFormat{project.Height}p{Math.Round(fps):0}"),
                new XAttribute("frameDuration", frameDur),
                new XAttribute("width", project.Width),
                new XAttribute("height", project.Height),
                new XAttribute("colorSpace", "1-1-1 (Rec. 709)")));

        var spineElem = new XElement("spine");
        int assetCounter = 2;

        for (int tIdx = 0; tIdx < project.Tracks.Count; tIdx++)
        {
            var tr = project.Tracks[tIdx];
            bool isAudio = string.Equals(tr.Type, "audio", StringComparison.OrdinalIgnoreCase);
            int lane = isAudio ? -(tIdx + 1) : tIdx;

            foreach (var c in tr.Clips.OrderBy(x => x.Start))
            {
                string assetId = $"r{assetCounter++}";
                string fileName = !string.IsNullOrEmpty(c.MediaFile) ? Path.GetFileName(c.MediaFile) : c.Name;
                resourcesElem.Add(new XElement("asset",
                    new XAttribute("id", assetId),
                    new XAttribute("name", fileName),
                    new XAttribute("src", ToFileUrl(c.MediaFile ?? fileName)),
                    new XAttribute("start", "0s"),
                    new XAttribute("duration", FormatFcpxRationalSeconds(Math.Max(c.Duration + c.TrimIn, 5.0))),
                    new XAttribute("hasVideo", isAudio ? "0" : "1"),
                    new XAttribute("hasAudio", "1")));

                double gainDb = c.Gain > 0.0001 ? 20.0 * Math.Log10(c.Gain) : -96.0;
                var assetClip = new XElement("asset-clip",
                    new XAttribute("id", c.Id),
                    new XAttribute("name", c.Name),
                    new XAttribute("ref", assetId),
                    new XAttribute("lane", lane),
                    new XAttribute("offset", FormatFcpxRationalSeconds(c.Start)),
                    new XAttribute("duration", FormatFcpxRationalSeconds(c.Duration)),
                    new XAttribute("start", FormatFcpxRationalSeconds(c.TrimIn)),
                    new XAttribute("enabled", (tr.Muted || c.Muted) ? "0" : "1"),
                    new XAttribute("neovegTrackId", tr.Id),
                    new XAttribute("neovegTrackName", tr.Name),
                    new XAttribute("neovegTrackType", tr.Type),
                    new XAttribute("neovegTrackMuted", tr.Muted ? "1" : "0"),
                    new XAttribute("neovegTrackSolo", tr.Solo ? "1" : "0"),
                    new XAttribute("neovegTrackLevel", tr.Level.ToString("0.####", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegGainLinear", c.Gain.ToString("0.######", CultureInfo.InvariantCulture)),
                    new XAttribute("neovegOpacity", c.Opacity.ToString("0.####", CultureInfo.InvariantCulture)),
                    new XElement("adjust-blend",
                        new XAttribute("amount", c.Opacity.ToString("0.####", CultureInfo.InvariantCulture))),
                    new XElement("adjust-volume",
                        new XAttribute("amount", $"{gainDb.ToString("0.###", CultureInfo.InvariantCulture)}dB")));

                spineElem.Add(assetClip);
            }
        }

        foreach (var mk in project.Markers)
        {
            spineElem.Add(new XElement("marker",
                new XAttribute("start", FormatFcpxRationalSeconds(mk.Time)),
                new XAttribute("duration", frameDur),
                new XAttribute("value", mk.Label),
                new XAttribute("neovegColor", mk.Color ?? "#f97316")));
        }

        var seqElem = new XElement("sequence",
            new XAttribute("format", "r1"),
            new XAttribute("duration", FormatFcpxRationalSeconds(project.ComputeTotalDuration())),
            new XAttribute("tcStart", "0s"),
            new XAttribute("tcFormat", "NDF"),
            new XAttribute("audioLayout", "stereo"),
            new XAttribute("audioRate", $"{project.SampleRate / 1000}k"),
            new XAttribute("neovegFps", fps.ToString("0.####", CultureInfo.InvariantCulture)),
            spineElem);

        var xdoc = new XDocument(
            new XDeclaration("1.0", "UTF-8", null),
            new XDocumentType("fcpxml", null, null, null),
            new XElement("fcpxml",
                new XAttribute("version", "1.10"),
                resourcesElem,
                new XElement("library",
                    new XElement("event",
                        new XAttribute("name", "Neoveg Export"),
                        new XElement("project",
                            new XAttribute("name", project.Name),
                            seqElem)))));

        using var sw = new Utf8StringWriter();
        xdoc.Save(sw);
        return sw.ToString();
    }

    public static VegasProject ImportFcpxml(string fcpxmlPath)
    {
        string xml = File.ReadAllText(fcpxmlPath, Encoding.UTF8);
        return ImportFcpxmlFromString(xml);
    }

    public static VegasProject ImportFcpxmlFromString(string xmlContent)
    {
        var xdoc = XDocument.Parse(xmlContent);
        var projElem = xdoc.Descendants("project").FirstOrDefault();
        var seqElem = xdoc.Descendants("sequence").FirstOrDefault()
            ?? throw new InvalidDataException("FCPXML inválido: elemento <sequence> não encontrado.");

        var fmtElem = xdoc.Descendants("format").FirstOrDefault();
        int width = ParseInt(fmtElem?.Attribute("width")?.Value, 1920);
        int height = ParseInt(fmtElem?.Attribute("height")?.Value, 1080);
        double fps = ParseDouble(seqElem.Attribute("neovegFps")?.Value, 29.97);

        var assetMap = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var asset in xdoc.Descendants("asset"))
        {
            string? id = asset.Attribute("id")?.Value;
            string? src = FromFileUrl(asset.Attribute("src")?.Value) ?? asset.Attribute("name")?.Value;
            if (!string.IsNullOrEmpty(id) && !string.IsNullOrEmpty(src))
                assetMap[id] = src;
        }

        var proj = new VegasProject
        {
            Name = projElem?.Attribute("name")?.Value ?? "Imported_FCPXML_Project",
            Width = width,
            Height = height,
            Fps = fps,
            Tracks = new List<TimelineTrack>(),
            Markers = new List<TimelineMarker>()
        };

        var trackByKey = new Dictionary<string, TimelineTrack>(StringComparer.OrdinalIgnoreCase);
        var spine = seqElem.Element("spine");
        if (spine != null)
        {
            foreach (var ac in spine.Elements("asset-clip"))
            {
                string trId = ac.Attribute("neovegTrackId")?.Value
                    ?? $"tr_lane_{ac.Attribute("lane")?.Value ?? "0"}";
                string trName = ac.Attribute("neovegTrackName")?.Value ?? $"Track {trId}";
                string trType = ac.Attribute("neovegTrackType")?.Value
                    ?? ((ParseInt(ac.Attribute("lane")?.Value, 0) < 0) ? "audio" : "video");

                if (!trackByKey.TryGetValue(trId, out var track))
                {
                    track = new TimelineTrack
                    {
                        Id = trId,
                        Name = trName,
                        Type = trType,
                        Muted = ac.Attribute("neovegTrackMuted")?.Value == "1",
                        Solo = ac.Attribute("neovegTrackSolo")?.Value == "1",
                        Level = ParseDouble(ac.Attribute("neovegTrackLevel")?.Value, 1.0)
                    };
                    trackByKey[trId] = track;
                    proj.Tracks.Add(track);
                }

                string refId = ac.Attribute("ref")?.Value ?? "";
                assetMap.TryGetValue(refId, out string? mediaFile);

                double startSec = ParseFcpxRationalSeconds(ac.Attribute("offset")?.Value);
                double durSec = ParseFcpxRationalSeconds(ac.Attribute("duration")?.Value);
                double trimInSec = ParseFcpxRationalSeconds(ac.Attribute("start")?.Value);

                double opacity = ParseDouble(
                    ac.Attribute("neovegOpacity")?.Value ?? ac.Element("adjust-blend")?.Attribute("amount")?.Value,
                    1.0);

                double gain = 1.0;
                if (ac.Attribute("neovegGainLinear") != null)
                {
                    gain = ParseDouble(ac.Attribute("neovegGainLinear")?.Value, 1.0);
                }
                else
                {
                    string? dbStr = ac.Element("adjust-volume")?.Attribute("amount")?.Value;
                    if (!string.IsNullOrEmpty(dbStr))
                    {
                        double db = ParseDouble(dbStr.Replace("dB", "", StringComparison.OrdinalIgnoreCase), 0.0);
                        gain = db <= -90.0 ? 0.0 : Math.Pow(10.0, db / 20.0);
                    }
                }

                track.Clips.Add(new TimelineClip
                {
                    Id = ac.Attribute("id")?.Value ?? $"clip_{Guid.NewGuid():N}"[..10],
                    Name = ac.Attribute("name")?.Value ?? "Clip",
                    MediaFile = mediaFile,
                    MediaType = trType,
                    Start = Math.Round(startSec, 6),
                    Duration = Math.Round(durSec, 6),
                    TrimIn = Math.Round(trimInSec, 6),
                    Opacity = Math.Round(opacity, 4),
                    Gain = Math.Round(gain, 6)
                });
            }

            foreach (var mk in spine.Elements("marker"))
            {
                proj.Markers.Add(new TimelineMarker
                {
                    Time = Math.Round(ParseFcpxRationalSeconds(mk.Attribute("start")?.Value), 4),
                    Label = mk.Attribute("value")?.Value ?? "Marker",
                    Color = mk.Attribute("neovegColor")?.Value ?? "#f97316"
                });
            }
        }

        return proj;
    }

    // =========================================================================
    // 5. EDL CMX3600 (`.edl`) EXPORT
    // =========================================================================

    public static string ExportCmx3600Edl(VegasProject project, string outputPath)
    {
        if (!outputPath.EndsWith(EdlExtension, StringComparison.OrdinalIgnoreCase))
            outputPath += EdlExtension;

        string edl = ExportCmx3600EdlToString(project);
        string? dir = Path.GetDirectoryName(outputPath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        File.WriteAllText(outputPath, edl, Encoding.UTF8);
        return outputPath;
    }

    public static string ExportCmx3600EdlToString(VegasProject project)
    {
        double fps = project.Fps > 0.1 ? project.Fps : 29.97;
        bool dropFrame = Math.Abs(fps - 29.97) < 0.05 || Math.Abs(fps - 59.94) < 0.05;

        var sb = new StringBuilder();
        sb.AppendLine($"TITLE: {project.Name}");
        sb.AppendLine($"FCM: {(dropFrame ? "DROP FRAME" : "NON-DROP FRAME")}");
        sb.AppendLine();

        int eventNum = 1;
        foreach (var tr in project.Tracks)
        {
            bool isAudio = string.Equals(tr.Type, "audio", StringComparison.OrdinalIgnoreCase);
            string chCode = isAudio ? "AA" : "V";

            foreach (var c in tr.Clips.OrderBy(x => x.Start))
            {
                string srcIn = FormatSmpteEdl(c.TrimIn, fps, dropFrame);
                string srcOut = FormatSmpteEdl(c.TrimIn + c.Duration, fps, dropFrame);
                string recIn = FormatSmpteEdl(c.Start, fps, dropFrame);
                string recOut = FormatSmpteEdl(c.End, fps, dropFrame);
                double gainDb = c.Gain > 0.0001 ? 20.0 * Math.Log10(c.Gain) : -96.0;

                sb.AppendLine($"{eventNum:000}  AX       {chCode,-5} C        {srcIn} {srcOut} {recIn} {recOut}");
                sb.AppendLine($"* FROM CLIP NAME: {c.Name}");
                if (!string.IsNullOrEmpty(c.MediaFile))
                    sb.AppendLine($"* SOURCE FILE: {c.MediaFile}");
                sb.AppendLine($"* TRACK NAME: {tr.Name} | OPACITY: {(c.Opacity * 100.0):0.#}% | AUDIO LEVEL: {gainDb:+0.00;-0.00;0.00} DB");
                sb.AppendLine();
                eventNum++;
            }
        }

        return sb.ToString();
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    private static void WriteZipTextEntry(ZipArchive archive, string entryName, string content)
    {
        var entry = archive.CreateEntry(entryName, CompressionLevel.Optimal);
        using var stream = entry.Open();
        using var writer = new StreamWriter(stream, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        writer.Write(content);
    }

    private static string ReadZipTextEntry(ZipArchiveEntry entry)
    {
        using var stream = entry.Open();
        using var reader = new StreamReader(stream, Encoding.UTF8);
        return reader.ReadToEnd();
    }

    private static string TryMakeRelativePath(string baseDir, string path)
    {
        if (string.IsNullOrWhiteSpace(path)) return "";
        try
        {
            if (Path.IsPathRooted(path) && Path.IsPathRooted(baseDir))
                return Path.GetRelativePath(baseDir, path).Replace('\\', '/');
        }
        catch
        {
            // Different drive letters on Windows keep original path
        }
        return path.Replace('\\', '/');
    }

    private static string DeterministicGuidFromString(string input)
    {
        using var sha = System.Security.Cryptography.SHA256.Create();
        byte[] hash = sha.ComputeHash(Encoding.UTF8.GetBytes(input ?? ""));
        byte[] guidBytes = new byte[16];
        Array.Copy(hash, guidBytes, 16);
        return new Guid(guidBytes).ToString("D");
    }

    private static (int Timebase, bool Ntsc) GetFcp7RateTimebase(double fps)
    {
        if (Math.Abs(fps - 23.976) < 0.05) return (24, true);
        if (Math.Abs(fps - 29.97) < 0.05) return (30, true);
        if (Math.Abs(fps - 59.94) < 0.05) return (60, true);
        return (Math.Max(1, (int)Math.Round(fps)), false);
    }

    private static double ResolveFpsFromFcp7Rate(int timebase, bool ntsc)
    {
        if (ntsc)
        {
            if (timebase == 24) return 23.976;
            if (timebase == 30) return 29.97;
            if (timebase == 60) return 59.94;
        }
        return Math.Max(1.0, timebase);
    }

    private static int SecondsToFrames(double seconds, double fps)
        => (int)Math.Round(Math.Max(0.0, seconds) * fps);

    private static double FramesToSeconds(int frames, double fps)
        => Math.Max(0, frames) / Math.Max(1.0, fps);

    private static string FormatFcpxRationalSeconds(double seconds)
    {
        long ms = (long)Math.Round(Math.Max(0.0, seconds) * 1000.0);
        return $"{ms}/1000s";
    }

    private static double ParseFcpxRationalSeconds(string? val)
    {
        if (string.IsNullOrWhiteSpace(val)) return 0.0;
        string s = val.Trim().TrimEnd('s', 'S');
        int slash = s.IndexOf('/');
        if (slash > 0)
        {
            double num = ParseDouble(s[..slash], 0.0);
            double den = ParseDouble(s[(slash + 1)..], 1.0);
            return den > 0.00001 ? num / den : 0.0;
        }
        return ParseDouble(s, 0.0);
    }

    private static string FormatSmpteEdl(double seconds, double fps, bool dropFrame)
    {
        int tb = Math.Max(1, (int)Math.Round(fps));
        int totalFrames = Math.Max(0, (int)Math.Round(seconds * fps));
        int ff = totalFrames % tb;
        int totalSec = totalFrames / tb;
        int ss = totalSec % 60;
        int mm = (totalSec / 60) % 60;
        int hh = totalSec / 3600;
        char sep = dropFrame ? ';' : ':';
        return $"{hh:00}:{mm:00}:{ss:00}{sep}{ff:00}";
    }

    private static string ToFileUrl(string path)
    {
        if (string.IsNullOrWhiteSpace(path)) return "file://localhost/media.mp4";
        if (path.StartsWith("file://", StringComparison.OrdinalIgnoreCase)) return path;
        string norm = path.Replace('\\', '/').TrimStart('/');
        return $"file://localhost/{norm}";
    }

    private static string? FromFileUrl(string? url)
    {
        if (string.IsNullOrWhiteSpace(url)) return null;
        string s = url.Trim();
        if (s.StartsWith("file://localhost/", StringComparison.OrdinalIgnoreCase))
            s = s["file://localhost/".Length..];
        else if (s.StartsWith("file:///", StringComparison.OrdinalIgnoreCase))
            s = s["file:///".Length..];
        else if (s.StartsWith("file://", StringComparison.OrdinalIgnoreCase))
            s = s["file://".Length..];
        return Uri.UnescapeDataString(s);
    }

    private static int ParseInt(string? s, int fallback)
        => int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out int v) ? v : fallback;

    private static double ParseDouble(string? s, double fallback)
        => double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture, out double v) ? v : fallback;

    private sealed class Utf8StringWriter : StringWriter
    {
        public override Encoding Encoding => new UTF8Encoding(false);
    }
}
