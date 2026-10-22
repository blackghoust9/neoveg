using System;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Views;

/// <summary>
/// Cross-Platform Avalonia Window for Settings:
/// - Tab 1: Geral / Idioma (Language) — Real-time UI language switcher (8 languages: pt-BR, en-US, es-ES, fr-FR, de-DE, zh-CN, ja-JP, ru-RU) persisted to `settings.json`
/// - Tab 2: Vídeo, Áudio & Engine (ACES Color Science, 32-bit Floating Point, 192 kHz Audio, GPU Hardware Acceleration, Rust FFI & Isolated Python FFmpeg)
/// </summary>
public sealed class AdvancedSettingsWindow : Window
{
    public AdvancedSettingsWindow(
        VegasProject project,
        RustCoreInterop rustInterop,
        string ffmpegStatusBanner,
        Action onSettingsApplied)
    {
        var loc = LocalizationManager.Instance;

        Title = loc.T("Settings.WindowTitle", "NEOVEG — Configurações (Geral / Idioma & Projeto)");
        Width = 800;
        Height = 580;
        MinWidth = 680;
        MinHeight = 480;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#12161f"));
        Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"));

        var rootGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,Auto"),
            Margin = new Thickness(16)
        };

        var lblHeaderTitle = new TextBlock
        {
            Text = loc.T("Settings.WindowTitle", "CONFIGURAÇÕES -> GERAL / IDIOMA & MOTOR MULTIPLATAFORMA"),
            FontWeight = FontWeight.Bold,
            FontSize = 13,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        };

        // Header
        var hdrBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#181e29")),
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(12, 8),
            Margin = new Thickness(0, 0, 0, 12),
            Child = new StackPanel
            {
                Spacing = 3,
                Children =
                {
                    lblHeaderTitle,
                    new TextBlock
                    {
                        Text = $"Rust FFI: {rustInterop.GetEngineBanner()} • {ffmpegStatusBanner}",
                        FontSize = 11,
                        Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
                    }
                }
            }
        };
        Grid.SetRow(hdrBorder, 0);
        rootGrid.Children.Add(hdrBorder);

        // Tabs: [Geral / Idioma (Language)] | [Vídeo, Áudio & Engine]
        var tabs = new TabControl
        {
            Background = new SolidColorBrush(Color.Parse("#141923"))
        };

        // TAB 1: Geral / Idioma (Language)
        var langStack = new StackPanel
        {
            Spacing = 12,
            Margin = new Thickness(12)
        };

        var lblLangHeading = new TextBlock
        {
            Text = loc.T("Settings.LanguageLabel", "Idioma da Interface (UI Language — Troca em Tempo Real):"),
            FontSize = 12,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#f8fafc"))
        };
        langStack.Children.Add(lblLangHeading);

        var cmbLanguage = new ComboBox
        {
            ItemsSource = LocalizationManager.Languages,
            SelectedItem = loc.CurrentLanguage,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Height = 32,
            FontSize = 12,
            Background = new SolidColorBrush(Color.Parse("#0f172a")),
            Foreground = new SolidColorBrush(Color.Parse("#f1f5f9")),
            BorderBrush = new SolidColorBrush(Color.Parse("#38bdf8"))
        };

        var lblSavedNote = new TextBlock
        {
            Text = $"{loc.T("Settings.LanguageSavedNote")} ({loc.SettingsFilePath})",
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#4ade80")),
            TextWrapping = TextWrapping.Wrap
        };

        cmbLanguage.SelectionChanged += (_, _) =>
        {
            if (cmbLanguage.SelectedItem is LanguageOption selectedLang)
            {
                loc.SetLanguage(selectedLang.Code, persistToSettings: true);
                Title = loc.T("Settings.WindowTitle");
                lblHeaderTitle.Text = loc.T("Settings.WindowTitle");
                lblLangHeading.Text = loc.T("Settings.LanguageLabel");
                lblSavedNote.Text = $"{loc.T("Settings.LanguageSavedNote")} ({loc.SettingsFilePath})";
                onSettingsApplied();
            }
        };
        langStack.Children.Add(cmbLanguage);
        langStack.Children.Add(lblSavedNote);

        // Quick 8-Language Selector Grid
        var quickLangWrap = new WrapPanel
        {
            Orientation = Orientation.Horizontal,
            Margin = new Thickness(0, 6, 0, 0)
        };
        foreach (var langOpt in LocalizationManager.Languages)
        {
            var captured = langOpt;
            var btnLang = CreateSquareButton(captured.DisplayName, "#1e293b", "#38bdf8");
            btnLang.Margin = new Thickness(0, 0, 8, 8);
            btnLang.Click += (_, _) =>
            {
                cmbLanguage.SelectedItem = captured;
            };
            quickLangWrap.Children.Add(btnLang);
        }
        langStack.Children.Add(quickLangWrap);

        var chkShowWelcome = new CheckBox
        {
            IsChecked = WelcomeWindow.ShowWelcomeOnStartup,
            Content = loc.T("Welcome.ShowOnStartup", "Exibir Tela de Boas-Vindas ao Iniciar"),
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0")),
            FontSize = 11.5
        };
        chkShowWelcome.IsCheckedChanged += (_, _) =>
        {
            WelcomeWindow.ShowWelcomeOnStartup = chkShowWelcome.IsChecked ?? true;
        };
        langStack.Children.Add(chkShowWelcome);

        // TAB 2: Vídeo, Áudio & Engine
        var formGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,*"),
            RowDefinitions = new RowDefinitions("Auto,Auto,Auto,Auto,Auto"),
            Margin = new Thickness(8)
        };

        var cmbRes = CreateCombo(new[]
        {
            "1280x720 (HD 720p)",
            "1920x1080 (Full HD 1080p)",
            "2560x1440 (2.5K QHD)",
            "3840x2160 (4K UHD)",
            "4096x2160 (4K DCI Cinema)"
        }, project.Width == 3840 ? 3 : 1);

        var cmbFps = CreateCombo(new[]
        {
            "23.976",
            "24.000",
            "25.000",
            "29.970",
            "59.940",
            "60.000"
        }, 3);

        var cmbPixel = CreateCombo(new[]
        {
            "8-bit (Unsigned Integer)",
            "32-bit Floating Point (Video Levels)",
            "32-bit Floating Point (Full Range ACES 1.3)"
        }, 0);

        var cmbColor = CreateCombo(new[]
        {
            "Rec.709 Standard (SDR Broadcast)",
            "ACES 1.2 / ACEScct (DaVinci Resolve Pipeline)",
            "Rec.2020 HDR10 / HLG"
        }, 0);

        var cmbResample = CreateCombo(new[]
        {
            "Disable Resample (Sem Ghosting)",
            "Smart Resample",
            "Optical Flow (Resolve Motion)"
        }, 0);

        var cmbMotionBlur = CreateCombo(new[]
        {
            "Gaussian (VEGAS Default)",
            "Pyramid",
            "Box"
        }, 0);

        var cmbSampleRate = CreateCombo(new[]
        {
            "44100",
            "48000",
            "96000",
            "192000"
        }, 1);

        var cmbBitDepth = CreateCombo(new[]
        {
            "16-bit PCM Stereo",
            "24-bit Neoveg Master",
            "32-bit IEEE Float (64-bit Mix Engine)"
        }, 1);

        var cmbGpu = CreateCombo(new[]
        {
            "Skia GPU + NVENC / VAAPI / VideoToolbox Auto",
            "CUDA / TensorRT (CorridorKey AI Neural Mode)",
            "Software CPU Safe Mode (So4 Guard Ativo)"
        }, 0);

        var cmbBus = CreateCombo(new[]
        {
            "Stereo Master Bus (2.0)",
            "5.1 Surround Broadcast Bus",
            "7.1 Dolby Atmos Bed"
        }, 0);

        AddField(formGrid, 0, 0, "Resolução do Projeto:", cmbRes);
        AddField(formGrid, 0, 1, "Frame Rate (FPS SMPTE):", cmbFps);
        AddField(formGrid, 1, 0, "Formato de Pixel & Bit Depth:", cmbPixel);
        AddField(formGrid, 1, 1, "Espaço de Cor (Color Science):", cmbColor);
        AddField(formGrid, 2, 0, "Modo de Resample (Anti-Ghosting):", cmbResample);
        AddField(formGrid, 2, 1, "Envelope de Motion Blur:", cmbMotionBlur);
        AddField(formGrid, 3, 0, "Taxa de Amostragem de Áudio (Hz):", cmbSampleRate);
        AddField(formGrid, 3, 1, "Profundidade de Bits de Áudio:", cmbBitDepth);
        AddField(formGrid, 4, 0, "Aceleração de Hardware (GPU Multiplataforma):", cmbGpu);
        AddField(formGrid, 4, 1, "Modo do Master Bus de Áudio:", cmbBus);

        tabs.ItemsSource = new[]
        {
            new TabItem
            {
                Header = loc.T("Settings.Tab.GeneralLanguage", "Geral / Idioma (Language)"),
                Content = new ScrollViewer { Content = langStack }
            },
            new TabItem
            {
                Header = loc.T("Settings.Tab.ProjectEngine", "Vídeo, Áudio & Engine"),
                Content = new ScrollViewer { Content = formGrid }
            }
        };

        Grid.SetRow(tabs, 1);
        rootGrid.Children.Add(tabs);

        // Footer Buttons
        var footer = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 10,
            Margin = new Thickness(0, 14, 0, 0)
        };

        var btnClose = CreateSquareButton(loc.T("Settings.CancelBtn", "Cancelar / Fechar"), "#1e293b", "#cbd5e1");
        btnClose.Click += (_, _) => Close();

        var btnApply = CreateSquareButton(loc.T("Settings.ApplyBtn", "Aplicar e Salvar Configurações"), "#2563eb", "#ffffff");
        btnApply.Click += (_, _) =>
        {
            if (cmbLanguage.SelectedItem is LanguageOption selectedLang)
            {
                loc.SetLanguage(selectedLang.Code, persistToSettings: true);
            }

            string resText = (cmbRes.SelectedItem as string) ?? "1920x1080";
            string dims = resText.Split(' ')[0];
            string[] wh = dims.Split('x');
            if (wh.Length == 2 && int.TryParse(wh[0], out int w) && int.TryParse(wh[1], out int h))
            {
                project.Width = w;
                project.Height = h;
            }
            if (double.TryParse(cmbFps.SelectedItem as string, System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out double fpsVal))
            {
                project.Fps = fpsVal;
            }
            project.PixelFormat = (cmbPixel.SelectedItem as string) ?? project.PixelFormat;
            project.ColorSpace = (cmbColor.SelectedItem as string) ?? project.ColorSpace;
            project.ResampleMode = (cmbResample.SelectedItem as string) ?? project.ResampleMode;
            project.MotionBlurType = (cmbMotionBlur.SelectedItem as string) ?? project.MotionBlurType;
            if (int.TryParse(cmbSampleRate.SelectedItem as string, out int sr))
            {
                project.SampleRate = sr;
            }
            project.MasterBusMode = (cmbBus.SelectedItem as string) ?? project.MasterBusMode;

            onSettingsApplied();
            Close();
        };

        footer.Children.Add(btnClose);
        footer.Children.Add(btnApply);
        Grid.SetRow(footer, 2);
        rootGrid.Children.Add(footer);

        Content = rootGrid;
    }

    private static ComboBox CreateCombo(string[] items, int selectedIdx)
    {
        return new ComboBox
        {
            ItemsSource = items,
            SelectedIndex = selectedIdx,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Background = new SolidColorBrush(Color.Parse("#0f172a")),
            Foreground = new SolidColorBrush(Color.Parse("#f1f5f9")),
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            CornerRadius = new CornerRadius(0)
        };
    }

    private static void AddField(Grid grid, int row, int col, string label, Control control)
    {
        var panel = new StackPanel
        {
            Spacing = 4,
            Margin = new Thickness(col == 0 ? 0 : 7, 0, col == 0 ? 7 : 0, 10),
            Children =
            {
                new TextBlock
                {
                    Text = label,
                    FontSize = 11.5,
                    FontWeight = FontWeight.SemiBold,
                    Foreground = new SolidColorBrush(Color.Parse("#cbd5e1"))
                },
                control
            }
        };
        Grid.SetRow(panel, row);
        Grid.SetColumn(panel, col);
        grid.Children.Add(panel);
    }

    private static Button CreateSquareButton(string text, string bgHex, string fgHex)
    {
        return new Button
        {
            Content = text,
            Background = new SolidColorBrush(Color.Parse(bgHex)),
            Foreground = new SolidColorBrush(Color.Parse(fgHex)),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(12, 6)
        };
    }
}
