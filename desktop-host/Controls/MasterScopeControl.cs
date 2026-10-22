using System;
using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Neoveg.Models;

namespace Neoveg.Controls;

/// <summary>
/// Cross-platform Avalonia UI Custom Control for the VEGAS Pro RGB Parade Video Scope
/// and Master Stereo Bus dBFS Meters.
/// </summary>
public sealed class MasterScopeControl : Control
{
    public VegasProject? Project { get; set; }
    public bool IsPlaying { get; set; }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var b = Bounds;
        if (b.Width < 10 || b.Height < 10) return;

        context.FillRectangle(new SolidColorBrush(Color.Parse("#090d14")), b);

        // 1. Top Half: RGB Parade Scope (IRE 0..100)
        double scopeH = Math.Min(92, b.Height * 0.44);
        var scopeRect = new Rect(4, 4, b.Width - 8, scopeH);
        context.FillRectangle(new SolidColorBrush(Color.Parse("#05070b")), scopeRect);
        context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse("#263042")), 1.0), scopeRect);

        double t = Project?.Playhead ?? 0.0;
        string[] chColors = { "#ef4444", "#22c55e", "#3b82f6" };
        double colW = scopeRect.Width / 3.0;

        for (int c = 0; c < 3; c++)
        {
            var pen = new Pen(new SolidColorBrush(Color.Parse(chColors[c])), 1.3);
            double x0 = scopeRect.X + c * colW + 4;
            Point? prev = null;
            for (int s = 0; s < 18; s++)
            {
                double x = x0 + (s / 17.0) * (colW - 8);
                double wave = 0.5 + 0.36 * Math.Sin(t * 3.2 + s * 0.45 + c * 1.4);
                double y = scopeRect.Bottom - 6 - wave * (scopeRect.Height - 12);
                var pt = new Point(x, y);
                if (prev.HasValue)
                    context.DrawLine(pen, prev.Value, pt);
                prev = pt;
            }
        }

        // 2. Bottom Half: Master Stereo Bus dBFS Peak Meters (L / R)
        double meterTop = scopeRect.Bottom + 10;
        double meterH = Math.Max(40, b.Height - meterTop - 8);
        var meterWellL = new Rect(34, meterTop, 20, meterH);
        var meterWellR = new Rect(60, meterTop, 20, meterH);

        context.FillRectangle(new SolidColorBrush(Color.Parse("#05070b")), meterWellL);
        context.FillRectangle(new SolidColorBrush(Color.Parse("#05070b")), meterWellR);

        double masterVol = Project?.MasterVolume ?? 1.0;
        double pulse = IsPlaying ? (0.62 + 0.28 * Math.Abs(Math.Sin(t * 7.5))) : 0.48;
        double levelL = Math.Clamp(pulse * masterVol, 0.05, 1.0);
        double levelR = Math.Clamp((pulse * 0.94 + 0.04 * Math.Cos(t * 5.1)) * masterVol, 0.05, 1.0);

        double fillHL = meterWellL.Height * levelL;
        double fillHR = meterWellR.Height * levelR;

        var meterBrush = new LinearGradientBrush
        {
            StartPoint = new RelativePoint(0, 1, RelativeUnit.Relative),
            EndPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
            GradientStops = new GradientStops
            {
                new GradientStop(Color.Parse("#10b981"), 0.0),
                new GradientStop(Color.Parse("#f59e0b"), 0.78),
                new GradientStop(Color.Parse("#ef4444"), 1.0)
            }
        };

        context.FillRectangle(meterBrush, new Rect(meterWellL.X + 1, meterWellL.Bottom - fillHL, meterWellL.Width - 2, fillHL));
        context.FillRectangle(meterBrush, new Rect(meterWellR.X + 1, meterWellR.Bottom - fillHR, meterWellR.Width - 2, fillHR));
        context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse("#334155")), 1.0), meterWellL);
        context.DrawRectangle(new Pen(new SolidColorBrush(Color.Parse("#334155")), 1.0), meterWellR);

        // dBFS scale labels
        string[] dbMarks = { " 0", "-6", "-12", "-24", "-48" };
        for (int i = 0; i < dbMarks.Length; i++)
        {
            double my = meterTop + (i / (double)(dbMarks.Length - 1)) * (meterH - 12);
            var ft = new FormattedText(
                dbMarks[i],
                CultureInfo.InvariantCulture,
                FlowDirection.LeftToRight,
                new Typeface("Consolas, monospace"),
                9,
                new SolidColorBrush(Color.Parse("#94a3b8")));
            context.DrawText(ft, new Point(6, my));
        }
    }
}
