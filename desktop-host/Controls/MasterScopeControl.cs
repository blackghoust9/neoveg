using System;
using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.ViewModels;

namespace Neoveg.Controls;

/// <summary>
/// Cross-platform Avalonia UI Custom Control for the Audio Master Stereo Bus dBFS Peak Meters (Left / Right).
/// Strictly driven by real-time audio telemetry emitted by the native FFI audio pipeline (<c>rust-core</c> / <c>cpp-extensions</c>).
/// Contains no RGB video scope and zero timers, random generators, or simulated signals: when playback is stopped
/// or no audio media is active on the timeline, both L and R meters remain static at 0.0 (-∞ dB).
/// </summary>
public sealed class MasterScopeControl : Control
{
    private static readonly IBrush StaticPanelBgBrush = new SolidColorBrush(Color.Parse("#090d14"));
    private static readonly IBrush StaticWellBgBrush = new SolidColorBrush(Color.Parse("#05070b"));
    private static readonly IPen StaticWellBorderPen = new Pen(new SolidColorBrush(Color.Parse("#334155")), 1.0);
    private static readonly IPen StaticTickPen = new Pen(new SolidColorBrush(Color.Parse("#1e293b")), 1.0);
    private static readonly IBrush StaticScaleTextBrush = new SolidColorBrush(Color.Parse("#94a3b8"));
    private static readonly IBrush StaticChannelLabelBrush = new SolidColorBrush(Color.Parse("#e2e8f0"));
    private static readonly Typeface StaticMonoTypeface = new("Consolas, monospace");

    private static readonly LinearGradientBrush StaticMeterGradientBrush = new()
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

    private static readonly string[] DbScaleMarks = { " 0", "-6", "-12", "-24", "-48", "-∞" };

    public VegasProject? Project { get; set; }
    public RustCoreInterop? RustInterop { get; set; }
    public MasterBusViewModel? ViewModel { get; set; }
    public bool IsPlaying { get; set; }

    /// <summary>
    /// Current Left (L) channel linear peak amplitude in [0.0, 1.0]. Always 0.0 when stopped or silent.
    /// </summary>
    public double LeftLevelLinear { get; set; }

    /// <summary>
    /// Current Right (R) channel linear peak amplitude in [0.0, 1.0]. Always 0.0 when stopped or silent.
    /// </summary>
    public double RightLevelLinear { get; set; }

    public double LeftPeakDb { get; set; } = double.NegativeInfinity;
    public double RightPeakDb { get; set; } = double.NegativeInfinity;

    /// <summary>
    /// Synchronizes Left (L) and Right (R) meter levels with the real-time FFI audio telemetry.
    /// </summary>
    public void RefreshTelemetryFromEngine()
    {
        if (ViewModel != null)
        {
            ViewModel.UpdateFromRealTimeAudioTelemetry(Project, IsPlaying, RustInterop);
            LeftLevelLinear = ViewModel.LeftLevelLinear;
            RightLevelLinear = ViewModel.RightLevelLinear;
            LeftPeakDb = ViewModel.LeftPeakDb;
            RightPeakDb = ViewModel.RightPeakDb;
            return;
        }

        if (!IsPlaying || Project == null || RustInterop == null)
        {
            LeftLevelLinear = 0.0;
            RightLevelLinear = 0.0;
            LeftPeakDb = double.NegativeInfinity;
            RightPeakDb = double.NegativeInfinity;
            return;
        }

        var (leftLin, rightLin, leftDb, rightDb) = RustInterop.ComputeMasterBusStereoTelemetry(Project, IsPlaying);
        LeftLevelLinear = leftLin;
        RightLevelLinear = rightLin;
        LeftPeakDb = leftDb;
        RightPeakDb = rightDb;
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var b = Bounds;
        if (b.Width < 10 || b.Height < 10) return;

        RefreshTelemetryFromEngine();

        context.FillRectangle(StaticPanelBgBrush, b);

        // Master Stereo Bus dBFS Peak Meters (Left / Right) — full vertical height (no RGB video scope)
        const double meterTop = 8.0;
        const double bottomFooterHeight = 20.0;
        double meterH = Math.Max(32.0, b.Height - meterTop - bottomFooterHeight);

        double wellWidth = Math.Clamp((b.Width - 52.0) * 0.5, 18.0, 28.0);
        double leftWellX = 36.0;
        double rightWellX = leftWellX + wellWidth + 8.0;

        var meterWellL = new Rect(leftWellX, meterTop, wellWidth, meterH);
        var meterWellR = new Rect(rightWellX, meterTop, wellWidth, meterH);

        context.FillRectangle(StaticWellBgBrush, meterWellL);
        context.FillRectangle(StaticWellBgBrush, meterWellR);

        double levelL = IsPlaying && double.IsFinite(LeftLevelLinear) ? Math.Clamp(LeftLevelLinear, 0.0, 1.0) : 0.0;
        double levelR = IsPlaying && double.IsFinite(RightLevelLinear) ? Math.Clamp(RightLevelLinear, 0.0, 1.0) : 0.0;

        if (levelL > 1e-6)
        {
            double fillHL = (meterWellL.Height - 2.0) * levelL;
            if (fillHL > 0.5)
            {
                context.FillRectangle(
                    StaticMeterGradientBrush,
                    new Rect(meterWellL.X + 1.0, meterWellL.Bottom - 1.0 - fillHL, meterWellL.Width - 2.0, fillHL));
            }
        }

        if (levelR > 1e-6)
        {
            double fillHR = (meterWellR.Height - 2.0) * levelR;
            if (fillHR > 0.5)
            {
                context.FillRectangle(
                    StaticMeterGradientBrush,
                    new Rect(meterWellR.X + 1.0, meterWellR.Bottom - 1.0 - fillHR, meterWellR.Width - 2.0, fillHR));
            }
        }

        context.DrawRectangle(StaticWellBorderPen, meterWellL);
        context.DrawRectangle(StaticWellBorderPen, meterWellR);

        // dBFS scale labels (0 dB down to -∞ dB)
        for (int i = 0; i < DbScaleMarks.Length; i++)
        {
            double normY = i / (double)(DbScaleMarks.Length - 1);
            double my = meterTop + normY * (meterH - 12.0);
            double tickY = meterTop + normY * meterH;
            context.DrawLine(StaticTickPen, new Point(28.0, tickY), new Point(leftWellX - 2.0, tickY));

            var ft = new FormattedText(
                DbScaleMarks[i],
                CultureInfo.InvariantCulture,
                FlowDirection.LeftToRight,
                StaticMonoTypeface,
                9.0,
                StaticScaleTextBrush);
            context.DrawText(ft, new Point(6.0, my));
        }

        // Channel labels (L / R) beneath the meter wells
        double labelY = meterWellL.Bottom + 3.0;
        var ftL = new FormattedText(
            "L",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            StaticMonoTypeface,
            9.5,
            StaticChannelLabelBrush);
        var ftR = new FormattedText(
            "R",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            StaticMonoTypeface,
            9.5,
            StaticChannelLabelBrush);

        context.DrawText(ftL, new Point(meterWellL.Center.X - ftL.Width * 0.5, labelY));
        context.DrawText(ftR, new Point(meterWellR.Center.X - ftR.Width * 0.5, labelY));
    }
}
