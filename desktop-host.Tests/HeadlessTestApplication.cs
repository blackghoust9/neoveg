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
        var emptyProject = Neoveg.Models.NeovegProject.CreateEmpty("Teste Master Bus");
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

    [AvaloniaFact]
    public void NeovegPrecisionSlider_has_flat_square_thumb_double_click_reset_ctrl_fine_drag_and_context_menu()
    {
        // 1. Opacity / Volume slider (0..1, default 1.0 = 100%)
        var levelSlider = Neoveg.Controls.NeovegPrecisionSlider.Create(
            minimum: 0.0,
            maximum: 1.0,
            initialValue: 0.45,
            defaultValue: 1.0,
            parameterLabel: "Opacidade da Faixa (%)",
            width: 100,
            displayAsPercentage: true);

        Assert.Equal(new CornerRadius(0), levelSlider.CornerRadius);
        Assert.Equal(0.45, levelSlider.Value, 3);
        Assert.Equal(1.0, levelSlider.DefaultValue, 3);

        // Double-click / ResetToDefault restores 100% (1.0)
        levelSlider.ResetToDefault();
        Assert.Equal(1.0, levelSlider.Value, 3);

        // 2. Pan slider (-1..+1, default 0.0)
        var panSlider = Neoveg.Controls.NeovegPrecisionSlider.Create(
            minimum: -1.0,
            maximum: 1.0,
            initialValue: -0.6,
            defaultValue: 0.0,
            parameterLabel: "Balanço Estéreo (Pan)",
            width: 100,
            displayAsPercentage: false);

        panSlider.ResetToDefault();
        Assert.Equal(0.0, panSlider.Value, 3);

        // 3. Ctrl Fine Drag reduces movement sensitivity to 1/10th (FineStepFactor = 0.1)
        // Moving +10px across a 100px track over range 2.0 (-1..+1) normally moves +0.20,
        // so with 1/10th fine-drag it moves +0.02.
        double fineVal = panSlider.ApplyFineDragDelta(deltaPx: 10.0, trackWidth: 100.0);
        Assert.Equal(0.02, fineVal, 4);

        // 4. Right-click ContextMenu includes 'Restaurar Padrão' and 'Digitar Valor...'
        Assert.NotNull(levelSlider.ContextMenu);
        var headers = new System.Collections.Generic.List<string>();
        foreach (var item in levelSlider.ContextMenu!.Items)
        {
            if (item is MenuItem mi && mi.Header is string s)
            {
                headers.Add(s);
            }
        }
        Assert.Contains("Restaurar Padrão", headers);
        Assert.Contains("Digitar Valor...", headers);
    }
}

public sealed class HeadlessTestApp : Application
{
}
