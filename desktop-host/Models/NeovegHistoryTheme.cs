using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;

namespace Neoveg.Models;

/// <summary>
/// Represents a historical Neoveg UI theme (Sonic Foundry 1-3, Classic NLE 4-13, Modern NLE 14-22, Contemporary NLE 2026)
/// loaded from `assets/neoveg_history_themes/theme_catalog.json`.
/// </summary>
public sealed class NeovegHistoryTheme
{
    public string Id { get; set; } = "contemporary_2026_electric_dark";
    public string EraFolder { get; set; } = "04_era_contemporary_2026";
    public int EraNumber { get; set; } = 4;
    public string Name { get; set; } = "Contemporary NLE / Neoveg Atual (2025–2026 Electric Blue)";
    public string ShortBadge { get; set; } = "Contemporary NLE • 2026";
    public string Years { get; set; } = "2025–2026";
    public string Vendor { get; set; } = "Neoveg Creative Software / Contemporary NLE Ecosystem";
    public string Description { get; set; } = "";

    // Semantic Color Palette
    public string WindowBg { get; set; } = "#0E1015";
    public string MenuBg { get; set; } = "#13161E";
    public string ToolbarBg { get; set; } = "#161A24";
    public string DockHeaderBg { get; set; } = "#1B202C";
    public string DockPanelBg { get; set; } = "#12161F";
    public string DockBorder { get; set; } = "#262F40";
    public string TimelineBg { get; set; } = "#0B0D12";
    public string RulerBg { get; set; } = "#151922";
    public string TrackVideoBg { get; set; } = "#172336";
    public string TrackAudioBg { get; set; } = "#191D2E";
    public string ClipHeaderBg { get; set; } = "#090D16";
    public string AccentPrimary { get; set; } = "#00A3FF";
    public string AccentSecondary { get; set; } = "#38BDF8";
    public string TimecodeBg { get; set; } = "#090B10";
    public string TimecodeFg { get; set; } = "#38BDF8";
    public string PlayheadColor { get; set; } = "#00A3FF";
    public string WaveformColor { get; set; } = "#7DD3FC";
    public string ButtonBg { get; set; } = "#1E2636";
    public string ButtonFg { get; set; } = "#F8FAFC";
    public string TextPrimary { get; set; } = "#F8FAFC";
    public string TextSecondary { get; set; } = "#94A3B8";
    public string StatusBarBg { get; set; } = "#10141C";

    // Relative Asset Paths inside `assets/neoveg_history_themes/`
    public string LogoSvgPath { get; set; } = "";
    public string LogoPngPath { get; set; } = "";
    public string IconsSvgPath { get; set; } = "";
    public string SwatchPngPath { get; set; } = "";

    public override string ToString() => Name;
}

public static class NeovegHistoryThemeCatalog
{
    public static List<NeovegHistoryTheme> LoadThemes(string rootDir)
    {
        string catalogPath = Path.Combine(rootDir, "assets", "neoveg_history_themes", "theme_catalog.json");
        if (File.Exists(catalogPath))
        {
            try
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(catalogPath));
                if (doc.RootElement.TryGetProperty("themes", out var arr) && arr.ValueKind == JsonValueKind.Array)
                {
                    var list = new List<NeovegHistoryTheme>();
                    foreach (var item in arr.EnumerateArray())
                    {
                        var t = new NeovegHistoryTheme
                        {
                            Id = GetStr(item, "id", "contemporary_2026_electric_dark"),
                            EraFolder = GetStr(item, "era_folder", "04_era_contemporary_2026"),
                            EraNumber = item.TryGetProperty("era_number", out var en) ? en.GetInt32() : 4,
                            Name = GetStr(item, "name", "Neoveg Theme"),
                            ShortBadge = GetStr(item, "short_badge", "Neoveg"),
                            Years = GetStr(item, "years", ""),
                            Vendor = GetStr(item, "vendor", ""),
                            Description = GetStr(item, "description", "")
                        };

                        if (item.TryGetProperty("palette", out var pal) && pal.ValueKind == JsonValueKind.Object)
                        {
                            t.WindowBg = GetStr(pal, "window_bg", t.WindowBg);
                            t.MenuBg = GetStr(pal, "menu_bg", t.MenuBg);
                            t.ToolbarBg = GetStr(pal, "toolbar_bg", t.ToolbarBg);
                            t.DockHeaderBg = GetStr(pal, "dock_header_bg", t.DockHeaderBg);
                            t.DockPanelBg = GetStr(pal, "dock_panel_bg", t.DockPanelBg);
                            t.DockBorder = GetStr(pal, "dock_border", t.DockBorder);
                            t.TimelineBg = GetStr(pal, "timeline_bg", t.TimelineBg);
                            t.RulerBg = GetStr(pal, "ruler_bg", t.RulerBg);
                            t.TrackVideoBg = GetStr(pal, "track_video_bg", t.TrackVideoBg);
                            t.TrackAudioBg = GetStr(pal, "track_audio_bg", t.TrackAudioBg);
                            t.ClipHeaderBg = GetStr(pal, "clip_header_bg", t.ClipHeaderBg);
                            t.accent_load(pal);
                        }

                        if (item.TryGetProperty("assets", out var assets) && assets.ValueKind == JsonValueKind.Object)
                        {
                            t.LogoSvgPath = GetStr(assets, "logo_svg", "");
                            t.LogoPngPath = GetStr(assets, "logo_png", "");
                            t.IconsSvgPath = GetStr(assets, "icons_svg", "");
                            t.SwatchPngPath = GetStr(assets, "swatch_png", "");
                        }

                        list.Add(t);
                    }

                    if (list.Count > 0)
                        return list;
                }
            }
            catch
            {
                // Fallback to built-in historical catalog below
            }
        }

        return GetBuiltInThemes();
    }

    private static void accent_load(this NeovegHistoryTheme t, JsonElement pal)
    {
        t.AccentPrimary = GetStr(pal, "accent_primary", t.AccentPrimary);
        t.AccentSecondary = GetStr(pal, "accent_secondary", t.AccentSecondary);
        t.TimecodeBg = GetStr(pal, "timecode_bg", t.TimecodeBg);
        t.TimecodeFg = GetStr(pal, "timecode_fg", t.TimecodeFg);
        t.PlayheadColor = GetStr(pal, "playhead_color", t.PlayheadColor);
        t.WaveformColor = GetStr(pal, "waveform_color", t.WaveformColor);
        t.ButtonBg = GetStr(pal, "button_bg", t.ButtonBg);
        t.ButtonFg = GetStr(pal, "button_fg", t.ButtonFg);
        t.TextPrimary = GetStr(pal, "text_primary", t.TextPrimary);
        t.TextSecondary = GetStr(pal, "text_secondary", t.TextSecondary);
        t.StatusBarBg = GetStr(pal, "status_bar_bg", t.StatusBarBg);
    }

    private static string GetStr(JsonElement el, string prop, string fallback)
    {
        return el.TryGetProperty(prop, out var v) && v.ValueKind == JsonValueKind.String
            ? v.GetString() ?? fallback
            : fallback;
    }

    public static List<NeovegHistoryTheme> GetBuiltInThemes() => new()
    {
        new NeovegHistoryTheme
        {
            Id = "sonic_foundry_v1_v3",
            EraFolder = "01_era_sonic_foundry_v1_v3",
            EraNumber = 1,
            Name = "Sonic Foundry Neoveg 1–3 (1999–2002)",
            ShortBadge = "SONIC FOUNDRY V3",
            Years = "1999–2002",
            Vendor = "Sonic Foundry, Inc.",
            WindowBg = "#B8B5AE", MenuBg = "#D4D0C8", ToolbarBg = "#C6C3BD",
            DockHeaderBg = "#84827E", DockPanelBg = "#CBC8C2", DockBorder = "#42413E",
            TimelineBg = "#A6A39C", RulerBg = "#94918A",
            TrackVideoBg = "#B4B9C2", TrackAudioBg = "#C2B4B2", ClipHeaderBg = "#3A3835",
            AccentPrimary = "#CC291B", AccentSecondary = "#FF6A00",
            TimecodeBg = "#181816", TimecodeFg = "#FF4500", PlayheadColor = "#CC291B",
            WaveformColor = "#8B1A10", ButtonBg = "#D4D0C8", ButtonFg = "#181816",
            TextPrimary = "#141413", TextSecondary = "#3E3D3A", StatusBarBg = "#84827E"
        },
        new NeovegHistoryTheme
        {
            Id = "classic_v4_v7_flaming_gold",
            EraFolder = "02_era_classic_silver_v4_v13",
            EraNumber = 2,
            Name = "Classic NLE 4–7 Flaming Gold (2003–2007)",
            ShortBadge = "NEOVEG Neoveg 7 GOLD",
            Years = "2003–2007",
            Vendor = "Classic NLE Media Software",
            WindowBg = "#DFDDD4", MenuBg = "#ECE9D8", ToolbarBg = "#E4E1D6",
            DockHeaderBg = "#C8C4B6", DockPanelBg = "#EAE7DC", DockBorder = "#9D998C",
            TimelineBg = "#F2F0E8", RulerBg = "#DCD9CE",
            TrackVideoBg = "#ADC2D9", TrackAudioBg = "#D4BBC0", ClipHeaderBg = "#2B4C6F",
            AccentPrimary = "#FF6D00", AccentSecondary = "#FFB300",
            TimecodeBg = "#23211D", TimecodeFg = "#FFB300", PlayheadColor = "#E65100",
            WaveformColor = "#7E222A", ButtonBg = "#DCD9CE", ButtonFg = "#1B1A17",
            TextPrimary = "#181715", TextSecondary = "#4E4C45", StatusBarBg = "#D4D0C4"
        },
        new NeovegHistoryTheme
        {
            Id = "classic_v8_v13_silver_dark",
            EraFolder = "02_era_classic_silver_v4_v13",
            EraNumber = 2,
            Name = "Classic NLE 8–13 Prata/Escuro (2008–2015)",
            ShortBadge = "NEOVEG NEOVEG 12",
            Years = "2008–2015",
            Vendor = "Classic NLE Creative Software",
            WindowBg = "#5A5D62", MenuBg = "#76797E", ToolbarBg = "#6B6E73",
            DockHeaderBg = "#4E5156", DockPanelBg = "#64676C", DockBorder = "#3B3D42",
            TimelineBg = "#787B80", RulerBg = "#5E6166",
            TrackVideoBg = "#6B86A2", TrackAudioBg = "#866B74", ClipHeaderBg = "#2B3A4A",
            AccentPrimary = "#3B82F6", AccentSecondary = "#F37021",
            TimecodeBg = "#383A3E", TimecodeFg = "#EAECEF", PlayheadColor = "#F37021",
            WaveformColor = "#1E3A5F", ButtonBg = "#52555A", ButtonFg = "#F3F4F6",
            TextPrimary = "#F3F4F6", TextSecondary = "#D1D5DB", StatusBarBg = "#4A4D52"
        },
        new NeovegHistoryTheme
        {
            Id = "modern_v14_v15_flat_gray",
            EraFolder = "03_era_modern_carbon_v14_v22",
            EraNumber = 3,
            Name = "Modern NLE 14–15 Flat Gray (2016–2017)",
            ShortBadge = "Modern NLE Neoveg 15",
            Years = "2016–2017",
            Vendor = "Modern NLE Software GmbH",
            WindowBg = "#7A7C80", MenuBg = "#94969A", ToolbarBg = "#888A8E",
            DockHeaderBg = "#686A6E", DockPanelBg = "#828488", DockBorder = "#525458",
            TimelineBg = "#727478", RulerBg = "#66686C",
            TrackVideoBg = "#6D879E", TrackAudioBg = "#8C6268", ClipHeaderBg = "#32363C",
            AccentPrimary = "#0088E8", AccentSecondary = "#00B4FF",
            TimecodeBg = "#4A4C50", TimecodeFg = "#FFFFFF", PlayheadColor = "#00A2FF",
            WaveformColor = "#E2E8F0", ButtonBg = "#686A6E", ButtonFg = "#FFFFFF",
            TextPrimary = "#F8FAFC", TextSecondary = "#E2E8F0", StatusBarBg = "#5E6064"
        },
        new NeovegHistoryTheme
        {
            Id = "modern_v16_v22_carbon_cyan",
            EraFolder = "03_era_modern_carbon_v14_v22",
            EraNumber = 3,
            Name = "Modern NLE 16–22 Carbon & Cyan (2018–2024)",
            ShortBadge = "Modern NLE Neoveg 21",
            Years = "2018–2024",
            Vendor = "Modern NLE / Neoveg Creative Software",
            WindowBg = "#232326", MenuBg = "#2D2D30", ToolbarBg = "#28282C",
            DockHeaderBg = "#333337", DockPanelBg = "#26262A", DockBorder = "#3E3E42",
            TimelineBg = "#1E1E21", RulerBg = "#2A2A2E",
            TrackVideoBg = "#3A4E63", TrackAudioBg = "#5C2F35", ClipHeaderBg = "#18181B",
            AccentPrimary = "#0078D7", AccentSecondary = "#18A0FB",
            TimecodeBg = "#1A1A1D", TimecodeFg = "#18A0FB", PlayheadColor = "#0099FF",
            WaveformColor = "#F3C6C8", ButtonBg = "#333338", ButtonFg = "#F1F5F9",
            TextPrimary = "#F1F5F9", TextSecondary = "#9CA3AF", StatusBarBg = "#1C1C1F"
        },
        new NeovegHistoryTheme
        {
            Id = "contemporary_2026_electric_dark",
            EraFolder = "04_era_contemporary_2026",
            EraNumber = 4,
            Name = "Contemporary NLE / Neoveg Atual (2025–2026 Electric Blue)",
            ShortBadge = "Contemporary NLE • 2026",
            Years = "2025–2026",
            Vendor = "Neoveg Creative Software / Contemporary NLE",
            WindowBg = "#0E1015", MenuBg = "#13161E", ToolbarBg = "#161A24",
            DockHeaderBg = "#1B202C", DockPanelBg = "#12161F", DockBorder = "#262F40",
            TimelineBg = "#0B0D12", RulerBg = "#151922",
            TrackVideoBg = "#172336", TrackAudioBg = "#191D2E", ClipHeaderBg = "#090D16",
            AccentPrimary = "#00A3FF", AccentSecondary = "#38BDF8",
            TimecodeBg = "#090B10", TimecodeFg = "#38BDF8", PlayheadColor = "#00A3FF",
            WaveformColor = "#7DD3FC", ButtonBg = "#1E2636", ButtonFg = "#F8FAFC",
            TextPrimary = "#F8FAFC", TextSecondary = "#94A3B8", StatusBarBg = "#10141C"
        }
    };
}
