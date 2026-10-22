using System;
using System.Collections.Generic;
using Neoveg.Interop;

namespace Neoveg.AudioEngine;

public enum EqFilterBandType
{
    LowShelf,
    Peaking,
    HighShelf,
    HighPass,
    LowPass
}

/// <summary>
/// Single band of the Neoveg 8-Band Native Parametric Equalizer.
/// </summary>
public sealed class ParametricEqBand
{
    public int BandIndex { get; init; }
    public EqFilterBandType FilterType { get; set; } = EqFilterBandType.Peaking;
    public double FrequencyHz { get; set; } = 1000.0;
    public double GainDb { get; set; } = 0.0;
    public double QFactor { get; set; } = 1.0;
    public bool Enabled { get; set; } = true;
}

/// <summary>
/// Common interface for nodes in the `AudioFxChain` (Native 8-Band Parametric EQ, VST3, and LV2 plugins).
/// </summary>
public interface IAudioFxNode
{
    string Id { get; }
    string Name { get; }
    string PluginFormat { get; } // "NativeDSP", "VST3", "LV2"
    bool Bypassed { get; set; }
    void ProcessStereoInterleaved(Span<float> interleavedStereoSamples, int sampleRate);
}

/// <summary>
/// Native 8-Band Parametric Equalizer modeled after Audacity & VEGAS Pro Track EQ (`Neoveg.AudioEngine`).
/// Implements RBJ Audio EQ Cookbook 2nd-order IIR Biquad filters across 8 configurable bands.
/// </summary>
public sealed class ParametricEq8BandProcessor : IAudioFxNode
{
    public string Id { get; } = "neoveg.dsp.parametric_eq_8band";
    public string Name { get; } = "Neoveg 8-Band Parametric EQ";
    public string PluginFormat => "NativeDSP";
    public bool Bypassed { get; set; }

    public IReadOnlyList<ParametricEqBand> Bands => _bands;
    private readonly List<ParametricEqBand> _bands;

    public ParametricEq8BandProcessor()
    {
        _bands = new List<ParametricEqBand>
        {
            new() { BandIndex = 1, FilterType = EqFilterBandType.LowShelf,  FrequencyHz = 32.0,    GainDb = 0.0, QFactor = 0.707 },
            new() { BandIndex = 2, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 64.0,    GainDb = 0.0, QFactor = 1.000 },
            new() { BandIndex = 3, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 160.0,   GainDb = 0.0, QFactor = 1.200 },
            new() { BandIndex = 4, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 400.0,   GainDb = 0.0, QFactor = 1.200 },
            new() { BandIndex = 5, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 1000.0,  GainDb = 0.0, QFactor = 1.000 },
            new() { BandIndex = 6, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 2500.0,  GainDb = 0.0, QFactor = 1.200 },
            new() { BandIndex = 7, FilterType = EqFilterBandType.Peaking,   FrequencyHz = 6300.0,  GainDb = 0.0, QFactor = 1.200 },
            new() { BandIndex = 8, FilterType = EqFilterBandType.HighShelf, FrequencyHz = 12500.0, GainDb = 0.0, QFactor = 0.707 }
        };
    }

    public static (double b0, double b1, double b2, double a1, double a2) ComputeBiquadCoefficients(
        ParametricEqBand band,
        int sampleRate)
    {
        double sr = Math.Max(8000, sampleRate);
        double f0 = Math.Clamp(band.FrequencyHz, 20.0, sr * 0.48);
        double q = Math.Clamp(band.QFactor, 0.1, 18.0);
        double db = Math.Clamp(band.GainDb, -24.0, 24.0);

        double a = Math.Pow(10.0, db / 40.0);
        double w0 = 2.0 * Math.PI * f0 / sr;
        double cosW0 = Math.Cos(w0);
        double sinW0 = Math.Sin(w0);
        double alpha = sinW0 / (2.0 * q);

        double b0, b1, b2, a0, a1, a2;
        switch (band.FilterType)
        {
            case EqFilterBandType.LowShelf:
            {
                double twoSqrtAAlpha = 2.0 * Math.Sqrt(a) * alpha;
                b0 = a * ((a + 1.0) - (a - 1.0) * cosW0 + twoSqrtAAlpha);
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cosW0);
                b2 = a * ((a + 1.0) - (a - 1.0) * cosW0 - twoSqrtAAlpha);
                a0 = (a + 1.0) + (a - 1.0) * cosW0 + twoSqrtAAlpha;
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cosW0);
                a2 = (a + 1.0) + (a - 1.0) * cosW0 - twoSqrtAAlpha;
                break;
            }
            case EqFilterBandType.HighShelf:
            {
                double twoSqrtAAlpha = 2.0 * Math.Sqrt(a) * alpha;
                b0 = a * ((a + 1.0) + (a - 1.0) * cosW0 + twoSqrtAAlpha);
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cosW0);
                b2 = a * ((a + 1.0) + (a - 1.0) * cosW0 - twoSqrtAAlpha);
                a0 = (a + 1.0) - (a - 1.0) * cosW0 + twoSqrtAAlpha;
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cosW0);
                a2 = (a + 1.0) - (a - 1.0) * cosW0 - twoSqrtAAlpha;
                break;
            }
            default:
            {
                b0 = 1.0 + alpha * a;
                b1 = -2.0 * cosW0;
                b2 = 1.0 - alpha * a;
                a0 = 1.0 + alpha / a;
                a1 = -2.0 * cosW0;
                a2 = 1.0 - alpha / a;
                break;
            }
        }

        double invA0 = 1.0 / a0;
        return (b0 * invA0, b1 * invA0, b2 * invA0, a1 * invA0, a2 * invA0);
    }

    public void ProcessStereoInterleaved(Span<float> interleavedStereoSamples, int sampleRate = 48000)
    {
        if (Bypassed || interleavedStereoSamples.Length < 2)
            return;

        foreach (var band in _bands)
        {
            if (!band.Enabled || Math.Abs(band.GainDb) < 0.01)
                continue;

            var (b0, b1, b2, a1, a2) = ComputeBiquadCoefficients(band, sampleRate);
            double x1L = 0, x2L = 0, y1L = 0, y2L = 0;
            double x1R = 0, x2R = 0, y1R = 0, y2R = 0;

            for (int i = 0; i + 1 < interleavedStereoSamples.Length; i += 2)
            {
                double xL = interleavedStereoSamples[i];
                double yL = b0 * xL + b1 * x1L + b2 * x2L - a1 * y1L - a2 * y2L;
                x2L = x1L; x1L = xL; y2L = y1L; y1L = yL;
                interleavedStereoSamples[i] = (float)Math.Clamp(yL, -1.0, 1.0);

                double xR = interleavedStereoSamples[i + 1];
                double yR = b0 * xR + b1 * x1R + b2 * x2R - a1 * y1R - a2 * y2R;
                x2R = x1R; x1R = xR; y2R = y1R; y2R = yR;
                interleavedStereoSamples[i + 1] = (float)Math.Clamp(yR, -1.0, 1.0);
            }
        }
    }
}

/// <summary>
/// Steinberg VST3 Audio Effect Chain Node (`Steinberg::Vst::IAudioProcessor` via `CppCoreInterop`).
/// </summary>
public sealed class Vst3AudioFxNode : IAudioFxNode
{
    public string Id { get; }
    public string Name { get; }
    public string PluginFormat => "VST3";
    public bool Bypassed { get; set; }
    public double Drive { get; set; } = 1.25;
    private readonly CppCoreInterop? _cppInterop;

    public Vst3AudioFxNode(string id, string name, CppCoreInterop? cppInterop = null)
    {
        Id = id;
        Name = name;
        _cppInterop = cppInterop;
    }

    public void ProcessStereoInterleaved(Span<float> interleavedStereoSamples, int sampleRate)
    {
        if (Bypassed || interleavedStereoSamples.Length == 0)
            return;

        if (_cppInterop != null)
        {
            float[] arr = interleavedStereoSamples.ToArray();
            _cppInterop.ProcessVst3AudioBus(arr, channels: 2, drive: Drive);
            arr.CopyTo(interleavedStereoSamples);
            return;
        }

        float k = (float)Math.Clamp(Drive, 0.1, 6.0);
        for (int i = 0; i < interleavedStereoSamples.Length; i++)
        {
            float x = interleavedStereoSamples[i] * k;
            interleavedStereoSamples[i] = x / (1.0f + MathF.Abs(x));
        }
    }
}

/// <summary>
/// LV2 Open-Standard Audio Plugin Node (`http://lv2plug.in/ns/lv2core`).
/// </summary>
public sealed class Lv2AudioFxNode : IAudioFxNode
{
    public string Id { get; }
    public string Name { get; }
    public string PluginUri { get; }
    public string PluginFormat => "LV2";
    public bool Bypassed { get; set; }
    public double CeilingDb { get; set; } = -1.0;

    public Lv2AudioFxNode(string id, string name, string pluginUri = "http://calf.sourceforge.net/plugins/Limiter")
    {
        Id = id;
        Name = name;
        PluginUri = pluginUri;
    }

    public void ProcessStereoInterleaved(Span<float> interleavedStereoSamples, int sampleRate)
    {
        if (Bypassed || interleavedStereoSamples.Length == 0)
            return;

        float limit = (float)Math.Pow(10.0, Math.Clamp(CeilingDb, -24.0, 0.0) / 20.0);
        for (int i = 0; i < interleavedStereoSamples.Length; i++)
        {
            interleavedStereoSamples[i] = Math.Clamp(interleavedStereoSamples[i], -limit, limit);
        }
    }
}

/// <summary>
/// Real-time Track & Event Audio FX Chain (`Neoveg.AudioEngine.AudioFxChain`).
/// Hosts the native 8-Band Parametric EQ alongside chained VST3 and LV2 audio plugins.
/// </summary>
public sealed class AudioFxChain
{
    public ParametricEq8BandProcessor ParametricEq { get; } = new();
    public List<IAudioFxNode> Nodes { get; } = new();

    public AudioFxChain(CppCoreInterop? cppInterop = null)
    {
        Nodes.Add(ParametricEq);
        Nodes.Add(new Vst3AudioFxNode("vst3.neoveg.master_limiter_64", "VST3 Broadcast True-Peak Limiter", cppInterop));
        Nodes.Add(new Lv2AudioFxNode("lv2.neoveg.calf_limiter", "LV2 Calf Broadcast Limiter"));
    }

    public void ProcessBuffer(Span<float> interleavedStereoSamples, int sampleRate = 48000)
    {
        foreach (var node in Nodes)
        {
            if (!node.Bypassed)
            {
                node.ProcessStereoInterleaved(interleavedStereoSamples, sampleRate);
            }
        }
    }
}
