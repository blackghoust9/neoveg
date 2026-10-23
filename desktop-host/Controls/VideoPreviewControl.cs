using System;
using System.Globalization;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.Controls;

/// <summary>
/// Cross-platform Avalonia UI Custom Control for the Neoveg Video Preview Monitor.
/// Renders active multi-track video/generator events, CorridorKey AI green screen unmixing,
/// Event Pan/Crop `F` frame gizmo, Split-Screen FX bypass, Safe Areas, and SMPTE HUD.
/// </summary>
public sealed class VideoPreviewControl : Control
{
    public NeovegProject? Project { get; set; }
    public TimelineClip? SelectedClip { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public CppCoreInterop? CppInterop { get; set; }
    public bool ShowPanCropGizmo { get; set; } = false;
    public bool ShowHudOverlay { get; set; } = false;
    public bool ShowClipOverlayLabels { get; set; } = false;
    public bool VideoFxBypassed { get; set; }
    public string PreviewQuality { get; set; } = "Good";
    public string PreviewResolution { get; set; } = "Auto";
    public event Action? OnPanCropUpdated;

    private bool _draggingGizmo;
    private Point _lastPointer;

    public VideoPreviewControl()
    {
        ClipToBounds = true;
        Cursor = new Cursor(StandardCursorType.Arrow);
        BuildPreviewOverlaysContextMenu();
    }

    public void BuildPreviewOverlaysContextMenu()
    {
        var menu = new ContextMenu();
        menu.Opening += (_, _) =>
        {
            var items = new System.Collections.Generic.List<object>();

            var miHud = new MenuItem
            {
                Header = $"{(ShowHudOverlay || (Project?.ShowPreviewHud ?? false) ? "✓ " : "   ")}Exibir HUD de Timecode & Resolução (SMPTE Overlay)"
            };
            miHud.Click += (_, _) =>
            {
                ShowHudOverlay = !ShowHudOverlay;
                if (Project != null) Project.ShowPreviewHud = ShowHudOverlay;
                InvalidateVisual();
            };
            items.Add(miHud);

            var miSafe = new MenuItem
            {
                Header = $"{(Project?.ShowSafeAreas == true ? "✓ " : "   ")}Exibir Áreas Seguras (Safe Areas 80% / 90%)"
            };
            miSafe.Click += (_, _) =>
            {
                if (Project != null) Project.ShowSafeAreas = !Project.ShowSafeAreas;
                InvalidateVisual();
            };
            items.Add(miSafe);

            var miGrid = new MenuItem
            {
                Header = $"{(Project?.ShowGrid == true ? "✓ " : "   ")}Exibir Grade de Terços (Rule of Thirds Grid)"
            };
            miGrid.Click += (_, _) =>
            {
                if (Project != null) Project.ShowGrid = !Project.ShowGrid;
                InvalidateVisual();
            };
            items.Add(miGrid);

            var miGizmo = new MenuItem
            {
                Header = $"{(ShowPanCropGizmo ? "✓ " : "   ")}Exibir Quadro Interativo Pan/Crop ('F' Gizmo)"
            };
            miGizmo.Click += (_, _) =>
            {
                ShowPanCropGizmo = !ShowPanCropGizmo;
                Cursor = new Cursor(ShowPanCropGizmo ? StandardCursorType.Cross : StandardCursorType.Arrow);
                InvalidateVisual();
            };
            items.Add(miGizmo);

            var miSplit = new MenuItem
            {
                Header = $"{(Project?.SplitScreenBypass == true ? "✓ " : "   ")}Exibir Split-Screen FX Bypass"
            };
            miSplit.Click += (_, _) =>
            {
                if (Project != null) Project.SplitScreenBypass = !Project.SplitScreenBypass;
                InvalidateVisual();
            };
            items.Add(miSplit);

            menu.ItemsSource = items;
        };
        ContextMenu = menu;
    }

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);
        if (SelectedClip != null && ShowPanCropGizmo)
        {
            _draggingGizmo = true;
            _lastPointer = e.GetPosition(this);
            e.Pointer.Capture(this);
        }
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);
        if (_draggingGizmo && SelectedClip != null && Bounds.Width > 20 && Bounds.Height > 20)
        {
            var pos = e.GetPosition(this);
            double dx = (pos.X - _lastPointer.X) / (Bounds.Width * 0.5);
            double dy = (pos.Y - _lastPointer.Y) / (Bounds.Height * 0.5);
            _lastPointer = pos;

            SelectedClip.PanCrop.X = Math.Clamp(Math.Round(SelectedClip.PanCrop.X + dx, 3), -1.0, 1.0);
            SelectedClip.PanCrop.Y = Math.Clamp(Math.Round(SelectedClip.PanCrop.Y + dy, 3), -1.0, 1.0);
            SelectedClip.PanCrop.SyncPixelsFromNormalized();
            InvalidateVisual();
            OnPanCropUpdated?.Invoke();
        }
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        if (_draggingGizmo)
        {
            _draggingGizmo = false;
            e.Pointer.Capture(null);
        }
    }

    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        base.OnPointerWheelChanged(e);
        if (SelectedClip != null && ShowPanCropGizmo)
        {
            // Inverted Neoveg zoom: scrolling up shrinks the 'F' box (zooms IN on image)
            double delta = e.Delta.Y > 0 ? -0.05 : 0.05;
            SelectedClip.PanCrop.Scale = Math.Clamp(Math.Round(SelectedClip.PanCrop.Scale + delta, 3), 0.15, 2.5);
            if (SelectedClip.PanCrop.LockAspect)
                SelectedClip.PanCrop.ScaleY = SelectedClip.PanCrop.Scale;
            SelectedClip.PanCrop.SyncPixelsFromNormalized();
            InvalidateVisual();
            OnPanCropUpdated?.Invoke();
        }
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var bounds = Bounds;
        if (bounds.Width < 10 || bounds.Height < 10) return;

        // Neoveg letterbox background
        context.FillRectangle(new SolidColorBrush(Color.Parse("#080a0f")), bounds);

        // Compute 16:9 stage rectangle inside control
        double targetAspect = Project != null && Project.Height > 0
            ? (double)Project.Width * Math.Max(0.01, Project.PixelAspectRatio) / Project.Height
            : 16.0 / 9.0;

        double stageW = bounds.Width - 12;
        double stageH = stageW / targetAspect;
        if (stageH > bounds.Height - 12)
        {
            stageH = bounds.Height - 12;
            stageW = stageH * targetAspect;
        }
        double stageX = (bounds.Width - stageW) * 0.5;
        double stageY = (bounds.Height - stageH) * 0.5;
        var stageRect = new Rect(stageX, stageY, stageW, stageH);

        context.FillRectangle(new SolidColorBrush(Color.Parse("#0d1117")), stageRect);

        if (Project != null)
        {
            double t = Project.Playhead;
            bool anySolo = Project.Tracks.Any(tr => tr.Type == "video" && tr.Solo);

            // Composite from bottom video track to top video track (Neoveg compositing order)
            for (int i = Project.Tracks.Count - 1; i >= 0; i--)
            {
                var track = Project.Tracks[i];
                if (track.Type != "video" || track.Muted || (anySolo && !track.Solo))
                    continue;

                foreach (var clip in track.Clips)
                {
                    if (clip.Muted || t < clip.Start || t > clip.End)
                        continue;

                    double localT = t - clip.Start;
                    double env = clip.Gain * track.Level;
                    if (clip.FadeIn > 0.01 && localT < clip.FadeIn && RustInterop != null)
                        env *= RustInterop.EvaluateFadeCurve(clip.FadeInCurve, localT / clip.FadeIn);
                    if (clip.FadeOut > 0.01 && (clip.Duration - localT) < clip.FadeOut && RustInterop != null)
                        env *= RustInterop.EvaluateFadeCurve(clip.FadeOutCurve, (clip.Duration - localT) / clip.FadeOut);

                    byte alpha = (byte)Math.Clamp(env * 255.0, 0, 255);
                    RenderClipLayer(context, stageRect, clip, t, alpha);
                }
            }

            // Split-Screen FX Bypass Indicator
            if (Project.SplitScreenBypass)
            {
                double midX = stageRect.X + stageRect.Width * 0.5;
                var bypassPen = new Pen(new SolidColorBrush(Color.Parse("#f59e0b")), 1.8);
                context.DrawLine(bypassPen, new Point(midX, stageRect.Y), new Point(midX, stageRect.Bottom));
                DrawText(context, "FX BYPASS (LEFT) | PROCESSED (RIGHT)", new Point(midX - 110, stageRect.Y + 8), 9.5, Color.Parse("#f59e0b"), true);
            }

            // Rule of Thirds Grid
            if (Project.ShowGrid)
            {
                var gridPen = new Pen(new SolidColorBrush(Color.FromArgb(95, 148, 163, 184)), 1.0);
                for (int g = 1; g <= 2; g++)
                {
                    double gx = stageRect.X + (stageRect.Width / 3.0) * g;
                    double gy = stageRect.Y + (stageRect.Height / 3.0) * g;
                    context.DrawLine(gridPen, new Point(gx, stageRect.Y), new Point(gx, stageRect.Bottom));
                    context.DrawLine(gridPen, new Point(stageRect.X, gy), new Point(stageRect.Right, gy));
                }
            }

            // Broadcast Safe Areas (90% Action Safe & 80% Title Safe)
            if (Project.ShowSafeAreas)
            {
                var safePen = new Pen(new SolidColorBrush(Color.FromArgb(90, 56, 189, 248)), 1.0, DashStyle.Dash);
                context.DrawRectangle(safePen, stageRect.Deflate(new Thickness(stageRect.Width * 0.05, stageRect.Height * 0.05)));
                context.DrawRectangle(safePen, stageRect.Deflate(new Thickness(stageRect.Width * 0.10, stageRect.Height * 0.10)));
            }

            // Event Pan/Crop Interactive `F` Frame Gizmo (optional overlay, disabled by default on startup)
            if (ShowPanCropGizmo && SelectedClip != null && SelectedClip.MediaType != "audio")
            {
                DrawPanCropGizmo(context, stageRect, SelectedClip.PanCrop);
            }

            // Optional Top-left SMPTE & Resolution HUD (disabled by default on startup; enabled via menu/toggle)
            if (ShowHudOverlay || Project.ShowPreviewHud)
            {
                string smpte = RustInterop != null
                    ? RustInterop.FormatSmpte(t, Project.Fps, true)
                    : "00:00:00;00";
                var hudRect = new Rect(stageRect.X + 8, stageRect.Y + 8, 250, 22);
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(185, 11, 15, 23)), hudRect);
                DrawText(
                    context,
                    $"TC {smpte} • {Project.Width}x{Project.Height} @{Project.Fps:0.##}p",
                    new Point(stageRect.X + 14, stageRect.Y + 12),
                    10.5,
                    Color.Parse("#38bdf8"),
                    true);
            }
        }

        // Outer frame border
        context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse("#334155")), 1.2), stageRect);
    }

    private void RenderClipLayer(DrawingContext context, Rect stageRect, TimelineClip clip, double time, byte alpha)
    {
        double localT = Math.Max(0.0, time - clip.Start);
        clip.PanCrop.EvaluateAtLocalTime(localT);

        // Neoveg Inverted Camera 2D Affine Transform (via C/C++ native bridge or exact camera inverse):
        // Shrinking the 'F' box (Scale < 1.0) magnifies the image (zoom = 1.0 / Scale),
        // and dragging the 'F' box right/down shifts the captured image left/up!
        double cropScaleX = Math.Clamp(clip.PanCrop.Scale, 0.15, 2.5);
        double cropScaleY = Math.Clamp(clip.PanCrop.LockAspect ? cropScaleX : (clip.PanCrop.ScaleY > 0.05 ? clip.PanCrop.ScaleY : cropScaleX), 0.15, 2.5);

        double zoomX = 1.0 / cropScaleX;
        double zoomY = 1.0 / cropScaleY;

        if (RustInterop != null)
        {
            var rustMat = RustInterop.ComputePanCropAffineMatrix(
                clip.PanCrop.SourceWidth,
                clip.PanCrop.SourceHeight,
                clip.PanCrop.Width,
                clip.PanCrop.Height,
                clip.PanCrop.XCenter,
                clip.PanCrop.YCenter,
                clip.PanCrop.Rotation);
            if (rustMat.Length == 6)
            {
                zoomX = Math.Clamp(clip.PanCrop.SourceWidth / Math.Max(32.0, clip.PanCrop.Width), 0.25, 6.0);
                zoomY = Math.Clamp(clip.PanCrop.SourceHeight / Math.Max(32.0, clip.PanCrop.Height), 0.25, 6.0);
            }
        }

        if (CppInterop != null)
        {
            var mat = CppInterop.ComputePanCropAffineMatrix(
                clip.PanCrop.SourceWidth,
                clip.PanCrop.SourceHeight,
                clip.PanCrop.Width,
                clip.PanCrop.Height,
                clip.PanCrop.XCenter,
                clip.PanCrop.YCenter,
                clip.PanCrop.Rotation);
            if (mat.Length == 6)
            {
                zoomX = Math.Clamp(clip.PanCrop.SourceWidth / Math.Max(32.0, clip.PanCrop.Width), 0.25, 6.0);
                zoomY = Math.Clamp(clip.PanCrop.SourceHeight / Math.Max(32.0, clip.PanCrop.Height), 0.25, 6.0);
            }
        }

        double cx = stageRect.Center.X - clip.PanCrop.X * (stageRect.Width * 0.45) * zoomX;
        double cy = stageRect.Center.Y - clip.PanCrop.Y * (stageRect.Height * 0.45) * zoomY;
        double w = stageRect.Width * zoomX;
        double h = stageRect.Height * zoomY;
        var dst = new Rect(cx - w * 0.5, cy - h * 0.5, w, h);

        double rotRad = -clip.PanCrop.Rotation * (Math.PI / 180.0);
        Matrix rotMatrix = Math.Abs(rotRad) > 1e-4
            ? Matrix.CreateTranslation(-stageRect.Center.X, -stageRect.Center.Y)
              * Matrix.CreateRotation(rotRad)
              * Matrix.CreateTranslation(stageRect.Center.X, stageRect.Center.Y)
            : Matrix.Identity;

        using (context.PushClip(stageRect))
        using (context.PushTransform(rotMatrix))
        {
            if (clip.IsOffline)
            {
                // Neoveg Offline Media Red Frame Slate
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(alpha, 127, 29, 29)), dst);
                var offlineStripePen = new Pen(new SolidColorBrush(Color.FromArgb(alpha, 220, 38, 38)), 2.0);
                context.DrawLine(offlineStripePen, dst.TopLeft, dst.BottomRight);
                context.DrawLine(offlineStripePen, dst.BottomLeft, dst.TopRight);

                var banner = new Rect(dst.X + dst.Width * 0.12, dst.Center.Y - 26, dst.Width * 0.76, 52);
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(230, 69, 10, 10)), banner);
                context.DrawRectangle(new Pen(new SolidColorBrush(Color.FromArgb(alpha, 248, 113, 113)), 1.5), banner);

                DrawText(
                    context,
                    "MEDIA OFFLINE — MÍDIA AUSENTE",
                    new Point(banner.X + 16, banner.Y + 8),
                    13.5,
                    Color.FromArgb(alpha, 254, 226, 226),
                    true);
                DrawText(
                    context,
                    clip.MediaFile ?? clip.Name,
                    new Point(banner.X + 16, banner.Y + 28),
                    10.0,
                    Color.FromArgb(alpha, 252, 165, 165),
                    false);
                return;
            }

            string gen = clip.GeneratorType ?? (clip.MediaType == "title" ? "titleText" : "smpteBars");

            if (gen == "smpteBars")
            {
                string[] barCols = { "#e2e8f0", "#facc15", "#22d3ee", "#22c55e", "#e879f9", "#ef4444", "#2563eb" };
                double bw = dst.Width / barCols.Length;
                for (int b = 0; b < barCols.Length; b++)
                {
                    var c = Color.Parse(barCols[b]);
                    var brush = new SolidColorBrush(Color.FromArgb(alpha, c.R, c.G, c.B));
                    context.FillRectangle(brush, new Rect(dst.X + b * bw, dst.Y, bw + 0.8, dst.Height * 0.78));
                }
                context.FillRectangle(
                    new SolidColorBrush(Color.FromArgb(alpha, 15, 23, 42)),
                    new Rect(dst.X, dst.Y + dst.Height * 0.78, dst.Width, dst.Height * 0.22));
                DrawText(
                    context,
                    $"SMPTE RP-219 HD • {clip.Name}",
                    new Point(dst.X + 16, dst.Y + dst.Height * 0.83),
                    11,
                    Color.FromArgb(alpha, 248, 250, 252),
                    true);
            }
            else if (gen == "greenScreenActor" || (!VideoFxBypassed && clip.Fx.Any(f => f.Type == "corridorKey" && f.Enabled)))
            {
                bool corridorKeyActive = !VideoFxBypassed && clip.Fx.Any(f => f.Type == "corridorKey" && f.Enabled);
                if (!corridorKeyActive)
                {
                    // Raw Chroma Green Screen backing
                    context.FillRectangle(new SolidColorBrush(Color.FromArgb(alpha, 22, 163, 74)), dst);
                }
                else
                {
                    // Unmixed transparent composite backdrop with subtle neoveg rim light
                    context.FillRectangle(new SolidColorBrush(Color.FromArgb((byte)(alpha * 0.35), 15, 23, 42)), dst);
                }

                // Draw human portrait subject silhouette + detailed face features
                double headR = Math.Min(dst.Width, dst.Height) * 0.16;
                var headCenter = new Point(dst.Center.X + Math.Sin(time * 1.6) * 14, dst.Center.Y - headR * 0.55);
                var skinBrush = new SolidColorBrush(Color.FromArgb(alpha, 241, 194, 155));
                var suitBrush = new SolidColorBrush(Color.FromArgb(alpha, 30, 58, 138));
                context.DrawEllipse(
                    suitBrush,
                    new Pen(new SolidColorBrush(Color.FromArgb(alpha, 56, 189, 248)), 1.5),
                    new Point(headCenter.X, dst.Bottom - headR * 0.4),
                    headR * 1.65,
                    headR * 1.25);
                context.DrawEllipse(
                    skinBrush,
                    new Pen(new SolidColorBrush(Color.FromArgb(alpha, 253, 224, 71)), 1.4),
                    headCenter,
                    headR * 0.85,
                    headR);

                string badge = corridorKeyActive
                    ? "CORRIDORKEY AI: ALPHA UNMIXED & DESPILLED"
                    : "CHROMA GREEN SOURCE (RAW)";
                DrawText(
                    context,
                    badge,
                    new Point(dst.X + 14, dst.Bottom - 26),
                    10.5,
                    Color.FromArgb(alpha, 74, 222, 128),
                    true);
            }
            else if (gen == "titleText" || clip.MediaType == "title")
            {
                var bannerRect = new Rect(dst.X + dst.Width * 0.07, dst.Bottom - dst.Height * 0.28, dst.Width * 0.86, dst.Height * 0.20);
                context.FillRectangle(new SolidColorBrush(Color.FromArgb((byte)(alpha * 0.86), 15, 23, 42)), bannerRect);
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(alpha, 37, 99, 235)), new Rect(bannerRect.X, bannerRect.Y, 5, bannerRect.Height));
                DrawText(
                    context,
                    clip.TitleText ?? clip.Name,
                    new Point(bannerRect.X + 16, bannerRect.Y + 8),
                    14,
                    Color.FromArgb(alpha, 248, 250, 252),
                    true);
                DrawText(
                    context,
                    clip.SubtitleText ?? "NEOVEG • AVALONIA UI + RUST FFI + PYTHON FFMPEG",
                    new Point(bannerRect.X + 16, bannerRect.Y + 29),
                    10.5,
                    Color.FromArgb(alpha, 56, 189, 248),
                    false);
            }
            else
            {
                // Synthwave / Video Layer
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(alpha, 23, 37, 84)), dst);
                var gridPen = new Pen(new SolidColorBrush(Color.FromArgb(alpha, 236, 72, 153)), 1.1);
                for (int l = 1; l <= 5; l++)
                {
                    double y = dst.Center.Y + (dst.Height * 0.09) * l;
                    context.DrawLine(gridPen, new Point(dst.X, y), new Point(dst.Right, y));
                }
                DrawText(context, clip.Name, new Point(dst.X + 18, dst.Center.Y - 12), 13, Color.FromArgb(alpha, 248, 250, 252), true);
            }

            // Render active Neoveg 3D LUT (.cube) color grading overlay in real time
            var activeLut = VideoFxBypassed
                ? null
                : clip.Fx.LastOrDefault(f => f.Enabled && f.Type != null && f.Type.StartsWith("lut3d.", StringComparison.OrdinalIgnoreCase));
            if (activeLut != null)
            {
                Color lutColor = activeLut.Type.ToLowerInvariant() switch
                {
                    "lut3d.teal_orange" => Color.FromArgb(52, 249, 115, 22),
                    "lut3d.cinematic" => Color.FromArgb(46, 234, 179, 8),
                    "lut3d.blue_tint" => Color.FromArgb(58, 56, 189, 248),
                    "lut3d.green_tint" => Color.FromArgb(54, 34, 197, 94),
                    _ => Color.FromArgb(45, 56, 189, 248)
                };
                var lutRect = Project != null && Project.SplitScreenBypass
                    ? new Rect(stageRect.X + stageRect.Width * 0.5, dst.Y, stageRect.Width * 0.5, dst.Height)
                    : dst;
                context.FillRectangle(new SolidColorBrush(lutColor), lutRect);
                DrawText(
                    context,
                    $"3D LUT ACTIVE: {activeLut.Name}",
                    new Point(stageRect.X + 14, stageRect.Bottom - 24),
                    9.5,
                    Color.FromArgb(235, 251, 191, 36),
                    true);
            }
        }
    }

    private static void DrawPanCropGizmo(DrawingContext context, Rect stageRect, PanCropState pc)
    {
        double w = stageRect.Width * Math.Clamp(pc.Scale, 0.3, 1.35) * 0.86;
        double h = stageRect.Height * Math.Clamp(pc.Scale, 0.3, 1.35) * 0.86;
        double cx = stageRect.Center.X + pc.X * (stageRect.Width * 0.3);
        double cy = stageRect.Center.Y + pc.Y * (stageRect.Height * 0.3);
        var box = new Rect(cx - w * 0.5, cy - h * 0.5, w, h);

        var gizmoPen = new Pen(new SolidColorBrush(Color.Parse("#38bdf8")), 1.4, DashStyle.Dash);
        context.DrawRectangle(gizmoPen, box);

        // Corner handles
        var handleBrush = new SolidColorBrush(Color.Parse("#38bdf8"));
        Point[] corners = { box.TopLeft, box.TopRight, box.BottomLeft, box.BottomRight };
        foreach (var pt in corners)
        {
            context.FillRectangle(handleBrush, new Rect(pt.X - 3.5, pt.Y - 3.5, 7, 7));
        }

        // Neoveg signature "F" orientation glyph inside the Pan/Crop frame
        DrawText(context, "F", new Point(cx - 5, cy - 9), 15, Color.FromArgb(190, 56, 189, 248), true);
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
