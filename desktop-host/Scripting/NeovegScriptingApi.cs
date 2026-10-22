using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Avalonia.Controls;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.Scripting;

/// <summary>
/// Represents a SMPTE / nanosecond-accurate Timecode in the Neoveg & VEGAS Pro Scripting API (`Neoveg.Scripting.Timecode`).
/// </summary>
public readonly struct Timecode : IComparable<Timecode>, IEquatable<Timecode>
{
    public double TotalSeconds { get; }
    public long Nanos => (long)Math.Round(TotalSeconds * 10_000_000.0);
    public long FrameCount => (long)Math.Round(TotalSeconds * 29.97);

    public Timecode(double seconds)
    {
        TotalSeconds = Math.Max(0.0, seconds);
    }

    public static Timecode FromSeconds(double seconds) => new(seconds);
    public static Timecode FromMilliseconds(double ms) => new(ms / 1000.0);
    public static Timecode FromFrames(long frames, double fps = 29.97) => new(frames / Math.Max(1.0, fps));
    public static Timecode FromNanos(long nanos) => new(nanos / 10_000_000.0);

    public static Timecode FromString(string smpte, double fps = 29.97)
    {
        if (string.IsNullOrWhiteSpace(smpte)) return new Timecode(0);
        var parts = smpte.Trim().Split(':', ';', '.');
        if (parts.Length == 4 &&
            int.TryParse(parts[0], out int hh) &&
            int.TryParse(parts[1], out int mm) &&
            int.TryParse(parts[2], out int ss) &&
            int.TryParse(parts[3], out int ff))
        {
            return new Timecode(hh * 3600.0 + mm * 60.0 + ss + (ff / Math.Max(1.0, fps)));
        }
        if (double.TryParse(smpte, NumberStyles.Float, CultureInfo.InvariantCulture, out double sec))
            return new Timecode(sec);
        return new Timecode(0);
    }

    public double ToMilliseconds() => TotalSeconds * 1000.0;
    public double ToSeconds() => TotalSeconds;

    public override string ToString()
    {
        int totalFrames = (int)Math.Round(TotalSeconds * 29.97);
        int ff = totalFrames % 30;
        int totalSec = totalFrames / 30;
        int ss = totalSec % 60;
        int mm = (totalSec / 60) % 60;
        int hh = totalSec / 3600;
        return $"{hh:D2}:{mm:D2}:{ss:D2};{ff:D2}";
    }

    public int CompareTo(Timecode other) => TotalSeconds.CompareTo(other.TotalSeconds);
    public bool Equals(Timecode other) => Math.Abs(TotalSeconds - other.TotalSeconds) < 1e-6;
    public override bool Equals(object? obj) => obj is Timecode tc && Equals(tc);
    public override int GetHashCode() => TotalSeconds.GetHashCode();

    public static Timecode operator +(Timecode a, Timecode b) => new(a.TotalSeconds + b.TotalSeconds);
    public static Timecode operator -(Timecode a, Timecode b) => new(Math.Max(0.0, a.TotalSeconds - b.TotalSeconds));
    public static bool operator <(Timecode a, Timecode b) => a.TotalSeconds < b.TotalSeconds - 1e-6;
    public static bool operator >(Timecode a, Timecode b) => a.TotalSeconds > b.TotalSeconds + 1e-6;
    public static bool operator <=(Timecode a, Timecode b) => a.TotalSeconds <= b.TotalSeconds + 1e-6;
    public static bool operator >=(Timecode a, Timecode b) => a.TotalSeconds >= b.TotalSeconds - 1e-6;
    public static bool operator ==(Timecode a, Timecode b) => a.Equals(b);
    public static bool operator !=(Timecode a, Timecode b) => !a.Equals(b);
}

public enum CurveType
{
    Linear,
    Smooth,
    Fast,
    Slow,
    Sharp,
    Hold
}

/// <summary>
/// Represents an event FadeIn or FadeOut envelope (`TrackEvent.FadeIn` / `TrackEvent.FadeOut`).
/// </summary>
public sealed class Fade
{
    private readonly TimelineClip _clip;
    private readonly bool _isFadeIn;

    internal Fade(TimelineClip clip, bool isFadeIn)
    {
        _clip = clip;
        _isFadeIn = isFadeIn;
    }

    public Timecode Length
    {
        get => Timecode.FromSeconds(_isFadeIn ? _clip.FadeIn : _clip.FadeOut);
        set
        {
            double clamped = Math.Clamp(value.TotalSeconds, 0.0, _clip.Duration * 0.5);
            if (_isFadeIn) _clip.FadeIn = Math.Round(clamped, 3);
            else _clip.FadeOut = Math.Round(clamped, 3);
        }
    }

    public CurveType Curve
    {
        get
        {
            string raw = _isFadeIn ? _clip.FadeInCurve : _clip.FadeOutCurve;
            return raw.ToLowerInvariant() switch
            {
                "linear" => CurveType.Linear,
                "fast" => CurveType.Fast,
                "slow" => CurveType.Slow,
                "sharp" => CurveType.Sharp,
                _ => CurveType.Smooth
            };
        }
        set
        {
            string c = value.ToString().ToLowerInvariant();
            if (_isFadeIn) _clip.FadeInCurve = c;
            else _clip.FadeOutCurve = c;
        }
    }

    public Timecode TransitionOffset { get; set; } = Timecode.FromSeconds(0);
}

/// <summary>
/// Represents a media Take inside a `TrackEvent` (`Neoveg.Scripting.Take`).
/// </summary>
public sealed class Take
{
    private readonly TimelineClip _clip;

    internal Take(TimelineClip clip, string name, string mediaPath, bool isActive = true)
    {
        _clip = clip;
        Name = name;
        MediaPath = mediaPath;
        IsActive = isActive;
    }

    public string Name
    {
        get => _clip.Name;
        set => _clip.Name = value;
    }

    public string MediaPath
    {
        get => _clip.MediaFile ?? _clip.Name;
        set => _clip.MediaFile = value;
    }

    public Timecode Offset
    {
        get => Timecode.FromSeconds(_clip.TrimIn);
        set => _clip.TrimIn = Math.Max(0.0, value.TotalSeconds);
    }

    public bool IsActive { get; set; }
}

public sealed class TakeList : IReadOnlyList<Take>
{
    private readonly List<Take> _takes;

    internal TakeList(TimelineClip clip)
    {
        _takes = new List<Take>
        {
            new Take(clip, clip.Name, clip.MediaFile ?? clip.Name, true)
        };
    }

    public int Count => _takes.Count;
    public Take this[int index] => _takes[index];
    public Take Add(Take take) { _takes.Add(take); return take; }
    public IEnumerator<Take> GetEnumerator() => _takes.GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

/// <summary>
/// Represents an OFX / Frei0r / LADSPA / Datamosh Effect instance on a `TrackEvent` or `Track` (`Neoveg.Scripting.Effect`).
/// </summary>
public sealed class Effect
{
    internal readonly VideoFxInstance UnderlyingFx;

    internal Effect(VideoFxInstance fx)
    {
        UnderlyingFx = fx;
    }

    public Effect(string plugInUniqueId, string description, bool enabled = true)
    {
        UnderlyingFx = new VideoFxInstance
        {
            Type = plugInUniqueId,
            Name = description,
            Enabled = enabled
        };
    }

    public string UniqueID
    {
        get => UnderlyingFx.Type;
        set => UnderlyingFx.Type = value;
    }

    public string Description
    {
        get => UnderlyingFx.Name;
        set => UnderlyingFx.Name = value;
    }

    public string PlugInName => UnderlyingFx.Name;

    public bool Bypass
    {
        get => !UnderlyingFx.Enabled;
        set => UnderlyingFx.Enabled = !value;
    }

    public string Preset { get; set; } = "(Default)";

    public Dictionary<string, double> Parameters => UnderlyingFx.Params;
}

public sealed class EffectList : IEnumerable<Effect>
{
    private readonly List<VideoFxInstance> _backingList;

    internal EffectList(List<VideoFxInstance> backingList)
    {
        _backingList = backingList;
    }

    public int Count => _backingList.Count;

    public Effect this[int index] => new(_backingList[index]);

    public Effect Add(Effect effect)
    {
        _backingList.Add(effect.UnderlyingFx);
        return effect;
    }

    public Effect AddEffect(string uniqueId, string description)
    {
        var inst = new VideoFxInstance
        {
            Type = uniqueId,
            Name = description,
            Enabled = true
        };
        _backingList.Add(inst);
        return new Effect(inst);
    }

    public bool Remove(Effect effect) => _backingList.Remove(effect.UnderlyingFx);
    public void Clear() => _backingList.Clear();

    public IEnumerator<Effect> GetEnumerator() => _backingList.Select(f => new Effect(f)).GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

/// <summary>
/// Base class for Timeline Events (`VideoEvent` and `AudioEvent`) in `Neoveg.Scripting`.
/// </summary>
public abstract class TrackEvent
{
    internal readonly TimelineClip UnderlyingClip;
    internal readonly TimelineTrack UnderlyingTrack;
    internal readonly Vegas OwnerVegas;

    protected TrackEvent(Vegas owner, TimelineTrack track, TimelineClip clip)
    {
        OwnerVegas = owner;
        UnderlyingTrack = track;
        UnderlyingClip = clip;
        FadeIn = new Fade(clip, isFadeIn: true);
        FadeOut = new Fade(clip, isFadeIn: false);
        Takes = new TakeList(clip);
        Effects = new EffectList(clip.Fx);
    }

    public string Id => UnderlyingClip.Id;

    public string Name
    {
        get => UnderlyingClip.Name;
        set => UnderlyingClip.Name = value;
    }

    public Track Track => UnderlyingTrack.Type == "audio"
        ? new AudioTrack(OwnerVegas, UnderlyingTrack, 0)
        : new VideoTrack(OwnerVegas, UnderlyingTrack, 0);

    public Timecode Start
    {
        get => Timecode.FromSeconds(UnderlyingClip.Start);
        set => UnderlyingClip.Start = Math.Round(Math.Max(0.0, value.TotalSeconds), 3);
    }

    public Timecode Length
    {
        get => Timecode.FromSeconds(UnderlyingClip.Duration);
        set => UnderlyingClip.Duration = Math.Round(Math.Max(0.05, value.TotalSeconds), 3);
    }

    public Timecode End => Timecode.FromSeconds(UnderlyingClip.End);

    public bool Selected
    {
        get => OwnerVegas.SelectedClipIds.Contains(UnderlyingClip.Id);
        set
        {
            if (value) OwnerVegas.SelectedClipIds.Add(UnderlyingClip.Id);
            else OwnerVegas.SelectedClipIds.Remove(UnderlyingClip.Id);
        }
    }

    public bool Mute
    {
        get => UnderlyingClip.Muted;
        set => UnderlyingClip.Muted = value;
    }

    public bool Locked
    {
        get => UnderlyingClip.Locked;
        set => UnderlyingClip.Locked = value;
    }

    public double PlaybackRate
    {
        get => UnderlyingClip.PlaybackRate;
        set => UnderlyingClip.PlaybackRate = Math.Clamp(value, 0.1, 4.0);
    }

    public Fade FadeIn { get; }
    public Fade FadeOut { get; }
    public TakeList Takes { get; }
    public Take ActiveTake => Takes[0];
    public EffectList Effects { get; }

    public bool IsVideo => UnderlyingTrack.Type != "audio";
    public bool IsAudio => UnderlyingTrack.Type == "audio";

    /// <summary>
    /// Splits this `TrackEvent` at `offset` relative to `Start` (just like VEGAS Pro `TrackEvent.Split(Timecode offset)`).
    /// </summary>
    public TrackEvent? Split(Timecode offset)
    {
        double splitTimelineSec = UnderlyingClip.Start + offset.TotalSeconds;
        if (OwnerVegas.RustInterop.TrySplitClip(UnderlyingClip, splitTimelineSec, out var rightHalf))
        {
            int idx = UnderlyingTrack.Clips.IndexOf(UnderlyingClip);
            if (idx >= 0 && idx + 1 <= UnderlyingTrack.Clips.Count)
                UnderlyingTrack.Clips.Insert(idx + 1, rightHalf);
            else
                UnderlyingTrack.Clips.Add(rightHalf);

            return IsAudio
                ? new AudioEvent(OwnerVegas, UnderlyingTrack, rightHalf)
                : new VideoEvent(OwnerVegas, UnderlyingTrack, rightHalf);
        }
        return null;
    }
}

/// <summary>
/// Represents a Video Event on a `VideoTrack` (`Neoveg.Scripting.VideoEvent`).
/// Supports Pan/Crop keyframes, velocity envelopes (VPFlow), and Datamosh / Optical Glitch effects.
/// </summary>
public sealed class VideoEvent : TrackEvent
{
    internal VideoEvent(Vegas owner, TimelineTrack track, TimelineClip clip)
        : base(owner, track, clip)
    {
    }

    public PanCropState PanCrop => UnderlyingClip.PanCrop;

    public Effect ApplyDatamoshGlitch(double intensity = 0.85, string mode = "I-Frame Drop + P-Vector Bloom")
    {
        var fx = Effects.AddEffect("neoveg.datamosh", $"Datamosh Glitch ({mode})");
        fx.Parameters["intensity"] = Math.Clamp(intensity, 0.0, 1.0);
        fx.Parameters["mvStretch"] = Math.Round(intensity * 2.4, 2);
        fx.Parameters["chromaShift"] = Math.Round(intensity * 0.35, 2);
        UnderlyingClip.Color = "#7c3aed";
        return fx;
    }

    public void ApplyVPFlowBezierCurve(string presetName, double zoomFactor = 1.18, double angleDeg = 0.0)
    {
        var pc = UnderlyingClip.PanCrop;
        pc.InterpolationType = presetName;
        pc.Smoothness = 85.0;
        pc.Width = Math.Round(pc.SourceWidth / Math.Max(0.25, zoomFactor), 1);
        pc.Height = Math.Round(pc.SourceHeight / Math.Max(0.25, zoomFactor), 1);
        pc.Rotation = angleDeg;
        pc.SyncNormalizedFromPixels();
    }
}

/// <summary>
/// Represents an Audio Event on an `AudioTrack` (`Neoveg.Scripting.AudioEvent`).
/// </summary>
public sealed class AudioEvent : TrackEvent
{
    internal AudioEvent(Vegas owner, TimelineTrack track, TimelineClip clip)
        : base(owner, track, clip)
    {
    }

    public double Gain
    {
        get => UnderlyingClip.Gain;
        set => UnderlyingClip.Gain = Math.Clamp(value, 0.0, 4.0);
    }

    public double PitchShiftSemitones { get; set; }

    public void Normalize(double targetUnity = 1.0)
    {
        UnderlyingClip.Gain = Math.Clamp(targetUnity, 0.1, 2.0);
    }
}

public sealed class TrackEventList : IReadOnlyList<TrackEvent>
{
    private readonly Vegas _owner;
    private readonly TimelineTrack _track;

    internal TrackEventList(Vegas owner, TimelineTrack track)
    {
        _owner = owner;
        _track = track;
    }

    public int Count => _track.Clips.Count;

    public TrackEvent this[int index] => Wrap(_track.Clips[index]);

    private TrackEvent Wrap(TimelineClip clip) =>
        _track.Type == "audio"
            ? new AudioEvent(_owner, _track, clip)
            : new VideoEvent(_owner, _track, clip);

    public IEnumerator<TrackEvent> GetEnumerator() =>
        _track.Clips.ToList().Select(Wrap).GetEnumerator();

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

/// <summary>
/// Represents a Track on the VEGAS / Neoveg Timeline (`Neoveg.Scripting.Track`).
/// </summary>
public class Track
{
    internal readonly Vegas OwnerVegas;
    internal readonly TimelineTrack UnderlyingTrack;

    internal Track(Vegas owner, TimelineTrack track, int index)
    {
        OwnerVegas = owner;
        UnderlyingTrack = track;
        Index = index;
        Events = new TrackEventList(owner, track);
        Effects = new EffectList(new List<VideoFxInstance>());
    }

    public int Index { get; }

    public string Name
    {
        get => UnderlyingTrack.Name;
        set => UnderlyingTrack.Name = value;
    }

    public bool Selected
    {
        get => OwnerVegas.SelectedTrackId == UnderlyingTrack.Id;
        set
        {
            if (value) OwnerVegas.SelectedTrackId = UnderlyingTrack.Id;
        }
    }

    public bool Mute
    {
        get => UnderlyingTrack.Muted;
        set => UnderlyingTrack.Muted = value;
    }

    public bool Solo
    {
        get => UnderlyingTrack.Solo;
        set => UnderlyingTrack.Solo = value;
    }

    public bool IsVideo => UnderlyingTrack.Type != "audio";
    public bool IsAudio => UnderlyingTrack.Type == "audio";

    public TrackEventList Events { get; }
    public EffectList Effects { get; }
}

public sealed class VideoTrack : Track
{
    internal VideoTrack(Vegas owner, TimelineTrack track, int index)
        : base(owner, track, index)
    {
    }

    public VideoEvent AddVideoEvent(Timecode start, Timecode length, string name = "Scripted Video Event")
    {
        var clip = new TimelineClip
        {
            Name = name,
            MediaType = "generator",
            GeneratorType = "cyberGrid",
            Start = start.TotalSeconds,
            Duration = length.TotalSeconds,
            Color = "#0e7490"
        };
        UnderlyingTrack.Clips.Add(clip);
        return new VideoEvent(OwnerVegas, UnderlyingTrack, clip);
    }
}

public sealed class AudioTrack : Track
{
    internal AudioTrack(Vegas owner, TimelineTrack track, int index)
        : base(owner, track, index)
    {
    }

    public AudioEvent AddAudioEvent(Timecode start, Timecode length, string name = "Scripted Audio Event")
    {
        var clip = new TimelineClip
        {
            Name = name,
            MediaType = "audio",
            Start = start.TotalSeconds,
            Duration = length.TotalSeconds,
            Color = "#881337"
        };
        UnderlyingTrack.Clips.Add(clip);
        return new AudioEvent(OwnerVegas, UnderlyingTrack, clip);
    }
}

public sealed class TrackList : IReadOnlyList<Track>
{
    private readonly Vegas _owner;
    private readonly VegasProject _project;

    internal TrackList(Vegas owner, VegasProject project)
    {
        _owner = owner;
        _project = project;
    }

    public int Count => _project.Tracks.Count;

    public Track this[int index] => Wrap(_project.Tracks[index], index);

    private Track Wrap(TimelineTrack t, int idx) =>
        t.Type == "audio"
            ? new AudioTrack(_owner, t, idx)
            : new VideoTrack(_owner, t, idx);

    public void Add(Track track)
    {
        if (!_project.Tracks.Contains(track.UnderlyingTrack))
            _project.Tracks.Add(track.UnderlyingTrack);
    }

    public VideoTrack AddVideoTrack(string name = "Script Video Track")
    {
        var tr = new TimelineTrack
        {
            Name = name,
            Type = "video",
            Color = "#0e7490",
            Height = 78.0
        };
        _project.Tracks.Insert(0, tr);
        return new VideoTrack(_owner, tr, 0);
    }

    public AudioTrack AddAudioTrack(string name = "Script Audio Track")
    {
        var tr = new TimelineTrack
        {
            Name = name,
            Type = "audio",
            Color = "#881337",
            Height = 78.0
        };
        _project.Tracks.Add(tr);
        return new AudioTrack(_owner, tr, _project.Tracks.Count - 1);
    }

    public IEnumerator<Track> GetEnumerator() =>
        _project.Tracks.Select((t, i) => Wrap(t, i)).GetEnumerator();

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

/// <summary>
/// Represents a Timeline Marker (`Neoveg.Scripting.Marker`).
/// </summary>
public sealed class Marker
{
    internal readonly TimelineMarker UnderlyingMarker;

    internal Marker(TimelineMarker marker)
    {
        UnderlyingMarker = marker;
    }

    public Marker(Timecode position, string label = "")
    {
        UnderlyingMarker = new TimelineMarker
        {
            Time = Math.Round(position.TotalSeconds, 3),
            Label = label,
            Color = "#f97316"
        };
    }

    public Timecode Position
    {
        get => Timecode.FromSeconds(UnderlyingMarker.Time);
        set => UnderlyingMarker.Time = Math.Round(value.TotalSeconds, 3);
    }

    public string Label
    {
        get => UnderlyingMarker.Label;
        set => UnderlyingMarker.Label = value;
    }
}

public sealed class MarkerList : IReadOnlyList<Marker>
{
    private readonly List<TimelineMarker> _markers;

    internal MarkerList(List<TimelineMarker> markers)
    {
        _markers = markers;
    }

    public int Count => _markers.Count;
    public Marker this[int index] => new(_markers[index]);

    public void Add(Marker marker)
    {
        _markers.Add(marker.UnderlyingMarker);
        _markers.Sort((a, b) => a.Time.CompareTo(b.Time));
    }

    public void Clear() => _markers.Clear();

    public IEnumerator<Marker> GetEnumerator() => _markers.Select(m => new Marker(m)).GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

/// <summary>
/// Represents a Timeline Region (`Neoveg.Scripting.Region`).
/// </summary>
public sealed class Region
{
    internal readonly TimelineRegion UnderlyingRegion;

    internal Region(TimelineRegion region)
    {
        UnderlyingRegion = region;
    }

    public Region(Timecode position, Timecode length, string label = "")
    {
        UnderlyingRegion = new TimelineRegion
        {
            Start = Math.Round(position.TotalSeconds, 3),
            End = Math.Round((position + length).TotalSeconds, 3),
            Label = label
        };
    }

    public Timecode Position
    {
        get => Timecode.FromSeconds(UnderlyingRegion.Start);
        set => UnderlyingRegion.Start = Math.Round(value.TotalSeconds, 3);
    }

    public Timecode Length
    {
        get => Timecode.FromSeconds(Math.Max(0.05, UnderlyingRegion.End - UnderlyingRegion.Start));
        set => UnderlyingRegion.End = Math.Round(UnderlyingRegion.Start + Math.Max(0.05, value.TotalSeconds), 3);
    }

    public Timecode End => Timecode.FromSeconds(UnderlyingRegion.End);

    public string Label
    {
        get => UnderlyingRegion.Label;
        set => UnderlyingRegion.Label = value;
    }
}

public sealed class RegionList : IReadOnlyList<Region>
{
    private readonly List<TimelineRegion> _regions;

    internal RegionList(List<TimelineRegion> regions)
    {
        _regions = regions;
    }

    public int Count => _regions.Count;
    public Region this[int index] => new(_regions[index]);

    public void Add(Region region) => _regions.Add(region.UnderlyingRegion);
    public void Clear() => _regions.Clear();

    public IEnumerator<Region> GetEnumerator() => _regions.Select(r => new Region(r)).GetEnumerator();
    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

public sealed class ProjectVideoProperties
{
    private readonly VegasProject _project;
    internal ProjectVideoProperties(VegasProject project) => _project = project;

    public int Width
    {
        get => _project.Width;
        set => _project.Width = Math.Clamp(value, 320, 7680);
    }

    public int Height
    {
        get => _project.Height;
        set => _project.Height = Math.Clamp(value, 240, 4320);
    }

    public double FrameRate
    {
        get => _project.Fps;
        set => _project.Fps = Math.Clamp(value, 12.0, 240.0);
    }
}

public sealed class ProjectAudioProperties
{
    private readonly VegasProject _project;
    internal ProjectAudioProperties(VegasProject project) => _project = project;

    public int SampleRate
    {
        get => _project.SampleRate;
        set => _project.SampleRate = value;
    }

    public double MasterBusVolume
    {
        get => _project.MasterVolume;
        set => _project.MasterVolume = Math.Clamp(value, 0.0, 2.0);
    }
}

/// <summary>
/// Represents the active VEGAS / Neovegject (`Neoveg.Scripting.Project` / `myVegas.Project`).
/// </summary>
public sealed class Project
{
    internal readonly VegasProject UnderlyingProject;

    internal Project(Vegas owner, VegasProject project)
    {
        UnderlyingProject = project;
        Tracks = new TrackList(owner, project);
        Markers = new MarkerList(project.Markers);
        Regions = new RegionList(project.Regions);
        Video = new ProjectVideoProperties(project);
        Audio = new ProjectAudioProperties(project);
    }

    public string Name
    {
        get => UnderlyingProject.Name;
        set => UnderlyingProject.Name = value;
    }

    public string FilePath { get; set; } = "projects/Neoveg_Broadcast_Demo.veg";

    public TrackList Tracks { get; }
    public MarkerList Markers { get; }
    public RegionList Regions { get; }
    public ProjectVideoProperties Video { get; }
    public ProjectAudioProperties Audio { get; }
}

/// <summary>
/// Root `Vegas` Scripting Host Object (`myVegas` / `vegas`) exposed to C# (`.cs`), compiled `.dll` extensions, and `EntryPoint.FromVegas(Vegas myVegas)`.
/// </summary>
public sealed class Vegas
{
    internal readonly VegasProject UnderlyingProject;
    internal readonly RustCoreInterop RustInterop;
    internal readonly Action OnUiRefreshRequested;
    internal readonly HashSet<string> SelectedClipIds = new(StringComparer.OrdinalIgnoreCase);
    internal string? SelectedTrackId;

    public Project Project { get; }
    public List<string> ScriptLogs { get; } = new();
    public string Version => "VEGAS Pro 21.0 / Neoveg Scripting Portal (.NET + Rust FFI + Python)";

    public Vegas(
        VegasProject project,
        RustCoreInterop rustInterop,
        string? selectedClipId,
        string? selectedTrackId,
        Action onUiRefreshRequested)
    {
        UnderlyingProject = project;
        RustInterop = rustInterop;
        SelectedTrackId = selectedTrackId;
        if (!string.IsNullOrWhiteSpace(selectedClipId))
        {
            SelectedClipIds.Add(selectedClipId);
        }
        else
        {
            var firstClip = project.Tracks.SelectMany(t => t.Clips).FirstOrDefault();
            if (firstClip != null)
                SelectedClipIds.Add(firstClip.Id);
        }

        if (string.IsNullOrWhiteSpace(SelectedTrackId))
        {
            SelectedTrackId = project.Tracks.FirstOrDefault()?.Id;
        }

        OnUiRefreshRequested = onUiRefreshRequested;
        Project = new Project(this, project);
    }

    public Timecode Cursor
    {
        get => Timecode.FromSeconds(UnderlyingProject.Playhead);
        set => UnderlyingProject.Playhead = Math.Round(Math.Max(0.0, value.TotalSeconds), 3);
    }

    public Timecode SelectionStart
    {
        get => Timecode.FromSeconds(UnderlyingProject.LoopStart);
        set => UnderlyingProject.LoopStart = Math.Round(Math.Max(0.0, value.TotalSeconds), 3);
    }

    public Timecode SelectionLength
    {
        get => Timecode.FromSeconds(Math.Max(0.1, UnderlyingProject.LoopEnd - UnderlyingProject.LoopStart));
        set => UnderlyingProject.LoopEnd = Math.Round(UnderlyingProject.LoopStart + Math.Max(0.1, value.TotalSeconds), 3);
    }

    public void Log(string message)
    {
        if (!string.IsNullOrWhiteSpace(message))
            ScriptLogs.Add(message);
    }

    public void ShowMessage(string title, string message)
    {
        ScriptLogs.Add($"[{title}] {message}");
    }

    public void UpdateUI()
    {
        OnUiRefreshRequested?.Invoke();
    }

    /// <summary>
    /// High-level helper for Batch Split + Datamosh Glitch across the selected video track.
    /// </summary>
    public int BatchSplitAndDatamosh(double intervalSeconds = 0.85, double glitchIntensity = 0.88, double speedFactor = 1.15)
    {
        double step = Math.Clamp(intervalSeconds, 0.25, 4.0);
        int affected = 0;

        var targetTracks = Project.Tracks.Where(t => t.IsVideo && t.Selected).ToList();
        if (targetTracks.Count == 0)
            targetTracks = Project.Tracks.Where(t => t.IsVideo).Take(1).ToList();

        foreach (var track in targetTracks)
        {
            var initialEvents = track.Events.ToList();
            foreach (var ev in initialEvents)
            {
                if (ev.Length.TotalSeconds > step * 1.4)
                {
                    var rightPiece = ev.Split(Timecode.FromSeconds(step));
                    if (rightPiece is VideoEvent rightVe)
                    {
                        rightVe.ApplyDatamoshGlitch(glitchIntensity, "P-Frame Motion Vector Bloom");
                        rightVe.PlaybackRate = speedFactor;
                        rightVe.FadeIn.Length = Timecode.FromSeconds(0.12);
                        rightVe.FadeIn.Curve = CurveType.Sharp;
                        Project.Markers.Add(new Marker(rightVe.Start, $"Datamosh Glitch #{affected + 1}"));
                        affected++;
                    }
                }
                if (ev is VideoEvent leftVe)
                {
                    leftVe.ApplyDatamoshGlitch(glitchIntensity * 0.85, "I-Frame Drop + Optical Flow");
                    affected++;
                }
            }
        }

        UpdateUI();
        return affected;
    }

    /// <summary>
    /// High-level helper for VPFlow Auto-Crossfade & Smooth S-Curve alignment.
    /// </summary>
    public int ApplyVPFlowAutoCrossfade(double overlapSeconds = 0.75, CurveType curve = CurveType.Smooth)
    {
        double ov = Math.Clamp(overlapSeconds, 0.15, 2.5);
        int transitions = 0;

        foreach (var track in UnderlyingProject.Tracks)
        {
            var sorted = track.Clips.OrderBy(c => c.Start).ToList();
            for (int i = 1; i < sorted.Count; i++)
            {
                var prev = sorted[i - 1];
                var curr = sorted[i];
                double prevEnd = prev.Start + prev.Duration;
                if (prev.Duration > ov * 1.4 && curr.Duration > ov * 1.4)
                {
                    curr.Start = Math.Round(Math.Max(0.0, prevEnd - ov), 3);
                    prev.FadeOut = Math.Round(ov, 3);
                    prev.FadeOutCurve = curve.ToString().ToLowerInvariant();
                    curr.FadeIn = Math.Round(ov, 3);
                    curr.FadeInCurve = curve.ToString().ToLowerInvariant();
                    transitions++;
                }
            }
        }

        UpdateUI();
        return transitions;
    }
}

// =================================================================================================
// DOCKABLE EXTENSIONS & CUSTOM COMMAND MODULE API (VPFlow, Datamosh, Vegasaur, HOS Compatibility)
// =================================================================================================

public enum CommandCategory
{
    View,
    Edit,
    Tools
}

public sealed class CustomCommand
{
    public CommandCategory Category { get; }
    public string CommandName { get; }
    public string DisplayName { get; set; }
    public event EventHandler? Invoked;

    public CustomCommand(CommandCategory category, string commandName)
    {
        Category = category;
        CommandName = commandName;
        DisplayName = commandName;
    }

    public void RaiseInvoked() => Invoked?.Invoke(this, EventArgs.Empty);
}

/// <summary>
/// Base class for Dockable Window Extensions in Neoveg (`Neoveg.Scripting.DockableControl`),
/// mirroring `ScriptPortal.Vegas.DockableControl` so third-party tools like VPFlow, Datamosh, Vegasaur, and HOS
/// can dock directly inside the Avalonia UI workspace or float on secondary monitors.
/// </summary>
public class DockableControl : Border
{
    public string ExtensionId { get; }
    public string DisplayName { get; set; }
    public Vegas? MyVegas { get; internal set; }
    public bool AutoLoadCommand { get; set; } = true;

    public DockableControl(string extensionId)
    {
        ExtensionId = extensionId;
        DisplayName = extensionId;
    }

    public virtual void OnLoaded(Vegas vegas)
    {
        MyVegas = vegas;
    }
}

/// <summary>
/// Interface implemented by compiled `.dll` Application Extensions (`ICustomCommandModule`).
/// </summary>
public interface ICustomCommandModule
{
    void InitializeModule(Vegas vegas);
    ICollection GetCustomCommands();
}

/// <summary>
/// Globals object passed to Roslyn C# scripts (`Microsoft.CodeAnalysis.CSharp.Scripting`).
/// Allows scripts to use `myVegas`, `vegas`, `Vegas`, or define `public class EntryPoint { public void FromVegas(Vegas myVegas) }`.
/// </summary>
public sealed class ScriptGlobals
{
    public Vegas myVegas { get; }
    public Vegas vegas => myVegas;
    public Vegas Vegas => myVegas;
    public Project Project => myVegas.Project;

    public ScriptGlobals(Vegas vegasInstance)
    {
        myVegas = vegasInstance;
    }
}


