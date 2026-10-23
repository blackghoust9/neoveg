using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Headless.XUnit;
using Xunit;

[assembly: AvaloniaTestApplication(typeof(Neoveg.Tests.HeadlessTestApplication))]

namespace Neoveg.Tests;

public sealed class HeadlessTestApplication
{
    public static AppBuilder BuildAvaloniaApp() =>
        AppBuilder.Configure<HeadlessTestApp>()
            .UseHeadless(new AvaloniaHeadlessPlatformOptions());

    [AvaloniaFact]
    public void Headless_platform_can_create_controls()
    {
        var button = new Button { Content = "Neoveg" };

        Assert.Equal("Neoveg", button.Content);
    }

    [AvaloniaFact]
    public void MasterBus_meters_are_zeroed_when_stopped_or_without_audio_and_respond_to_real_telemetry()
    {
        using var rustInterop = new Neoveg.Interop.RustCoreInterop(System.AppContext.BaseDirectory);
        var emptyProject = Neoveg.Models.VegasProject.CreateEmpty("Teste Master Bus");
        var view = new Neoveg.Views.MasterBusView(emptyProject, rustInterop);

        // 1. Stopped on empty timeline -> strictly 0.0 linear / -∞ dB
        view.IsPlaying = false;
        Assert.Equal(0.0, view.ViewModel.LeftLevelLinear);
        Assert.Equal(0.0, view.ViewModel.RightLevelLinear);
        Assert.True(double.IsNegativeInfinity(view.ViewModel.LeftPeakDb));
        Assert.True(double.IsNegativeInfinity(view.ViewModel.RightPeakDb));
        Assert.Equal("-∞ dB", view.ViewModel.LeftPeakDbText);
        Assert.Equal("-∞ dB", view.ViewModel.RightPeakDbText);

        // 2. Playing on empty timeline -> still strictly 0.0 linear / -∞ dB (no fake oscillation)
        view.IsPlaying = true;
        Assert.Equal(0.0, view.ViewModel.LeftLevelLinear);
        Assert.Equal(0.0, view.ViewModel.RightLevelLinear);
        Assert.Equal("-∞ dB", view.ViewModel.LeftPeakDbText);
        Assert.Equal("-∞ dB", view.ViewModel.RightPeakDbText);

        // 3. Playing with an active unmuted audio clip on the timeline -> responds to real clip telemetry
        var audioTrack = new Neoveg.Models.TimelineTrack
        {
            Name = "Audio 1",
            Type = "audio",
            Level = 1.0,
            Pan = 0.0
        };
        audioTrack.Clips.Add(new Neoveg.Models.TimelineClip
        {
            Name = "Tone.wav",
            MediaType = "audio",
            Start = 0.0,
            Duration = 4.0,
            Peaks = new System.Collections.Generic.List<float> { 0.5f, 0.5f, 0.5f, 0.5f }
        });
        emptyProject.Tracks.Add(audioTrack);
        emptyProject.Playhead = 1.0;

        view.RefreshTelemetry();
        Assert.Equal(0.5, view.ViewModel.LeftLevelLinear, 3);
        Assert.Equal(0.5, view.ViewModel.RightLevelLinear, 3);
        Assert.NotEqual("-∞ dB", view.ViewModel.LeftPeakDbText);

        // 4. Stopping playback immediately zeroes the meters back to -∞ dB
        view.IsPlaying = false;
        Assert.Equal(0.0, view.ViewModel.LeftLevelLinear);
        Assert.Equal(0.0, view.ViewModel.RightLevelLinear);
        Assert.Equal("-∞ dB", view.ViewModel.LeftPeakDbText);
        Assert.Equal("-∞ dB", view.ViewModel.RightPeakDbText);
    }
}

public sealed class HeadlessTestApp : Application
{
}
