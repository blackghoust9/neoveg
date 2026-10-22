using System;
using System.Collections.Generic;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Controls;

/// <summary>
/// `RenderStatusRuler` (`RenderStatusRuler.axaml`) — VEGAS Pro Selective Prerender & Dynamic RAM Preview Status Bar.
/// Narrow horizontal bar (`5px` height, within `4px`..`6px` spec) positioned immediately below the SMPTE Timecode Ruler
/// and above the first track on the timeline.
/// Renders dynamic rectangular frame segments colored by `neoveg_get_timeline_render_status`:
/// - Green (`#2ECC71`): Cached in Dynamic RAM Preview Pool (`Shift+B`) or Disk Prerender (`Shift+M` in `cache/prerender/`).
/// - Yellow (`#F1C40F`): Simple single-stream frames or queued in Background Prerender Worker.
/// - Red (`#E74C3C`): Regions with multiple active FX, crossfades/transitions, overlapping tracks, or uncached/offline media.
/// </summary>
public partial class RenderStatusRuler : Control
{
    public const double DefaultBarHeight = 5.0; // 4px to 6px specification

    public static readonly Color ColorCachedGreen = Color.Parse("#2ECC71");
    public static readonly Color ColorSimpleOrQueuedYellow = Color.Parse("#F1C40F");
    public static readonly Color ColorComplexUncachedRed = Color.Parse("#E74C3C");

    public VegasProject? Project { get; set; }
    public SelectivePrerenderService? PrerenderService { get; set; }
    public double PixelsPerSecond { get; set; } = 95.0;

    public RenderStatusRuler()
    {
        Height = DefaultBarHeight;
        MinHeight = 4.0;
        MaxHeight = 6.0;
        ClipToBounds = true;
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var bounds = Bounds;
        if (bounds.Width < 4 || bounds.Height < 2) return;

        DrawStatusBarSegments(
            context,
            new Rect(0, 0, bounds.Width, bounds.Height),
            Project,
            PrerenderService,
            PixelsPerSecond);
    }

    /// <summary>
    /// Draws the dynamic Green (`#2ECC71`), Yellow (`#F1C40F`), and Red (`#E74C3C`) segments into `barRect`.
    /// Shared by standalone `RenderStatusRuler` and embedded `MultiTrackTimelineControl` header band.
    /// </summary>
    public static void DrawStatusBarSegments(
        DrawingContext context,
        Rect barRect,
        VegasProject? project,
        SelectivePrerenderService? prerenderService,
        double pixelsPerSecond)
    {
        // Base dark groove behind segments
        context.FillRectangle(new SolidColorBrush(Color.Parse("#0B0F17")), barRect);

        if (project == null || prerenderService == null)
        {
            context.FillRectangle(new SolidColorBrush(ColorSimpleOrQueuedYellow), barRect);
            return;
        }

        double pps = Math.Max(25.0, pixelsPerSecond);
        double visibleSec = Math.Max(6.0, barRect.Width / pps);
        var segments = prerenderService.BuildRenderStatusSegments(project, visibleSec, bucketCount: 220);

        var brushGreen = new SolidColorBrush(ColorCachedGreen);
        var brushYellow = new SolidColorBrush(ColorSimpleOrQueuedYellow);
        var brushRed = new SolidColorBrush(ColorComplexUncachedRed);

        foreach (var seg in segments)
        {
            double x1 = barRect.X + seg.StartSec * pps;
            double x2 = barRect.X + seg.EndSec * pps;
            if (x1 >= barRect.Right) break;
            double clampedRight = Math.Min(barRect.Right, x2);
            double segW = Math.Max(1.5, clampedRight - x1 - 0.6); // 0.6px micro-gap for crisp frame segment ticks

            IBrush segBrush = seg.Status switch
            {
                FrameRenderStatus.CachedGreen => brushGreen,
                FrameRenderStatus.SimpleOrQueuedYellow => brushYellow,
                _ => brushRed
            };

            var segRect = new Rect(x1, barRect.Y + 0.5, segW, Math.Max(3.0, barRect.Height - 1.0));
            context.FillRectangle(segBrush, segRect);
        }

        // Bottom 1px border separating the RenderStatusRuler from Track 1
        context.DrawLine(
            new Pen(new SolidColorBrush(Color.FromArgb(140, 15, 23, 42)), 1.0),
            new Point(barRect.Left, barRect.Bottom),
            new Point(barRect.Right, barRect.Bottom));
    }
}
