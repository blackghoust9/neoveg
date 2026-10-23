using System;
using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;
using Neoveg.Interop;
using Neoveg.Models;

namespace Neoveg.ViewModels;

/// <summary>
/// MVVM ViewModel for the Audio Master Bus panel (<see cref="Neoveg.Views.MasterBusView"/>).
/// Strictly bound to real-time stereo audio telemetry emitted by the native FFI audio pipeline
/// (<c>rust-core</c> / <c>cpp-extensions</c>).
/// Contains zero timers, random generators, or simulated signals: when playback is stopped or
/// no active audio media exists on the timeline, Left (L) and Right (R) meters remain static at 0.0 (-∞ dB).
/// </summary>
public sealed class MasterBusViewModel : INotifyPropertyChanged
{
    private double _leftLevelLinear;
    private double _rightLevelLinear;
    private double _leftPeakDb = double.NegativeInfinity;
    private double _rightPeakDb = double.NegativeInfinity;
    private double _masterVolume = 1.0;
    private bool _isPlaying;

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>
    /// Real-time Left (L) channel linear peak amplitude in [0.0, 1.0]. Always 0.0 when stopped or silent.
    /// </summary>
    public double LeftLevelLinear
    {
        get => _leftLevelLinear;
        private set
        {
            double clamped = double.IsFinite(value) ? Math.Clamp(value, 0.0, 1.0) : 0.0;
            if (Math.Abs(_leftLevelLinear - clamped) > 1e-6)
            {
                _leftLevelLinear = clamped;
                OnPropertyChanged();
                OnPropertyChanged(nameof(LeftPeakDbText));
            }
        }
    }

    /// <summary>
    /// Real-time Right (R) channel linear peak amplitude in [0.0, 1.0]. Always 0.0 when stopped or silent.
    /// </summary>
    public double RightLevelLinear
    {
        get => _rightLevelLinear;
        private set
        {
            double clamped = double.IsFinite(value) ? Math.Clamp(value, 0.0, 1.0) : 0.0;
            if (Math.Abs(_rightLevelLinear - clamped) > 1e-6)
            {
                _rightLevelLinear = clamped;
                OnPropertyChanged();
                OnPropertyChanged(nameof(RightPeakDbText));
            }
        }
    }

    /// <summary>
    /// Real-time Left (L) channel peak level in dBFS (-∞ dB when stopped or silent).
    /// </summary>
    public double LeftPeakDb
    {
        get => _leftPeakDb;
        private set
        {
            if (_leftPeakDb != value)
            {
                _leftPeakDb = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(LeftPeakDbText));
            }
        }
    }

    /// <summary>
    /// Real-time Right (R) channel peak level in dBFS (-∞ dB when stopped or silent).
    /// </summary>
    public double RightPeakDb
    {
        get => _rightPeakDb;
        private set
        {
            if (_rightPeakDb != value)
            {
                _rightPeakDb = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(RightPeakDbText));
            }
        }
    }

    public string LeftPeakDbText => FormatDbReadout(_leftPeakDb, _leftLevelLinear);

    public string RightPeakDbText => FormatDbReadout(_rightPeakDb, _rightLevelLinear);

    public double MasterVolume
    {
        get => _masterVolume;
        set
        {
            double clamped = double.IsFinite(value) ? Math.Clamp(value, 0.0, 1.4) : 1.0;
            if (Math.Abs(_masterVolume - clamped) > 1e-6)
            {
                _masterVolume = clamped;
                OnPropertyChanged();
                OnPropertyChanged(nameof(MasterBusDbLabel));
            }
        }
    }

    public string MasterBusDbLabel
    {
        get
        {
            if (_masterVolume <= 0.001)
                return "MASTER BUS: -∞ dB";

            double db = 20.0 * Math.Log10(_masterVolume);
            string sign = db >= 0.0 ? "+" : "";
            return string.Create(CultureInfo.InvariantCulture, $"MASTER BUS: {sign}{db:0.0} dB");
        }
    }

    public bool IsPlaying
    {
        get => _isPlaying;
        private set
        {
            if (_isPlaying != value)
            {
                _isPlaying = value;
                OnPropertyChanged();
            }
        }
    }

    /// <summary>
    /// Resets both Left (L) and Right (R) VU meters to 0.0 linear (-∞ dB).
    /// </summary>
    public void ResetMetersToNegativeInfinity()
    {
        IsPlaying = false;
        LeftLevelLinear = 0.0;
        RightLevelLinear = 0.0;
        LeftPeakDb = double.NegativeInfinity;
        RightPeakDb = double.NegativeInfinity;
    }

    /// <summary>
    /// Updates the Master Bus Left (L) and Right (R) peak levels exclusively from the native FFI audio engine.
    /// If playback is stopped or there is no active audio media on the timeline, meters are strictly zeroed (-∞ dB).
    /// </summary>
    public void UpdateFromRealTimeAudioTelemetry(
        VegasProject? project,
        bool isPlaying,
        RustCoreInterop? rustInterop)
    {
        if (project != null)
        {
            MasterVolume = project.MasterVolume;
        }

        IsPlaying = isPlaying;

        if (!isPlaying || project == null || rustInterop == null)
        {
            LeftLevelLinear = 0.0;
            RightLevelLinear = 0.0;
            LeftPeakDb = double.NegativeInfinity;
            RightPeakDb = double.NegativeInfinity;
            return;
        }

        var (leftLin, rightLin, leftDb, rightDb) = rustInterop.ComputeMasterBusStereoTelemetry(project, isPlaying);
        LeftLevelLinear = leftLin;
        RightLevelLinear = rightLin;
        LeftPeakDb = leftDb;
        RightPeakDb = rightDb;
    }

    public static string FormatDbReadout(double dbValue, double linearLevel)
    {
        if (linearLevel <= 1e-6 || !double.IsFinite(dbValue) || dbValue <= -120.0)
        {
            return "-∞ dB";
        }
        return string.Create(CultureInfo.InvariantCulture, $"{dbValue:0.0} dB");
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
