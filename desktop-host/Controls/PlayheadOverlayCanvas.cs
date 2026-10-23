using System;
using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.Controls;

/// <summary>
/// Dedicated 60 FPS High-Performance Overlay Layer (`PlayheadOverlayCanvas` / `PlayheadOverlay.axaml` / `TimelineCursor.cs`)
/// for the Neoveg Playhead (CTI) & Classic VEGAS Pro Edit Cursor Blink:
/// 1. Cabeçote Azul Pentagonal com Bandeira (`#2980B9` / `#3498DB`):
///    - Pentagonal head with pointed tip (`bico`) aimed at the vertical guide line,
///    - Horizontal right-extending flag (`extensão horizontal para a direita`) on the timecode ruler.
/// 2. Linha de Guia Vertical (`1px` thickness) com Edit Cursor Blink (VEGAS Pro):
///    - Paused / Stopped (`IsPlaying == false && !IsScrubbing`): `TimelineCursor` (`DispatcherTimer` ~`500ms`)
///      alternates `VerticalLineOpacity` between `1.0` (visible) and `0.0` (invisible) at the fixed X position.
///    - Playing (`IsPlaying == true`) or Scrubbing (`IsScrubbing == true`): Stops the blink timer immediately,
///      locks `VerticalLineOpacity = 1.0` (solid), and updates `X` in real time at `60 FPS`.
///    - Keyboard Frame Stepping (`Left` / `Right` Arrow Keys): `NotifyKeyboardCursorMove()` resets the blink timer
///      so the vertical line stays solid (`1.0`) while stepping through frames and resumes blinking `500ms` after the last key press.
/// 3. Régua de Timecode SMPTE (`00;00;50;00` Drop-Frame / `00:00:50:00` Non-Drop-Frame):
///    - Timecode readout box positioned immediately above the tip of the marker with automatic or explicit
///      Drop-Frame (`;`) and Non-Drop-Frame (`:`) formatting via Rust FFI (`neoveg_seconds_to_smpte`).
/// </summary>
public sealed class PlayheadOverlayCanvas : Control
{
    public static readonly Color PrimaryBlueColor = Color.Parse("#2980B9");
    public static readonly Color HighlightBlueColor = Color.Parse("#3498DB");
    public static readonly Color ReadoutBoxBgColor = Color.FromArgb(235, 13, 22, 36);

    public TimelineCursor EditCursor { get; } = new();

    public VegasProject? Project { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public double PixelsPerSecond { get; set; } = 105.0;
    public double HorizontalOffsetSeconds { get; set; }
    public double RulerHeight { get; set; } = MultiTrackTimelineControl.RulerHeight;

    /// <summary>
    /// When `null`, automatically selects Drop-Frame (`;`) for NTSC fractional rates (29.97 / 59.94)
    /// and Non-Drop-Frame (`:`) for integer rates (24 / 25 / 30 / 60). Can also be toggled explicitly.
    /// </summary>
    public bool? ForceDropFrame { get; set; }

    public bool IsPlaying
    {
        get => EditCursor.IsPlaying;
        set => EditCursor.IsPlaying = value;
    }

    public bool IsScrubbing
    {
        get => EditCursor.IsScrubbing;
        set => EditCursor.IsScrubbing = value;
    }

    public double VerticalLineOpacity => EditCursor.VerticalLineOpacity;

    public PlayheadOverlayCanvas()
    {
        IsHitTestVisible = false;
        ClipToBounds = true;
        UseLayoutRounding = true;

        EditCursor.OnCursorVisualChanged += () => InvalidateVisual();
    }

    /// <summary>
    /// Resets the ~500ms Edit Cursor Blink timer when the user navigates frame-by-frame via the keyboard
    /// (`Left` / `Right` arrow keys), keeping the vertical cursor line solid (`Opacity = 1.0`) during movement
    /// and resuming the blink `500ms` after the last key press.
    /// </summary>
    public void NotifyKeyboardCursorMove()
    {
        EditCursor.ResetBlinkTimerOnKeyPress();
    }

    public static bool ResolveIsDropFrame(double fps, bool? forceDropFrame = null)
    {
        if (forceDropFrame.HasValue)
            return forceDropFrame.Value;
        return Math.Abs(fps - 29.97) < 0.06 || Math.Abs(fps - 59.94) < 0.06;
    }

    public static string FormatPlayheadSmpte(
        double playheadSeconds,
        double fps,
        RustCoreInterop? rustInterop,
        bool? forceDropFrame = null)
    {
        bool dropFrame = ResolveIsDropFrame(fps, forceDropFrame);
        if (rustInterop != null)
        {
            string formatted = rustInterop.FormatSmpte(playheadSeconds, fps, dropFrame);
            if (dropFrame)
            {
                // Premiere Pro full Drop-Frame notation uses ';' across fields (00;00;50;00)
                return formatted.Replace(':', ';');
            }
            return formatted.Replace(';', ':');
        }

        int nomFps = Math.Max(1, (int)Math.Round(fps));
        int totalFrames = Math.Max(0, (int)Math.Floor(Math.Max(0.0, playheadSeconds) * fps));
        int ff = totalFrames % nomFps;
        int totalSecs = totalFrames / nomFps;
        int ss = totalSecs % 60;
        int mm = (totalSecs / 60) % 60;
        int hh = totalSecs / 3600;
        char sep = dropFrame ? ';' : ':';
        return $"{hh:00}{sep}{mm:00}{sep}{ss:00}{sep}{ff:00}";
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        if (Project == null) return;

        var b = Bounds;
        if (b.Width < 4 || b.Height < 4) return;

        EditCursor.UpdatePosition(Project.Playhead, PixelsPerSecond, HorizontalOffsetSeconds);
        double px = EditCursor.CursorX;
        if (px < 0.0 || px > b.Width)
            return;

        double lineOpacity = (IsPlaying || IsScrubbing) ? 1.0 : EditCursor.VerticalLineOpacity;

        DrawPremierePlayhead(
            context,
            px,
            b.Width,
            b.Height,
            RulerHeight,
            Project.Playhead,
            Project.Fps,
            RustInterop,
            ForceDropFrame,
            lineOpacity,
            TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0);
    }

    /// <summary>
    /// Draws the Premiere Pro style Playhead (CTI) with VEGAS Pro Edit Cursor Blink:
    /// 1. SMPTE Timecode Readout Box (`00;00;50;00` / `00:00:50:00`) immediately above the marker tip,
    /// 2. Blue Pentagonal Head (`#2980B9` / `#3498DB`) with pointed bottom beak (`bico`) and horizontal right flag (`bandeira`),
    /// 3. 1px Vertical Blue Guide Line descending from the tip across all timeline tracks with `verticalLineOpacity` (`1.0` or `0.0`).
    /// </summary>
    public static void DrawPremierePlayhead(
        DrawingContext context,
        double playheadX,
        double viewportWidth,
        double viewportHeight,
        double rulerHeight,
        double playheadSeconds,
        double fps,
        RustCoreInterop? rustInterop,
        bool? forceDropFrame = null,
        double verticalLineOpacity = 1.0,
        double renderScaling = 1.0)
    {
        // Pixel-snapped: the guide line is centered on one physical pixel so it is exactly 1px wide
        // (never blurred/thickened) at any timeline zoom, scroll offset or monitor DPI scaling.
        double scale = renderScaling > 0.0 ? renderScaling : 1.0;
        double px = Math.Clamp(
            TimelineFrameGrid.SnapToDevicePixel(playheadX, scale),
            0.5 / scale,
            Math.Max(1.5 / scale, viewportWidth - 0.5 / scale));

        var primaryBlue = PrimaryBlueColor;     // #2980B9
        var highlightBlue = HighlightBlueColor; // #3498DB

        // ---------------------------------------------------------------------
        // 1. Linha de Guia Vertical (1px de espessura cruzando todas as faixas até o fundo)
        //    Pisca entre Opacity = 1.0 (visível) e Opacity = 0.0 (invisível) a cada ~500ms quando parado/pausado,
        //    e trava em Opacity = 1.0 (sólida a 60 FPS) durante Play, Scrubbing ou navegação por setas (Left/Right).
        // ---------------------------------------------------------------------
        double tipY = rulerHeight;
        double clampedLineOpacity = Math.Clamp(verticalLineOpacity, 0.0, 1.0);
        if (clampedLineOpacity > 0.01)
        {
            byte lineAlpha = (byte)Math.Clamp(Math.Round(clampedLineOpacity * 255.0), 0, 255);
            var lineColor = Color.FromArgb(lineAlpha, highlightBlue.R, highlightBlue.G, highlightBlue.B);
            var guideLinePen = new Pen(new SolidColorBrush(lineColor), 1.0 / scale);
            using (context.PushRenderOptions(new RenderOptions { EdgeMode = EdgeMode.Aliased }))
            {
                context.DrawLine(guideLinePen, new Point(px, tipY), new Point(px, viewportHeight));
            }
        }

        // ---------------------------------------------------------------------
        // 2. Cabeçote Azul Pentagonal com Bandeira (Extensão Horizontal para a Direita)
        // ---------------------------------------------------------------------
        const double halfHeadW = 6.5;
        double headTopY = Math.Max(13.0, rulerHeight - 14.0);
        double shoulderY = rulerHeight - 5.5;
        double flagRightX = Math.Min(viewportWidth - 2.0, px + halfHeadW + 26.0);
        double flagBottomY = headTopY + 5.5;

        // Combined Pentagonal Head + Right Horizontal Flag Geometry
        var pentagonWithFlag = new StreamGeometry();
        using (var g = pentagonWithFlag.Open())
        {
            g.BeginFigure(new Point(px - halfHeadW, headTopY), isFilled: true);
            // Top edge extending horizontally to the right into the flag (bandeira)
            g.LineTo(new Point(flagRightX, headTopY));
            g.LineTo(new Point(flagRightX, flagBottomY));
            g.LineTo(new Point(px + halfHeadW, flagBottomY));
            // Right shoulder of the pentagon
            g.LineTo(new Point(px + halfHeadW, shoulderY));
            // Pointed beak (bico) aimed straight at the 1px vertical guide line
            g.LineTo(new Point(px, tipY));
            // Left shoulder of the pentagon
            g.LineTo(new Point(px - halfHeadW, shoulderY));
            g.EndFigure(isClosed: true);
        }

        var headFillBrush = new LinearGradientBrush
        {
            StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
            EndPoint = new RelativePoint(0, 1, RelativeUnit.Relative),
            GradientStops = new GradientStops
            {
                new GradientStop(highlightBlue, 0.0), // #3498DB top highlight
                new GradientStop(primaryBlue, 1.0)    // #2980B9 bottom body
            }
        };
        var headBorderPen = new Pen(new SolidColorBrush(highlightBlue), 1.0);
        context.DrawGeometry(headFillBrush, headBorderPen, pentagonWithFlag);

        // Inner vertical center notch on the pentagonal head for crisp visual alignment
        context.DrawLine(
            new Pen(new SolidColorBrush(Color.FromArgb(180, 255, 255, 255)), 1.0),
            new Point(px, headTopY + 2.0),
            new Point(px, shoulderY - 1.0));

        // ---------------------------------------------------------------------
        // 3. Caixa de Texto de Timecode SMPTE (00;00;50;00) Imediatamente Acima do Marcador
        // ---------------------------------------------------------------------
        string smpteText = FormatPlayheadSmpte(playheadSeconds, fps, rustInterop, forceDropFrame);
        var ft = new FormattedText(
            smpteText,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Consolas, Inter, Segoe UI, monospace", FontStyle.Normal, FontWeight.Bold),
            9.5,
            new SolidColorBrush(Color.Parse("#F8FAFC")));

        double boxW = Math.Max(64.0, ft.Width + 10.0);
        double boxH = 13.0;
        double boxX = Math.Clamp(px - 6.5, 1.0, Math.Max(1.0, viewportWidth - boxW - 1.0));
        double boxY = 0.5;
        var readoutRect = new Rect(boxX, boxY, boxW, boxH);

        context.FillRectangle(new SolidColorBrush(ReadoutBoxBgColor), readoutRect);
        context.DrawRectangle(new Pen(new SolidColorBrush(highlightBlue), 1.0), readoutRect);
        context.DrawText(ft, new Point(boxX + 5.0, boxY + 0.5));
    }
}
