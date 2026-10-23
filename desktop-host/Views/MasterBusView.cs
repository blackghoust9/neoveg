using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Neoveg.Controls;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.ViewModels;

namespace Neoveg.Views;

/// <summary>
/// Audio Master Bus Panel View (<c>desktop-host/Views/MasterBusView.axaml</c> / <c>MasterBusView.cs</c>).
/// Displays the Left (L) and Right (R) stereo dBFS peak meters and Master Volume fader without any RGB video scope.
/// Bound to <see cref="MasterBusViewModel"/> and driven strictly by real-time FFI audio telemetry.
/// </summary>
public sealed class MasterBusView : UserControl
{
    public MasterBusViewModel ViewModel { get; }
    public MasterScopeControl MeterControl { get; }

    private readonly TextBlock _leftPeakReadout;
    private readonly TextBlock _rightPeakReadout;
    private readonly TextBlock _masterBusDbLabel;
    private readonly Slider _masterVolumeSlider;
    private bool _updatingSliderFromModel;

    public NeovegProject? Project
    {
        get => MeterControl.Project;
        set
        {
            MeterControl.Project = value;
            if (value != null)
            {
                _updatingSliderFromModel = true;
                ViewModel.MasterVolume = value.MasterVolume;
                _masterVolumeSlider.Value = value.MasterVolume;
                _masterBusDbLabel.Text = ViewModel.MasterBusDbLabel;
                _updatingSliderFromModel = false;
            }
            RefreshTelemetry();
        }
    }

    public RustCoreInterop? RustInterop
    {
        get => MeterControl.RustInterop;
        set
        {
            MeterControl.RustInterop = value;
            RefreshTelemetry();
        }
    }

    public bool IsPlaying
    {
        get => MeterControl.IsPlaying;
        set
        {
            MeterControl.IsPlaying = value;
            RefreshTelemetry();
        }
    }

    public MasterBusView(
        NeovegProject? project = null,
        RustCoreInterop? rustInterop = null,
        MasterScopeControl? meterControl = null,
        MasterBusViewModel? viewModel = null)
    {
        ViewModel = viewModel ?? new MasterBusViewModel();
        DataContext = ViewModel;

        MeterControl = meterControl ?? new MasterScopeControl();
        MeterControl.ViewModel = ViewModel;
        MeterControl.Project = project;
        MeterControl.RustInterop = rustInterop;

        if (project != null)
        {
            ViewModel.MasterVolume = project.MasterVolume;
        }

        _leftPeakReadout = new TextBlock
        {
            Text = ViewModel.LeftPeakDbText,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 9.5,
            HorizontalAlignment = HorizontalAlignment.Center,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        };

        _rightPeakReadout = new TextBlock
        {
            Text = ViewModel.RightPeakDbText,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 9.5,
            HorizontalAlignment = HorizontalAlignment.Center,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        };

        _masterBusDbLabel = new TextBlock
        {
            Text = ViewModel.MasterBusDbLabel,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        };

        _masterVolumeSlider = NeovegPrecisionSlider.Create(
            minimum: 0.0,
            maximum: 1.4,
            initialValue: ViewModel.MasterVolume,
            defaultValue: 1.0,
            parameterLabel: "Volume do Master Bus",
            displayAsPercentage: true);

        _masterVolumeSlider.PropertyChanged += (_, e) =>
        {
            if (_updatingSliderFromModel || e.Property.Name != nameof(Slider.Value))
                return;

            ViewModel.MasterVolume = _masterVolumeSlider.Value;
            if (MeterControl.Project != null)
            {
                MeterControl.Project.MasterVolume = ViewModel.MasterVolume;
            }
            _masterBusDbLabel.Text = ViewModel.MasterBusDbLabel;
            RefreshTelemetry();
        };

        var grid = new Grid
        {
            RowDefinitions = new RowDefinitions("*,Auto,Auto"),
            Margin = new Thickness(6)
        };

        Grid.SetRow(MeterControl, 0);
        grid.Children.Add(MeterControl);

        var readoutGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,*"),
            Margin = new Thickness(0, 4, 0, 2)
        };
        Grid.SetColumn(_leftPeakReadout, 0);
        Grid.SetColumn(_rightPeakReadout, 1);
        readoutGrid.Children.Add(_leftPeakReadout);
        readoutGrid.Children.Add(_rightPeakReadout);
        Grid.SetRow(readoutGrid, 1);
        grid.Children.Add(readoutGrid);

        var faderStack = new StackPanel
        {
            Spacing = 4,
            Margin = new Thickness(0, 4, 0, 0)
        };
        faderStack.Children.Add(_masterBusDbLabel);
        faderStack.Children.Add(_masterVolumeSlider);
        Grid.SetRow(faderStack, 2);
        grid.Children.Add(faderStack);

        Content = grid;
        RefreshTelemetry();
    }

    public void RefreshTelemetry()
    {
        ViewModel.UpdateFromRealTimeAudioTelemetry(MeterControl.Project, MeterControl.IsPlaying, MeterControl.RustInterop);
        MeterControl.LeftLevelLinear = ViewModel.LeftLevelLinear;
        MeterControl.RightLevelLinear = ViewModel.RightLevelLinear;
        MeterControl.LeftPeakDb = ViewModel.LeftPeakDb;
        MeterControl.RightPeakDb = ViewModel.RightPeakDb;

        _leftPeakReadout.Text = ViewModel.LeftPeakDbText;
        _rightPeakReadout.Text = ViewModel.RightPeakDbText;
        _masterBusDbLabel.Text = ViewModel.MasterBusDbLabel;

        MeterControl.InvalidateVisual();
    }
}
