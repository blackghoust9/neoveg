using System;
using System.Collections.Generic;
using System.Linq;
using Avalonia;
using Avalonia.Media;
using Neoveg.Models;

namespace Neoveg.Services;

/// <summary>
/// Gerenciador de Temas Visuais Baseado na História Oficial do Neoveg (1999–2026) no Avalonia UI:
/// - Era 1: Tema Sonic Foundry (Neoveg 1–3): Cinza industrial metálico com detalhes em vermelho e bordas chanfradas clássicas (3D bevel).
/// - Era 2: Tema Classic NLE Classic / Silver (Neoveg 4–13): Tons de prata refinado e o clássico degradê flamejante nas barras e ícones.
/// - Era 3: Tema Modern NLE Modern Dark (Neoveg 14–22): Padrão cinza escuro profissional com detalhes em azul ciano e menus compactos flat design.
/// - Era 4: Tema Contemporary NLE / Atual (2026): Visual contemporâneo em azul elétrico (#00A3FF) e fundo totalmente escuro (#0E1015).
/// </summary>
public sealed class NeovegThemeManager
{
    public IReadOnlyList<NeovegHistoryTheme> Themes { get; }
    public NeovegHistoryTheme ActiveTheme { get; private set; }
    public event Action<NeovegHistoryTheme>? OnThemeChanged;

    public NeovegThemeManager(string rootDir)
    {
        var loaded = NeovegHistoryThemeCatalog.LoadThemes(rootDir);
        Themes = loaded;
        ActiveTheme = loaded.FirstOrDefault(t => t.Id == "contemporary_2026_electric_dark")
                      ?? loaded.LastOrDefault()
                      ?? new NeovegHistoryTheme();
    }

    public NeovegHistoryTheme GetThemeByEra(int eraNumber)
    {
        string preferredId = eraNumber switch
        {
            1 => "sonic_foundry_v1_v3",
            2 => "classic_v4_v7_flaming_gold",
            3 => "modern_v16_v22_carbon_cyan",
            4 => "contemporary_2026_electric_dark",
            _ => "contemporary_2026_electric_dark"
        };

        return Themes.FirstOrDefault(t => t.Id == preferredId)
               ?? Themes.FirstOrDefault(t => t.EraNumber == eraNumber)
               ?? ActiveTheme;
    }

    public NeovegHistoryTheme GetThemeById(string id)
    {
        return Themes.FirstOrDefault(t => string.Equals(t.Id, id, StringComparison.OrdinalIgnoreCase))
               ?? ActiveTheme;
    }

    public void SetActiveTheme(NeovegHistoryTheme theme)
    {
        ActiveTheme = theme;
        OnThemeChanged?.Invoke(theme);
    }

    /// <summary>
    /// Creates the era-authentic brush for dock headers:
    /// - Era 1 (Sonic Foundry): Industrial Metallic Bevel Gradient (#9A9792 -> #6E6C68)
    /// - Era 2 (Classic NLE Classic/Silver): Classic Flaming Gold / Silver Accent Gradient
    /// - Era 3 (Modern NLE Modern Dark): Flat Compact Dark Gray (#333337)
    /// - Era 4 (Contemporary NLE 2026): Deep Obsidian + Electric Blue tint (#1B202C -> #111827)
    /// </summary>
    public static IBrush CreateDockHeaderBrush(NeovegHistoryTheme theme)
    {
        var baseHdr = Color.Parse(theme.DockHeaderBg);
        var acc1 = Color.Parse(theme.AccentPrimary);
        var acc2 = Color.Parse(theme.AccentSecondary);

        return theme.EraNumber switch
        {
            1 => new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(0, 1, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(Color.Parse("#A4A19B"), 0.0),
                    new GradientStop(baseHdr, 0.7),
                    new GradientStop(Color.Parse("#5E5C58"), 1.0)
                }
            },
            2 => new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(1, 0, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(baseHdr, 0.0),
                    new GradientStop(baseHdr, 0.68),
                    new GradientStop(Color.FromArgb(180, acc1.R, acc1.G, acc1.B), 0.90),
                    new GradientStop(Color.FromArgb(210, acc2.R, acc2.G, acc2.B), 1.0)
                }
            },
            3 => new SolidColorBrush(baseHdr), // Modern NLE Flat Design
            _ => new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(1, 0, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(baseHdr, 0.0),
                    new GradientStop(Color.FromArgb(70, acc1.R, acc1.G, acc1.B), 1.0)
                }
            }
        };
    }

    /// <summary>
    /// Creates the era-authentic brush for the main icon toolbar.
    /// </summary>
    public static IBrush CreateToolbarBrush(NeovegHistoryTheme theme)
    {
        var tbCol = Color.Parse(theme.ToolbarBg);
        if (theme.EraNumber == 2)
        {
            var acc2 = Color.Parse(theme.AccentSecondary);
            return new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(0, 1, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(tbCol, 0.0),
                    new GradientStop(Color.FromArgb(40, acc2.R, acc2.G, acc2.B), 1.0)
                }
            };
        }
        return new SolidColorBrush(tbCol);
    }

    /// <summary>
    /// Returns the border thickness according to the historical era:
    /// Sonic Foundry (1-3) uses 2px classic 3D beveled borders; Modern NLE (14-22) and Contemporary NLE (2026) use 1px flat borders.
    /// </summary>
    public static Thickness GetDockBorderThickness(NeovegHistoryTheme theme)
    {
        return theme.EraNumber == 1 ? new Thickness(2, 2, 2, 2) : new Thickness(1);
    }

    /// <summary>
    /// Returns compact menu padding for Modern NLE Flat Design (Era 3) vs classic spacing.
    /// </summary>
    public static Thickness GetMenuPadding(NeovegHistoryTheme theme)
    {
        return theme.EraNumber == 3 ? new Thickness(4, 1) : new Thickness(6, 2);
    }
}
