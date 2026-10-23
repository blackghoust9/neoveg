using Neoveg.Controls;
using Xunit;

namespace Neoveg.Tests;

public sealed class TimelineFrameGridTests
{
    [Theory]
    [InlineData(24.0)]
    [InlineData(30.0)]
    [InlineData(60.0)]
    public void CloseZoom_DrawsOneLinePerFrame(double fps)
    {
        // 2400 px/s: every frame is wider than the minimum minor spacing
        var grid = TimelineFrameGrid.Compute(fps, 2400.0, 0.0, 1200.0);
        var step = TimelineFrameGrid.ChooseStep(fps, 2400.0, TimelineFrameGrid.MinMinorSpacingPx);

        Assert.True(step.IsFrameStep);
        Assert.Equal(1.0, step.Count);
        // The line at 1 frame exists either as a minor line or (when frames are very wide) as a labeled major line
        Assert.Contains(grid.MinorTimes.Concat(grid.MajorTimes), t => System.Math.Abs(t - 1.0 / fps) < 1e-9);
    }

    [Fact]
    public void FarZoom_GroupsLinesIntoSeconds()
    {
        var minor = TimelineFrameGrid.ChooseStep(29.97, 20.0, TimelineFrameGrid.MinMinorSpacingPx);
        var major = TimelineFrameGrid.ChooseStep(29.97, 20.0, TimelineFrameGrid.MinMajorSpacingPx);

        Assert.False(minor.IsFrameStep);
        Assert.Equal(1.0, minor.Count);
        Assert.False(major.IsFrameStep);
        Assert.Equal(5.0, major.Count);
    }

    [Fact]
    public void MinorLines_NeverDuplicateMajorLines()
    {
        var grid = TimelineFrameGrid.Compute(30.0, 300.0, 0.0, 3000.0);
        foreach (double major in grid.MajorTimes)
            Assert.DoesNotContain(grid.MinorTimes, t => System.Math.Abs(t - major) < 1e-9);
    }

    [Theory]
    [InlineData(9, 270)]      // 00:00:09;00
    [InlineData(60, 1800)]    // 00:01:00;00 does not exist in drop-frame: the second starts at 00:01:00;02
    [InlineData(61, 1828)]    // 00:01:01;00
    [InlineData(600, 17982)]  // tenth minute: no drop, 00:10:00;00 exists
    public void DropFrame2997_MajorSecondsMatchTimecode(long tcSeconds, long expectedFrame)
    {
        Assert.Equal(expectedFrame, TimelineFrameGrid.TimecodeSecondsToFrame(tcSeconds, 29.97));
    }

    [Fact]
    public void SnapToDevicePixel_CentersOnPhysicalPixel()
    {
        Assert.Equal(10.5, TimelineFrameGrid.SnapToDevicePixel(10.9, 1.0), 9);
        // 125% scaling: 1 physical pixel = 0.8 DIP, line centered at (12 + 0.5) / 1.25
        Assert.Equal(10.0, TimelineFrameGrid.SnapToDevicePixel(9.9, 1.25), 9);
    }
}
