using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Views;

public enum PanCropDragMode
{
    None,
    MoveBox,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
    ResizeTop,
    ResizeBottom,
    ResizeLeft,
    ResizeRight,
    RotateOuterCircle
}

/// <summary>
/// Zone 1: Central Interactive Canvas for the VEGAS Pro Event Pan/Crop 'F' Gizmo.
/// Renders:
/// - Dotted workspace backdrop + video frame preview in the center
/// - Dashed bounding rectangle with the signature 'F' orientation glyph in the center
/// - 8 resize handles (4 corners + 4 edges) implementing VEGAS Pro's inverted camera logic
///   (shrinking the 'F' frame zooms IN on the video; dragging the 'F' frame pans the camera)
/// - Outer wide dotted rotation circle allowing smooth 360-degree camera rotation
/// </summary>
public sealed class PanCropGizmoCanvasControl : Control
{
    public VegasProject? Project { get; set; }
    public new TimelineClip? Clip { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public CppCoreInterop? CppInterop { get; set; }

    public event Action? OnGizmoTransformed;

    private PanCropDragMode _dragMode = PanCropDragMode.None;
    private Point _lastPointer;
    private double _startRotationAngle;
    private double _startPointerAngleRad;

    public PanCropGizmoCanvasControl()
    {
        ClipToBounds = true;
        Cursor = new Cursor(StandardCursorType.Cross);
    }

    private Rect GetSourceFrameStageRect(Rect bounds, PanCropState pc)
    {
        double srcW = Math.Max(16.0, pc.SourceWidth);
        double srcH = Math.Max(16.0, pc.SourceHeight);
        double aspect = srcW / srcH;

        double zoomFactor = Math.Clamp(pc.WorkspaceZoom / 100.0, 0.35, 2.5);
        double availW = (bounds.Width * 0.62) * zoomFactor;
        double availH = (bounds.Height * 0.62) * zoomFactor;

        double stageW = availW;
        double stageH = stageW / aspect;
        if (stageH > availH)
        {
            stageH = availH;
            stageW = stageH * aspect;
        }

        double cx = bounds.Width * 0.5 + pc.WorkspaceXOffset;
        double cy = bounds.Height * 0.5 + pc.WorkspaceYOffset;
        return new Rect(cx - stageW * 0.5, cy - stageH * 0.5, stageW, stageH);
    }

    private static Point RotatePoint(Point pt, Point pivot, double angleDeg)
    {
        double rad = angleDeg * (Math.PI / 180.0);
        double cos = Math.Cos(rad);
        double sin = Math.Sin(rad);
        double dx = pt.X - pivot.X;
        double dy = pt.Y - pivot.Y;
        return new Point(pivot.X + dx * cos - dy * sin, pivot.Y + dx * sin + dy * cos);
    }

    private static Point InverseRotatePoint(Point pt, Point pivot, double angleDeg) =>
        RotatePoint(pt, pivot, -angleDeg);

    private (Rect UnrotatedBox, Point Center, double Radius) GetGizmoGeometry(Rect stageRect, PanCropState pc)
    {
        double srcW = Math.Max(16.0, pc.SourceWidth);
        double srcH = Math.Max(16.0, pc.SourceHeight);

        double normX = (pc.XCenter - srcW * 0.5) / srcW;
        double normY = (pc.YCenter - srcH * 0.5) / srcH;

        double boxW = stageRect.Width * (pc.Width / srcW);
        double boxH = stageRect.Height * (pc.Height / srcH);
        double cx = stageRect.Center.X + normX * stageRect.Width;
        double cy = stageRect.Center.Y + normY * stageRect.Height;

        var box = new Rect(cx - boxW * 0.5, cy - boxH * 0.5, boxW, boxH);
        double radius = Math.Max(28.0, Math.Sqrt(boxW * boxW + boxH * boxH) * 0.56);
        return (box, new Point(cx, cy), radius);
    }

    private PanCropDragMode HitTestGizmo(Point pos)
    {
        if (Clip == null || Bounds.Width < 20 || Bounds.Height < 20)
            return PanCropDragMode.None;

        var pc = Clip.PanCrop;
        var stageRect = GetSourceFrameStageRect(Bounds, pc);
        var (box, center, radius) = GetGizmoGeometry(stageRect, pc);

        // Transform pointer into the unrotated 'F' box local frame
        var localPos = InverseRotatePoint(pos, center, pc.Rotation);
        const double handleHitRadius = 8.5;

        // 1. Check 4 corner handles
        if (Point.Distance(localPos, box.TopLeft) <= handleHitRadius) return PanCropDragMode.ResizeTopLeft;
        if (Point.Distance(localPos, box.TopRight) <= handleHitRadius) return PanCropDragMode.ResizeTopRight;
        if (Point.Distance(localPos, box.BottomLeft) <= handleHitRadius) return PanCropDragMode.ResizeBottomLeft;
        if (Point.Distance(localPos, box.BottomRight) <= handleHitRadius) return PanCropDragMode.ResizeBottomRight;

        // 2. Check 4 edge midpoint handles
        var topMid = new Point(box.Center.X, box.Top);
        var botMid = new Point(box.Center.X, box.Bottom);
        var leftMid = new Point(box.Left, box.Center.Y);
        var rightMid = new Point(box.Right, box.Center.Y);

        if (Point.Distance(localPos, topMid) <= handleHitRadius) return PanCropDragMode.ResizeTop;
        if (Point.Distance(localPos, botMid) <= handleHitRadius) return PanCropDragMode.ResizeBottom;
        if (Point.Distance(localPos, leftMid) <= handleHitRadius) return PanCropDragMode.ResizeLeft;
        if (Point.Distance(localPos, rightMid) <= handleHitRadius) return PanCropDragMode.ResizeRight;

        // 3. Check outer rotation dotted circle ring
        double distFromCenter = Point.Distance(pos, center);
        if (Math.Abs(distFromCenter - radius) <= 10.0)
        {
            return PanCropDragMode.RotateOuterCircle;
        }

        // 4. Check inside 'F' bounding rectangle -> move camera framing
        if (box.Contains(localPos))
        {
            return PanCropDragMode.MoveBox;
        }

        // Outside the box also allows rotating if near or outside the circle
        if (distFromCenter > radius * 0.85 && distFromCenter < radius * 1.25)
        {
            return PanCropDragMode.RotateOuterCircle;
        }

        return PanCropDragMode.MoveBox;
    }

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);
        if (Clip == null) return;

        var pos = e.GetPosition(this);
        _dragMode = HitTestGizmo(pos);
        _lastPointer = pos;

        if (_dragMode == PanCropDragMode.RotateOuterCircle)
        {
            var stageRect = GetSourceFrameStageRect(Bounds, Clip.PanCrop);
            var (_, center, _) = GetGizmoGeometry(stageRect, Clip.PanCrop);
            _startRotationAngle = Clip.PanCrop.Rotation;
            _startPointerAngleRad = Math.Atan2(pos.Y - center.Y, pos.X - center.X);
        }

        e.Pointer.Capture(this);
        e.Handled = true;
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);
        if (Clip == null || Bounds.Width < 20 || Bounds.Height < 20) return;

        var pos = e.GetPosition(this);
        var pc = Clip.PanCrop;
        var stageRect = GetSourceFrameStageRect(Bounds, pc);

        if (_dragMode == PanCropDragMode.None)
        {
            var hoverMode = HitTestGizmo(pos);
            Cursor = hoverMode switch
            {
                PanCropDragMode.ResizeTopLeft or PanCropDragMode.ResizeBottomRight => new Cursor(StandardCursorType.TopLeftCorner),
                PanCropDragMode.ResizeTopRight or PanCropDragMode.ResizeBottomLeft => new Cursor(StandardCursorType.TopRightCorner),
                PanCropDragMode.ResizeTop or PanCropDragMode.ResizeBottom => new Cursor(StandardCursorType.SizeNorthSouth),
                PanCropDragMode.ResizeLeft or PanCropDragMode.ResizeRight => new Cursor(StandardCursorType.SizeWestEast),
                PanCropDragMode.RotateOuterCircle => new Cursor(StandardCursorType.Hand),
                PanCropDragMode.MoveBox => new Cursor(StandardCursorType.SizeAll),
                _ => new Cursor(StandardCursorType.Cross)
            };
            return;
        }

        double srcW = Math.Max(16.0, pc.SourceWidth);
        double srcH = Math.Max(16.0, pc.SourceHeight);
        double pxScaleX = srcW / Math.Max(10.0, stageRect.Width);
        double pxScaleY = srcH / Math.Max(10.0, stageRect.Height);

        if (_dragMode == PanCropDragMode.RotateOuterCircle)
        {
            var (_, center, _) = GetGizmoGeometry(stageRect, pc);
            double curAngleRad = Math.Atan2(pos.Y - center.Y, pos.X - center.X);
            double deltaDeg = (curAngleRad - _startPointerAngleRad) * (180.0 / Math.PI);
            double newAngle = _startRotationAngle + deltaDeg;
            if (pc.EnableSnapping && Math.Abs(newAngle % 15.0) < 1.2)
            {
                newAngle = Math.Round(newAngle / 15.0) * 15.0;
            }
            pc.Rotation = Math.Clamp(Math.Round(newAngle, 1), -360.0, 360.0);
        }
        else if (_dragMode == PanCropDragMode.MoveBox)
        {
            double dxSrc = (pos.X - _lastPointer.X) * pxScaleX;
            double dySrc = (pos.Y - _lastPointer.Y) * pxScaleY;
            _lastPointer = pos;

            if (pc.MoveAxisMode != "Y")
                pc.XCenter = Math.Round(pc.XCenter + dxSrc, 1);
            if (pc.MoveAxisMode != "X")
                pc.YCenter = Math.Round(pc.YCenter + dySrc, 1);

            pc.SyncNormalizedFromPixels();
        }
        else
        {
            // Resize handle dragged in unrotated local coordinates
            var (_, center, _) = GetGizmoGeometry(stageRect, pc);
            var prevLocal = InverseRotatePoint(_lastPointer, center, pc.Rotation);
            var curLocal = InverseRotatePoint(pos, center, pc.Rotation);
            _lastPointer = pos;

            double dLocalX = (curLocal.X - prevLocal.X) * pxScaleX;
            double dLocalY = (curLocal.Y - prevLocal.Y) * pxScaleY;
            double aspect = srcW / srcH;
            double symFactor = pc.SizeAboutCenter ? 2.0 : 1.0;

            double newW = pc.Width;
            double newH = pc.Height;

            switch (_dragMode)
            {
                case PanCropDragMode.ResizeRight:
                case PanCropDragMode.ResizeTopRight:
                case PanCropDragMode.ResizeBottomRight:
                    newW += dLocalX * symFactor;
                    if (!pc.SizeAboutCenter) pc.XCenter += dLocalX * 0.5;
                    break;
                case PanCropDragMode.ResizeLeft:
                case PanCropDragMode.ResizeTopLeft:
                case PanCropDragMode.ResizeBottomLeft:
                    newW -= dLocalX * symFactor;
                    if (!pc.SizeAboutCenter) pc.XCenter += dLocalX * 0.5;
                    break;
            }

            switch (_dragMode)
            {
                case PanCropDragMode.ResizeBottom:
                case PanCropDragMode.ResizeBottomLeft:
                case PanCropDragMode.ResizeBottomRight:
                    newH += dLocalY * symFactor;
                    if (!pc.SizeAboutCenter) pc.YCenter += dLocalY * 0.5;
                    break;
                case PanCropDragMode.ResizeTop:
                case PanCropDragMode.ResizeTopLeft:
                case PanCropDragMode.ResizeTopRight:
                    newH -= dLocalY * symFactor;
                    if (!pc.SizeAboutCenter) pc.YCenter += dLocalY * 0.5;
                    break;
            }

            newW = Math.Clamp(newW, srcW * 0.12, srcW * 3.0);
            newH = Math.Clamp(newH, srcH * 0.12, srcH * 3.0);

            if (pc.LockAspect)
            {
                if (_dragMode is PanCropDragMode.ResizeTop or PanCropDragMode.ResizeBottom)
                    newW = newH * aspect;
                else
                    newH = newW / aspect;
            }

            pc.Width = Math.Round(newW, 1);
            pc.Height = Math.Round(newH, 1);
            pc.SyncNormalizedFromPixels();
        }

        InvalidateVisual();
        OnGizmoTransformed?.Invoke();
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        if (_dragMode != PanCropDragMode.None)
        {
            _dragMode = PanCropDragMode.None;
            e.Pointer.Capture(null);
            InvalidateVisual();
        }
    }

    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        base.OnPointerWheelChanged(e);
        if (Clip == null) return;

        var pc = Clip.PanCrop;
        if (e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            pc.WorkspaceZoom = Math.Clamp(pc.WorkspaceZoom + (e.Delta.Y > 0 ? 10.0 : -10.0), 40.0, 240.0);
        }
        else
        {
            // Inverted VEGAS camera logic: wheel up shrinks 'F' box (zooms IN on video)
            double factor = e.Delta.Y > 0 ? 0.94 : 1.06;
            pc.Width = Math.Clamp(Math.Round(pc.Width * factor, 1), pc.SourceWidth * 0.12, pc.SourceWidth * 3.0);
            if (pc.LockAspect)
                pc.Height = Math.Round(pc.Width * (pc.SourceHeight / Math.Max(16.0, pc.SourceWidth)), 1);
            else
                pc.Height = Math.Clamp(Math.Round(pc.Height * factor, 1), pc.SourceHeight * 0.12, pc.SourceHeight * 3.0);

            pc.SyncNormalizedFromPixels();
        }

        InvalidateVisual();
        OnGizmoTransformed?.Invoke();
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var b = Bounds;
        if (b.Width < 20 || b.Height < 20) return;

        // 1. Classic VEGAS Pro Pan/Crop Workspace Background + Fine Dotted Grid
        context.FillRectangle(new SolidColorBrush(Color.Parse("#181c24")), b);

        var pc = Clip?.PanCrop ?? new PanCropState();
        int step = Math.Clamp(pc.WorkspaceGridSpacing, 8, 64);
        var gridPen = new Pen(new SolidColorBrush(Color.FromArgb(40, 148, 163, 184)), 1.0);
        for (double gx = 0; gx < b.Width; gx += step)
            context.DrawLine(gridPen, new Point(gx, 0), new Point(gx, b.Height));
        for (double gy = 0; gy < b.Height; gy += step)
            context.DrawLine(gridPen, new Point(0, gy), new Point(b.Width, gy));

        // 2. Render Source Video Frame inside the Workspace
        var stageRect = GetSourceFrameStageRect(b, pc);
        DrawSourceVideoPreviewBackdrop(context, stageRect);

        // 3. Compute 'F' Gizmo Geometry & Outer Dotted Rotation Circle
        var (box, center, radius) = GetGizmoGeometry(stageRect, pc);

        // Outer Rotation Dotted Circle (Círculo Exterior de Linha Pontilhada Ampla)
        var circleDash = new DashStyle(new double[] { 3.0, 4.0 }, 0);
        var circlePen = new Pen(new SolidColorBrush(Color.FromArgb(215, 226, 232, 240)), 1.4, circleDash);
        context.DrawEllipse(null, circlePen, center, radius, radius);

        // Rotation Pivot Center Crosshair
        var pivotPen = new Pen(new SolidColorBrush(Color.Parse("#38bdf8")), 1.2);
        context.DrawEllipse(null, pivotPen, center, 4.5, 4.5);
        context.DrawLine(pivotPen, new Point(center.X - 7, center.Y), new Point(center.X + 7, center.Y));
        context.DrawLine(pivotPen, new Point(center.X, center.Y - 7), new Point(center.X, center.Y + 7));

        // Rotation Handle on Top of Outer Circle
        var rotHandlePos = RotatePoint(new Point(center.X, center.Y - radius), center, pc.Rotation);
        context.DrawLine(
            new Pen(new SolidColorBrush(Color.FromArgb(140, 56, 189, 248)), 1.1, DashStyle.Dash),
            center,
            rotHandlePos);
        context.DrawEllipse(
            new SolidColorBrush(Color.Parse("#38bdf8")),
            new Pen(Brushes.White, 1.2),
            rotHandlePos,
            5.0,
            5.0);

        // 4. Draw Rotated Dashed 'F' Bounding Rectangle, Orientation 'F', and 8 Resize Handles
        double rotRad = pc.Rotation * (Math.PI / 180.0);
        var rotMatrix = Matrix.CreateTranslation(-center.X, -center.Y)
                        * Matrix.CreateRotation(rotRad)
                        * Matrix.CreateTranslation(center.X, center.Y);

        using (context.PushTransform(rotMatrix))
        {
            // Subtle highlight inside the active 'F' camera frame
            context.FillRectangle(new SolidColorBrush(Color.FromArgb(24, 56, 189, 248)), box);

            // Dashed Bounding Rectangle (Retângulo Delimitador Tracejado)
            var boxDash = new DashStyle(new double[] { 5.0, 3.0 }, 0);
            var boxShadowPen = new Pen(new SolidColorBrush(Color.FromArgb(180, 0, 0, 0)), 2.2);
            var boxPen = new Pen(new SolidColorBrush(Color.Parse("#38bdf8")), 1.6, boxDash);
            context.DrawRectangle(boxShadowPen, box);
            context.DrawRectangle(boxPen, box);

            // Top-center orientation tick mark on top edge of 'F' rectangle
            context.DrawLine(
                new Pen(Brushes.White, 1.8),
                new Point(box.Center.X, box.Top),
                new Point(box.Center.X, box.Top + Math.Min(12.0, box.Height * 0.12)));

            // Central 'F' Orientation Glyph (scales with box)
            DrawCentralFGlyph(context, box, pc.FlipH, pc.FlipV);

            // 8 Resize Handles (4 Corners + 4 Edge Midpoints)
            Point[] handles =
            {
                box.TopLeft,
                box.TopRight,
                box.BottomLeft,
                box.BottomRight,
                new Point(box.Center.X, box.Top),
                new Point(box.Center.X, box.Bottom),
                new Point(box.Left, box.Center.Y),
                new Point(box.Right, box.Center.Y)
            };

            var handleFill = new SolidColorBrush(Color.Parse("#0f172a"));
            var handleBorder = new Pen(new SolidColorBrush(Color.Parse("#f8fafc")), 1.4);
            foreach (var hPt in handles)
            {
                var hRect = new Rect(hPt.X - 4.0, hPt.Y - 4.0, 8.0, 8.0);
                context.FillRectangle(handleFill, hRect);
                context.DrawRectangle(handleBorder, hRect);
            }
        }

        // 5. Top-Left Workspace HUD (Inverted Zoom Readout + Angle + Coordinates)
        double invZoomPct = (pc.SourceWidth / Math.Max(16.0, pc.Width)) * 100.0;
        string hud = $" Quadro 'F': {pc.Width:0.0} × {pc.Height:0.0} px  |  Centro: ({pc.XCenter:0.0}, {pc.YCenter:0.0})  |  Rot: {pc.Rotation:0.0}°  |  Zoom Efetivo no Preview: {invZoomPct:0}% ";
        var hudRect = new Rect(8, 8, Math.Min(b.Width - 16, 610), 22);
        context.FillRectangle(new SolidColorBrush(Color.FromArgb(205, 11, 15, 23)), hudRect);
        DrawLabel(context, hud, new Point(12, 12), 10.0, Color.Parse("#38bdf8"), true);
    }

    private void DrawSourceVideoPreviewBackdrop(DrawingContext context, Rect stageRect)
    {
        context.FillRectangle(new SolidColorBrush(Color.Parse("#0b0f17")), stageRect);

        using (context.PushClip(stageRect))
        {
            string gen = Clip?.GeneratorType ?? (Clip?.MediaType == "title" ? "titleText" : "smpteBars");
            if (gen == "smpteBars")
            {
                string[] barCols = { "#cbd5e1", "#eab308", "#06b6d4", "#22c55e", "#d946ef", "#ef4444", "#2563eb" };
                double bw = stageRect.Width / barCols.Length;
                for (int i = 0; i < barCols.Length; i++)
                {
                    var c = Color.Parse(barCols[i]);
                    context.FillRectangle(
                        new SolidColorBrush(Color.FromArgb(145, c.R, c.G, c.B)),
                        new Rect(stageRect.X + i * bw, stageRect.Y, bw + 0.5, stageRect.Height * 0.78));
                }
                context.FillRectangle(
                    new SolidColorBrush(Color.FromArgb(190, 15, 23, 42)),
                    new Rect(stageRect.X, stageRect.Y + stageRect.Height * 0.78, stageRect.Width, stageRect.Height * 0.22));
            }
            else if (gen == "greenScreenActor")
            {
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(130, 22, 163, 74)), stageRect);
                double r = Math.Min(stageRect.Width, stageRect.Height) * 0.16;
                context.DrawEllipse(
                    new SolidColorBrush(Color.FromArgb(185, 241, 194, 155)),
                    new Pen(Brushes.White, 1.2),
                    new Point(stageRect.Center.X, stageRect.Center.Y - r * 0.4),
                    r * 0.85,
                    r);
            }
            else
            {
                context.FillRectangle(new SolidColorBrush(Color.FromArgb(165, 23, 37, 84)), stageRect);
            }

            DrawLabel(
                context,
                Clip != null ? $"SOURCE: {Clip.Name}" : "SOURCE FRAME (1920x1080)",
                new Point(stageRect.X + 10, stageRect.Bottom - 18),
                9.5,
                Color.FromArgb(190, 226, 232, 240),
                true);
        }

        // Source frame border
        context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse("#475569")), 1.2), stageRect);
    }

    private static void DrawCentralFGlyph(DrawingContext context, Rect box, bool flipH, bool flipV)
    {
        double glyphH = Math.Clamp(Math.Min(box.Width, box.Height) * 0.34, 18.0, 96.0);
        double glyphW = glyphH * 0.62;
        double sx = flipH ? -1.0 : 1.0;
        double sy = flipV ? -1.0 : 1.0;

        var fTransform = Matrix.CreateScale(sx, sy) * Matrix.CreateTranslation(box.Center.X, box.Center.Y);
        using (context.PushTransform(fTransform))
        {
            double left = -glyphW * 0.5;
            double top = -glyphH * 0.5;
            double bottom = glyphH * 0.5;
            double right = glyphW * 0.5;
            double midY = -glyphH * 0.04;
            double midRight = glyphW * 0.28;

            var shadowPen = new Pen(new SolidColorBrush(Color.FromArgb(170, 0, 0, 0)), 4.0)
            {
                LineCap = PenLineCap.Round,
                LineJoin = PenLineJoin.Round
            };
            var fPen = new Pen(new SolidColorBrush(Color.FromArgb(225, 248, 250, 252)), 2.4)
            {
                LineCap = PenLineCap.Round,
                LineJoin = PenLineJoin.Round
            };

            // Spine of 'F'
            context.DrawLine(shadowPen, new Point(left, top), new Point(left, bottom));
            context.DrawLine(shadowPen, new Point(left, top), new Point(right, top));
            context.DrawLine(shadowPen, new Point(left, midY), new Point(midRight, midY));

            context.DrawLine(fPen, new Point(left, top), new Point(left, bottom));
            context.DrawLine(fPen, new Point(left, top), new Point(right, top));
            context.DrawLine(fPen, new Point(left, midY), new Point(midRight, midY));
        }
    }

    private static void DrawLabel(DrawingContext context, string text, Point origin, double size, Color color, bool bold)
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

/// <summary>
/// Zone 3: Dedicated Keyframe Animation Mini-Timeline at the Bottom of Event Pan/Crop.
/// Renders:
/// - SMPTE Timecode Ruler (`00:00:00;00`) spanning the exact duration of the selected video event (`Clip.Duration`)
/// - `Position` and `Mask` keyframe lanes
/// - Diamond-shaped keyframes (`♦` losangos) that can be clicked, dragged, added, or deleted
/// </summary>
public sealed class PanCropKeyframeTimelineControl : Control
{
    public const double RulerHeight = 22.0;
    public const double RowHeight = 24.0;

    public new TimelineClip? Clip { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public double Fps { get; set; } = 29.97;
    public double LocalCursorTime { get; set; } = 0.0;
    public string SelectedKeyframeId { get; set; } = "kf_pos_0";
    public string ActiveTrackType { get; set; } = "position";

    public event Action<double>? OnLocalCursorScrubbed;
    public event Action<PanCropKeyframe>? OnKeyframeSelected;
    public event Action? OnKeyframesMutated;

    private bool _scrubbing;
    private PanCropKeyframe? _draggingKf;

    public PanCropKeyframeTimelineControl()
    {
        ClipToBounds = true;
    }

    private double GetClipDuration() => Math.Max(0.5, Clip?.Duration ?? 5.0);

    private double TimeToX(double t)
    {
        double usableW = Math.Max(40.0, Bounds.Width - 24.0);
        return 12.0 + Math.Clamp(t / GetClipDuration(), 0.0, 1.0) * usableW;
    }

    private double XToTime(double x)
    {
        double usableW = Math.Max(40.0, Bounds.Width - 24.0);
        double norm = Math.Clamp((x - 12.0) / usableW, 0.0, 1.0);
        return Math.Round(norm * GetClipDuration(), 3);
    }

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);
        if (Clip == null) return;

        var pos = e.GetPosition(this);
        double clickedT = XToTime(pos.X);

        // Check if user clicked on a diamond keyframe (`♦`) in Position or Mask lane
        foreach (var kf in Clip.PanCrop.Keyframes)
        {
            double kx = TimeToX(kf.Time);
            double ky = string.Equals(kf.TrackType, "mask", StringComparison.OrdinalIgnoreCase)
                ? RulerHeight + RowHeight + RowHeight * 0.5
                : RulerHeight + RowHeight * 0.5;

            if (Point.Distance(pos, new Point(kx, ky)) <= 9.0)
            {
                SelectedKeyframeId = kf.Id;
                ActiveTrackType = kf.TrackType;
                LocalCursorTime = kf.Time;
                _draggingKf = kf;
                e.Pointer.Capture(this);
                OnKeyframeSelected?.Invoke(kf);
                OnLocalCursorScrubbed?.Invoke(LocalCursorTime);
                InvalidateVisual();
                return;
            }
        }

        if (pos.Y > RulerHeight + RowHeight)
            ActiveTrackType = "mask";
        else if (pos.Y > RulerHeight)
            ActiveTrackType = "position";

        _scrubbing = true;
        LocalCursorTime = clickedT;
        e.Pointer.Capture(this);
        OnLocalCursorScrubbed?.Invoke(LocalCursorTime);
        InvalidateVisual();
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);
        if (Clip == null) return;
        var pos = e.GetPosition(this);
        double t = XToTime(pos.X);

        if (_draggingKf != null)
        {
            _draggingKf.Time = t;
            LocalCursorTime = t;
            OnKeyframesMutated?.Invoke();
            OnLocalCursorScrubbed?.Invoke(LocalCursorTime);
            InvalidateVisual();
            return;
        }

        if (_scrubbing)
        {
            LocalCursorTime = t;
            OnLocalCursorScrubbed?.Invoke(LocalCursorTime);
            InvalidateVisual();
        }
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        if (_scrubbing || _draggingKf != null)
        {
            _scrubbing = false;
            _draggingKf = null;
            e.Pointer.Capture(null);
            InvalidateVisual();
        }
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var b = Bounds;
        if (b.Width < 20 || b.Height < 20) return;

        context.FillRectangle(new SolidColorBrush(Color.Parse("#121722")), b);

        // 1. Top SMPTE Ruler (00:00:00;00 up to Clip.Duration)
        var rulerRect = new Rect(0, 0, b.Width, RulerHeight);
        context.FillRectangle(new SolidColorBrush(Color.Parse("#19202e")), rulerRect);
        context.DrawLine(new Pen(new SolidColorBrush(Color.Parse("#2d3748")), 1.0), new Point(0, RulerHeight), new Point(b.Width, RulerHeight));

        double dur = GetClipDuration();
        int tickCount = 6;
        var tickPen = new Pen(new SolidColorBrush(Color.Parse("#64748b")), 1.0);
        var gridPen = new Pen(new SolidColorBrush(Color.FromArgb(40, 100, 116, 139)), 1.0);

        for (int i = 0; i <= tickCount; i++)
        {
            double t = (dur / tickCount) * i;
            double x = TimeToX(t);
            context.DrawLine(tickPen, new Point(x, RulerHeight - 7), new Point(x, RulerHeight));
            context.DrawLine(gridPen, new Point(x, RulerHeight), new Point(x, b.Height));

            string tc = RustInterop != null
                ? RustInterop.FormatSmpte(t, Fps, true)
                : $"00:00:{(int)t:D2};{(int)((t % 1.0) * 30):D2}";
            double labelX = i == tickCount ? Math.Max(4, x - 56) : x + 3;
            DrawSmallText(context, tc, new Point(labelX, 4), 9.0, Color.Parse("#cbd5e1"), false);
        }

        // 2. Lane 1: Position Track & Lane 2: Mask Track
        var posLaneRect = new Rect(0, RulerHeight, b.Width, RowHeight);
        var maskLaneRect = new Rect(0, RulerHeight + RowHeight, b.Width, RowHeight);

        context.FillRectangle(
            new SolidColorBrush(Color.Parse(ActiveTrackType == "position" ? "#1c2536" : "#141a26")),
            posLaneRect);
        context.FillRectangle(
            new SolidColorBrush(Color.Parse(ActiveTrackType == "mask" ? "#1c2536" : "#141a26")),
            maskLaneRect);

        context.DrawLine(new Pen(new SolidColorBrush(Color.Parse("#2d3748")), 1.0), new Point(0, posLaneRect.Bottom), new Point(b.Width, posLaneRect.Bottom));
        context.DrawLine(new Pen(new SolidColorBrush(Color.Parse("#2d3748")), 1.0), new Point(0, maskLaneRect.Bottom), new Point(b.Width, maskLaneRect.Bottom));

        // 3. Draw Diamond Keyframes (`♦` losangos)
        if (Clip != null)
        {
            foreach (var kf in Clip.PanCrop.Keyframes)
            {
                double kx = TimeToX(kf.Time);
                bool isMask = string.Equals(kf.TrackType, "mask", StringComparison.OrdinalIgnoreCase);
                double ky = isMask
                    ? maskLaneRect.Center.Y
                    : posLaneRect.Center.Y;
                bool isSel = kf.Id == SelectedKeyframeId;

                DrawDiamondKeyframe(context, new Point(kx, ky), 5.8, isSel, isMask);
            }
        }

        // 4. Draw Local Playhead Cursor across Mini-Timeline
        double cx = TimeToX(LocalCursorTime);
        var curPen = new Pen(new SolidColorBrush(Color.Parse("#38bdf8")), 1.6);
        context.DrawLine(curPen, new Point(cx, 0), new Point(cx, b.Height));
        context.FillRectangle(new SolidColorBrush(Color.Parse("#38bdf8")), new Rect(cx - 4, 0, 8, 8));
    }

    private static void DrawDiamondKeyframe(DrawingContext context, Point center, double r, bool selected, bool isMask)
    {
        var geo = new StreamGeometry();
        using (var ctx = geo.Open())
        {
            ctx.BeginFigure(new Point(center.X, center.Y - r), true);
            ctx.LineTo(new Point(center.X + r, center.Y));
            ctx.LineTo(new Point(center.X, center.Y + r));
            ctx.LineTo(new Point(center.X - r, center.Y));
            ctx.EndFigure(true);
        }

        var fill = selected
            ? new SolidColorBrush(Color.Parse("#facc15"))
            : new SolidColorBrush(Color.Parse(isMask ? "#a855f7" : "#38bdf8"));
        var pen = new Pen(Brushes.White, selected ? 1.6 : 1.1);
        context.DrawGeometry(fill, pen, geo);
    }

    private static void DrawSmallText(DrawingContext context, string text, Point origin, double size, Color color, bool bold)
    {
        var ft = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Consolas, Inter, sans-serif", FontStyle.Normal, bold ? FontWeight.Bold : FontWeight.Normal),
            size,
            new SolidColorBrush(color));
        context.DrawText(ft, origin);
    }
}

/// <summary>
/// Full VEGAS Pro 'Event Pan/Crop' (Video Event FX) Window & 3-Zone Workspace:
/// 1. Left Zone: Extreme-Left Vertical Icon Toolbar + Expandable Hierarchical Property Grid
///    (Position, Rotation, Keyframe interpolation, Source, Workspace)
/// 2. Center Zone: Interactive 'F' Gizmo Canvas with 8 Resize Handles (Inverted Camera Logic)
///    and Outer Dotted Rotation Circle
/// 3. Bottom Zone: Dedicated Keyframe Mini-Timeline (`00:00:00;00` of clip duration) with
///    `Position` and `Mask` tracks, diamond keyframes (`♦`), and bottom-left transport controls
/// </summary>
public sealed class EventPanCropWindow : Window
{
    // Vector SVG Icons for the Left Vertical Toolbar
    private static readonly StreamGeometry SvgToolSelect = StreamGeometry.Parse("M 3,2 L 3,13 L 6.5,10 L 9,14 L 10.8,13 L 8.2,9 L 12,9 Z");
    private static readonly StreamGeometry SvgToolZoom = StreamGeometry.Parse("M 6.5,2.5 A 4,4 0 1 0 6.5,10.5 A 4,4 0 1 0 6.5,2.5 M 9.5,9.5 L 13.5,13.5");
    private static readonly StreamGeometry SvgSnapGrid = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 8,2 L 8,14 M 2,8 L 14,8");
    private static readonly StreamGeometry SvgLockAspect = StreamGeometry.Parse("M 4,7 L 12,7 L 12,14 L 4,14 Z M 5.5,7 L 5.5,4.5 A 2.5,2.5 0 0 1 10.5,4.5 L 10.5,7");
    private static readonly StreamGeometry SvgSizeCenter = StreamGeometry.Parse("M 8,2 L 8,14 M 2,8 L 14,8 M 5,5 L 11,5 L 11,11 L 5,11 Z");
    private static readonly StreamGeometry SvgMoveAxis = StreamGeometry.Parse("M 8,1.5 L 8,14.5 M 1.5,8 L 14.5,8 M 6,3.5 L 8,1.5 L 10,3.5 M 12.5,6 L 14.5,8 L 12.5,10");
    private static readonly StreamGeometry SvgRestoreBox = StreamGeometry.Parse("M 3,3 L 13,3 L 13,13 L 3,13 Z M 5.5,8 L 10.5,8");

    private readonly VegasProject _project;
    private readonly TimelineClip _clip;
    private readonly RustCoreInterop _rustInterop;
    private readonly CppCoreInterop? _cppInterop;
    private readonly Action _onPanCropChanged;

    private readonly PanCropGizmoCanvasControl _gizmoCanvas;
    private readonly PanCropKeyframeTimelineControl _keyframeTimeline;
    private Control? _workspaceControl;

    // Property Grid Numeric / Combo Controls (synchronized bidirectionally in real time)
    private readonly TextBox _txtPosWidth;
    private readonly TextBox _txtPosHeight;
    private readonly TextBox _txtPosXCenter;
    private readonly TextBox _txtPosYCenter;

    private readonly TextBox _txtRotAngle;
    private readonly TextBox _txtRotXCenter;
    private readonly TextBox _txtRotYCenter;

    private readonly TextBox _txtKfSmoothness;
    private readonly ComboBox _cmbKfInterpType;

    private readonly ComboBox _cmbSrcMaintainAspect;
    private readonly ComboBox _cmbSrcStretchFill;

    private readonly TextBox _txtWsZoom;
    private readonly TextBox _txtWsXOffset;
    private readonly TextBox _txtWsYOffset;
    private readonly TextBox _txtWsGridSpacing;

    // Left Toolbar Toggle Buttons
    private Button? _btnLockAspect;
    private Button? _btnSizeCenter;
    private Button? _btnSnapGrid;
    private Button? _btnMoveAxis;
    private Button? _btnSyncCursor;
    private TextBlock? _lblMatrixStatus;

    private bool _suppressPropertyEvents;

    public EventPanCropWindow(
        VegasProject project,
        TimelineClip clip,
        RustCoreInterop rustInterop,
        CppCoreInterop? cppInterop,
        Action onPanCropChanged,
        bool deferWorkspaceBuild = false)
    {
        _project = project;
        _clip = clip;
        _rustInterop = rustInterop;
        _cppInterop = cppInterop;
        _onPanCropChanged = onPanCropChanged;

        _clip.PanCrop.SourceWidth = Math.Max(16, _project.Width);
        _clip.PanCrop.SourceHeight = Math.Max(16, _project.Height);
        _clip.PanCrop.SyncPixelsFromNormalized();

        var loc = LocalizationManager.Instance;
        Title = $"{loc.T("PanCrop.WindowTitle", "Video Event FX — Event Pan/Crop")} ({_clip.Name})";
        Width = 1040;
        Height = 680;
        MinWidth = 820;
        MinHeight = 520;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#141923"));
        Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"));

        _gizmoCanvas = new PanCropGizmoCanvasControl
        {
            Project = _project,
            Clip = _clip,
            RustInterop = _rustInterop,
            CppInterop = _cppInterop
        };

        _keyframeTimeline = new PanCropKeyframeTimelineControl
        {
            Clip = _clip,
            RustInterop = _rustInterop,
            Fps = _project.Fps,
            LocalCursorTime = Math.Clamp(_project.Playhead - _clip.Start, 0.0, _clip.Duration)
        };

        // Initialize Property Grid Controls
        _txtPosWidth = CreatePropTextBox(_clip.PanCrop.Width.ToString("0.0", CultureInfo.InvariantCulture));
        _txtPosHeight = CreatePropTextBox(_clip.PanCrop.Height.ToString("0.0", CultureInfo.InvariantCulture));
        _txtPosXCenter = CreatePropTextBox(_clip.PanCrop.XCenter.ToString("0.0", CultureInfo.InvariantCulture));
        _txtPosYCenter = CreatePropTextBox(_clip.PanCrop.YCenter.ToString("0.0", CultureInfo.InvariantCulture));

        _txtRotAngle = CreatePropTextBox(_clip.PanCrop.Rotation.ToString("0.0", CultureInfo.InvariantCulture));
        _txtRotXCenter = CreatePropTextBox(_clip.PanCrop.RotationXCenter.ToString("0.0", CultureInfo.InvariantCulture));
        _txtRotYCenter = CreatePropTextBox(_clip.PanCrop.RotationYCenter.ToString("0.0", CultureInfo.InvariantCulture));

        _txtKfSmoothness = CreatePropTextBox(_clip.PanCrop.Smoothness.ToString("0.0", CultureInfo.InvariantCulture));
        _cmbKfInterpType = new ComboBox
        {
            ItemsSource = new[] { "Linear", "Smooth", "Fast", "Slow", "Sharp", "Hold" },
            SelectedItem = _clip.PanCrop.InterpolationType,
            FontSize = 10.5,
            Height = 24,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };

        _cmbSrcMaintainAspect = new ComboBox
        {
            ItemsSource = new[] { "Yes", "No" },
            SelectedItem = _clip.PanCrop.MaintainAspectRatio ? "Yes" : "No",
            FontSize = 10.5,
            Height = 24,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };
        _cmbSrcStretchFill = new ComboBox
        {
            ItemsSource = new[] { "Yes", "No" },
            SelectedItem = _clip.PanCrop.StretchToFillFrame ? "Yes" : "No",
            FontSize = 10.5,
            Height = 24,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };

        _txtWsZoom = CreatePropTextBox(_clip.PanCrop.WorkspaceZoom.ToString("0.0", CultureInfo.InvariantCulture));
        _txtWsXOffset = CreatePropTextBox(_clip.PanCrop.WorkspaceXOffset.ToString("0.0", CultureInfo.InvariantCulture));
        _txtWsYOffset = CreatePropTextBox(_clip.PanCrop.WorkspaceYOffset.ToString("0.0", CultureInfo.InvariantCulture));
        _txtWsGridSpacing = CreatePropTextBox(_clip.PanCrop.WorkspaceGridSpacing.ToString(CultureInfo.InvariantCulture));

        WireBidirectionalEvents();
        if (!deferWorkspaceBuild)
        {
            _workspaceControl = BuildMainLayout();
            Content = _workspaceControl;
        }
        SyncPropertyGridFromPanCropState();
    }

    private void WireBidirectionalEvents()
    {
        // 1. Canvas -> Property Grid & Video Preview Monitor
        _gizmoCanvas.OnGizmoTransformed += () =>
        {
            SaveCurrentStateIntoActiveKeyframe();
            SyncPropertyGridFromPanCropState();
            _onPanCropChanged();
        };

        // 2. Property Grid -> Canvas & Video Preview Monitor
        void BindNumericBox(TextBox box, Action<double> applyValue)
        {
            box.KeyDown += (_, ke) =>
            {
                if (ke.Key == Key.Enter) ApplyBox(box, applyValue);
            };
            box.LostFocus += (_, _) => ApplyBox(box, applyValue);
        }

        void ApplyBox(TextBox box, Action<double> applyValue)
        {
            if (_suppressPropertyEvents) return;
            if (double.TryParse(box.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out double val))
            {
                applyValue(val);
                _clip.PanCrop.SyncNormalizedFromPixels();
                SaveCurrentStateIntoActiveKeyframe();
                SyncPropertyGridFromPanCropState();
                _gizmoCanvas.InvalidateVisual();
                _onPanCropChanged();
            }
        }

        BindNumericBox(_txtPosWidth, v =>
        {
            _clip.PanCrop.Width = Math.Clamp(v, 64.0, _clip.PanCrop.SourceWidth * 3.0);
            if (_clip.PanCrop.LockAspect)
                _clip.PanCrop.Height = Math.Round(_clip.PanCrop.Width * (_clip.PanCrop.SourceHeight / _clip.PanCrop.SourceWidth), 1);
        });
        BindNumericBox(_txtPosHeight, v =>
        {
            _clip.PanCrop.Height = Math.Clamp(v, 64.0, _clip.PanCrop.SourceHeight * 3.0);
            if (_clip.PanCrop.LockAspect)
                _clip.PanCrop.Width = Math.Round(_clip.PanCrop.Height * (_clip.PanCrop.SourceWidth / _clip.PanCrop.SourceHeight), 1);
        });
        BindNumericBox(_txtPosXCenter, v => _clip.PanCrop.XCenter = v);
        BindNumericBox(_txtPosYCenter, v => _clip.PanCrop.YCenter = v);

        BindNumericBox(_txtRotAngle, v => _clip.PanCrop.Rotation = Math.Clamp(v, -360.0, 360.0));
        BindNumericBox(_txtRotXCenter, v => _clip.PanCrop.RotationXCenter = v);
        BindNumericBox(_txtRotYCenter, v => _clip.PanCrop.RotationYCenter = v);

        BindNumericBox(_txtKfSmoothness, v => _clip.PanCrop.Smoothness = Math.Clamp(v, 0.0, 100.0));
        BindNumericBox(_txtWsZoom, v => _clip.PanCrop.WorkspaceZoom = Math.Clamp(v, 40.0, 240.0));
        BindNumericBox(_txtWsXOffset, v => _clip.PanCrop.WorkspaceXOffset = v);
        BindNumericBox(_txtWsYOffset, v => _clip.PanCrop.WorkspaceYOffset = v);
        BindNumericBox(_txtWsGridSpacing, v => _clip.PanCrop.WorkspaceGridSpacing = (int)Math.Clamp(v, 8, 64));

        _cmbKfInterpType.SelectionChanged += (_, _) =>
        {
            if (_suppressPropertyEvents) return;
            if (_cmbKfInterpType.SelectedItem is string s)
            {
                _clip.PanCrop.InterpolationType = s;
                SaveCurrentStateIntoActiveKeyframe();
                _onPanCropChanged();
            }
        };

        _cmbSrcMaintainAspect.SelectionChanged += (_, _) =>
        {
            if (_suppressPropertyEvents) return;
            _clip.PanCrop.MaintainAspectRatio = (_cmbSrcMaintainAspect.SelectedItem as string) == "Yes";
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };

        _cmbSrcStretchFill.SelectionChanged += (_, _) =>
        {
            if (_suppressPropertyEvents) return;
            _clip.PanCrop.StretchToFillFrame = (_cmbSrcStretchFill.SelectedItem as string) == "Yes";
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };

        // 3. Keyframe Mini-Timeline -> Main Timeline Playhead & Gizmo Canvas
        _keyframeTimeline.OnLocalCursorScrubbed += localT =>
        {
            _clip.PanCrop.EvaluateAtLocalTime(localT);
            if (_clip.PanCrop.SyncCursor)
            {
                _project.Playhead = Math.Round(_clip.Start + localT, 3);
            }
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };

        _keyframeTimeline.OnKeyframeSelected += kf =>
        {
            _clip.PanCrop.ApplyKeyframeToCurrent(kf);
            if (_clip.PanCrop.SyncCursor)
            {
                _project.Playhead = Math.Round(_clip.Start + kf.Time, 3);
            }
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };

        _keyframeTimeline.OnKeyframesMutated += () =>
        {
            _clip.PanCrop.EvaluateAtLocalTime(_keyframeTimeline.LocalCursorTime);
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };
    }

    private void SaveCurrentStateIntoActiveKeyframe()
    {
        var pc = _clip.PanCrop;
        var existing = pc.Keyframes.FirstOrDefault(k =>
            k.Id == _keyframeTimeline.SelectedKeyframeId ||
            (string.Equals(k.TrackType, "position", StringComparison.OrdinalIgnoreCase) &&
             Math.Abs(k.Time - _keyframeTimeline.LocalCursorTime) < 0.04));

        if (existing != null)
        {
            existing.Width = pc.Width;
            existing.Height = pc.Height;
            existing.XCenter = pc.XCenter;
            existing.YCenter = pc.YCenter;
            existing.Rotation = pc.Rotation;
            existing.InterpolationType = pc.InterpolationType;
            existing.Smoothness = pc.Smoothness;
        }
    }

    public void SyncPropertyGridFromPanCropState()
    {
        _suppressPropertyEvents = true;
        try
        {
            var pc = _clip.PanCrop;
            _txtPosWidth.Text = pc.Width.ToString("0.0", CultureInfo.InvariantCulture);
            _txtPosHeight.Text = pc.Height.ToString("0.0", CultureInfo.InvariantCulture);
            _txtPosXCenter.Text = pc.XCenter.ToString("0.0", CultureInfo.InvariantCulture);
            _txtPosYCenter.Text = pc.YCenter.ToString("0.0", CultureInfo.InvariantCulture);

            _txtRotAngle.Text = pc.Rotation.ToString("0.0", CultureInfo.InvariantCulture);
            _txtRotXCenter.Text = pc.RotationXCenter.ToString("0.0", CultureInfo.InvariantCulture);
            _txtRotYCenter.Text = pc.RotationYCenter.ToString("0.0", CultureInfo.InvariantCulture);

            _txtKfSmoothness.Text = pc.Smoothness.ToString("0.0", CultureInfo.InvariantCulture);
            _cmbKfInterpType.SelectedItem = pc.InterpolationType;

            _cmbSrcMaintainAspect.SelectedItem = pc.MaintainAspectRatio ? "Yes" : "No";
            _cmbSrcStretchFill.SelectedItem = pc.StretchToFillFrame ? "Yes" : "No";

            _txtWsZoom.Text = pc.WorkspaceZoom.ToString("0.0", CultureInfo.InvariantCulture);
            _txtWsXOffset.Text = pc.WorkspaceXOffset.ToString("0.0", CultureInfo.InvariantCulture);
            _txtWsYOffset.Text = pc.WorkspaceYOffset.ToString("0.0", CultureInfo.InvariantCulture);
            _txtWsGridSpacing.Text = pc.WorkspaceGridSpacing.ToString(CultureInfo.InvariantCulture);

            UpdateToolbarButtonVisuals();

            if (_lblMatrixStatus != null)
            {
                double[] mat = _rustInterop.ComputePanCropAffineMatrix(pc.SourceWidth, pc.SourceHeight, pc.Width, pc.Height, pc.XCenter, pc.YCenter, pc.Rotation);
                if (_cppInterop != null)
                {
                    mat = _cppInterop.ComputePanCropAffineMatrix(pc.SourceWidth, pc.SourceHeight, pc.Width, pc.Height, pc.XCenter, pc.YCenter, pc.Rotation);
                }
                _lblMatrixStatus.Text =
                    $"Matriz 2D Rust/C++/Skia [M11:{mat[0]:0.00} M12:{mat[1]:0.00} M21:{mat[2]:0.00} M22:{mat[3]:0.00} TX:{mat[4]:0.0} TY:{mat[5]:0.0}]";
            }
        }
        finally
        {
            _suppressPropertyEvents = false;
        }
    }

    /// <summary>
    /// Creates the full 3-Zone Event Pan/Crop interactive workspace control so it can be hosted either inside
    /// a detachable floating `EventPanCropWindow` OR docked directly inside the Upper Center Panel of `MainWindow`.
    /// </summary>
    public Control CreateDetachableWorkspaceControl() =>
        _workspaceControl ??= BuildMainLayout();

    private Control BuildMainLayout()
    {
        var outerGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,5,140")
        };

        // Top Preset Bar (Preset Selector + Restore / Flip / Center Buttons)
        var topBar = BuildTopPresetHeaderBar();
        Grid.SetRow(topBar, 0);
        outerGrid.Children.Add(topBar);

        // Middle Workspace: Left Toolbar + Expandable Property Grid | Splitter | Central 'F' Gizmo Canvas
        var midGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("340,4,*")
        };

        var leftZone = BuildLeftToolbarAndPropertyGridZone();
        Grid.SetColumn(leftZone, 0);
        midGrid.Children.Add(leftZone);

        var vSplitter = new GridSplitter
        {
            Background = new SolidColorBrush(Color.Parse("#263042")),
            ResizeDirection = GridResizeDirection.Columns
        };
        Grid.SetColumn(vSplitter, 1);
        midGrid.Children.Add(vSplitter);

        var canvasBorder = new Border
        {
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(1),
            Child = _gizmoCanvas
        };
        Grid.SetColumn(canvasBorder, 2);
        midGrid.Children.Add(canvasBorder);

        Grid.SetRow(midGrid, 1);
        outerGrid.Children.Add(midGrid);

        // Horizontal Splitter above Mini-Timeline
        var hSplitter = new GridSplitter
        {
            Background = new SolidColorBrush(Color.Parse("#263042")),
            ResizeDirection = GridResizeDirection.Rows
        };
        Grid.SetRow(hSplitter, 2);
        outerGrid.Children.Add(hSplitter);

        // Bottom Zone: Mini-Timeline de Animação (Position + Mask + Diamond Keyframes + Transport Bar)
        var bottomZone = BuildBottomAnimationTimelineZone();
        Grid.SetRow(bottomZone, 3);
        outerGrid.Children.Add(bottomZone);

        return outerGrid;
    }

    private Border BuildTopPresetHeaderBar()
    {
        var loc = LocalizationManager.Instance;
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,220,Auto,*,Auto")
        };

        grid.Children.Add(new TextBlock
        {
            Text = loc.T("PanCrop.Preset", "Preset:"),
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
            [Grid.ColumnProperty] = 0
        });

        var cmbPresets = new ComboBox
        {
            ItemsSource = new[]
            {
                "(Default) 16:9 Widescreen TV Aspect Ratio",
                "2x Zoom In Center (960x540)",
                "16:9 Close-Up Upper Third",
                "4:3 Standard TV Aspect Ratio",
                "9:16 Vertical Mobile Crop",
                "Dutch Angle Tilt (+12.0°)"
            },
            SelectedIndex = 0,
            FontSize = 10.5,
            Height = 26
        };
        cmbPresets.SelectionChanged += (_, _) =>
        {
            var pc = _clip.PanCrop;
            switch (cmbPresets.SelectedIndex)
            {
                case 0:
                    pc.Width = pc.SourceWidth;
                    pc.Height = pc.SourceHeight;
                    pc.XCenter = pc.SourceWidth * 0.5;
                    pc.YCenter = pc.SourceHeight * 0.5;
                    pc.Rotation = 0.0;
                    break;
                case 1:
                    pc.Width = pc.SourceWidth * 0.5;
                    pc.Height = pc.SourceHeight * 0.5;
                    pc.XCenter = pc.SourceWidth * 0.5;
                    pc.YCenter = pc.SourceHeight * 0.5;
                    pc.Rotation = 0.0;
                    break;
                case 2:
                    pc.Width = pc.SourceWidth * 0.65;
                    pc.Height = pc.SourceHeight * 0.65;
                    pc.XCenter = pc.SourceWidth * 0.5;
                    pc.YCenter = pc.SourceHeight * 0.38;
                    pc.Rotation = 0.0;
                    break;
                case 3:
                    pc.Height = pc.SourceHeight;
                    pc.Width = Math.Round(pc.SourceHeight * (4.0 / 3.0), 1);
                    break;
                case 4:
                    pc.Height = pc.SourceHeight;
                    pc.Width = Math.Round(pc.SourceHeight * (9.0 / 16.0), 1);
                    break;
                case 5:
                    pc.Width = pc.SourceWidth * 0.82;
                    pc.Height = pc.SourceHeight * 0.82;
                    pc.Rotation = 12.0;
                    break;
            }
            pc.SyncNormalizedFromPixels();
            SaveCurrentStateIntoActiveKeyframe();
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        };
        Grid.SetColumn(cmbPresets, 1);
        grid.Children.Add(cmbPresets);

        var quickBtns = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            Margin = new Thickness(10, 0, 0, 0)
        };

        var btnRestore = CreateActionButton(loc.T("PanCrop.Restore", "Restaurar (Restore)"), "#1e293b", "#e2e8f0", () =>
        {
            var pc = _clip.PanCrop;
            pc.Width = pc.SourceWidth;
            pc.Height = pc.SourceHeight;
            pc.XCenter = pc.SourceWidth * 0.5;
            pc.YCenter = pc.SourceHeight * 0.5;
            pc.Rotation = 0.0;
            pc.FlipH = false;
            pc.FlipV = false;
            pc.SyncNormalizedFromPixels();
            SaveCurrentStateIntoActiveKeyframe();
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        });

        var btnCenter = CreateActionButton(loc.T("PanCrop.Center", "Centralizar (Center)"), "#1e293b", "#e2e8f0", () =>
        {
            var pc = _clip.PanCrop;
            pc.XCenter = pc.SourceWidth * 0.5;
            pc.YCenter = pc.SourceHeight * 0.5;
            pc.SyncNormalizedFromPixels();
            SaveCurrentStateIntoActiveKeyframe();
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        });

        var btnFlipH = CreateActionButton(loc.T("PanCrop.FlipH", "Flip Horizontal"), "#1e293b", "#38bdf8", () =>
        {
            _clip.PanCrop.FlipH = !_clip.PanCrop.FlipH;
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        });

        quickBtns.Children.Add(btnRestore);
        quickBtns.Children.Add(btnCenter);
        quickBtns.Children.Add(btnFlipH);
        Grid.SetColumn(quickBtns, 2);
        grid.Children.Add(quickBtns);

        _lblMatrixStatus = new TextBlock
        {
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 9.5,
            Foreground = new SolidColorBrush(Color.Parse("#4ade80")),
            VerticalAlignment = VerticalAlignment.Center
        };
        Grid.SetColumn(_lblMatrixStatus, 4);
        grid.Children.Add(_lblMatrixStatus);

        return new Border
        {
            Background = new SolidColorBrush(Color.Parse("#181f2c")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(10, 6),
            Child = grid
        };
    }

    private Control BuildLeftToolbarAndPropertyGridZone()
    {
        var loc = LocalizationManager.Instance;
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("38,*")
        };

        // 1. Extreme-Left Vertical Icon Toolbar
        var vToolbar = new StackPanel
        {
            Spacing = 5,
            Margin = new Thickness(4, 8),
            HorizontalAlignment = HorizontalAlignment.Center
        };

        vToolbar.Children.Add(CreateVerticalToolButton(SvgToolSelect, loc.T("PanCrop.Tool.NormalEdit", "Normal Edit Tool (Mover/Redimensionar Quadro 'F')"), () => { }, true));
        vToolbar.Children.Add(CreateVerticalToolButton(SvgToolZoom, loc.T("PanCrop.Tool.Zoom", "Zoom Workspace (+20%)"), () =>
        {
            _clip.PanCrop.WorkspaceZoom = _clip.PanCrop.WorkspaceZoom >= 180 ? 100.0 : _clip.PanCrop.WorkspaceZoom + 20.0;
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
        }));

        _btnSnapGrid = CreateVerticalToolButton(SvgSnapGrid, loc.T("PanCrop.Tool.EnableSnapping", "Enable Snapping (F8 — Alinhar à Grade)"), () =>
        {
            _clip.PanCrop.EnableSnapping = !_clip.PanCrop.EnableSnapping;
            UpdateToolbarButtonVisuals();
        }, _clip.PanCrop.EnableSnapping);
        vToolbar.Children.Add(_btnSnapGrid);

        _btnLockAspect = CreateVerticalToolButton(SvgLockAspect, loc.T("PanCrop.Tool.LockAspect", "Travar Proporção de Aspeto (Lock Aspect Ratio)"), () =>
        {
            _clip.PanCrop.LockAspect = !_clip.PanCrop.LockAspect;
            UpdateToolbarButtonVisuals();
        }, _clip.PanCrop.LockAspect);
        vToolbar.Children.Add(_btnLockAspect);

        _btnSizeCenter = CreateVerticalToolButton(SvgSizeCenter, loc.T("PanCrop.Tool.SizeAboutCenter", "Dimensionar em Torno do Centro (Size About Center)"), () =>
        {
            _clip.PanCrop.SizeAboutCenter = !_clip.PanCrop.SizeAboutCenter;
            UpdateToolbarButtonVisuals();
        }, _clip.PanCrop.SizeAboutCenter);
        vToolbar.Children.Add(_btnSizeCenter);

        _btnMoveAxis = CreateVerticalToolButton(SvgMoveAxis, loc.T("PanCrop.Tool.MoveAxis", "Alternar Restrição de Movimento (Move Freely XY / X Only / Y Only)"), () =>
        {
            _clip.PanCrop.MoveAxisMode = _clip.PanCrop.MoveAxisMode switch
            {
                "XY" => "X",
                "X" => "Y",
                _ => "XY"
            };
            UpdateToolbarButtonVisuals();
        }, true);
        vToolbar.Children.Add(_btnMoveAxis);

        vToolbar.Children.Add(CreateVerticalToolButton(SvgRestoreBox, loc.T("PanCrop.Tool.RestoreBox", "Restaurar Quadro 'F' Padrão (1920x1080)"), () =>
        {
            _clip.PanCrop.Width = _clip.PanCrop.SourceWidth;
            _clip.PanCrop.Height = _clip.PanCrop.SourceHeight;
            _clip.PanCrop.XCenter = _clip.PanCrop.SourceWidth * 0.5;
            _clip.PanCrop.YCenter = _clip.PanCrop.SourceHeight * 0.5;
            _clip.PanCrop.Rotation = 0.0;
            _clip.PanCrop.SyncNormalizedFromPixels();
            SaveCurrentStateIntoActiveKeyframe();
            SyncPropertyGridFromPanCropState();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        }));

        var vToolbarBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(0, 0, 1, 0),
            Child = vToolbar
        };
        Grid.SetColumn(vToolbarBorder, 0);
        grid.Children.Add(vToolbarBorder);

        // 2. Expandable Hierarchical Property Grid (Position, Rotation, Keyframe interpolation, Source, Workspace)
        var propStack = new StackPanel
        {
            Spacing = 4,
            Margin = new Thickness(6)
        };

        propStack.Children.Add(BuildExpandablePropertyGroup(loc.T("PanCrop.Group.Position", "Position"), new (string Label, Control Editor)[]
        {
            (loc.T("PanCrop.Prop.Width", "Width"), _txtPosWidth),
            (loc.T("PanCrop.Prop.Height", "Height"), _txtPosHeight),
            (loc.T("PanCrop.Prop.XCenter", "X Center"), _txtPosXCenter),
            (loc.T("PanCrop.Prop.YCenter", "Y Center"), _txtPosYCenter)
        }));

        propStack.Children.Add(BuildExpandablePropertyGroup(loc.T("PanCrop.Group.Rotation", "Rotation"), new (string Label, Control Editor)[]
        {
            (loc.T("PanCrop.Prop.Angle", "Angle (°)"), _txtRotAngle),
            (loc.T("PanCrop.Prop.XCenter", "X Center"), _txtRotXCenter),
            (loc.T("PanCrop.Prop.YCenter", "Y Center"), _txtRotYCenter)
        }));

        propStack.Children.Add(BuildExpandablePropertyGroup(loc.T("PanCrop.Group.KeyframeInterp", "Keyframe interpolation"), new (string Label, Control Editor)[]
        {
            (loc.T("PanCrop.Prop.Smoothness", "Smoothness"), _txtKfSmoothness),
            (loc.T("PanCrop.Prop.InterpType", "Type"), _cmbKfInterpType)
        }));

        propStack.Children.Add(BuildExpandablePropertyGroup(loc.T("PanCrop.Group.Source", "Source"), new (string Label, Control Editor)[]
        {
            (loc.T("PanCrop.Prop.MaintainAspect", "Maintain aspect ratio"), _cmbSrcMaintainAspect),
            (loc.T("PanCrop.Prop.StretchToFill", "Stretch to fill frame"), _cmbSrcStretchFill)
        }));

        propStack.Children.Add(BuildExpandablePropertyGroup(loc.T("PanCrop.Group.Workspace", "Workspace"), new (string Label, Control Editor)[]
        {
            (loc.T("PanCrop.Prop.Zoom", "Zoom (%)"), _txtWsZoom),
            (loc.T("PanCrop.Prop.XOffset", "X Offset"), _txtWsXOffset),
            (loc.T("PanCrop.Prop.YOffset", "Y Offset"), _txtWsYOffset),
            (loc.T("PanCrop.Prop.GridSpacing", "Grid Spacing"), _txtWsGridSpacing)
        }));

        var propScroll = new ScrollViewer
        {
            Background = new SolidColorBrush(Color.Parse("#151b26")),
            Content = propStack
        };
        Grid.SetColumn(propScroll, 1);
        grid.Children.Add(propScroll);

        return grid;
    }

    private Control BuildBottomAnimationTimelineZone()
    {
        var outerGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("*,34")
        };

        // Row 0: Left Track Headers (Position / Mask) + Right Keyframe Mini-Timeline
        var tracksGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("180,1,*")
        };

        var leftHeaders = new StackPanel
        {
            Background = new SolidColorBrush(Color.Parse("#151b26"))
        };

        var loc = LocalizationManager.Instance;
        // Top spacer matching RulerHeight (22px)
        leftHeaders.Children.Add(new Border
        {
            Height = PanCropKeyframeTimelineControl.RulerHeight,
            Background = new SolidColorBrush(Color.Parse("#19202e")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2d3748")),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(8, 0),
            Child = new TextBlock
            {
                Text = loc.T("PanCrop.Keyframe.Controller", "KEYFRAME CONTROLLER"),
                FontSize = 9.0,
                FontWeight = FontWeight.Bold,
                Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
                VerticalAlignment = VerticalAlignment.Center
            }
        });

        // Track 1 Header: Position
        leftHeaders.Children.Add(new Border
        {
            Height = PanCropKeyframeTimelineControl.RowHeight,
            BorderBrush = new SolidColorBrush(Color.Parse("#2d3748")),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(8, 0),
            Child = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 6,
                VerticalAlignment = VerticalAlignment.Center,
                Children =
                {
                    new TextBlock { Text = "♦", Foreground = new SolidColorBrush(Color.Parse("#38bdf8")), FontSize = 11 },
                    new TextBlock { Text = loc.T("PanCrop.Group.Position", "Position"), FontSize = 10.5, FontWeight = FontWeight.SemiBold, Foreground = Brushes.White }
                }
            }
        });

        // Track 2 Header: Mask
        var chkMask = new CheckBox
        {
            IsChecked = _clip.PanCrop.MaskEnabled,
            Content = loc.T("PanCrop.Keyframe.Mask", "Mask"),
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0")),
            VerticalAlignment = VerticalAlignment.Center
        };
        chkMask.IsCheckedChanged += (_, _) =>
        {
            _clip.PanCrop.MaskEnabled = chkMask.IsChecked ?? false;
            _keyframeTimeline.ActiveTrackType = _clip.PanCrop.MaskEnabled ? "mask" : "position";
            _keyframeTimeline.InvalidateVisual();
        };

        leftHeaders.Children.Add(new Border
        {
            Height = PanCropKeyframeTimelineControl.RowHeight,
            BorderBrush = new SolidColorBrush(Color.Parse("#2d3748")),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(8, 0),
            Child = chkMask
        });

        Grid.SetColumn(leftHeaders, 0);
        tracksGrid.Children.Add(leftHeaders);

        tracksGrid.Children.Add(new Border
        {
            Background = new SolidColorBrush(Color.Parse("#2d3748")),
            [Grid.ColumnProperty] = 1
        });

        Grid.SetColumn(_keyframeTimeline, 2);
        tracksGrid.Children.Add(_keyframeTimeline);

        Grid.SetRow(tracksGrid, 0);
        outerGrid.Children.Add(tracksGrid);

        // Row 1: Bottom-Left Keyframe Control Footer (First |<, Prev <, Next >, Last >|, Add +♦, Delete -♦, Sync Cursor 🔗)
        var footerBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(8, 3)
        };

        var footerStack = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 5,
            VerticalAlignment = VerticalAlignment.Center
        };

        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.First", "|< Primeiro"), "#1e293b", "#e2e8f0", JumpToFirstKeyframe));
        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.Prev", "< Anterior"), "#1e293b", "#e2e8f0", JumpToPreviousKeyframe));
        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.Next", "Seguinte >"), "#1e293b", "#e2e8f0", JumpToNextKeyframe));
        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.Last", "Último >|"), "#1e293b", "#e2e8f0", JumpToLastKeyframe));

        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.Add", "+♦ Adicionar Keyframe"), "#2563eb", "#ffffff", AddKeyframeAtCursor));
        footerStack.Children.Add(CreateActionButton(loc.T("PanCrop.Keyframe.Delete", "-♦ Apagar Keyframe"), "#7f1d1d", "#ffffff", DeleteSelectedKeyframe));

        _btnSyncCursor = CreateActionButton(
            _clip.PanCrop.SyncCursor
                ? loc.T("PanCrop.Keyframe.SyncCursorOn", "🔗 Sync Cursor: ON")
                : loc.T("PanCrop.Keyframe.SyncCursorOff", "🔗 Sync Cursor: OFF"),
            _clip.PanCrop.SyncCursor ? "#0e7490" : "#1e293b",
            "#ffffff",
            () =>
            {
                _clip.PanCrop.SyncCursor = !_clip.PanCrop.SyncCursor;
                UpdateToolbarButtonVisuals();
                if (_clip.PanCrop.SyncCursor)
                {
                    _project.Playhead = Math.Round(_clip.Start + _keyframeTimeline.LocalCursorTime, 3);
                    _onPanCropChanged();
                }
            });
        footerStack.Children.Add(_btnSyncCursor);

        footerBorder.Child = footerStack;
        Grid.SetRow(footerBorder, 1);
        outerGrid.Children.Add(footerBorder);

        return outerGrid;
    }

    private void AddKeyframeAtCursor()
    {
        var pc = _clip.PanCrop;
        double t = Math.Clamp(Math.Round(_keyframeTimeline.LocalCursorTime, 3), 0.0, _clip.Duration);
        var kf = new PanCropKeyframe
        {
            Time = t,
            TrackType = _keyframeTimeline.ActiveTrackType,
            Width = pc.Width,
            Height = pc.Height,
            XCenter = pc.XCenter,
            YCenter = pc.YCenter,
            Rotation = pc.Rotation,
            InterpolationType = pc.InterpolationType,
            Smoothness = pc.Smoothness
        };
        pc.Keyframes.Add(kf);
        _keyframeTimeline.SelectedKeyframeId = kf.Id;
        _keyframeTimeline.InvalidateVisual();
        _onPanCropChanged();
    }

    private void DeleteSelectedKeyframe()
    {
        var pc = _clip.PanCrop;
        var target = pc.Keyframes.FirstOrDefault(k => k.Id == _keyframeTimeline.SelectedKeyframeId);
        if (target != null && pc.Keyframes.Count(k => k.TrackType == target.TrackType) > 1)
        {
            pc.Keyframes.Remove(target);
            var fallback = pc.Keyframes.First();
            _keyframeTimeline.SelectedKeyframeId = fallback.Id;
            _keyframeTimeline.LocalCursorTime = fallback.Time;
            pc.ApplyKeyframeToCurrent(fallback);
            SyncPropertyGridFromPanCropState();
            _keyframeTimeline.InvalidateVisual();
            _gizmoCanvas.InvalidateVisual();
            _onPanCropChanged();
        }
    }

    private void JumpToFirstKeyframe()
    {
        var first = _clip.PanCrop.Keyframes.OrderBy(k => k.Time).FirstOrDefault();
        if (first != null) SelectAndJumpToKeyframe(first);
    }

    private void JumpToLastKeyframe()
    {
        var last = _clip.PanCrop.Keyframes.OrderByDescending(k => k.Time).FirstOrDefault();
        if (last != null) SelectAndJumpToKeyframe(last);
    }

    private void JumpToPreviousKeyframe()
    {
        var prev = _clip.PanCrop.Keyframes
            .Where(k => k.Time < _keyframeTimeline.LocalCursorTime - 0.01)
            .OrderByDescending(k => k.Time)
            .FirstOrDefault();
        if (prev != null) SelectAndJumpToKeyframe(prev);
        else JumpToFirstKeyframe();
    }

    private void JumpToNextKeyframe()
    {
        var next = _clip.PanCrop.Keyframes
            .Where(k => k.Time > _keyframeTimeline.LocalCursorTime + 0.01)
            .OrderBy(k => k.Time)
            .FirstOrDefault();
        if (next != null) SelectAndJumpToKeyframe(next);
        else JumpToLastKeyframe();
    }

    private void SelectAndJumpToKeyframe(PanCropKeyframe kf)
    {
        _keyframeTimeline.SelectedKeyframeId = kf.Id;
        _keyframeTimeline.LocalCursorTime = kf.Time;
        _clip.PanCrop.ApplyKeyframeToCurrent(kf);
        if (_clip.PanCrop.SyncCursor)
            _project.Playhead = Math.Round(_clip.Start + kf.Time, 3);
        SyncPropertyGridFromPanCropState();
        _keyframeTimeline.InvalidateVisual();
        _gizmoCanvas.InvalidateVisual();
        _onPanCropChanged();
    }

    private void UpdateToolbarButtonVisuals()
    {
        var pc = _clip.PanCrop;
        if (_btnLockAspect != null)
            _btnLockAspect.Background = new SolidColorBrush(Color.Parse(pc.LockAspect ? "#2563eb" : "#1e293b"));
        if (_btnSizeCenter != null)
            _btnSizeCenter.Background = new SolidColorBrush(Color.Parse(pc.SizeAboutCenter ? "#2563eb" : "#1e293b"));
        if (_btnSnapGrid != null)
            _btnSnapGrid.Background = new SolidColorBrush(Color.Parse(pc.EnableSnapping ? "#2563eb" : "#1e293b"));
        if (_btnMoveAxis != null)
            ToolTip.SetTip(_btnMoveAxis, $"Restrição de Eixo Ativa: Move {pc.MoveAxisMode} (Clique para alternar XY / X / Y)");
        if (_btnSyncCursor != null)
        {
            var loc = LocalizationManager.Instance;
            _btnSyncCursor.Content = pc.SyncCursor
                ? loc.T("PanCrop.Keyframe.SyncCursorOn", "🔗 Sync Cursor: ON")
                : loc.T("PanCrop.Keyframe.SyncCursorOff", "🔗 Sync Cursor: OFF");
            _btnSyncCursor.Background = new SolidColorBrush(Color.Parse(pc.SyncCursor ? "#0e7490" : "#1e293b"));
        }
    }

    private static Border BuildExpandablePropertyGroup(string groupTitle, (string Label, Control Editor)[] rows)
    {
        var outerStack = new StackPanel { Spacing = 0 };
        var bodyStack = new StackPanel { Spacing = 2, Margin = new Thickness(6, 4, 4, 6) };

        foreach (var (label, editor) in rows)
        {
            var rowGrid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("118,*")
            };
            rowGrid.Children.Add(new TextBlock
            {
                Text = label,
                FontSize = 10.5,
                Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
                VerticalAlignment = VerticalAlignment.Center,
                [Grid.ColumnProperty] = 0
            });
            Grid.SetColumn(editor, 1);
            rowGrid.Children.Add(editor);
            bodyStack.Children.Add(rowGrid);
        }

        var toggleSign = new TextBlock
        {
            Text = "[-] ",
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        };

        var headerBtn = new Button
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            Background = new SolidColorBrush(Color.Parse("#1e2636")),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(6, 3),
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Children =
                {
                    toggleSign,
                    new TextBlock
                    {
                        Text = groupTitle,
                        FontSize = 10.5,
                        FontWeight = FontWeight.Bold,
                        Foreground = new SolidColorBrush(Color.Parse("#f8fafc"))
                    }
                }
            }
        };

        headerBtn.Click += (_, _) =>
        {
            bodyStack.IsVisible = !bodyStack.IsVisible;
            toggleSign.Text = bodyStack.IsVisible ? "[-] " : "[+] ";
        };

        outerStack.Children.Add(headerBtn);
        outerStack.Children.Add(bodyStack);

        return new Border
        {
            Background = new SolidColorBrush(Color.Parse("#181f2c")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(1),
            Child = outerStack
        };
    }

    private static TextBox CreatePropTextBox(string initialText)
    {
        return new TextBox
        {
            Text = initialText,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            Height = 23,
            MinHeight = 23,
            Padding = new Thickness(5, 2),
            Background = new SolidColorBrush(Color.Parse("#0f141d")),
            Foreground = new SolidColorBrush(Color.Parse("#f8fafc")),
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            CornerRadius = new CornerRadius(0)
        };
    }

    private static Button CreateVerticalToolButton(StreamGeometry icon, string tooltip, Action onClick, bool active = false)
    {
        var btn = new Button
        {
            Width = 28,
            Height = 28,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Color.Parse(active ? "#2563eb" : "#1e293b")),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            Content = new PathIcon
            {
                Data = icon,
                Width = 14,
                Height = 14,
                Foreground = Brushes.White
            }
        };
        ToolTip.SetTip(btn, tooltip);
        btn.Click += (_, _) => onClick();
        return btn;
    }

    private static Button CreateActionButton(string label, string bgHex, string fgHex, Action onClick)
    {
        var btn = new Button
        {
            Content = label,
            FontSize = 10,
            FontWeight = FontWeight.SemiBold,
            Background = new SolidColorBrush(Color.Parse(bgHex)),
            Foreground = new SolidColorBrush(Color.Parse(fgHex)),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(8, 3)
        };
        btn.Click += (_, _) => onClick();
        return btn;
    }
}
