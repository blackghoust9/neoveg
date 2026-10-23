using System;
using System.Collections.Generic;
using Avalonia.Media;

namespace Neoveg.Controls;

/// <summary>
/// Zoom-adaptive timeline frame grid (see docs/research/vegas_features_research.txt, entry 4):
/// - Close zoom: one minor line per frame (1/24, 1/25, 1/30, 1/60 s... from the project FPS).
/// - Medium/far zoom: lines grouped into 2/5/10 frames, then 1 s, 2 s, 5 s, 10 s, 15 s, 30 s, 1 min, 2 min, 5 min...
/// - Major lines carry the ruler labels; minor lines are the subtle subdivisions between them.
/// Grid times are always derived from integer frame/step indices (never accumulated) so lines stay on exact
/// frame boundaries at any zoom or scroll position.
/// </summary>
public sealed class TimelineFrameGrid
{
    public static readonly Color MinorLineColor = Color.Parse("#2C3038");
    public static readonly Color MajorLineColor = Color.Parse("#4A505E");

    public const double MinMinorSpacingPx = 8.0;
    public const double MinMajorSpacingPx = 90.0;
    private const int MaxLinesPerList = 4096;

    private static readonly int[] FrameStepLadder = { 1, 2, 5, 10 };
    private static readonly double[] SecondStepLadder = { 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600 };

    public List<double> MinorTimes { get; } = new();
    public List<double> MajorTimes { get; } = new();

    /// <summary>Grid step expressed either in frames or in seconds.</summary>
    public readonly record struct GridStep(bool IsFrameStep, double Count)
    {
        public double Seconds(double fps) => IsFrameStep ? Count / fps : Count;
    }

    public static TimelineFrameGrid Compute(double projectFps, double pixelsPerSecond, double startSeconds, double viewportWidthPx)
    {
        double fps = projectFps > 1.0 ? projectFps : 29.97;
        // Same lower bound as MultiTrackTimelineControl.TimeToX
        double pps = Math.Max(20.0, pixelsPerSecond);
        double tStart = Math.Max(0.0, startSeconds);
        double tEnd = startSeconds + Math.Max(0.0, viewportWidthPx) / pps;

        var minor = ChooseStep(fps, pps, MinMinorSpacingPx);
        var major = ChooseStep(fps, pps, MinMajorSpacingPx);
        if (major.Seconds(fps) < minor.Seconds(fps))
            major = minor;

        var grid = new TimelineFrameGrid();
        AppendTimes(grid.MajorTimes, major, fps, tStart, tEnd);

        // Minor lines that coincide with a major line are drawn only once (as major)
        var minorAll = new List<double>();
        AppendTimes(minorAll, minor, fps, tStart, tEnd);
        double halfPixelSec = 0.5 / pps;
        int mi = 0;
        foreach (double t in minorAll)
        {
            while (mi < grid.MajorTimes.Count && grid.MajorTimes[mi] < t - halfPixelSec) mi++;
            bool onMajor = mi < grid.MajorTimes.Count && Math.Abs(grid.MajorTimes[mi] - t) <= halfPixelSec;
            if (!onMajor)
                grid.MinorTimes.Add(t);
        }
        return grid;
    }

    /// <summary>Smallest step (frames first, then seconds) whose on-screen spacing is at least <paramref name="minSpacingPx"/>.</summary>
    public static GridStep ChooseStep(double fps, double pixelsPerSecond, double minSpacingPx)
    {
        foreach (int frames in FrameStepLadder)
        {
            if (frames >= fps) break;
            if (frames / fps * pixelsPerSecond >= minSpacingPx)
                return new GridStep(true, frames);
        }
        foreach (double sec in SecondStepLadder)
        {
            if (sec * pixelsPerSecond >= minSpacingPx)
                return new GridStep(false, sec);
        }
        return new GridStep(false, SecondStepLadder[^1]);
    }

    private static void AppendTimes(List<double> target, GridStep step, double fps, double tStart, double tEnd)
    {
        if (step.IsFrameStep)
        {
            // Frame index space: t = frameIndex / fps
            long first = (long)Math.Ceiling(tStart * fps / step.Count - 1e-9);
            long last = (long)Math.Floor(tEnd * fps / step.Count + 1e-9);
            for (long i = Math.Max(0, first); i <= last && target.Count < MaxLinesPerList; i++)
                target.Add(i * step.Count / fps);
            return;
        }

        // Second steps land on the frame where the SMPTE timecode reads HH:MM:SS;00 (not on wall-clock seconds),
        // so major lines and ruler labels agree at fractional rates (29.97 / 59.94, drop-frame or not).
        int nominal = Math.Max(1, (int)Math.Round(fps));
        double tcSecondsPerSecond = fps / nominal;
        long firstStep = (long)Math.Floor(tStart * tcSecondsPerSecond / step.Count) - 1;
        long lastStep = (long)Math.Ceiling(tEnd * tcSecondsPerSecond / step.Count) + 1;
        for (long i = Math.Max(0, firstStep); i <= lastStep && target.Count < MaxLinesPerList; i++)
        {
            double t = TimecodeSecondsToFrame((long)(i * step.Count), fps) / fps;
            if (t >= tStart - 1e-9 && t <= tEnd + 1e-9)
                target.Add(t);
        }
    }

    /// <summary>
    /// Frame number at which the SMPTE timecode shows <paramref name="timecodeSeconds"/> with ;00 / :00 frames.
    /// Drop-frame (29.97 / 59.94) skips 2 / 4 frame numbers each minute except every tenth minute.
    /// </summary>
    public static long TimecodeSecondsToFrame(long timecodeSeconds, double fps)
    {
        int nominal = Math.Max(1, (int)Math.Round(fps));
        long frames = timecodeSeconds * nominal;
        if (PlayheadOverlayCanvas.ResolveIsDropFrame(fps) && (nominal == 30 || nominal == 60))
        {
            long dropPerMinute = nominal == 30 ? 2 : 4;
            long totalMinutes = timecodeSeconds / 60;
            frames -= dropPerMinute * (totalMinutes - totalMinutes / 10);
            // At a dropped minute boundary ";00" does not exist: the first frame of that second is ";02" (";04" at 59.94)
            if (timecodeSeconds % 60 == 0 && totalMinutes % 10 != 0)
                frames += dropPerMinute;
        }
        return frames;
    }

    /// <summary>
    /// Centers a 1-device-pixel line on a physical pixel so it renders crisp (never blurred over two pixels).
    /// </summary>
    public static double SnapToDevicePixel(double x, double renderScaling)
    {
        double scale = renderScaling > 0.0 ? renderScaling : 1.0;
        return (Math.Floor(x * scale) + 0.5) / scale;
    }
}
