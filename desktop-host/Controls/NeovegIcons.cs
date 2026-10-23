using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Media;

namespace Neoveg.Controls;

/// <summary>
/// Shared 16x16 vector icon set for every Neoveg toolbar and panel button (VEGAS Pro icon-only buttons).
/// Outline icons are stroked; transport glyphs (play, stop, record...) are stroked and filled.
/// The function name is exposed only through the button tooltip.
/// </summary>
public static class NeovegIcons
{
    // File
    public static readonly StreamGeometry NewProject = StreamGeometry.Parse("M 3.5,1.5 L 10,1.5 L 13.5,5 L 13.5,14.5 L 3.5,14.5 Z M 10,1.5 L 10,5 L 13.5,5");
    public static readonly StreamGeometry Open = StreamGeometry.Parse("M 1.5,3.5 L 6,3.5 L 7.5,5.5 L 14.5,5.5 L 14.5,13.5 L 1.5,13.5 Z");
    public static readonly StreamGeometry Save = StreamGeometry.Parse("M 2,2 L 11,2 L 14,5 L 14,14 L 2,14 Z M 5,2 L 5,6 L 10,6 L 10,2 M 4,9 L 12,9 L 12,14 L 4,14 Z");
    public static readonly StreamGeometry ImportMedia = StreamGeometry.Parse("M 1.5,4 L 6,4 L 7.5,5.5 L 14.5,5.5 L 14.5,13.5 L 1.5,13.5 Z M 8,7 L 8,12 M 6,10 L 8,12 L 10,10");
    public static readonly StreamGeometry Render = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 5,2 L 5,14 M 11,2 L 11,14 M 2,8 L 14,8");
    public static readonly StreamGeometry Gear = StreamGeometry.Parse("M 8,5 A 3,3 0 1 0 8,11 A 3,3 0 1 0 8,5 M 8,1.5 L 8,3.5 M 8,12.5 L 8,14.5 M 1.5,8 L 3.5,8 M 12.5,8 L 14.5,8 M 3.4,3.4 L 4.8,4.8 M 11.2,11.2 L 12.6,12.6 M 3.4,12.6 L 4.8,11.2 M 11.2,4.8 L 12.6,3.4");
    public static readonly StreamGeometry Welcome = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 2,5.5 L 14,5.5 M 4,8 L 8,8 L 8,12 L 4,12 Z");
    public static readonly StreamGeometry Popout = StreamGeometry.Parse("M 2,4 L 11,4 L 11,13 L 2,13 Z M 9,2 L 14,2 L 14,7 M 8,8 L 14,2");
    public static readonly StreamGeometry Dock = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 2,5 L 14,5 M 6,8 L 10,12 M 10,8 L 10,12 L 6,12");

    // Transport
    public static readonly StreamGeometry Record = StreamGeometry.Parse("M 8,3.5 A 4.5,4.5 0 1 0 8,12.5 A 4.5,4.5 0 1 0 8,3.5 Z");
    public static readonly StreamGeometry Loop = StreamGeometry.Parse("M 3,7 L 3,4.5 L 12.5,4.5 M 10.5,2.5 L 12.5,4.5 L 10.5,6.5 M 13,9 L 13,11.5 L 3.5,11.5 M 5.5,9.5 L 3.5,11.5 L 5.5,13.5");
    public static readonly StreamGeometry PlayFromStart = StreamGeometry.Parse("M 3,3 L 3,13 M 5.5,3 L 13.5,8 L 5.5,13 Z");
    public static readonly StreamGeometry Play = StreamGeometry.Parse("M 4.5,2.5 L 13.5,8 L 4.5,13.5 Z");
    public static readonly StreamGeometry Pause = StreamGeometry.Parse("M 4,3 L 6.5,3 L 6.5,13 L 4,13 Z M 9.5,3 L 12,3 L 12,13 L 9.5,13 Z");
    public static readonly StreamGeometry Stop = StreamGeometry.Parse("M 4,4 L 12,4 L 12,12 L 4,12 Z");
    public static readonly StreamGeometry GoToStart = StreamGeometry.Parse("M 2.5,3 L 2.5,13 M 9,3.5 L 4,8 L 9,12.5 Z M 14.5,3.5 L 9.5,8 L 14.5,12.5 Z");
    public static readonly StreamGeometry GoToEnd = StreamGeometry.Parse("M 13.5,3 L 13.5,13 M 7,3.5 L 12,8 L 7,12.5 Z M 1.5,3.5 L 6.5,8 L 1.5,12.5 Z");
    public static readonly StreamGeometry PrevFrame = StreamGeometry.Parse("M 4.5,3.5 L 4.5,12.5 M 12,3.5 L 6.5,8 L 12,12.5 Z");
    public static readonly StreamGeometry NextFrame = StreamGeometry.Parse("M 11.5,3.5 L 11.5,12.5 M 4,3.5 L 9.5,8 L 4,12.5 Z");

    // Edit
    public static readonly StreamGeometry Split = StreamGeometry.Parse("M 8,1 L 8,15 M 2,5 L 6,5 L 6,11 L 2,11 Z M 10,5 L 14,5 L 14,11 L 10,11 Z");
    public static readonly StreamGeometry Group = StreamGeometry.Parse("M 2,2.5 L 7,2.5 L 7,7 L 2,7 Z M 2,9 L 7,9 L 7,13.5 L 2,13.5 Z M 7,4.7 L 12,4.7 L 12,11.3 L 7,11.3");
    public static readonly StreamGeometry Ungroup = StreamGeometry.Parse("M 1.5,2.5 L 6.5,2.5 L 6.5,7 L 1.5,7 Z M 9.5,9 L 14.5,9 L 14.5,13.5 L 9.5,13.5 Z M 9,3 L 13,7 M 13,3 L 9,7");
    public static readonly StreamGeometry Snap = StreamGeometry.Parse("M 3,2 L 6,2 L 6,9 A 2,2 0 0 0 10,9 L 10,2 L 13,2 L 13,9 A 5,5 0 0 1 3,9 Z M 3,5 L 6,5 M 10,5 L 13,5");
    public static readonly StreamGeometry AutoRipple = StreamGeometry.Parse("M 1.5,8 L 8.5,8 M 6.5,5.5 L 9,8 L 6.5,10.5 M 11,4 L 11,12 M 14,4 L 14,12");
    public static readonly StreamGeometry Marker = StreamGeometry.Parse("M 4,14.5 L 4,1.5 M 4,2 L 12.5,2 L 10.5,5 L 12.5,8 L 4,8");
    public static readonly StreamGeometry Region = StreamGeometry.Parse("M 2,2 L 2,14 M 14,2 L 14,14 M 2,2.5 L 6,2.5 L 6,6 L 2,6 M 14,2.5 L 10,2.5 L 10,6 L 14,6 M 4,10 L 12,10");
    public static readonly StreamGeometry AddVideoTrack = StreamGeometry.Parse("M 1.5,3.5 L 10.5,3.5 L 10.5,12.5 L 1.5,12.5 Z M 3.5,3.5 L 3.5,12.5 M 8.5,3.5 L 8.5,12.5 M 13,5.5 L 13,11.5 M 10,8.5 L 16,8.5");
    public static readonly StreamGeometry AddAudioTrack = StreamGeometry.Parse("M 1,6 L 3.5,6 L 7,3 L 7,13 L 3.5,10 L 1,10 Z M 12.5,5.5 L 12.5,11.5 M 9.5,8.5 L 15.5,8.5");
    public static readonly StreamGeometry TrackHeightGrow = StreamGeometry.Parse("M 2,1.5 L 14,1.5 M 2,14.5 L 14,14.5 M 8,4 L 8,12 M 5.5,6.5 L 8,4 L 10.5,6.5 M 5.5,9.5 L 8,12 L 10.5,9.5");
    public static readonly StreamGeometry TrackHeightShrink = StreamGeometry.Parse("M 2,8 L 14,8 M 8,1.5 L 8,5.5 M 5.5,3 L 8,5.5 L 10.5,3 M 8,14.5 L 8,10.5 M 5.5,13 L 8,10.5 L 10.5,13");
    public static readonly StreamGeometry ZoomIn = StreamGeometry.Parse("M 6.5,2 A 4.5,4.5 0 1 0 6.5,11 A 4.5,4.5 0 1 0 6.5,2 M 10,10 L 14.5,14.5 M 4.5,6.5 L 8.5,6.5 M 6.5,4.5 L 6.5,8.5");
    public static readonly StreamGeometry ZoomOut = StreamGeometry.Parse("M 6.5,2 A 4.5,4.5 0 1 0 6.5,11 A 4.5,4.5 0 1 0 6.5,2 M 10,10 L 14.5,14.5 M 4.5,6.5 L 8.5,6.5");
    public static readonly StreamGeometry RamPreview = StreamGeometry.Parse("M 3,4 L 13,4 L 13,12 L 3,12 Z M 5,2 L 5,4 M 8,2 L 8,4 M 11,2 L 11,4 M 5,12 L 5,14 M 8,12 L 8,14 M 11,12 L 11,14 M 5.5,8 L 10.5,8");
    public static readonly StreamGeometry Prerender = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 4.5,8 L 7,10.5 L 11.5,5.5");
    public static readonly StreamGeometry Timecode = StreamGeometry.Parse("M 8,2 A 6,6 0 1 0 8,14 A 6,6 0 1 0 8,2 M 8,4.5 L 8,8 L 10.5,9.5");
    public static readonly StreamGeometry Delete = StreamGeometry.Parse("M 2.5,4 L 13.5,4 M 6,4 L 6,2 L 10,2 L 10,4 M 4,4 L 5,14 L 11,14 L 12,4 M 6.8,6.5 L 6.8,11.5 M 9.2,6.5 L 9.2,11.5");
    public static readonly StreamGeometry Plus = StreamGeometry.Parse("M 8,2.5 L 8,13.5 M 2.5,8 L 13.5,8");
    public static readonly StreamGeometry Search = StreamGeometry.Parse("M 6.5,2 A 4.5,4.5 0 1 0 6.5,11 A 4.5,4.5 0 1 0 6.5,2 M 10,10 L 14.5,14.5");
    public static readonly StreamGeometry MarkIn = StreamGeometry.Parse("M 7,2.5 L 4,2.5 L 4,13.5 L 7,13.5 M 9,5 L 12,8 L 9,11");
    public static readonly StreamGeometry MarkOut = StreamGeometry.Parse("M 9,2.5 L 12,2.5 L 12,13.5 L 9,13.5 M 7,5 L 4,8 L 7,11");
    public static readonly StreamGeometry InsertToTimeline = StreamGeometry.Parse("M 8,1.5 L 8,10.5 M 5,7.5 L 8,10.5 L 11,7.5 M 1.5,13.5 L 14.5,13.5");

    // Track header
    public static readonly StreamGeometry TrackColor = StreamGeometry.Parse("M 8,1.5 C 8,1.5 3,7.5 3,10.5 A 5,5 0 0 0 13,10.5 C 13,7.5 8,1.5 8,1.5 Z");
    public static readonly StreamGeometry TrackMotion = StreamGeometry.Parse("M 8,1.5 L 8,14.5 M 1.5,8 L 14.5,8 M 6,3.5 L 8,1.5 L 10,3.5 M 12.5,6 L 14.5,8 L 12.5,10 M 6,12.5 L 8,14.5 L 10,12.5 M 3.5,6 L 1.5,8 L 3.5,10");
    public static readonly StreamGeometry Fx = StreamGeometry.Parse("M 2.5,13.5 L 2.5,4.5 C 2.5,3 3.5,2.5 5,2.5 L 6.5,2.5 M 1,7 L 5.5,7 M 8,6 L 14,13.5 M 14,6 L 8,13.5");
    public static readonly StreamGeometry Rename = StreamGeometry.Parse("M 2,14 L 3,10.5 L 11,2.5 L 13.5,5 L 5.5,13 Z M 9.5,4 L 12,6.5");
    public static readonly StreamGeometry Mute = StreamGeometry.Parse("M 8,2 A 6,6 0 1 0 8,14 A 6,6 0 1 0 8,2 M 3.8,12.2 L 12.2,3.8");
    public static readonly StreamGeometry Solo = StreamGeometry.Parse("M 8,2 L 8,10.5 M 8,13 L 8,14");
    public static readonly StreamGeometry Lock = StreamGeometry.Parse("M 4,7 L 12,7 L 12,14 L 4,14 Z M 5.5,7 L 5.5,4.5 A 2.5,2.5 0 0 1 10.5,4.5 L 10.5,7");

    // Preview
    public static readonly StreamGeometry SplitScreen = StreamGeometry.Parse("M 1.5,3 L 14.5,3 L 14.5,13 L 1.5,13 Z M 8,3 L 8,13");
    public static readonly StreamGeometry Overlays = StreamGeometry.Parse("M 2,2 L 14,2 L 14,14 L 2,14 Z M 6,2 L 6,14 M 10,2 L 10,14 M 2,6 L 14,6 M 2,10 L 14,10");
    public static readonly StreamGeometry SafeAreas = StreamGeometry.Parse("M 1.5,2.5 L 14.5,2.5 L 14.5,13.5 L 1.5,13.5 Z M 4,5 L 12,5 L 12,11 L 4,11 Z");
    public static readonly StreamGeometry Info = StreamGeometry.Parse("M 8,1.5 A 6.5,6.5 0 1 0 8,14.5 A 6.5,6.5 0 1 0 8,1.5 M 8,7 L 8,11.5 M 8,4.5 L 8,5");
    public static readonly StreamGeometry PanCrop = StreamGeometry.Parse("M 4.5,1.5 L 4.5,11.5 L 14.5,11.5 M 1.5,4.5 L 11.5,4.5 L 11.5,14.5");
    public static readonly StreamGeometry Restore = StreamGeometry.Parse("M 3,3 L 13,3 L 13,13 L 3,13 Z M 5.5,8 L 10.5,8");
    public static readonly StreamGeometry Center = StreamGeometry.Parse("M 8,1.5 L 8,5.5 M 8,10.5 L 8,14.5 M 1.5,8 L 5.5,8 M 10.5,8 L 14.5,8 M 8,6.5 A 1.5,1.5 0 1 0 8,9.5 A 1.5,1.5 0 1 0 8,6.5");
    public static readonly StreamGeometry FlipHorizontal = StreamGeometry.Parse("M 8,1.5 L 8,14.5 M 6,4 L 2,12 L 6,12 Z M 10,4 L 14,12 L 10,12 Z");
    public static readonly StreamGeometry SyncCursor = StreamGeometry.Parse("M 6.5,9.5 L 9.5,6.5 M 7,4.5 L 9,2.5 A 2.5,2.5 0 0 1 13.5,7 L 11.5,9 M 9,11.5 L 7,13.5 A 2.5,2.5 0 0 1 2.5,9 L 4.5,7");
    public static readonly StreamGeometry KeyframeAdd = StreamGeometry.Parse("M 6,3 L 10,7 L 6,11 L 2,7 Z M 12.5,9.5 L 12.5,15 M 9.75,12.25 L 15.25,12.25");
    public static readonly StreamGeometry KeyframeDelete = StreamGeometry.Parse("M 6,3 L 10,7 L 6,11 L 2,7 Z M 9.75,12.25 L 15.25,12.25");
    public static readonly StreamGeometry PreviewBeside = StreamGeometry.Parse("M 1.5,3 L 9,3 L 9,13 L 1.5,13 Z M 10.5,5 L 14.5,5 L 14.5,11 L 10.5,11 Z");
    public static readonly StreamGeometry Snapshot = StreamGeometry.Parse("M 2,5 L 5,5 L 6.5,3 L 10.5,3 L 12,5 L 14,5 L 14,13 L 2,13 Z M 8,6 A 2.3,2.3 0 1 0 8,10.6 A 2.3,2.3 0 1 0 8,6");
    public static readonly StreamGeometry CopyImage = StreamGeometry.Parse("M 5,2 L 13,2 L 13,10 M 3,5 L 11,5 L 11,14 L 3,14 Z M 5,8 L 9,8 M 5,10.5 L 9,10.5");

    private static readonly IBrush DefaultIconBrush = new SolidColorBrush(Color.Parse("#e2e8f0"));

    /// <summary>
    /// Creates a crisp stroked icon (16x16 design space). <paramref name="filled"/> also fills closed figures
    /// (used for transport glyphs such as Play, Stop and Record).
    /// </summary>
    public static Control Create(StreamGeometry geometry, double size = 15, IBrush? brush = null, bool filled = false)
    {
        var stroke = brush ?? DefaultIconBrush;
        return new Viewbox
        {
            Width = size,
            Height = size,
            Stretch = Stretch.Uniform,
            IsHitTestVisible = false,
            Child = new Canvas
            {
                Width = 16,
                Height = 16,
                Children =
                {
                    new Avalonia.Controls.Shapes.Path
                    {
                        Data = geometry,
                        Stroke = stroke,
                        StrokeThickness = 1.3,
                        StrokeLineCap = PenLineCap.Round,
                        StrokeJoin = PenLineJoin.Round,
                        Fill = filled ? stroke : null
                    }
                }
            }
        };
    }

    /// <summary>
    /// Creates a square icon-only button: the action name appears exclusively as a tooltip.
    /// </summary>
    public static Button CreateButton(
        StreamGeometry geometry,
        string tooltip,
        Action? onClick = null,
        string bgHex = "#1e293b",
        string fgHex = "#e2e8f0",
        bool filled = false,
        double size = 26)
    {
        var btn = new Button
        {
            Width = size + 2,
            Height = size,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Color.Parse(bgHex)),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            Content = Create(geometry, Math.Round(size * 0.55), new SolidColorBrush(Color.Parse(fgHex)), filled)
        };
        ToolTip.SetTip(btn, tooltip);
        if (onClick != null)
            btn.Click += (_, _) => onClick();
        return btn;
    }

    /// <summary>
    /// Swaps the glyph of an existing icon button (e.g. Play ⇄ Pause) keeping its colors.
    /// </summary>
    public static void SetButtonIcon(Button btn, StreamGeometry geometry, string fgHex = "#e2e8f0", bool filled = false)
    {
        btn.Content = Create(geometry, Math.Round(btn.Height * 0.55), new SolidColorBrush(Color.Parse(fgHex)), filled);
    }
}
