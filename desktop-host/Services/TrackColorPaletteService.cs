using System;
using System.Collections.Generic;
using Avalonia.Media;

namespace Neoveg.Services;

public sealed record TrackColorPreset(string Name, string HexCode);

/// <summary>
/// Curated Automatic Track Palette Assignment Service (`TrackColorPaletteService.cs`).
/// Assigns high-contrast accent colors to Video and Audio tracks (`VideoTrackViewModel` / `AudioTrackViewModel`),
/// guarantees adjacent tracks never repeat the same color, and computes soft Clip Header / Body / Border tints.
/// </summary>
public sealed class TrackColorPaletteService
{
    public static readonly IReadOnlyList<TrackColorPreset> CuratedPalette = new[]
    {
        new TrackColorPreset("Azul Elétrico (Electric Blue)", "#3498DB"),
        new TrackColorPreset("Verde Esmeralda (Emerald Green)", "#2ECC71"),
        new TrackColorPreset("Laranja (Vivid Orange)", "#E67E22"),
        new TrackColorPreset("Roxo (Amethyst Purple)", "#9B59B6"),
        new TrackColorPreset("Ciano (Turquoise Cyan)", "#1ABC9C"),
        new TrackColorPreset("Vermelho Coral (Coral Red)", "#E74C3C"),
        new TrackColorPreset("Amarelo Ocre (Ochre Yellow)", "#F1C40F")
    };

    public static TrackColorPaletteService Instance { get; } = new();

    private string? _lastAssignedColorHex;

    /// <summary>
    /// Returns the next curated track accent color cyclically by `trackIndex`,
    /// guaranteeing that the assigned color never repeats the immediately preceding track's color.
    /// </summary>
    public string GetNextTrackColor(int trackIndex, string? previousTrackColor = null)
    {
        int count = CuratedPalette.Count;
        int idx = Math.Abs(trackIndex) % count;
        string candidate = CuratedPalette[idx].HexCode;

        string? comparePrev = !string.IsNullOrWhiteSpace(previousTrackColor)
            ? previousTrackColor
            : _lastAssignedColorHex;

        if (!string.IsNullOrWhiteSpace(comparePrev) &&
            string.Equals(candidate, comparePrev.Trim(), StringComparison.OrdinalIgnoreCase))
        {
            idx = (idx + 1) % count;
            candidate = CuratedPalette[idx].HexCode;
        }

        _lastAssignedColorHex = candidate;
        return candidate;
    }

    public static Color ParseSafeColor(string? hex, string fallbackHex = "#3498DB")
    {
        if (!string.IsNullOrWhiteSpace(hex) && Color.TryParse(hex.Trim(), out Color parsed))
            return parsed;
        return Color.Parse(fallbackHex);
    }

    public static Color ParseAccentColor(string? hex, string fallbackHex = "#3498DB") =>
        ParseSafeColor(hex, fallbackHex);

    /// <summary>
    /// Computes the Clip Header fill color derived from the parent track's `TrackAccentColor`.
    /// </summary>
    public static Color GetClipHeaderTint(string? trackAccentHex, bool isMuted = false)
    {
        var baseCol = ParseSafeColor(trackAccentHex);
        byte alpha = isMuted ? (byte)155 : (byte)242;
        return Color.FromArgb(
            alpha,
            (byte)Math.Clamp((int)(baseCol.R * 0.82), 18, 240),
            (byte)Math.Clamp((int)(baseCol.G * 0.82), 18, 240),
            (byte)Math.Clamp((int)(baseCol.B * 0.82), 18, 240));
    }

    /// <summary>
    /// Computes a soft translucent Clip Body tint based on the parent track's `TrackAccentColor`.
    /// </summary>
    public static Color GetClipBodyTint(string? trackAccentHex, bool isMuted = false)
    {
        var baseCol = ParseSafeColor(trackAccentHex);
        byte alpha = isMuted ? (byte)75 : (byte)148;
        return Color.FromArgb(
            alpha,
            (byte)Math.Clamp(baseCol.R / 2 + 16, 20, 210),
            (byte)Math.Clamp(baseCol.G / 2 + 20, 20, 210),
            (byte)Math.Clamp(baseCol.B / 2 + 26, 24, 220));
    }

    /// <summary>
    /// Computes the Clip Border tint derived from the parent track's `TrackAccentColor`.
    /// </summary>
    public static Color GetClipBorderTint(string? trackAccentHex, bool isSelected = false)
    {
        if (isSelected)
            return Color.Parse("#facc15");

        var baseCol = ParseSafeColor(trackAccentHex);
        return Color.FromArgb(
            235,
            (byte)Math.Min(255, baseCol.R + 28),
            (byte)Math.Min(255, baseCol.G + 28),
            (byte)Math.Min(255, baseCol.B + 28));
    }
}
