using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Controls;

public enum EventHeaderAction
{
    GeneratedMedia,
    EventPanCrop,
    EventFx,
    HamburgerMenu
}

public sealed record EventHeaderButtonHit(
    Rect Bounds,
    TimelineTrack Track,
    TimelineClip Clip,
    EventHeaderAction Action);

/// <summary>
/// Cross-platform Avalonia UI Custom Control for the VEGAS Pro Multi-Track Timeline.
/// Renders the SMPTE Timecode Ruler, Loop Region, Markers, Regions, Video & Audio Tracks,
/// Clip Events with the authentic VEGAS Pro Event Header (narrow top bar matching track base color,
/// left-aligned media name, and right-aligned white SVG icons: Generated Media, Event Pan/Crop,
/// Video/Audio Event FX, and Event Hamburger Menu ☰), real WAV PCM Waveforms (analyzed via Rust FFI),
/// 5-Curve Fade Envelopes, and Automatic X-Crossfade Overlaps.
/// </summary>
public sealed class MultiTrackTimelineControl : Control
{
    public const double RulerHeight = 28.0;
    public const double RenderStatusRulerHeight = 5.0; // 4px to 6px Selective Prerender & RAM Preview Status Bar
    public const double TotalHeaderHeight = RulerHeight + RenderStatusRulerHeight;
    public const double EventHeaderHeight = 19.0;
    public const double HeaderButtonSlotWidth = 18.0;

    /// <summary>Zoom range: far overview up to sample level (10px per sample at 48 kHz).</summary>
    public const double MinPixelsPerSecond = 15.0;
    public const double MaxPixelsPerSecond = 480_000.0;

    // Small White Vector SVG Geometries (12x12 coordinate space) for the VEGAS Pro Event Header
    private static readonly StreamGeometry SvgGeneratedMedia = StreamGeometry.Parse(
        "M 1.5,2 L 10.5,2 L 10.5,10 L 1.5,10 Z M 3.5,2 L 3.5,10 M 8.5,2 L 8.5,10 M 1.5,4.5 L 3.5,4.5 M 8.5,4.5 L 10.5,4.5 M 1.5,7.5 L 3.5,7.5 M 8.5,7.5 L 10.5,7.5");

    private static readonly StreamGeometry SvgEventPanCrop = StreamGeometry.Parse(
        "M 3.2,0.8 L 3.2,8.8 L 11.2,8.8 M 0.8,3.2 L 8.8,3.2 L 8.8,11.2");

    private static readonly StreamGeometry SvgEventFx = StreamGeometry.Parse(
        "M 1.8,9.8 L 1.8,3.4 C 1.8,2.1 2.5,1.5 3.9,1.5 L 4.9,1.5 M 0.9,5.4 L 4.3,5.4 M 6.2,4.0 L 10.8,9.8 M 10.8,4.0 L 6.2,9.8");

    private static readonly StreamGeometry SvgHamburgerMenu = StreamGeometry.Parse(
        "M 1.5,2.6 L 10.5,2.6 M 1.5,6.0 L 10.5,6.0 M 1.5,9.4 L 10.5,9.4");

    public VegasProject? Project { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public SelectivePrerenderService? PrerenderService { get; set; }
    public PlayheadOverlayCanvas? PlayheadOverlay { get; set; }
    public VegasHistoryTheme ActiveTheme { get; set; } = new();
    public EditToolMode ActiveTool { get; set; } = EditToolMode.Normal;
    public double PixelsPerSecond { get; set; } = 105.0;
    public double HorizontalOffsetSeconds { get; set; }
    public double VerticalOffset { get; set; }
    public string? SelectedClipId { get; set; }
    public string? SelectedTrackId { get; set; }
    public List<CrossfadeOverlap> Crossfades { get; private set; } = new();

    public event Action? OnZoomChanged;
    public event Action<double>? OnPlayheadScrubbed;
    public event Action<TimelineClip?>? OnClipSelected;
    public event Action<string>? OnTimelineMutated;

    // VEGAS Pro Event Header callbacks
    public event Action<TimelineClip, TimelineTrack>? OnGeneratedMediaRequested;
    public event Action<TimelineClip, TimelineTrack>? OnEventPanCropRequested;
    public event Action<TimelineClip, TimelineTrack>? OnEventFxRequested;
    public event Action<TimelineClip, TimelineTrack>? OnClipCutRequested;
    public event Action<TimelineClip, TimelineTrack>? OnClipCopyRequested;
    public event Action<TimelineClip, TimelineTrack>? OnClipPasteRequested;
    public event Action<TimelineClip, TimelineTrack>? OnClipDeleteRequested;
    public event Action<TimelineClip, TimelineTrack>? OnClipPropertiesRequested;

    private readonly List<EventHeaderButtonHit> _headerButtonHits = new();
    private EventHeaderButtonHit? _hoveredHeaderButton;
    private bool _scrubbingRuler;
    private TimelineClip? _draggingClip;
    private double _dragClipOffsetSec;

    public MultiTrackTimelineControl()
    {
        ClipToBounds = true;
    }

    public static bool IsGeneratorEvent(TimelineClip clip)
    {
        return clip.MediaType is "generator" or "title" ||
               !string.IsNullOrWhiteSpace(clip.GeneratorType);
    }

    public static bool IsVideoEvent(TimelineTrack track, TimelineClip clip)
    {
        return track.Type == "video" ||
               clip.MediaType is "video" or "generator" or "title";
    }

    /// <summary>
    /// Resolves the Event Header bar background color so it matches the base color of the track
    /// (e.g. teal-blue #0e7490 for video tracks, burgundy #881337 for audio tracks).
    /// </summary>
    public static Color ResolveTrackHeaderBaseColor(TimelineTrack track)
    {
        if (!string.IsNullOrWhiteSpace(track.TrackAccentColor))
        {
            return TrackColorPaletteService.ParseAccentColor(track.TrackAccentColor);
        }

        if (!string.IsNullOrWhiteSpace(track.Color))
        {
            try
            {
                var parsed = Color.Parse(track.Color);
                // Ensure classic VEGAS distinction if default blue/cyan is still present on audio tracks
                if (track.Type == "audio" && (track.Color.Equals("#0891b2", StringComparison.OrdinalIgnoreCase) ||
                                              track.Color.Equals("#0d9488", StringComparison.OrdinalIgnoreCase)))
                {
                    return Color.Parse("#881337"); // Burgundy / Maroon for Audio
                }
                return parsed;
            }
            catch
            {
                // Fallback below
            }
        }
        return track.Type == "audio"
            ? Color.Parse("#881337") // Bordô / Burgundy for Audio
            : Color.Parse("#0e7490"); // Verde-azulado / Teal-Blue for Video
    }

    /// <summary>
    /// Computes which Event Header buttons should be visible for a given clip width.
    /// - Returns all applicable buttons ([GeneratedMedia?], [EventPanCrop?], [EventFx], [HamburgerMenu]) when width is sufficient.
    /// - Returns ONLY [HamburgerMenu] on very short clips.
    /// </summary>
    public static List<EventHeaderAction> GetVisibleEventHeaderActions(TimelineTrack track, TimelineClip clip, double clipPixelWidth)
    {
        var fullList = new List<EventHeaderAction>(4);
        if (IsGeneratorEvent(clip))
            fullList.Add(EventHeaderAction.GeneratedMedia);
        if (IsVideoEvent(track, clip))
            fullList.Add(EventHeaderAction.EventPanCrop);

        fullList.Add(EventHeaderAction.EventFx);
        fullList.Add(EventHeaderAction.HamburgerMenu);

        double requiredWidthForAll = fullList.Count * HeaderButtonSlotWidth + 12.0;
        if (clipPixelWidth >= requiredWidthForAll)
        {
            return fullList;
        }

        // Very short clip -> show only the Hamburger Menu (☰) aligned to top-right
        if (clipPixelWidth >= HeaderButtonSlotWidth - 2.0)
        {
            return new List<EventHeaderAction> { EventHeaderAction.HamburgerMenu };
        }

        return new List<EventHeaderAction> { EventHeaderAction.HamburgerMenu };
    }

    public void RecalculateCrossfades()
    {
        if (Project != null && RustInterop != null)
        {
            Crossfades = RustInterop.RecalculateAutoCrossfades(Project.Tracks, Project.AutoCrossfadeEnabled);
            if (Project.AutoCrossfadeEnabled && Crossfades.Count > 0)
            {
                foreach (var xf in Crossfades)
                {
                    var tr = Project.Tracks.FirstOrDefault(t => t.Id == xf.TrackId);
                    if (tr == null) continue;
                    var outClip = tr.Clips.FirstOrDefault(c => c.Id == xf.OutgoingClipId);
                    var inClip = tr.Clips.FirstOrDefault(c => c.Id == xf.IncomingClipId);
                    if (outClip != null && xf.Duration > 0.01)
                        outClip.FadeOut = Math.Round(xf.Duration, 3);
                    if (inClip != null && xf.Duration > 0.01)
                        inClip.FadeIn = Math.Round(xf.Duration, 3);
                }
            }
        }
    }

    private double XToTime(double x) =>
        Math.Max(0.0, HorizontalOffsetSeconds + x / Math.Max(20.0, PixelsPerSecond));

    private double TimeToX(double sec) =>
        (sec - HorizontalOffsetSeconds) * Math.Max(20.0, PixelsPerSecond);

    /// <summary>Width in pixels of a time span (durations must not include the horizontal scroll offset).</summary>
    private double DurationToWidth(double seconds) => seconds * Math.Max(20.0, PixelsPerSecond);

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);
        if (Project == null) return;

        var pos = e.GetPosition(this);
        double clickedTime = XToTime(pos.X);

        // 0. Check if user clicked on a VEGAS Pro Event Header icon button (top-right of clip)
        var hitButton = _headerButtonHits.LastOrDefault(h => h.Bounds.Contains(pos));
        if (hitButton != null)
        {
            SelectedTrackId = hitButton.Track.Id;
            SelectedClipId = hitButton.Clip.Id;
            OnClipSelected?.Invoke(hitButton.Clip);

            switch (hitButton.Action)
            {
                case EventHeaderAction.GeneratedMedia:
                    OnGeneratedMediaRequested?.Invoke(hitButton.Clip, hitButton.Track);
                    break;
                case EventHeaderAction.EventPanCrop:
                    OnEventPanCropRequested?.Invoke(hitButton.Clip, hitButton.Track);
                    break;
                case EventHeaderAction.EventFx:
                    OnEventFxRequested?.Invoke(hitButton.Clip, hitButton.Track);
                    break;
                case EventHeaderAction.HamburgerMenu:
                    OpenEventHamburgerContextMenu(hitButton.Clip, hitButton.Track);
                    break;
            }

            e.Handled = true;
            InvalidateVisual();
            return;
        }

        // 1. Clicked on SMPTE Ruler or RenderStatusRuler -> Scrub Playhead
        if (pos.Y <= TotalHeaderHeight)
        {
            _scrubbingRuler = true;
            if (PlayheadOverlay != null)
            {
                PlayheadOverlay.IsScrubbing = true;
                PlayheadOverlay.PixelsPerSecond = PixelsPerSecond;
            }
            e.Pointer.Capture(this);
            Project.Playhead = clickedTime;
            OnPlayheadScrubbed?.Invoke(Project.Playhead);
            PlayheadOverlay?.InvalidateVisual();
            InvalidateVisual();
            return;
        }

        // 2. Find Track and Clip at Y coordinate
        double yCursor = TotalHeaderHeight - VerticalOffset;
        foreach (var track in Project.Tracks)
        {
            double th = Math.Clamp(track.Height, 46.0, 220.0);
            if (pos.Y >= yCursor && pos.Y <= yCursor + th)
            {
                SelectedTrackId = track.Id;
                var hitClip = track.Clips.LastOrDefault(c => clickedTime >= c.Start && clickedTime <= c.End);
                if (hitClip != null)
                {
                    SelectedClipId = hitClip.Id;
                    OnClipSelected?.Invoke(hitClip);

                    // Right-click anywhere on clip also opens the Event Hamburger ContextMenu
                    if (e.GetCurrentPoint(this).Properties.IsRightButtonPressed)
                    {
                        OpenEventHamburgerContextMenu(hitClip, track);
                        e.Handled = true;
                        InvalidateVisual();
                        return;
                    }

                    if (ActiveTool == EditToolMode.Razor && RustInterop != null)
                    {
                        if (RustInterop.TrySplitClip(hitClip, clickedTime, out var rightHalf))
                        {
                            track.Clips.Add(rightHalf);
                            SelectedClipId = rightHalf.Id;
                            PrerenderService?.InvalidateCacheRange(hitClip.Start, rightHalf.End, Project.Fps);
                            RecalculateCrossfades();
                            OnTimelineMutated?.Invoke($"Clip dividido via Rust FFI em {RustInterop.FormatSmpte(clickedTime, Project.Fps)} (Cache invalidado -> Vermelho #E74C3C)");
                        }
                    }
                    else if (!hitClip.Locked && !track.Locked)
                    {
                        _draggingClip = hitClip;
                        _dragClipOffsetSec = clickedTime - hitClip.Start;
                        e.Pointer.Capture(this);
                    }
                }
                else
                {
                    SelectedClipId = null;
                    Project.Playhead = clickedTime;
                    OnPlayheadScrubbed?.Invoke(Project.Playhead);
                    OnClipSelected?.Invoke(null);
                    PlayheadOverlay?.InvalidateVisual();
                }
                InvalidateVisual();
                return;
            }
            yCursor += th;
        }

        Project.Playhead = clickedTime;
        OnPlayheadScrubbed?.Invoke(Project.Playhead);
        PlayheadOverlay?.InvalidateVisual();
        InvalidateVisual();
    }

    private void OpenEventHamburgerContextMenu(TimelineClip clip, TimelineTrack track)
    {
        var loc = LocalizationManager.Instance;
        var menu = new ContextMenu();
        var items = new List<object>();

        var miCut = new MenuItem { Header = loc.T("EventHeader.Menu.Cut", "Cortar (Cut)") };
        miCut.Click += (_, _) => OnClipCutRequested?.Invoke(clip, track);
        items.Add(miCut);

        var miCopy = new MenuItem { Header = loc.T("EventHeader.Menu.Copy", "Copiar (Copy)") };
        miCopy.Click += (_, _) => OnClipCopyRequested?.Invoke(clip, track);
        items.Add(miCopy);

        var miPaste = new MenuItem { Header = loc.T("EventHeader.Menu.Paste", "Colar (Paste)") };
        miPaste.Click += (_, _) => OnClipPasteRequested?.Invoke(clip, track);
        items.Add(miPaste);

        items.Add(new Separator());

        if (IsGeneratorEvent(clip))
        {
            var miGen = new MenuItem { Header = loc.T("EventHeader.GeneratedMedia", "Generated Media...") };
            miGen.Click += (_, _) => OnGeneratedMediaRequested?.Invoke(clip, track);
            items.Add(miGen);
        }

        if (IsVideoEvent(track, clip))
        {
            var miPanCrop = new MenuItem { Header = loc.T("EventHeader.EventPanCrop", "Event Pan/Crop...") };
            miPanCrop.Click += (_, _) => OnEventPanCropRequested?.Invoke(clip, track);
            items.Add(miPanCrop);
        }

        var miFx = new MenuItem
        {
            Header = track.Type == "audio"
                ? loc.T("EventHeader.AudioEventFx", "Audio Event FX...")
                : loc.T("EventHeader.VideoEventFx", "Video Event FX...")
        };
        miFx.Click += (_, _) => OnEventFxRequested?.Invoke(clip, track);
        items.Add(miFx);

        items.Add(new Separator());

        var miDelete = new MenuItem { Header = loc.T("EventHeader.Menu.Delete", "Excluir (Delete)") };
        miDelete.Click += (_, _) => OnClipDeleteRequested?.Invoke(clip, track);
        items.Add(miDelete);

        var miProps = new MenuItem { Header = loc.T("EventHeader.Menu.Properties", "Propriedades (Properties)...") };
        miProps.Click += (_, _) => OnClipPropertiesRequested?.Invoke(clip, track);
        items.Add(miProps);

        menu.ItemsSource = items;
        ContextMenu = menu;
        menu.Open(this);
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);
        if (Project == null) return;
        var pos = e.GetPosition(this);
        double t = XToTime(pos.X);

        if (!_scrubbingRuler && _draggingClip == null)
        {
            var hovered = _headerButtonHits.LastOrDefault(h => h.Bounds.Contains(pos));
            if (!Equals(hovered, _hoveredHeaderButton))
            {
                _hoveredHeaderButton = hovered;
                Cursor = hovered != null ? new Cursor(StandardCursorType.Hand) : Cursor.Default;
                ToolTip.SetTip(this, hovered != null ? GetEventHeaderButtonTooltip(hovered) : null);
                InvalidateVisual();
            }
        }

        if (_scrubbingRuler)
        {
            Project.Playhead = t;
            OnPlayheadScrubbed?.Invoke(Project.Playhead);
            if (PlayheadOverlay != null)
            {
                PlayheadOverlay.IsScrubbing = true;
                PlayheadOverlay.PixelsPerSecond = PixelsPerSecond;
                PlayheadOverlay.InvalidateVisual();
            }
            else
            {
                InvalidateVisual();
            }
            return;
        }

        if (_draggingClip != null)
        {
            double oldStart = _draggingClip.Start;
            double newStart = Math.Max(0.0, t - _dragClipOffsetSec);
            if (Project.SnapEnabled)
            {
                if (Math.Abs(newStart - Project.Playhead) < 0.08)
                    newStart = Project.Playhead;
            }
            newStart = Math.Round(newStart, 6);
            if (Project.SnapEnabled)
            {
                const double snapThresholdSec = 0.12;
                if (Math.Abs(newStart - Project.Playhead) <= snapThresholdSec)
                    newStart = Project.Playhead;
                else
                {
                    foreach (var mk in Project.Markers)
                    {
                        if (Math.Abs(newStart - mk.Time) <= snapThresholdSec)
                        {
                            newStart = mk.Time;
                            break;
                        }
                    }
                }
            }
            double delta = newStart - oldStart;
            _draggingClip.Start = newStart;

            // Synchronize event-level GroupId members (grouped via shortcut G)
            if (!string.IsNullOrEmpty(_draggingClip.GroupId) && Math.Abs(delta) > 1e-6)
            {
                foreach (var tr in Project.Tracks.Where(t => !t.Locked))
                {
                    foreach (var grpClip in tr.Clips.Where(c => c.Id != _draggingClip.Id && c.GroupId == _draggingClip.GroupId && !c.Locked))
                    {
                        grpClip.Start = Math.Max(0.0, Math.Round(grpClip.Start + delta, 6));
                    }
                }
            }

            // Synchronize linked V+A grouped tracks with sub-frame precision
            var ownerTrack = Project.Tracks.FirstOrDefault(tr => tr.Clips.Contains(_draggingClip));
            if (ownerTrack != null && !string.IsNullOrEmpty(ownerTrack.GroupId) && Math.Abs(delta) > 1e-6)
            {
                foreach (var sibling in Project.Tracks.Where(tr => tr.Id != ownerTrack.Id && tr.GroupId == ownerTrack.GroupId && !tr.Locked))
                {
                    foreach (var linkedClip in sibling.Clips.Where(c => !c.Locked && Math.Abs(c.Start - oldStart) < 0.25))
                    {
                        linkedClip.Start = Math.Max(0.0, Math.Round(linkedClip.Start + delta, 6));
                    }
                }
            }

            if (Math.Abs(delta) > 1e-6)
            {
                double minSec = Math.Min(oldStart, newStart);
                double maxSec = Math.Max(oldStart, newStart) + _draggingClip.Duration;
                PrerenderService?.InvalidateCacheRange(minSec, maxSec, Project.Fps);
            }

            RecalculateCrossfades();
            InvalidateVisual();
        }
    }

    private static string GetEventHeaderButtonTooltip(EventHeaderButtonHit hit)
    {
        var loc = LocalizationManager.Instance;
        return hit.Action switch
        {
            EventHeaderAction.GeneratedMedia => $"{loc.T("EventHeader.GeneratedMedia", "Generated Media")} ({hit.Clip.Name})",
            EventHeaderAction.EventPanCrop => $"{loc.T("EventHeader.EventPanCrop", "Event Pan/Crop")} ({hit.Clip.Name})",
            EventHeaderAction.EventFx => hit.Track.Type == "audio"
                ? $"{loc.T("EventHeader.AudioEventFx", "Audio Event FX")} ({hit.Clip.Fx.Count} • {hit.Clip.Name})"
                : $"{loc.T("EventHeader.VideoEventFx", "Video Event FX")} ({hit.Clip.Fx.Count} • {hit.Clip.Name})",
            EventHeaderAction.HamburgerMenu => $"{loc.T("EventHeader.HamburgerMenu", "Menu do Evento ☰")} ({hit.Clip.Name})",
            _ => hit.Clip.Name
        };
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        if (_scrubbingRuler || _draggingClip != null)
        {
            _scrubbingRuler = false;
            if (PlayheadOverlay != null)
            {
                PlayheadOverlay.IsScrubbing = false;
                PlayheadOverlay.InvalidateVisual();
            }
            if (_draggingClip != null)
            {
                OnTimelineMutated?.Invoke($"Evento \"{_draggingClip.Name}\" posicionado em {_draggingClip.Start:0.00}s");
                _draggingClip = null;
            }
            e.Pointer.Capture(null);
            InvalidateVisual();
        }
    }

    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        base.OnPointerWheelChanged(e);
        if (e.KeyModifiers.HasFlag(KeyModifiers.Alt) && Project != null)
        {
            double deltaH = e.Delta.Y > 0 ? 10.0 : -10.0;
            foreach (var tr in Project.Tracks)
            {
                tr.Height = Math.Clamp(tr.Height + deltaH, 46.0, 220.0);
            }
            OnTimelineMutated?.Invoke("Altura das faixas ajustada (Alt + Scroll)");
            PlayheadOverlay?.InvalidateVisual();
            InvalidateVisual();
        }
        else
        {
            // Zoom anchored at the mouse pointer: the time under the cursor stays under the cursor
            double factor = e.Delta.Y > 0 ? 1.15 : 0.87;
            double anchorX = e.GetPosition(this).X;
            double anchorTime = XToTime(anchorX);
            PixelsPerSecond = Math.Clamp(PixelsPerSecond * factor, MinPixelsPerSecond, MaxPixelsPerSecond);
            HorizontalOffsetSeconds = Math.Max(0.0, anchorTime - anchorX / Math.Max(20.0, PixelsPerSecond));
            if (PlayheadOverlay != null)
                PlayheadOverlay.HorizontalOffsetSeconds = HorizontalOffsetSeconds;
            if (PlayheadOverlay != null)
            {
                PlayheadOverlay.PixelsPerSecond = PixelsPerSecond;
                PlayheadOverlay.InvalidateVisual();
            }
            OnZoomChanged?.Invoke();
            InvalidateVisual();
        }
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        _headerButtonHits.Clear();

        var b = Bounds;
        if (b.Width < 10 || b.Height < 10) return;

        // Background
        context.FillRectangle(new SolidColorBrush(Color.Parse(ActiveTheme.TimelineBg)), b);

        if (Project == null) return;

        // 1. Draw Top SMPTE Ruler
        var rulerRect = new Rect(0, 0, b.Width, RulerHeight);
        context.FillRectangle(new SolidColorBrush(Color.Parse(ActiveTheme.RulerBg)), rulerRect);
        context.DrawLine(new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.DockBorder)), 1.0), new Point(0, RulerHeight), new Point(b.Width, RulerHeight));

        // Loop Region highlight on ruler
        if (Project.LoopEnabled && Project.LoopEnd > Project.LoopStart)
        {
            double lx1 = TimeToX(Project.LoopStart);
            double lx2 = TimeToX(Project.LoopEnd);
            var accCol = Color.Parse(ActiveTheme.AccentPrimary);
            var loopRect = new Rect(lx1, 0, Math.Max(4, lx2 - lx1), RulerHeight);
            context.FillRectangle(new SolidColorBrush(Color.FromArgb(65, accCol.R, accCol.G, accCol.B)), loopRect);
            context.DrawLine(new Pen(new SolidColorBrush(accCol), 2.0), new Point(lx1, 0), new Point(lx2, 0));
        }

        // Adaptive frame/second grid (zoom-aware) aligned with the SMPTE ruler labels
        double renderScaling = TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0;
        var grid = TimelineFrameGrid.Compute(Project.Fps, PixelsPerSecond, HorizontalOffsetSeconds, b.Width);
        var minorTickPen = new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.TextSecondary)), 1.0 / renderScaling);
        var majorTickPen = new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.TextPrimary)), 1.0 / renderScaling);

        using (context.PushRenderOptions(new RenderOptions { EdgeMode = EdgeMode.Aliased }))
        {
            foreach (double t in grid.MinorTimes)
            {
                double x = TimelineFrameGrid.SnapToDevicePixel(TimeToX(t), renderScaling);
                context.DrawLine(minorTickPen, new Point(x, RulerHeight - 4), new Point(x, RulerHeight));
            }
            foreach (double t in grid.MajorTimes)
            {
                double x = TimelineFrameGrid.SnapToDevicePixel(TimeToX(t), renderScaling);
                context.DrawLine(majorTickPen, new Point(x, RulerHeight - 10), new Point(x, RulerHeight));
            }
        }

        foreach (double t in grid.MajorTimes)
        {
            double x = TimeToX(t);
            // Quarter-frame bias: t is an exact frame boundary, floating-point error must not show the previous frame
            double labelT = t + 0.25 / Math.Max(1.0, Project.Fps);
            string tc = RustInterop != null
                ? RustInterop.FormatSmpte(labelT, Project.Fps, true)
                : PlayheadOverlayCanvas.FormatPlayheadSmpte(labelT, Project.Fps, null);
            DrawText(context, tc, new Point(x + 4, 6), 9.5, Color.Parse(ActiveTheme.TextPrimary), false);
        }

        // Markers on Ruler
        foreach (var mk in Project.Markers)
        {
            double mx = TimeToX(mk.Time);
            var mCol = Color.Parse(mk.Color);
            context.DrawLine(new Pen(new SolidColorBrush(mCol), 1.3), new Point(mx, 0), new Point(mx, b.Height));
            context.FillRectangle(new SolidColorBrush(mCol), new Rect(mx, 2, 62, 13));
            DrawText(context, mk.Label, new Point(mx + 3, 2), 8.5, Colors.Black, true);
        }

        // 1B. Draw Selective Prerender & Dynamic RAM Preview Status Bar (RenderStatusRuler: 5px height immediately below SMPTE Ruler and above Track 1)
        var statusBarRect = new Rect(0, RulerHeight, b.Width, RenderStatusRulerHeight);
        RenderStatusRuler.DrawStatusBarSegments(context, statusBarRect, Project, PrerenderService, PixelsPerSecond);

        // 2. Draw Tracks & Clip Events
        double y = TotalHeaderHeight - VerticalOffset;
        for (int tIdx = 0; tIdx < Project.Tracks.Count; tIdx++)
        {
            var track = Project.Tracks[tIdx];
            double th = Math.Clamp(track.Height, 46.0, 220.0);
            var laneRect = new Rect(0, y, b.Width, th);

            var laneBg = track.Type == "video" ? Color.Parse(ActiveTheme.TrackVideoBg) : Color.Parse(ActiveTheme.TrackAudioBg);
            context.FillRectangle(new SolidColorBrush(laneBg), laneRect);
            DrawGridLinesInLane(context, grid, laneRect, renderScaling);

            // Track Accent Color left 4px indicator on lane
            var trackAccentCol = ResolveTrackHeaderBaseColor(track);
            context.FillRectangle(new SolidColorBrush(Color.FromArgb(185, trackAccentCol.R, trackAccentCol.G, trackAccentCol.B)), new Rect(0, y, 4, th));

            // Track Group left bracket indicator on lane
            if (!string.IsNullOrEmpty(track.GroupId))
            {
                var grpCol = Color.Parse(track.GroupColor ?? "#f59e0b");
                context.FillRectangle(new SolidColorBrush(grpCol), new Rect(4, y, 3, th));
            }

            foreach (var clip in track.Clips)
            {
                DrawClipEvent(context, track, clip, y, th);
            }

            // Draw Auto-Crossfade X-Lines on this track
            foreach (var xf in Crossfades.Where(x => x.TrackId == track.Id))
            {
                double x1 = TimeToX(xf.Start);
                double x2 = TimeToX(xf.End);
                var xfRect = new Rect(x1, y + 4, Math.Max(4, x2 - x1), th - 8);
                var xfPen = new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.AccentSecondary)), 1.5);
                context.DrawLine(xfPen, xfRect.TopLeft, xfRect.BottomRight);
                context.DrawLine(xfPen, xfRect.BottomLeft, xfRect.TopRight);
            }

            // Bottom track separator line
            context.DrawLine(
                new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.DockBorder)), 1.0),
                new Point(0, y + th),
                new Point(b.Width, y + th));

            y += th;
        }

        // 3. Draw Playhead Cursor (Premiere Pro style blue pentagonal CTI with flag, 1px guide line, and SMPTE readout)
        if (PlayheadOverlay != null)
        {
            PlayheadOverlay.Project = Project;
            PlayheadOverlay.RustInterop = RustInterop;
            PlayheadOverlay.PixelsPerSecond = PixelsPerSecond;
            PlayheadOverlay.HorizontalOffsetSeconds = HorizontalOffsetSeconds;
            PlayheadOverlay.RulerHeight = RulerHeight;
        }
        else
        {
            double px = TimeToX(Project.Playhead);
            if (px < 0.0 || px > b.Width)
                return;

            PlayheadOverlayCanvas.DrawPremierePlayhead(
                context,
                px,
                b.Width,
                b.Height,
                RulerHeight,
                Project.Playhead,
                Project.Fps,
                RustInterop,
                null,
                1.0,
                renderScaling);
        }
    }

    private static readonly Pen GridMinorPenTemplate = new(new SolidColorBrush(TimelineFrameGrid.MinorLineColor), 1.0);
    private static readonly Pen GridMajorPenTemplate = new(new SolidColorBrush(TimelineFrameGrid.MajorLineColor), 1.0);

    /// <summary>
    /// Draws the frame grid behind the events of one lane: minor lines (frames or grouped intervals, #2C3038)
    /// and major lines aligned with the ruler labels (#4A505E), 1 device pixel wide and pixel-snapped.
    /// </summary>
    private void DrawGridLinesInLane(DrawingContext context, TimelineFrameGrid grid, Rect laneRect, double renderScaling)
    {
        double thickness = 1.0 / renderScaling;
        var minorPen = Math.Abs(thickness - 1.0) < 1e-6 ? GridMinorPenTemplate : new Pen(GridMinorPenTemplate.Brush, thickness);
        var majorPen = Math.Abs(thickness - 1.0) < 1e-6 ? GridMajorPenTemplate : new Pen(GridMajorPenTemplate.Brush, thickness);

        using (context.PushRenderOptions(new RenderOptions { EdgeMode = EdgeMode.Aliased }))
        {
            foreach (double t in grid.MinorTimes)
            {
                double x = TimelineFrameGrid.SnapToDevicePixel(TimeToX(t), renderScaling);
                context.DrawLine(minorPen, new Point(x, laneRect.Top), new Point(x, laneRect.Bottom));
            }
            foreach (double t in grid.MajorTimes)
            {
                double x = TimelineFrameGrid.SnapToDevicePixel(TimeToX(t), renderScaling);
                context.DrawLine(majorPen, new Point(x, laneRect.Top), new Point(x, laneRect.Bottom));
            }
        }
    }

    private void DrawClipEvent(DrawingContext context, TimelineTrack track, TimelineClip clip, double laneY, double laneH)
    {
        double x = TimeToX(clip.Start);
        double w = Math.Max(18.0, DurationToWidth(clip.Duration));
        // Skip events outside the viewport; at deep (sample-level) zoom an event can be hundreds of millions of
        // pixels wide, so the drawn rectangle is limited to ±10,000px around the viewport (float precision in Skia).
        // The true start X is kept for the sample-accurate waveform mapping.
        if (x + w < -1.0 || x > Bounds.Width + 1.0)
            return;
        const double drawMarginPx = 10_000.0;
        double drawLeft = Math.Max(x, -drawMarginPx);
        double drawRight = Math.Min(x + w, Bounds.Width + drawMarginPx);
        double trueClipStartX = x;
        var clipRect = new Rect(drawLeft, laneY + 4, Math.Max(1.0, drawRight - drawLeft), laneH - 8);

        var trackBaseCol = clip.IsOffline
            ? Color.Parse("#b91c1c")
            : ResolveTrackHeaderBaseColor(track);
        string accentHex = $"#{trackBaseCol.R:X2}{trackBaseCol.G:X2}{trackBaseCol.B:X2}";
        bool isSelected = SelectedClipId == clip.Id;

        // Clip body area (soft inherited tint from TrackAccentColor via TrackColorPaletteService when PropagateTintToClips is enabled)
        var bodyCol = clip.IsOffline
            ? Color.FromArgb(clip.Muted ? (byte)110 : (byte)215, 127, 29, 29)
            : (track.PropagateTintToClips
                ? TrackColorPaletteService.GetClipBodyTint(accentHex, clip.Muted)
                : Color.FromArgb(
                    clip.Muted ? (byte)75 : (byte)155,
                    (byte)Math.Min(255, trackBaseCol.R + 24),
                    (byte)Math.Min(255, trackBaseCol.G + 28),
                    (byte)Math.Min(255, trackBaseCol.B + 32)));
        context.FillRectangle(new SolidColorBrush(bodyCol), clipRect);

        // VEGAS Pro Event Header: Narrow horizontal bar at the top of the clip matching the track's accent tint
        double hdrHeight = Math.Min(EventHeaderHeight, Math.Max(14.0, clipRect.Height * 0.36));
        var hdrRect = new Rect(clipRect.X, clipRect.Y, clipRect.Width, hdrHeight);

        var headerBgBrush = new SolidColorBrush(
            track.PropagateTintToClips && !clip.IsOffline
                ? TrackColorPaletteService.GetClipHeaderTint(accentHex, clip.Muted)
                : Color.FromArgb(
                    clip.Muted ? (byte)160 : (byte)245,
                    trackBaseCol.R,
                    trackBaseCol.G,
                    trackBaseCol.B));
        context.FillRectangle(headerBgBrush, hdrRect);

        // Subtle separator line below Event Header
        var hdrBottomPen = new Pen(new SolidColorBrush(Color.FromArgb(95, 0, 0, 0)), 1.0);
        context.DrawLine(hdrBottomPen, new Point(hdrRect.Left, hdrRect.Bottom), new Point(hdrRect.Right, hdrRect.Bottom));

        using (context.PushClip(clipRect))
        {
            // Determine visible Event Header buttons (all applicable buttons vs only Hamburger Menu on short clips)
            var visibleActions = GetVisibleEventHeaderActions(track, clip, clipRect.Width);
            double buttonsTotalWidth = visibleActions.Count * HeaderButtonSlotWidth;
            double buttonsStartX = Math.Max(clipRect.X, clipRect.Right - buttonsTotalWidth - 1.0);

            // 1. Draw Media / File Name aligned left inside the Event Header if there is enough room
            double availableTextWidth = buttonsStartX - (clipRect.X + 6.0);
            if (availableTextWidth >= 24.0)
            {
                var textClipRect = new Rect(clipRect.X + 4.0, hdrRect.Y, availableTextWidth, hdrRect.Height);
                using (context.PushClip(textClipRect))
                {
                    DrawText(
                        context,
                        clip.Name,
                        new Point(clipRect.X + 6.0, hdrRect.Y + 2.5),
                        9.5,
                        Colors.White,
                        true);
                }
            }

            // 2. Draw Small White SVG Vector Icon Buttons flush at the top-right edge of the Event Header
            var whiteIconPen = new Pen(Brushes.White, 1.35)
            {
                LineCap = PenLineCap.Round,
                LineJoin = PenLineJoin.Round
            };
            var activeFxIconPen = new Pen(new SolidColorBrush(Color.Parse("#a7f3d0")), 1.45)
            {
                LineCap = PenLineCap.Round,
                LineJoin = PenLineJoin.Round
            };

            for (int i = 0; i < visibleActions.Count; i++)
            {
                var action = visibleActions[i];
                double bx = buttonsStartX + i * HeaderButtonSlotWidth;
                var btnRect = new Rect(bx, hdrRect.Y, HeaderButtonSlotWidth, hdrRect.Height);
                var hitRecord = new EventHeaderButtonHit(btnRect, track, clip, action);
                _headerButtonHits.Add(hitRecord);

                bool isHovered = _hoveredHeaderButton != null &&
                                 _hoveredHeaderButton.Clip.Id == clip.Id &&
                                 _hoveredHeaderButton.Action == action;

                if (isHovered)
                {
                    context.FillRectangle(new SolidColorBrush(Color.FromArgb(75, 255, 255, 255)), btnRect);
                }

                // Active FX subtle pill highlight when clip has active FX
                if (action == EventHeaderAction.EventFx && clip.Fx.Count > 0)
                {
                    context.FillRectangle(new SolidColorBrush(Color.FromArgb(55, 16, 185, 129)), btnRect.Deflate(1.5));
                }

                var iconGeo = action switch
                {
                    EventHeaderAction.GeneratedMedia => SvgGeneratedMedia,
                    EventHeaderAction.EventPanCrop => SvgEventPanCrop,
                    EventHeaderAction.EventFx => SvgEventFx,
                    _ => SvgHamburgerMenu
                };

                var penToUse = (action == EventHeaderAction.EventFx && clip.Fx.Count > 0)
                    ? activeFxIconPen
                    : whiteIconPen;

                // Center the 12x12 SVG icon inside the 18x19 button slot
                double iconOffsetX = btnRect.X + (btnRect.Width - 12.0) * 0.5;
                double iconOffsetY = btnRect.Y + (btnRect.Height - 12.0) * 0.5;
                using (context.PushTransform(Matrix.CreateTranslation(iconOffsetX, iconOffsetY)))
                {
                    context.DrawGeometry(null, penToUse, iconGeo);
                }
            }

            // 3. Offline Media Red Slate Banner or Audio Waveform (from Rust FFI DSP Peaks / .sfk Cache)
            if (clip.IsOffline)
            {
                double bodyY = clipRect.Y + hdrRect.Height;
                double bodyH = Math.Max(8.0, clipRect.Height - hdrRect.Height);
                var diagPen = new Pen(new SolidColorBrush(Color.FromArgb(165, 248, 113, 113)), 1.2);
                context.DrawLine(diagPen, new Point(clipRect.X, bodyY), new Point(clipRect.Right, clipRect.Bottom));
                context.DrawLine(diagPen, new Point(clipRect.X, clipRect.Bottom), new Point(clipRect.Right, bodyY));
                if (clipRect.Width >= 70 && bodyH >= 16)
                {
                    DrawText(
                        context,
                        "MEDIA OFFLINE",
                        new Point(clipRect.X + 8.0, bodyY + (bodyH - 12.0) * 0.5),
                        9.5,
                        Color.Parse("#fecaca"),
                        true);
                }
            }
            else if (track.Type == "audio")
            {
                double bodyY = clipRect.Y + hdrRect.Height;
                double bodyH = Math.Max(8.0, clipRect.Height - hdrRect.Height);
                var waveBodyRect = new Rect(clipRect.X, bodyY, clipRect.Width, bodyH);
                var wvCol = Color.Parse(ActiveTheme.WaveformColor);
                double trackDb = 20.0 * Math.Log10(Math.Max(0.001, track.Level));
                AudioWaveformRenderer.DrawStereoAudacityWaveform(
                    context,
                    waveBodyRect,
                    clip,
                    trackDb,
                    wvCol,
                    RustInterop,
                    InvalidateVisual,
                    visibleLeft: 0.0,
                    visibleRight: Bounds.Width,
                    pixelsPerSecond: Math.Max(20.0, PixelsPerSecond),
                    clipStartX: trueClipStartX);
            }

            // 4. Sub-frame 5-Curve Fade-In & Fade-Out Envelopes (evaluated via Rust FFI) + Top Corner Handles + Opacity/Gain Line
            var fadePen = new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.AccentSecondary)), 1.6);
            const int curveSegs = 12;
            // Fades use the true event edges (clipRect may be limited to the viewport margin at deep zoom)
            double trueClipRightX = trueClipStartX + w;
            double fadeInEndX = trueClipStartX;
            if (clip.FadeIn > 0.005)
            {
                double fw = Math.Min(w * 0.5, DurationToWidth(clip.FadeIn));
                fadeInEndX = trueClipStartX + fw;
                Point prevPt = new Point(trueClipStartX, clipRect.Bottom);
                for (int seg = 1; seg <= curveSegs; seg++)
                {
                    double p = seg / (double)curveSegs;
                    double env = RustInterop != null
                        ? RustInterop.EvaluateFadeCurve(clip.FadeInCurve, p)
                        : p;
                    var nextPt = new Point(trueClipStartX + fw * p, clipRect.Bottom - env * clipRect.Height);
                    context.DrawLine(fadePen, prevPt, nextPt);
                    prevPt = nextPt;
                }
            }

            double fadeOutStartX = trueClipRightX;
            if (clip.FadeOut > 0.005)
            {
                double fw = Math.Min(w * 0.5, DurationToWidth(clip.FadeOut));
                double startX = trueClipRightX - fw;
                fadeOutStartX = startX;
                Point prevPt = new Point(startX, clipRect.Y);
                for (int seg = 1; seg <= curveSegs; seg++)
                {
                    double p = seg / (double)curveSegs;
                    double env = RustInterop != null
                        ? RustInterop.EvaluateFadeCurve(clip.FadeOutCurve, 1.0 - p)
                        : (1.0 - p);
                    var nextPt = new Point(startX + fw * p, clipRect.Bottom - env * clipRect.Height);
                    context.DrawLine(fadePen, prevPt, nextPt);
                    prevPt = nextPt;
                }
            }

            // 5. VEGAS Pro Top-Left & Top-Right Fade In / Fade Out Corner Handles
            DrawFadeCornerHandles(context, clipRect, fadeInEndX, fadeOutStartX);

            // 6. VEGAS Pro Opacity / Gain Envelope Line across the event body
            DrawOpacityOrGainEnvelopeLine(context, track, clip, clipRect, hdrRect.Height);
        }

        // Era-aware Clip Border:
        // Era 1 (Sonic Foundry Vegas 1-3): Classic 3D Beveled Chamfer Border
        if (ActiveTheme.EraNumber == 1)
        {
            var bevelLight = new Pen(new SolidColorBrush(Color.Parse("#F5F3EE")), 1.5);
            var bevelDark = new Pen(new SolidColorBrush(Color.Parse("#2B2A27")), 1.5);
            context.DrawLine(bevelLight, clipRect.TopLeft, clipRect.TopRight);
            context.DrawLine(bevelLight, clipRect.TopLeft, clipRect.BottomLeft);
            context.DrawLine(bevelDark, clipRect.BottomLeft, clipRect.BottomRight);
            context.DrawLine(bevelDark, clipRect.TopRight, clipRect.BottomRight);
            if (isSelected)
            {
                context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse(ActiveTheme.AccentPrimary)), 2.0), clipRect.Deflate(1.5));
            }
        }
        else
        {
            var borderColor = track.PropagateTintToClips && !clip.IsOffline
                ? TrackColorPaletteService.GetClipBorderTint(accentHex, isSelected)
                : (isSelected ? Colors.White : Color.Parse(ActiveTheme.DockBorder));
            context.DrawRectangle(new Pen(new SolidColorBrush(borderColor), isSelected ? 1.8 : 1.15), clipRect);
        }
    }

    private static void DrawFadeCornerHandles(DrawingContext context, Rect clipRect, double fadeInEndX, double fadeOutStartX)
    {
        var handleBrush = new SolidColorBrush(Color.FromArgb(225, 250, 204, 21));
        var handleBorderPen = new Pen(new SolidColorBrush(Color.FromArgb(220, 15, 23, 42)), 1.0);

        // Top-left Fade-In handle
        double leftHandleX = Math.Clamp(fadeInEndX - 3.5, clipRect.Left + 1.0, clipRect.Right - 8.0);
        var leftRect = new Rect(leftHandleX, clipRect.Top + 1.0, 6.5, 6.5);
        context.FillRectangle(handleBrush, leftRect);
        context.DrawRectangle(handleBorderPen, leftRect);

        // Top-right Fade-Out handle
        double rightHandleX = Math.Clamp(fadeOutStartX - 3.5, clipRect.Left + 8.0, clipRect.Right - 7.5);
        var rightRect = new Rect(rightHandleX, clipRect.Top + 1.0, 6.5, 6.5);
        context.FillRectangle(handleBrush, rightRect);
        context.DrawRectangle(handleBorderPen, rightRect);
    }

    private static void DrawOpacityOrGainEnvelopeLine(DrawingContext context, TimelineTrack track, TimelineClip clip, Rect clipRect, double hdrHeight)
    {
        if (track.Type == "audio")
            return; // Audio clips already draw their Gain Line inside AudioWaveformRenderer

        double bodyTop = clipRect.Top + hdrHeight + 2.0;
        double bodyBottom = clipRect.Bottom - 2.0;
        double bodyHeight = Math.Max(6.0, bodyBottom - bodyTop);
        double normOpacity = Math.Clamp(clip.Opacity, 0.0, 1.0);
        double lineY = bodyBottom - normOpacity * bodyHeight;

        var opacityPen = new Pen(new SolidColorBrush(Color.FromArgb(215, 56, 189, 248)), 1.5);
        context.DrawLine(opacityPen, new Point(clipRect.Left + 2.0, lineY), new Point(clipRect.Right - 2.0, lineY));
    }

    private static void DrawText(DrawingContext context, string text, Point origin, double size, Color color, bool bold)
    {
        var ft = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Inter, Segoe UI, sans-serif", FontStyle.Normal, bold ? FontWeight.Bold : FontWeight.Normal),
            size,
            new SolidColorBrush(color));
        context.DrawText(ft, origin);
    }
}
