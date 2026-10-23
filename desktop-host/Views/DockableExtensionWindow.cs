using System;
using System.IO;
using System.Linq;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Scripting;
using Neoveg.Services;

namespace Neoveg.Views;

/// <summary>
/// Interactive Bezier Curve Preview Control for VPFlow 2.4 Dockable Extension.
/// </summary>
public sealed class VPFlowBezierPreviewControl : Control
{
    public string CurvePreset { get; set; } = "Ease In / Ease Out (S-Curve)";
    public double Smoothness { get; set; } = 85.0;

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var b = Bounds;
        if (b.Width < 20 || b.Height < 20) return;

        context.FillRectangle(new SolidColorBrush(Color.Parse("#0f141d")), b);

        var gridPen = new Pen(new SolidColorBrush(Color.FromArgb(45, 148, 163, 184)), 1.0);
        for (int i = 1; i < 4; i++)
        {
            double x = b.Width * (i / 4.0);
            double y = b.Height * (i / 4.0);
            context.DrawLine(gridPen, new Point(x, 0), new Point(x, b.Height));
            context.DrawLine(gridPen, new Point(0, y), new Point(b.Width, y));
        }

        var pad = 16.0;
        var p0 = new Point(pad, b.Height - pad);
        var p3 = new Point(b.Width - pad, pad);

        Point p1, p2;
        if (CurvePreset.Contains("Exponential", StringComparison.OrdinalIgnoreCase))
        {
            p1 = new Point(b.Width * 0.78, b.Height - pad);
            p2 = new Point(b.Width * 0.90, pad + 10);
        }
        else if (CurvePreset.Contains("Fast Out", StringComparison.OrdinalIgnoreCase))
        {
            p1 = new Point(b.Width * 0.14, pad + 8);
            p2 = new Point(b.Width * 0.35, pad);
        }
        else
        {
            double s = Math.Clamp(Smoothness / 100.0, 0.15, 0.95);
            p1 = new Point(pad + (b.Width - 2 * pad) * s, b.Height - pad);
            p2 = new Point(b.Width - pad - (b.Width - 2 * pad) * s, pad);
        }

        // Control handles
        var handlePen = new Pen(new SolidColorBrush(Color.Parse("#f59e0b")), 1.2, DashStyle.Dash);
        context.DrawLine(handlePen, p0, p1);
        context.DrawLine(handlePen, p3, p2);

        // Sample cubic Bezier
        var curvePen = new Pen(new SolidColorBrush(Color.Parse("#38bdf8")), 2.6);
        Point prev = p0;
        for (int step = 1; step <= 40; step++)
        {
            double t = step / 40.0;
            double u = 1.0 - t;
            double bx = u * u * u * p0.X + 3 * u * u * t * p1.X + 3 * u * t * t * p2.X + t * t * t * p3.X;
            double by = u * u * u * p0.Y + 3 * u * u * t * p1.Y + 3 * u * t * t * p2.Y + t * t * t * p3.Y;
            var cur = new Point(bx, by);
            context.DrawLine(curvePen, prev, cur);
            prev = cur;
        }

        context.DrawEllipse(new SolidColorBrush(Color.Parse("#f59e0b")), new Pen(Brushes.White, 1), p1, 4.5, 4.5);
        context.DrawEllipse(new SolidColorBrush(Color.Parse("#f59e0b")), new Pen(Brushes.White, 1), p2, 4.5, 4.5);
        context.DrawEllipse(new SolidColorBrush(Color.Parse("#38bdf8")), new Pen(Brushes.White, 1.2), p0, 5.0, 5.0);
        context.DrawEllipse(new SolidColorBrush(Color.Parse("#38bdf8")), new Pen(Brushes.White, 1.2), p3, 5.0, 5.0);
    }
}

/// <summary>
/// Dockable / Floating Window Extension Host (`Exibir -> Extensões de Janela` / `Ferramentas -> Scripts`):
/// Hosts third-party compatible panels (VPFlow 2.4, Datamosh Neoveg, Vegasaur 4.0, HOS) and a live C# Roslyn / Python Script Console.
/// </summary>
public sealed class DockableExtensionWindow : Window
{
    private readonly VegasProject _project;
    private readonly RustCoreInterop _rustInterop;
    private readonly ScriptingEngineService _scriptingService;
    private readonly string? _selectedClipId;
    private readonly string? _selectedTrackId;
    private readonly Action<string> _onProjectMutated;
    private readonly Action<DockableControl>? _onDockIntoMainWindowRequested;

    private readonly TabControl _tabs;
    private readonly TextBox _scriptEditorBox;
    private readonly TextBox _consoleOutputBox;
    private readonly ComboBox _cmbDiscoveredScripts;

    public DockableExtensionWindow(
        VegasProject project,
        RustCoreInterop rustInterop,
        ScriptingEngineService scriptingService,
        string? selectedClipId,
        string? selectedTrackId,
        string initialExtensionId,
        Action<string> onProjectMutated,
        Action<DockableControl>? onDockIntoMainWindowRequested = null)
    {
        _project = project;
        _rustInterop = rustInterop;
        _scriptingService = scriptingService;
        _selectedClipId = selectedClipId;
        _selectedTrackId = selectedTrackId;
        _onProjectMutated = onProjectMutated;
        _onDockIntoMainWindowRequested = onDockIntoMainWindowRequested;

        var loc = LocalizationManager.Instance;
        Title = loc.T("Scripting.WindowTitle", "Neoveg — Extensões de Janela & Motor de Scripts (Roslyn C# + Python + .DLL)");
        Width = 920;
        Height = 640;
        MinWidth = 720;
        MinHeight = 480;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#111621"));
        Foreground = new SolidColorBrush(Color.Parse("#f8fafc"));

        _scriptEditorBox = new TextBox
        {
            AcceptsReturn = true,
            TextWrapping = TextWrapping.NoWrap,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 11,
            Height = 220,
            Background = new SolidColorBrush(Color.Parse("#0b0f17")),
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0"))
        };

        _consoleOutputBox = new TextBox
        {
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            Height = 110,
            Background = new SolidColorBrush(Color.Parse("#090d14")),
            Foreground = new SolidColorBrush(Color.Parse("#4ade80")),
            Text = "[Neoveg Scripting Engine] Roslyn C# (Neoveg.Scripting) + Python (vegas_scripting.py) prontos."
        };

        _cmbDiscoveredScripts = new ComboBox
        {
            ItemsSource = _scriptingService.DiscoveredScripts.ToList(),
            SelectedIndex = 0,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };
        _cmbDiscoveredScripts.SelectionChanged += (_, _) => LoadSelectedScriptIntoEditor();

        _tabs = new TabControl
        {
            Background = new SolidColorBrush(Color.Parse("#141a26")),
            ItemsSource = new[]
            {
                new TabItem { Header = "VPFlow 2.4 (Curves & Crossfades)", Content = BuildVPFlowDockableControl() },
                new TabItem { Header = "Datamosh Neoveg (Glitch & Split)", Content = BuildDatamoshDockableControl() },
                new TabItem { Header = "Vegasaur & HOS Toolkit", Content = BuildVegasaurAndHosDockableControl() },
                new TabItem { Header = "Console Roslyn C# & Python (.cs / .py)", Content = BuildScriptIdeAndRunnerTab() }
            }
        };

        Neoveg.Styles.NeovegDockTabStyles.Apply(_tabs);
        Content = _tabs;
        LoadSelectedScriptIntoEditor();

        _tabs.SelectedIndex = initialExtensionId.ToLowerInvariant() switch
        {
            var s when s.Contains("datamosh") => 1,
            var s when s.Contains("vegasaur") || s.Contains("hos") => 2,
            var s when s.Contains("script") || s.Contains("console") => 3,
            _ => 0
        };
    }

    public DockableControl BuildVPFlowDockableControl()
    {
        var dock = new DockableControl("vpflow_extension")
        {
            DisplayName = "VPFlow 2.4 — Motion Curve & Velocity Graph Editor",
            Padding = new Thickness(14)
        };

        var stack = new StackPanel { Spacing = 10 };
        stack.Children.Add(new TextBlock
        {
            Text = "VPFLOW 2.4 — EDITOR DE CURVAS BEZIER, PAN/CROP KEYFRAMES & AUTO-CROSSFADE",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        var bezierPreview = new VPFlowBezierPreviewControl { Height = 175 };
        var cmbPreset = new ComboBox
        {
            ItemsSource = new[]
            {
                "Ease In / Ease Out (S-Curve)",
                "Exponential Whip Zoom",
                "Fast Out / Smooth Landing"
            },
            SelectedIndex = 0,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };
        cmbPreset.SelectionChanged += (_, _) =>
        {
            bezierPreview.CurvePreset = cmbPreset.SelectedItem as string ?? "Ease In / Ease Out (S-Curve)";
            bezierPreview.InvalidateVisual();
        };

        stack.Children.Add(cmbPreset);
        stack.Children.Add(new Border
        {
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            BorderThickness = new Thickness(1),
            Child = bezierPreview
        });

        var btnRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };

        var btnRunPy = CreateButton("Executar VPFlow_AutoCrossfade_Helper.py (Python)", "#2563eb", async () =>
        {
            string pyPath = Path.Combine(_scriptingService.ScriptsDirectory, "VPFlow_AutoCrossfade_Helper.py");
            var res = await _scriptingService.ExecuteScriptFileAsync(
                pyPath,
                _project,
                _selectedClipId,
                _selectedTrackId,
                () => _onProjectMutated("VPFlow_AutoCrossfade_Helper.py aplicado."));
            AppendConsoleLog(res);
            _onProjectMutated(res.SummaryMessage);
        });

        var btnApplyCurve = CreateButton("Aplicar Curva Bezier VPFlow ao Evento Selecionado", "#0e7490", () =>
        {
            var vegas = new Vegas(_project, _rustInterop, _selectedClipId, _selectedTrackId, () => _onProjectMutated("Curva VPFlow aplicada."));
            foreach (var tr in vegas.Project.Tracks.Where(t => t.IsVideo))
            {
                foreach (var ev in tr.Events.OfType<VideoEvent>())
                {
                    ev.ApplyVPFlowBezierCurve("Smooth", 1.16, 0.0);
                }
            }
            vegas.UpdateUI();
        });

        btnRow.Children.Add(btnRunPy);
        btnRow.Children.Add(btnApplyCurve);

        if (_onDockIntoMainWindowRequested != null)
        {
            var btnDock = CreateButton("Acoplar VPFlow no Painel Central do Estúdio [📌]", "#1e293b", () =>
            {
                _onDockIntoMainWindowRequested(BuildVPFlowDockableControl());
                Close();
            });
            btnRow.Children.Add(btnDock);
        }

        stack.Children.Add(btnRow);
        dock.Child = new ScrollViewer { Content = stack };
        return dock;
    }

    public DockableControl BuildDatamoshDockableControl()
    {
        var dock = new DockableControl("datamosh_extension")
        {
            DisplayName = "Datamosh Neoveg — I-Frame Drop & P-Vector Glitch",
            Padding = new Thickness(14)
        };

        var stack = new StackPanel { Spacing = 10 };
        stack.Children.Add(new TextBlock
        {
            Text = "DATAMOSH NEOVEG — I-FRAME DROP, P-VECTOR BLOOM & CORTES RÍTMICOS (C# ROSLYN)",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#a855f7"))
        });

        var sldInterval = new Slider { Minimum = 0.30, Maximum = 2.00, Value = 0.85 };
        var sldIntensity = new Slider { Minimum = 0.20, Maximum = 1.00, Value = 0.88 };
        var sldSpeed = new Slider { Minimum = 0.50, Maximum = 2.00, Value = 1.15 };

        stack.Children.Add(new TextBlock { Text = "Intervalo de Corte Automático (segundos): 0.85s", FontSize = 11 });
        stack.Children.Add(sldInterval);
        stack.Children.Add(new TextBlock { Text = "Intensidade do Datamosh / Optical Vector Bloom: 88%", FontSize = 11 });
        stack.Children.Add(sldIntensity);
        stack.Children.Add(new TextBlock { Text = "Playback Rate nos Segmentos Glitched: 1.15x", FontSize = 11 });
        stack.Children.Add(sldSpeed);

        var btnRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };

        var btnRunCs = CreateButton("Executar Batch_Split_and_Datamosh.cs (Roslyn C#)", "#7c3aed", async () =>
        {
            string csPath = Path.Combine(_scriptingService.ScriptsDirectory, "Batch_Split_and_Datamosh.cs");
            var res = await _scriptingService.ExecuteScriptFileAsync(
                csPath,
                _project,
                _selectedClipId,
                _selectedTrackId,
                () => _onProjectMutated("Batch_Split_and_Datamosh.cs executado."));
            AppendConsoleLog(res);
            _onProjectMutated(res.SummaryMessage);
        });

        var btnQuickMosh = CreateButton("Aplicar Datamosh Customizado Agora", "#2563eb", () =>
        {
            var vegas = new Vegas(_project, _rustInterop, _selectedClipId, _selectedTrackId, () => { });
            int count = vegas.BatchSplitAndDatamosh(sldInterval.Value, sldIntensity.Value, sldSpeed.Value);
            _onProjectMutated($"Datamosh Neoveg: {count} segmento(s) cortados e processados com P-Vector Glitch.");
        });

        btnRow.Children.Add(btnRunCs);
        btnRow.Children.Add(btnQuickMosh);

        if (_onDockIntoMainWindowRequested != null)
        {
            var btnDock = CreateButton("Acoplar Datamosh no Painel Central [📌]", "#1e293b", () =>
            {
                _onDockIntoMainWindowRequested(BuildDatamoshDockableControl());
                Close();
            });
            btnRow.Children.Add(btnDock);
        }

        stack.Children.Add(btnRow);
        dock.Child = new ScrollViewer { Content = stack };
        return dock;
    }

    public DockableControl BuildVegasaurAndHosDockableControl()
    {
        var dock = new DockableControl("vegasaur_extension")
        {
            DisplayName = "Vegasaur 4.0 & HOS Automation Toolkit",
            Padding = new Thickness(14)
        };

        var stack = new StackPanel { Spacing = 10 };
        stack.Children.Add(new TextBlock
        {
            Text = "VEGASAUR 4.0 & HOS (HAPPY OTTER SCRIPTS) — AUTOMAÇÃO EM LOTE DE TIMELINE",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#f59e0b"))
        });

        stack.Children.Add(CreateButton("Vegasaur: Fechar Todas as Lacunas entre Eventos (Ripple Delete Gaps)", "#1e293b", () =>
        {
            int moved = 0;
            foreach (var tr in _project.Tracks)
            {
                var clips = tr.Clips.OrderBy(c => c.Start).ToList();
                double cur = 0.0;
                foreach (var c in clips)
                {
                    if (Math.Abs(c.Start - cur) > 0.01) { c.Start = Math.Round(cur, 3); moved++; }
                    cur = c.Start + c.Duration;
                }
            }
            _onProjectMutated($"Vegasaur Auditor: {moved} evento(s) alinhados sem lacunas (Ripple Close Gaps).");
        }));

        stack.Children.Add(CreateButton("Vegasaur: Gerar Grade de Marcadores 120 BPM (Beat Grid Markers)", "#1e293b", () =>
        {
            var vegas = new Vegas(_project, _rustInterop, _selectedClipId, _selectedTrackId, () => { });
            for (int bar = 1; bar <= 8; bar++)
            {
                vegas.Project.Markers.Add(new Marker(Timecode.FromSeconds((bar - 1) * 2.0), $"120BPM Bar {bar}"));
            }
            _onProjectMutated("Vegasaur Beat Grid: 8 marcadores de compasso (120 BPM) adicionados à Timeline.");
        }));

        stack.Children.Add(CreateButton("HOS (Happy Otter Scripts): Normalizar Áudio p/ 0.0 dB + Fades Suaves 150ms", "#0e7490", () =>
        {
            int count = 0;
            foreach (var tr in _project.Tracks.Where(t => t.Type == "audio"))
            {
                foreach (var c in tr.Clips)
                {
                    c.Gain = 1.0;
                    c.FadeIn = Math.Max(c.FadeIn, 0.15);
                    c.FadeOut = Math.Max(c.FadeOut, 0.15);
                    count++;
                }
            }
            _onProjectMutated($"HOS Audio Assistant: {count} evento(s) de áudio normalizados em 0.0 dB com fades de 150ms.");
        }));

        dock.Child = new ScrollViewer { Content = stack };
        return dock;
    }

    private Control BuildScriptIdeAndRunnerTab()
    {
        var stack = new StackPanel { Spacing = 8, Margin = new Thickness(14) };

        stack.Children.Add(new TextBlock
        {
            Text = "MOTOR DINÂMICO DE SCRIPTS (.CS ROSLYN + .PY PYTHON + .DLL EXTENSIONS)",
            FontSize = 12.5,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        var topRow = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto,Auto") };
        Grid.SetColumn(_cmbDiscoveredScripts, 0);
        topRow.Children.Add(_cmbDiscoveredScripts);

        var btnRescan = CreateButton("Recarregar Pasta scripts/", "#1e293b", () =>
        {
            _scriptingService.RescanScriptsFolder();
            _cmbDiscoveredScripts.ItemsSource = _scriptingService.DiscoveredScripts.ToList();
            if (_scriptingService.DiscoveredScripts.Count > 0)
                _cmbDiscoveredScripts.SelectedIndex = 0;
        });
        btnRescan.Margin = new Thickness(6, 0, 0, 0);
        Grid.SetColumn(btnRescan, 1);
        topRow.Children.Add(btnRescan);

        var btnRunSelected = CreateButton("▶ Executar Script Agora", "#16a34a", async () =>
        {
            if (_cmbDiscoveredScripts.SelectedItem is ScriptFileEntry entry)
            {
                if (entry.FullPath.EndsWith(".cs", StringComparison.OrdinalIgnoreCase))
                {
                    var vegas = new Vegas(_project, _rustInterop, _selectedClipId, _selectedTrackId, () => { });
                    var res = await _scriptingService.ExecuteCSharpScriptCodeAsync(entry.FileName, _scriptEditorBox.Text ?? "", vegas);
                    AppendConsoleLog(res);
                    _onProjectMutated(res.SummaryMessage);
                }
                else
                {
                    var res = await _scriptingService.ExecuteScriptFileAsync(
                        entry.FullPath,
                        _project,
                        _selectedClipId,
                        _selectedTrackId,
                        () => { });
                    AppendConsoleLog(res);
                    _onProjectMutated(res.SummaryMessage);
                }
            }
        });
        btnRunSelected.Margin = new Thickness(6, 0, 0, 0);
        Grid.SetColumn(btnRunSelected, 2);
        topRow.Children.Add(btnRunSelected);

        stack.Children.Add(topRow);
        stack.Children.Add(_scriptEditorBox);
        stack.Children.Add(new TextBlock { Text = "Saída do Console de Scripting:", FontSize = 10.5, FontWeight = FontWeight.Bold });
        stack.Children.Add(_consoleOutputBox);

        return new ScrollViewer { Content = stack };
    }

    private void LoadSelectedScriptIntoEditor()
    {
        if (_cmbDiscoveredScripts.SelectedItem is ScriptFileEntry entry &&
            File.Exists(entry.FullPath) &&
            !entry.FullPath.EndsWith(".dll", StringComparison.OrdinalIgnoreCase))
        {
            _scriptEditorBox.Text = File.ReadAllText(entry.FullPath);
        }
    }

    private void AppendConsoleLog(ScriptExecutionResult res)
    {
        string logLines = res.Logs.Count > 0 ? string.Join("\n", res.Logs) : res.SummaryMessage;
        _consoleOutputBox.Text = $"[{DateTime.Now:HH:mm:ss}] ({res.EngineUsed}) {res.ScriptName} -> {(res.Ok ? "OK" : "ERRO")}\n{logLines}";
    }

    private static Button CreateButton(string text, string bgHex, Action onClick)
    {
        var btn = new Button
        {
            Content = text,
            FontSize = 10.5,
            FontWeight = FontWeight.Bold,
            Background = new SolidColorBrush(Color.Parse(bgHex)),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(10, 5)
        };
        btn.Click += (_, _) => onClick();
        return btn;
    }
}
