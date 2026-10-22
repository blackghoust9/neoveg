using System;
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace Neoveg.Models;

public enum FadeCurveType
{
    Linear = 0,
    Fast = 1,
    Slow = 2,
    Smooth = 3,
    Sharp = 4
}

public enum EditToolMode
{
    Normal,
    Razor,
    Slip,
    RateStretch
}

public sealed class PanCropKeyframe
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"kf_{Guid.NewGuid():N}"[..10];

    [JsonPropertyName("time")]
    public double Time { get; set; } = 0.0;

    [JsonPropertyName("trackType")]
    public string TrackType { get; set; } = "position"; // "position" or "mask"

    [JsonPropertyName("width")]
    public double Width { get; set; } = 1920.0;

    [JsonPropertyName("height")]
    public double Height { get; set; } = 1080.0;

    [JsonPropertyName("xCenter")]
    public double XCenter { get; set; } = 960.0;

    [JsonPropertyName("yCenter")]
    public double YCenter { get; set; } = 540.0;

    [JsonPropertyName("rotation")]
    public double Rotation { get; set; } = 0.0;

    [JsonPropertyName("interpolation")]
    public string InterpolationType { get; set; } = "Linear"; // "Linear", "Smooth", "Fast", "Slow", "Sharp", "Hold"

    [JsonPropertyName("smoothness")]
    public double Smoothness { get; set; } = 0.0;

    [JsonPropertyName("maskFeather")]
    public double MaskFeather { get; set; } = 0.0;

    public PanCropKeyframe Clone() => new()
    {
        Id = $"kf_{Guid.NewGuid():N}"[..10],
        Time = Time,
        TrackType = TrackType,
        Width = Width,
        Height = Height,
        XCenter = XCenter,
        YCenter = YCenter,
        Rotation = Rotation,
        InterpolationType = InterpolationType,
        Smoothness = Smoothness,
        MaskFeather = MaskFeather
    };
}

public sealed class PanCropState
{
    [JsonPropertyName("sourceWidth")]
    public double SourceWidth { get; set; } = 1920.0;

    [JsonPropertyName("sourceHeight")]
    public double SourceHeight { get; set; } = 1080.0;

    [JsonPropertyName("x")]
    public double X { get; set; } = 0.0;

    [JsonPropertyName("y")]
    public double Y { get; set; } = 0.0;

    [JsonPropertyName("scale")]
    public double Scale { get; set; } = 1.0;

    [JsonPropertyName("scaleY")]
    public double ScaleY { get; set; } = 1.0;

    // Position Group (Width, Height, X Center, Y Center in source frame pixels)
    [JsonPropertyName("width")]
    public double Width { get; set; } = 1920.0;

    [JsonPropertyName("height")]
    public double Height { get; set; } = 1080.0;

    [JsonPropertyName("xCenter")]
    public double XCenter { get; set; } = 960.0;

    [JsonPropertyName("yCenter")]
    public double YCenter { get; set; } = 540.0;

    // Rotation Group (Angle, X Center, Y Center)
    [JsonPropertyName("rotation")]
    public double Rotation { get; set; } = 0.0;

    [JsonPropertyName("rotationXCenter")]
    public double RotationXCenter { get; set; } = 960.0;

    [JsonPropertyName("rotationYCenter")]
    public double RotationYCenter { get; set; } = 540.0;

    // Keyframe Interpolation Group
    [JsonPropertyName("smoothness")]
    public double Smoothness { get; set; } = 0.0;

    [JsonPropertyName("interpolationType")]
    public string InterpolationType { get; set; } = "Linear";

    // Source Group
    [JsonPropertyName("maintainAspectRatio")]
    public bool MaintainAspectRatio { get; set; } = true;

    [JsonPropertyName("stretchToFillFrame")]
    public bool StretchToFillFrame { get; set; } = true;

    // Workspace & Left Vertical Toolbar Toggles
    [JsonPropertyName("flipH")]
    public bool FlipH { get; set; } = false;

    [JsonPropertyName("flipV")]
    public bool FlipV { get; set; } = false;

    [JsonPropertyName("lockAspect")]
    public bool LockAspect { get; set; } = true;

    [JsonPropertyName("sizeAboutCenter")]
    public bool SizeAboutCenter { get; set; } = true;

    [JsonPropertyName("enableSnapping")]
    public bool EnableSnapping { get; set; } = true;

    [JsonPropertyName("moveAxisMode")]
    public string MoveAxisMode { get; set; } = "XY"; // "XY", "X", "Y"

    [JsonPropertyName("workspaceZoom")]
    public double WorkspaceZoom { get; set; } = 100.0;

    [JsonPropertyName("workspaceXOffset")]
    public double WorkspaceXOffset { get; set; } = 0.0;

    [JsonPropertyName("workspaceYOffset")]
    public double WorkspaceYOffset { get; set; } = 0.0;

    [JsonPropertyName("workspaceGridSpacing")]
    public int WorkspaceGridSpacing { get; set; } = 16;

    // Animation Mini-Timeline (Position & Mask Keyframes + Cursor Sync)
    [JsonPropertyName("syncCursor")]
    public bool SyncCursor { get; set; } = true;

    [JsonPropertyName("maskEnabled")]
    public bool MaskEnabled { get; set; } = false;

    [JsonPropertyName("keyframes")]
    public List<PanCropKeyframe> Keyframes { get; set; } = new()
    {
        new PanCropKeyframe
        {
            Id = "kf_pos_0",
            Time = 0.0,
            TrackType = "position",
            Width = 1920.0,
            Height = 1080.0,
            XCenter = 960.0,
            YCenter = 540.0,
            Rotation = 0.0,
            InterpolationType = "Linear"
        },
        new PanCropKeyframe
        {
            Id = "kf_mask_0",
            Time = 0.0,
            TrackType = "mask",
            Width = 1920.0,
            Height = 1080.0,
            XCenter = 960.0,
            YCenter = 540.0,
            Rotation = 0.0,
            InterpolationType = "Linear",
            MaskFeather = 0.0
        }
    };

    /// <summary>
    /// Updates normalized `X`, `Y`, `Scale`, `ScaleY` from pixel-based `Width`, `Height`, `XCenter`, `YCenter`.
    /// </summary>
    public void SyncNormalizedFromPixels()
    {
        double sw = Math.Max(16.0, SourceWidth);
        double sh = Math.Max(16.0, SourceHeight);
        Width = Math.Clamp(Math.Round(Width, 1), sw * 0.12, sw * 3.0);
        Height = Math.Clamp(Math.Round(Height, 1), sh * 0.12, sh * 3.0);
        XCenter = Math.Clamp(Math.Round(XCenter, 1), -sw * 0.5, sw * 1.5);
        YCenter = Math.Clamp(Math.Round(YCenter, 1), -sh * 0.5, sh * 1.5);

        Scale = Math.Round(Width / sw, 4);
        ScaleY = Math.Round(Height / sh, 4);
        X = Math.Round((XCenter - sw * 0.5) / (sw * 0.5), 4);
        Y = Math.Round((YCenter - sh * 0.5) / (sh * 0.5), 4);
    }

    /// <summary>
    /// Updates pixel-based `Width`, `Height`, `XCenter`, `YCenter` from normalized `X`, `Y`, `Scale`, `ScaleY`.
    /// </summary>
    public void SyncPixelsFromNormalized()
    {
        double sw = Math.Max(16.0, SourceWidth);
        double sh = Math.Max(16.0, SourceHeight);
        Scale = Math.Clamp(Scale, 0.12, 3.0);
        if (LockAspect || ScaleY <= 0.01)
            ScaleY = Scale;
        else
            ScaleY = Math.Clamp(ScaleY, 0.12, 3.0);

        Width = Math.Round(sw * Scale, 1);
        Height = Math.Round(sh * ScaleY, 1);
        XCenter = Math.Round(sw * 0.5 + X * (sw * 0.5), 1);
        YCenter = Math.Round(sh * 0.5 + Y * (sh * 0.5), 1);
    }

    /// <summary>
    /// Evaluates keyframe interpolation at `localTimeSec` (0..clip.Duration) on the Position track
    /// and updates the active Pan/Crop state when multiple position keyframes exist.
    /// </summary>
    public void EvaluateAtLocalTime(double localTimeSec)
    {
        if (Keyframes == null || Keyframes.Count == 0) return;

        var posKfs = new List<PanCropKeyframe>();
        foreach (var k in Keyframes)
        {
            if (string.Equals(k.TrackType, "position", StringComparison.OrdinalIgnoreCase))
                posKfs.Add(k);
        }
        if (posKfs.Count <= 1) return;

        posKfs.Sort((a, b) => a.Time.CompareTo(b.Time));
        if (localTimeSec <= posKfs[0].Time)
        {
            ApplyKeyframeToCurrent(posKfs[0]);
            return;
        }
        if (localTimeSec >= posKfs[^1].Time)
        {
            ApplyKeyframeToCurrent(posKfs[^1]);
            return;
        }

        for (int i = 0; i < posKfs.Count - 1; i++)
        {
            var k0 = posKfs[i];
            var k1 = posKfs[i + 1];
            if (localTimeSec >= k0.Time && localTimeSec <= k1.Time)
            {
                double span = Math.Max(0.0001, k1.Time - k0.Time);
                double t = Math.Clamp((localTimeSec - k0.Time) / span, 0.0, 1.0);
                double u = EvaluateCurveFactor(k0.InterpolationType, t);

                Width = Math.Round(k0.Width + (k1.Width - k0.Width) * u, 1);
                Height = Math.Round(k0.Height + (k1.Height - k0.Height) * u, 1);
                XCenter = Math.Round(k0.XCenter + (k1.XCenter - k0.XCenter) * u, 1);
                YCenter = Math.Round(k0.YCenter + (k1.YCenter - k0.YCenter) * u, 1);
                Rotation = Math.Round(k0.Rotation + (k1.Rotation - k0.Rotation) * u, 1);
                SyncNormalizedFromPixels();
                return;
            }
        }
    }

    public void ApplyKeyframeToCurrent(PanCropKeyframe kf)
    {
        Width = kf.Width;
        Height = kf.Height;
        XCenter = kf.XCenter;
        YCenter = kf.YCenter;
        Rotation = kf.Rotation;
        InterpolationType = kf.InterpolationType;
        Smoothness = kf.Smoothness;
        SyncNormalizedFromPixels();
    }

    private static double EvaluateCurveFactor(string type, double t)
    {
        return (type ?? "Linear").ToLowerInvariant() switch
        {
            "hold" => 0.0,
            "smooth" => t * t * (3.0 - 2.0 * t),
            "fast" => 1.0 - (1.0 - t) * (1.0 - t),
            "slow" => t * t,
            "sharp" => t < 0.5 ? 4.0 * t * t * t : 1.0 - Math.Pow(-2.0 * t + 2.0, 3) * 0.5,
            _ => t
        };
    }

    public PanCropState Clone()
    {
        var copy = new PanCropState
        {
            SourceWidth = SourceWidth,
            SourceHeight = SourceHeight,
            X = X,
            Y = Y,
            Scale = Scale,
            ScaleY = ScaleY,
            Width = Width,
            Height = Height,
            XCenter = XCenter,
            YCenter = YCenter,
            Rotation = Rotation,
            RotationXCenter = RotationXCenter,
            RotationYCenter = RotationYCenter,
            Smoothness = Smoothness,
            InterpolationType = InterpolationType,
            MaintainAspectRatio = MaintainAspectRatio,
            StretchToFillFrame = StretchToFillFrame,
            FlipH = FlipH,
            FlipV = FlipV,
            LockAspect = LockAspect,
            SizeAboutCenter = SizeAboutCenter,
            EnableSnapping = EnableSnapping,
            MoveAxisMode = MoveAxisMode,
            WorkspaceZoom = WorkspaceZoom,
            WorkspaceXOffset = WorkspaceXOffset,
            WorkspaceYOffset = WorkspaceYOffset,
            WorkspaceGridSpacing = WorkspaceGridSpacing,
            SyncCursor = SyncCursor,
            MaskEnabled = MaskEnabled,
            Keyframes = new List<PanCropKeyframe>()
        };
        foreach (var kf in Keyframes)
            copy.Keyframes.Add(kf.Clone());
        return copy;
    }
}

public sealed class VideoFxInstance
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = Guid.NewGuid().ToString("N")[..8];

    [JsonPropertyName("type")]
    public string Type { get; set; } = "brightnessContrast";

    [JsonPropertyName("name")]
    public string Name { get; set; } = "VEGAS Brightness & Contrast";

    [JsonPropertyName("enabled")]
    public bool Enabled { get; set; } = true;

    [JsonPropertyName("params")]
    public Dictionary<string, double> Params { get; set; } = new();
}

public sealed class TimelineClip
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"clip_{Guid.NewGuid():N}"[..13];

    [JsonPropertyName("name")]
    public string Name { get; set; } = "New Event";

    [JsonPropertyName("mediaId")]
    public string MediaId { get; set; } = "";

    [JsonPropertyName("mediaFile")]
    public string? MediaFile { get; set; }

    [JsonPropertyName("mediaType")]
    public string MediaType { get; set; } = "video"; // "video", "audio", "generator", "title"

    [JsonPropertyName("generatorType")]
    public string? GeneratorType { get; set; }

    [JsonPropertyName("titleText")]
    public string? TitleText { get; set; }

    [JsonPropertyName("subtitleText")]
    public string? SubtitleText { get; set; }

    [JsonPropertyName("color")]
    public string Color { get; set; } = "#3498DB";

    [JsonPropertyName("trackAccentColor")]
    public string? TrackAccentColor { get; set; }

    [JsonPropertyName("propagateTintToClips")]
    public bool PropagateTintToClips { get; set; } = true;

    [JsonPropertyName("start")]
    public double Start { get; set; } = 0.0;

    [JsonPropertyName("duration")]
    public double Duration { get; set; } = 5.0;

    [JsonPropertyName("trimIn")]
    public double TrimIn { get; set; } = 0.0;

    [JsonPropertyName("fadeIn")]
    public double FadeIn { get; set; } = 0.35;

    [JsonPropertyName("fadeOut")]
    public double FadeOut { get; set; } = 0.35;

    [JsonPropertyName("fadeInCurve")]
    public string FadeInCurve { get; set; } = "smooth";

    [JsonPropertyName("fadeOutCurve")]
    public string FadeOutCurve { get; set; } = "smooth";

    [JsonPropertyName("gain")]
    public double Gain { get; set; } = 1.0;

    [JsonPropertyName("opacity")]
    public double Opacity { get; set; } = 1.0;

    [JsonPropertyName("playbackRate")]
    public double PlaybackRate { get; set; } = 1.0;

    [JsonPropertyName("muted")]
    public bool Muted { get; set; } = false;

    [JsonPropertyName("locked")]
    public bool Locked { get; set; } = false;

    [JsonPropertyName("isOffline")]
    public bool IsOffline { get; set; } = false;

    [JsonPropertyName("panCrop")]
    public PanCropState PanCrop { get; set; } = new();

    [JsonPropertyName("groupId")]
    public string? GroupId { get; set; }

    [JsonPropertyName("mediaFx")]
    public List<VideoFxInstance> MediaFx { get; set; } = new();

    [JsonPropertyName("takeFx")]
    public List<VideoFxInstance> TakeFx { get; set; } = new();

    [JsonPropertyName("fx")]
    public List<VideoFxInstance> Fx { get; set; } = new();

    [JsonIgnore]
    public List<VideoFxInstance> EventFx => Fx;

    [JsonPropertyName("peaks")]
    public List<float>? Peaks { get; set; }

    [JsonIgnore]
    public double End => Start + Duration;

    public TimelineClip Clone()
    {
        return new TimelineClip
        {
            Id = $"clip_{Guid.NewGuid():N}"[..13],
            Name = Name,
            MediaId = MediaId,
            MediaFile = MediaFile,
            MediaType = MediaType,
            GeneratorType = GeneratorType,
            TitleText = TitleText,
            SubtitleText = SubtitleText,
            Color = Color,
            Start = Start,
            Duration = Duration,
            TrimIn = TrimIn,
            FadeIn = FadeIn,
            FadeOut = FadeOut,
            FadeInCurve = FadeInCurve,
            FadeOutCurve = FadeOutCurve,
            Gain = Gain,
            Opacity = Opacity,
            PlaybackRate = PlaybackRate,
            Muted = Muted,
            Locked = Locked,
            IsOffline = IsOffline,
            PanCrop = PanCrop.Clone(),
            GroupId = GroupId,
            MediaFx = new List<VideoFxInstance>(MediaFx),
            TakeFx = new List<VideoFxInstance>(TakeFx),
            Fx = new List<VideoFxInstance>(Fx),
            Peaks = Peaks != null ? new List<float>(Peaks) : null
        };
    }
}

public sealed class TimelineTrack
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"tr_{Guid.NewGuid():N}"[..10];

    [JsonPropertyName("name")]
    public string Name { get; set; } = "Track";

    [JsonPropertyName("type")]
    public string Type { get; set; } = "video"; // "video" or "audio"

    [JsonPropertyName("color")]
    public string Color { get; set; } = "#3498DB";

    [JsonPropertyName("trackAccentColor")]
    public string? TrackAccentColor { get; set; }

    [JsonPropertyName("propagateTintToClips")]
    public bool PropagateTintToClips { get; set; } = true;

    [JsonPropertyName("height")]
    public double Height { get; set; } = 78.0;

    [JsonPropertyName("groupId")]
    public string? GroupId { get; set; }

    [JsonPropertyName("groupName")]
    public string? GroupName { get; set; }

    [JsonPropertyName("groupColor")]
    public string? GroupColor { get; set; }

    [JsonPropertyName("level")]
    public double Level { get; set; } = 1.0;

    [JsonPropertyName("pan")]
    public double Pan { get; set; } = 0.0;

    [JsonPropertyName("blendMode")]
    public string BlendMode { get; set; } = "sourceAlpha";

    [JsonPropertyName("muted")]
    public bool Muted { get; set; } = false;

    [JsonPropertyName("solo")]
    public bool Solo { get; set; } = false;

    [JsonPropertyName("locked")]
    public bool Locked { get; set; } = false;

    [JsonPropertyName("showEnvelope")]
    public bool ShowEnvelope { get; set; } = true;

    [JsonPropertyName("trackMotion")]
    public PanCropState TrackMotion { get; set; } = new();

    [JsonPropertyName("fx")]
    public List<VideoFxInstance> Fx { get; set; } = new();

    [JsonIgnore]
    public List<VideoFxInstance> TrackFx => Fx;

    [JsonPropertyName("clips")]
    public List<TimelineClip> Clips { get; set; } = new();
}

public sealed class TimelineMarker
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"mk_{Guid.NewGuid():N}"[..8];

    [JsonPropertyName("time")]
    public double Time { get; set; }

    [JsonPropertyName("label")]
    public string Label { get; set; } = "Marker";

    [JsonPropertyName("color")]
    public string Color { get; set; } = "#f97316";
}

public sealed class TimelineRegion
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"rg_{Guid.NewGuid():N}"[..8];

    [JsonPropertyName("start")]
    public double Start { get; set; }

    [JsonPropertyName("end")]
    public double End { get; set; }

    [JsonPropertyName("label")]
    public string Label { get; set; } = "Region";
}

public sealed class CrossfadeOverlap
{
    public string TrackId { get; set; } = "";
    public int TrackIndex { get; set; }
    public string OutgoingClipId { get; set; } = "";
    public string IncomingClipId { get; set; } = "";
    public double Start { get; set; }
    public double End { get; set; }
    public double Duration { get; set; }
}

public sealed class MediaAssetItem
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = Guid.NewGuid().ToString("N")[..8];

    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    [JsonPropertyName("fileName")]
    public string? FileName { get; set; }

    [JsonPropertyName("filePath")]
    public string? FilePath { get; set; }

    [JsonPropertyName("type")]
    public string Type { get; set; } = "video"; // "video", "audio", "generator", "title"

    [JsonPropertyName("generatorType")]
    public string? GeneratorType { get; set; }

    [JsonPropertyName("duration")]
    public double Duration { get; set; } = 5.0;

    [JsonPropertyName("width")]
    public int Width { get; set; } = 1920;

    [JsonPropertyName("height")]
    public int Height { get; set; } = 1080;

    [JsonPropertyName("fps")]
    public double Fps { get; set; } = 29.97;

    [JsonPropertyName("codec")]
    public string Codec { get; set; } = "H264";

    [JsonPropertyName("sampleRate")]
    public int SampleRate { get; set; } = 48000;

    [JsonPropertyName("peakDb")]
    public double PeakDb { get; set; } = -1.2;

    [JsonPropertyName("rmsDb")]
    public double RmsDb { get; set; } = -14.0;

    [JsonPropertyName("color")]
    public string Color { get; set; } = "#3498DB";

    [JsonPropertyName("trackAccentColor")]
    public string? TrackAccentColor { get; set; }

    [JsonPropertyName("propagateTintToClips")]
    public bool PropagateTintToClips { get; set; } = true;

    [JsonPropertyName("isOffline")]
    public bool IsOffline { get; set; } = false;

    [JsonPropertyName("peaks")]
    public List<float>? Peaks { get; set; }

    [JsonPropertyName("mediaFx")]
    public List<VideoFxInstance> MediaFx { get; set; } = new();
}

public sealed class HistoryActionEntry
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = $"act_{Guid.NewGuid():N}"[..10];

    [JsonPropertyName("timestampUtc")]
    public string TimestampUtc { get; set; } = DateTime.UtcNow.ToString("O");

    [JsonPropertyName("actionType")]
    public string ActionType { get; set; } = "Edit";

    [JsonPropertyName("description")]
    public string Description { get; set; } = "";
}

public sealed class VegasProject
{
    [JsonPropertyName("apiVersion")]
    public string ApiVersion { get; set; } = "2.1.0";

    [JsonPropertyName("name")]
    public string Name { get; set; } = "Neoveg_Project";

    [JsonPropertyName("isDemo")]
    public bool IsDemo { get; set; } = false;

    [JsonPropertyName("description")]
    public string Description { get; set; } = "Projeto multiplataforma Neoveg (Avalonia UI + Rust FFI + Python FFmpeg)";

    [JsonPropertyName("width")]
    public int Width { get; set; } = 1920;

    [JsonPropertyName("height")]
    public int Height { get; set; } = 1080;

    [JsonPropertyName("fps")]
    public double Fps { get; set; } = 29.97;

    [JsonPropertyName("pixelFormat")]
    public string PixelFormat { get; set; } = "8-bit (Unsigned Integer)";

    [JsonPropertyName("colorSpace")]
    public string ColorSpace { get; set; } = "Rec.709 Standard";

    [JsonPropertyName("resampleMode")]
    public string ResampleMode { get; set; } = "Disable Resample";

    [JsonPropertyName("motionBlurType")]
    public string MotionBlurType { get; set; } = "Gaussian";

    [JsonPropertyName("sampleRate")]
    public int SampleRate { get; set; } = 48000;

    [JsonPropertyName("audioBitDepth")]
    public int AudioBitDepth { get; set; } = 24;

    [JsonPropertyName("masterBusMode")]
    public string MasterBusMode { get; set; } = "Stereo";

    [JsonPropertyName("playhead")]
    public double Playhead { get; set; } = 0.0;

    [JsonPropertyName("loopStart")]
    public double LoopStart { get; set; } = 0.0;

    [JsonPropertyName("loopEnd")]
    public double LoopEnd { get; set; } = 8.0;

    [JsonPropertyName("loopEnabled")]
    public bool LoopEnabled { get; set; } = true;

    [JsonPropertyName("snapEnabled")]
    public bool SnapEnabled { get; set; } = true;

    [JsonPropertyName("rippleEnabled")]
    public bool RippleEnabled { get; set; } = false;

    [JsonPropertyName("autoCrossfadeEnabled")]
    public bool AutoCrossfadeEnabled { get; set; } = true;

    [JsonPropertyName("splitScreenBypass")]
    public bool SplitScreenBypass { get; set; } = false;

    [JsonPropertyName("showSafeAreas")]
    public bool ShowSafeAreas { get; set; } = true;

    [JsonPropertyName("showGrid")]
    public bool ShowGrid { get; set; } = false;

    [JsonPropertyName("masterVolume")]
    public double MasterVolume { get; set; } = 1.0;

    [JsonPropertyName("masterFx")]
    public List<VideoFxInstance> MasterFx { get; set; } = new();

    [JsonPropertyName("dynamicRamPreviewMb")]
    public int DynamicRamPreviewMb { get; set; } = 1024;

    [JsonPropertyName("tracks")]
    public List<TimelineTrack> Tracks { get; set; } = new();

    [JsonPropertyName("markers")]
    public List<TimelineMarker> Markers { get; set; } = new();

    [JsonPropertyName("regions")]
    public List<TimelineRegion> Regions { get; set; } = new();

    [JsonPropertyName("mediaPool")]
    public List<MediaAssetItem> MediaPool { get; set; } = new();

    [JsonPropertyName("undoStack")]
    public List<HistoryActionEntry> UndoStack { get; set; } = new();

    [JsonPropertyName("redoStack")]
    public List<HistoryActionEntry> RedoStack { get; set; } = new();

    public double ComputeTotalDuration()
    {
        double max = 0.0;
        foreach (var tr in Tracks)
        {
            foreach (var c in tr.Clips)
            {
                if (c.End > max) max = c.End;
            }
        }
        return max > 0.1 ? max : Math.Max(6.0, LoopEnd);
    }

    public static VegasProject CreateEmpty(string name = "Novo_Projeto_Neoveg", int width = 1920, int height = 1080, double fps = 29.97)
    {
        return new VegasProject
        {
            Name = name,
            IsDemo = false,
            Width = width,
            Height = height,
            Fps = fps,
            Tracks = new List<TimelineTrack>
            {
                new TimelineTrack
                {
                    Id = "track_v1",
                    Name = "Titles & Text FX",
                    Type = "video",
                    Color = "#0e7490",
                    Height = 76
                },
                new TimelineTrack
                {
                    Id = "track_v2",
                    Name = "Program Video Track",
                    Type = "video",
                    Color = "#155e75",
                    Height = 82
                },
                new TimelineTrack
                {
                    Id = "track_a1",
                    Name = "Master Stereo Bus Track",
                    Type = "audio",
                    Color = "#881337",
                    Height = 78
                },
                new TimelineTrack
                {
                    Id = "track_a2",
                    Name = "SFX & Voiceover Track",
                    Type = "audio",
                    Color = "#7f1d1d",
                    Height = 74
                }
            }
        };
    }

    public static VegasProject CreateBroadcastDemo()
    {
        var proj = CreateEmpty("Neoveg_Broadcast_Demo", 1920, 1080, 29.97);
        proj.IsDemo = true;
        proj.Description = "Projeto de Demonstração Broadcast (CorridorKey AI + Frei0r + LADSPA + Auto-Crossfade)";

        // Group Video 2 + Audio 1 by default to demonstrate Track Grouping
        proj.Tracks[1].GroupId = "grp_av_1";
        proj.Tracks[1].GroupName = "Grupo V+A 1";
        proj.Tracks[1].GroupColor = "#f59e0b";
        proj.Tracks[2].GroupId = "grp_av_1";
        proj.Tracks[2].GroupName = "Grupo V+A 1";
        proj.Tracks[2].GroupColor = "#f59e0b";

        proj.Tracks[0].Clips.Add(new TimelineClip
        {
            Id = "clip_title_1",
            Name = "NEOVEG BROADCAST LOWER-THIRD",
            MediaType = "title",
            GeneratorType = "titleText",
            TitleText = "NEOVEG MULTIPLATAFORMA 21",
            SubtitleText = "Avalonia UI (C#) • Rust FFI Core • Python Isolated FFmpeg",
            Color = "#6366f1",
            Start = 0.5,
            Duration = 4.5,
            FadeIn = 0.5,
            FadeOut = 0.5
        });

        proj.Tracks[1].Clips.Add(new TimelineClip
        {
            Id = "clip_vid_1",
            Name = "Seafront - 4743",
            MediaFile = "vegas_smpte_bars.mp4",
            MediaType = "video",
            GeneratorType = "smpteBars",
            Color = "#0e7490",
            Start = 0.0,
            Duration = 4.2,
            FadeIn = 0.3,
            FadeOut = 1.0
        });

        proj.Tracks[1].Clips.Add(new TimelineClip
        {
            Id = "clip_vid_2",
            Name = "CorridorKey Green Screen Actor",
            MediaFile = "vegas_cyber_scene.mp4",
            MediaType = "generator",
            GeneratorType = "greenScreenActor",
            Color = "#16a34a",
            Start = 3.2, // Overlaps clip_vid_1 by 1.0s -> Auto-Crossfade!
            Duration = 4.8,
            FadeIn = 1.0,
            FadeOut = 0.5,
            Fx = new List<VideoFxInstance>
            {
                new VideoFxInstance
                {
                    Type = "corridorKey",
                    Name = "CorridorKey AI Green Screen Unmixer",
                    Enabled = true,
                    Params = new Dictionary<string, double>
                    {
                        ["similarity"] = 0.28,
                        ["despill"] = 0.90,
                        ["blend"] = 0.08
                    }
                }
            }
        });

        var demoPeaks = new List<float>();
        for (int i = 0; i < 96; i++)
        {
            demoPeaks.Add((float)(0.25 + 0.65 * Math.Abs(Math.Sin(i * 0.24) * Math.Cos(i * 0.07))));
        }

        proj.Tracks[2].Clips.Add(new TimelineClip
        {
            Id = "clip_aud_1",
            Name = "Neoveg Synth Pad (48kHz PCM WAV)",
            MediaFile = "vegas_synth_pad.wav",
            MediaType = "audio",
            Color = "#0891b2",
            Start = 0.0,
            Duration = 6.5,
            FadeIn = 0.4,
            FadeOut = 0.6,
            Peaks = demoPeaks,
            Fx = new List<VideoFxInstance>
            {
                new VideoFxInstance
                {
                    Type = "ladspa.sc4_1882",
                    Name = "LADSPA SC4 Stereo Compressor (1882)",
                    Enabled = true
                }
            }
        });

        proj.Markers.Add(new TimelineMarker { Time = 0.5, Label = "Intro Title", Color = "#f97316" });
        proj.Markers.Add(new TimelineMarker { Time = 3.2, Label = "X-Fade + CorridorKey", Color = "#10b981" });
        proj.Regions.Add(new TimelineRegion { Start = 0.0, End = 8.0, Label = "Broadcast Master Cut" });

        return proj;
    }

    /// <summary>
    /// Creates a project whose MediaPool and timeline clips point to moved/missing file paths
    /// so the VEGAS Pro "Missing Media" (Search & Relink) workflow can be tested at any time.
    /// </summary>
    public static VegasProject CreateMovedMediaRelinkDemo()
    {
        var proj = CreateBroadcastDemo();
        proj.Name = "Neoveg_MovedMedia_RelinkTest";
        proj.Description = "Projeto com Mídias Movidas/Renomeadas para Teste do Search & Relink (Neoveg.Core.MediaRelinker)";

        string oldPath1 = @"C:\Old_Neoveg_Archive\Camera_A\vegas_smpte_bars.mp4";
        string oldPath2 = @"D:\External_SSD_2025\Rushes\CAM01_neoveg_broadcast_slate_FINAL.mp4";
        string oldPath3 = @"E:\Audio_Backup_Drive\Stems\synth_bass_120bpm.wav";

        proj.MediaPool = new List<MediaAssetItem>
        {
            new MediaAssetItem
            {
                Id = "media_rel_1",
                Name = "Seafront - 4743 (SMPTE Master)",
                FileName = "vegas_smpte_bars.mp4",
                FilePath = oldPath1,
                Type = "video",
                Duration = 4.0,
                Width = 1280,
                Height = 720,
                Fps = 29.97
            },
            new MediaAssetItem
            {
                Id = "media_rel_2",
                Name = "Broadcast Slate Camera A (Renomeado)",
                FileName = "CAM01_neoveg_broadcast_slate_FINAL.mp4",
                FilePath = oldPath2,
                Type = "video",
                Duration = 4.0,
                Width = 1280,
                Height = 720,
                Fps = 29.97
            },
            new MediaAssetItem
            {
                Id = "media_rel_3",
                Name = "Neoveg Synth Bass 120BPM (48kHz PCM WAV)",
                FileName = "synth_bass_120bpm.wav",
                FilePath = oldPath3,
                Type = "audio",
                Duration = 4.0,
                SampleRate = 48000
            }
        };

        if (proj.Tracks.Count > 1 && proj.Tracks[1].Clips.Count >= 2)
        {
            proj.Tracks[1].Clips[0].MediaId = "media_rel_1";
            proj.Tracks[1].Clips[0].MediaFile = oldPath1;

            proj.Tracks[1].Clips[1].MediaId = "media_rel_2";
            proj.Tracks[1].Clips[1].MediaFile = oldPath2;
        }

        if (proj.Tracks.Count > 2 && proj.Tracks[2].Clips.Count >= 1)
        {
            proj.Tracks[2].Clips[0].MediaId = "media_rel_3";
            proj.Tracks[2].Clips[0].MediaFile = oldPath3;
        }

        return proj;
    }
}
