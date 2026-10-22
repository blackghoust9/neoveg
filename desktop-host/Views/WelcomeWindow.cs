using System;
using System.Collections.Generic;
using System.IO;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Views;

/// <summary>
/// Tela de Boas-Vindas oficial no estilo VEGAS Pro (New Project / Open Project / Getting Started):
/// - Barra superior escura com o ícone azul [V] + "VEGAS Pro" centralizado e botão fechar [✕]
/// - Barra lateral esquerda vertical com "New Project" (azul #0095FF quando ativo), "Open Project" e "Getting Started"
/// - Banner fotográfico superior da câmera de cinema VEGAS Pro em fundo azul royal
/// - 5 blocos seletores de proporção de tela: Widescreen (16:9), Fullscreen (4:3), Portrait (9:16), Square (1:1) e Scope (2.4:1)
/// - Seção "Create a new project" com dropdowns de Resolution, Framerate + Drop Frame e botões "Create Project" e "Advanced Settings..."
/// - Rodapé inferior com CheckBox "Show Welcome Screen on Startup"
/// </summary>
public sealed class WelcomeWindow : Window
{
    public static bool ShowWelcomeOnStartup { get; set; } = true;

    private readonly VegasProject _currentProject;
    private readonly FFmpegPythonService _pythonService;
    private readonly RustCoreInterop _rustInterop;
    private readonly string _ffmpegStatusBanner;
    private readonly Action<VegasProject, string> _onProjectSelected;
    private readonly Action _onSettingsUpdated;

    private readonly Border[] _leftNavTiles = new Border[3];
    private readonly Border[] _aspectTiles = new Border[5];
    private readonly ContentControl _lowerRightHost = new();
    private ComboBox? _cmbResolution;
    private ComboBox? _cmbFramerate;
    private ComboBox? _cmbDropFrame;
    private int _selectedAspectIndex = 0;

    // Ícones Vetoriais da Barra Lateral Esquerda
    private static readonly StreamGeometry GeoDocNew = StreamGeometry.Parse("M 4,1 L 15,1 L 21,7 L 21,25 L 4,25 Z M 15,1 L 15,7 L 21,7");
    private static readonly StreamGeometry GeoFolderOpen = StreamGeometry.Parse("M 1,4 L 9,4 L 12,7 L 25,7 L 25,22 L 1,22 Z");
    private static readonly StreamGeometry GeoGradCap = StreamGeometry.Parse("M 13,2 L 25,8 L 13,14 L 1,8 Z M 5,11 L 5,18 C 5,21 21,21 21,18 L 21,11 M 24,9 L 24,18");

    // Ícones Vetoriais dos 5 Presets de Proporção (Aspect Ratio)
    private static readonly StreamGeometry GeoAspect16x9 = StreamGeometry.Parse("M 1,3 L 27,3 L 27,19 L 1,19 Z M 3.5,5.5 L 24.5,5.5 L 24.5,16.5 L 3.5,16.5 Z M 9,22 L 19,22 M 14,19 L 14,22");
    private static readonly StreamGeometry GeoAspect4x3 = StreamGeometry.Parse("M 3,2 L 25,2 L 25,20 L 3,20 Z M 5,4 L 23,4 L 23,18 L 5,18 Z");
    private static readonly StreamGeometry GeoAspect9x16 = StreamGeometry.Parse("M 7,1 L 21,1 L 21,23 L 7,23 Z M 9,3.5 L 19,3.5 L 19,19.5 L 9,19.5 Z M 13,21.2 L 15,21.2");
    private static readonly StreamGeometry GeoAspect1x1 = StreamGeometry.Parse("M 4,2 L 24,2 L 24,19 L 15,19 L 11,23 L 11,19 L 4,19 Z");
    private static readonly StreamGeometry GeoAspectScope = StreamGeometry.Parse("M 1,4 L 27,4 L 27,16 L 1,16 Z M 9,16 L 6,21 L 22,21 L 19,16");

    private static readonly (string Label, StreamGeometry Icon, (string Text, int W, int H)[] Resolutions)[] AspectPresets =
    {
        (
            "Widescreen (16:9)",
            GeoAspect16x9,
            new[]
            {
                ("FullHD (1920 x 1080)", 1920, 1080),
                ("4K UHD (3840 x 2160)", 3840, 2160),
                ("HD 720p (1280 x 720)", 1280, 720),
                ("8K UHD (7680 x 4320)", 7680, 4320)
            }
        ),
        (
            "Fullscreen (4:3)",
            GeoAspect4x3,
            new[]
            {
                ("SD NTSC (720 x 480)", 720, 480),
                ("SXGA 4:3 (1440 x 1080)", 1440, 1080),
                ("UXGA 4:3 (1600 x 1200)", 1600, 1200)
            }
        ),
        (
            "Portrait (9:16)",
            GeoAspect9x16,
            new[]
            {
                ("Vertical FullHD (1080 x 1920)", 1080, 1920),
                ("Vertical 4K UHD (2160 x 3840)", 2160, 3840),
                ("Vertical HD (720 x 1280)", 720, 1280)
            }
        ),
        (
            "Square (1:1)",
            GeoAspect1x1,
            new[]
            {
                ("Square FullHD (1080 x 1080)", 1080, 1080),
                ("Square 2K (2160 x 2160)", 2160, 2160),
                ("Square 720p (720 x 720)", 720, 720)
            }
        ),
        (
            "Scope (2.4:1)",
            GeoAspectScope,
            new[]
            {
                ("CinemaScope 4K (4096 x 1716)", 4096, 1716),
                ("Ultrawide 2.4:1 (2560 x 1080)", 2560, 1080),
                ("Anamorphic 1080p (1920 x 800)", 1920, 800)
            }
        )
    };

    private static readonly (string Label, double Fps)[] FrameratePresets =
    {
        ("30p (29.970)", 29.97),
        ("60p (59.940)", 59.94),
        ("24p (23.976 - Cinema)", 23.976),
        ("25p (25.000 - PAL)", 25.0),
        ("50p (50.000 - PAL)", 50.0)
    };

    public WelcomeWindow(
        VegasProject currentProject,
        FFmpegPythonService pythonService,
        RustCoreInterop rustInterop,
        string ffmpegStatusBanner,
        Action<VegasProject, string> onProjectSelected,
        Action onSettingsUpdated)
    {
        _currentProject = currentProject;
        _pythonService = pythonService;
        _rustInterop = rustInterop;
        _ffmpegStatusBanner = ffmpegStatusBanner;
        _onProjectSelected = onProjectSelected;
        _onSettingsUpdated = onSettingsUpdated;

        var loc = LocalizationManager.Instance;
        Title = loc.T("Welcome.Title", "VEGAS Pro — Welcome");
        Width = 780;
        Height = 548;
        CanResize = false;
        SystemDecorations = SystemDecorations.None;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#0F1012"));
        Foreground = new SolidColorBrush(Color.Parse("#F3F4F6"));

        var outerBorder = new Border
        {
            BorderBrush = new SolidColorBrush(Color.Parse("#26282E")),
            BorderThickness = new Thickness(1),
            Background = new SolidColorBrush(Color.Parse("#0F1012")),
            Child = BuildMainLayout()
        };

        Content = outerBorder;
        SelectLeftNavTab(0);
    }

    private Control BuildMainLayout()
    {
        var rootGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("44,*,34")
        };

        // 1. TOP HEADER BAR ([V] VEGAS Pro + Botão [✕])
        var topHeader = BuildTopHeaderBar();
        Grid.SetRow(topHeader, 0);
        rootGrid.Children.Add(topHeader);

        // 2. MIDDLE WORKSPACE (Left Vertical Nav Sidebar + Right Hero Banner & Panel)
        var centerGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("98,*")
        };

        var leftNav = BuildLeftNavigationSidebar();
        Grid.SetColumn(leftNav, 0);
        centerGrid.Children.Add(leftNav);

        var rightContent = BuildRightContentWorkspace();
        Grid.SetColumn(rightContent, 1);
        centerGrid.Children.Add(rightContent);

        Grid.SetRow(centerGrid, 1);
        rootGrid.Children.Add(centerGrid);

        // 3. BOTTOM FOOTER BAR ("Show Welcome Screen on Startup")
        var bottomFooter = BuildBottomFooterBar();
        Grid.SetRow(bottomFooter, 2);
        rootGrid.Children.Add(bottomFooter);

        return rootGrid;
    }

    private Control BuildTopHeaderBar()
    {
        var headerGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto,*"),
            Background = new SolidColorBrush(Color.Parse("#0B0C0E"))
        };

        // Permite arrastar a janela pela barra superior para outros monitores
        headerGrid.PointerPressed += (_, e) =>
        {
            if (e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
            {
                BeginMoveDrag(e);
            }
        };

        // Centro: Ícone azul [V] + "VEGAS Pro"
        var centerBrand = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 9,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center
        };

        var vBadge = new Border
        {
            Width = 26,
            Height = 26,
            CornerRadius = new CornerRadius(4),
            Background = new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(1, 1, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(Color.Parse("#365CFF"), 0.0),
                    new GradientStop(Color.Parse("#1D39E6"), 1.0)
                }
            },
            Child = new TextBlock
            {
                Text = "V",
                FontSize = 16,
                FontWeight = FontWeight.Black,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center
            }
        };

        var brandText = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 5,
            VerticalAlignment = VerticalAlignment.Center,
            Children =
            {
                new TextBlock
                {
                    Text = "VEGAS",
                    FontSize = 17.5,
                    FontWeight = FontWeight.Bold,
                    Foreground = Brushes.White
                },
                new TextBlock
                {
                    Text = "Pro",
                    FontSize = 17.5,
                    FontWeight = FontWeight.Light,
                    Foreground = new SolidColorBrush(Color.Parse("#E5E7EB"))
                }
            }
        };

        centerBrand.Children.Add(vBadge);
        centerBrand.Children.Add(brandText);
        Grid.SetColumn(centerBrand, 1);
        headerGrid.Children.Add(centerBrand);

        // Direita: Botão Fechar [✕]
        var btnClose = new Button
        {
            Content = "✕",
            FontSize = 13.5,
            Width = 42,
            Height = 40,
            Background = Brushes.Transparent,
            Foreground = new SolidColorBrush(Color.Parse("#9CA3AF")),
            BorderThickness = new Thickness(0),
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center
        };
        btnClose.Click += (_, _) => Close();
        Grid.SetColumn(btnClose, 2);
        headerGrid.Children.Add(btnClose);

        return headerGrid;
    }

    private Control BuildLeftNavigationSidebar()
    {
        var navStack = new StackPanel
        {
            Spacing = 0,
            Background = new SolidColorBrush(Color.Parse("#0F1012"))
        };

        var loc = LocalizationManager.Instance;
        var items = new (string Label, StreamGeometry Icon)[]
        {
            (loc.T("Welcome.NewProject", "New Project"), GeoDocNew),
            (loc.T("Welcome.OpenProject", "Open Project"), GeoFolderOpen),
            (loc.T("Welcome.GettingStarted", "Getting Started"), GeoGradCap)
        };

        for (int i = 0; i < items.Length; i++)
        {
            int idx = i;
            var iconPath = new PathIcon
            {
                Data = items[i].Icon,
                Width = 24,
                Height = 24,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center
            };

            var label = new TextBlock
            {
                Text = items[i].Label,
                FontSize = 11.5,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center,
                Margin = new Thickness(0, 8, 0, 0)
            };

            var tile = new Border
            {
                Height = 92,
                Background = new SolidColorBrush(Color.Parse("#0F1012")),
                Cursor = new Cursor(StandardCursorType.Hand),
                Child = new StackPanel
                {
                    VerticalAlignment = VerticalAlignment.Center,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    Children = { iconPath, label }
                }
            };
            tile.PointerPressed += (_, _) => SelectLeftNavTab(idx);
            _leftNavTiles[i] = tile;
            navStack.Children.Add(tile);
        }

        return navStack;
    }

    private Control BuildRightContentWorkspace()
    {
        var rightGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("228,*"),
            Background = new SolidColorBrush(Color.Parse("#18191C"))
        };

        // Row 0: Cinema Camera Hero Banner (Carrega o banner da câmera VEGAS Pro sobre fundo azul royal)
        var bannerControl = BuildCinemaCameraBanner();
        Grid.SetRow(bannerControl, 0);
        rightGrid.Children.Add(bannerControl);

        // Row 1: Painel inferior dinâmico (New Project / Open Project / Getting Started)
        Grid.SetRow(_lowerRightHost, 1);
        rightGrid.Children.Add(_lowerRightHost);

        return rightGrid;
    }

    private Control BuildCinemaCameraBanner()
    {
        string[] candidates =
        {
            Path.Combine(AppContext.BaseDirectory, "assets", "vegas_history_themes", "vegas_welcome_camera_banner.png"),
            Path.Combine(Directory.GetCurrentDirectory(), "assets", "vegas_history_themes", "vegas_welcome_camera_banner.png")
        };

        foreach (string path in candidates)
        {
            if (File.Exists(path))
            {
                try
                {
                    var bmp = new Bitmap(path);
                    return new Border
                    {
                        ClipToBounds = true,
                        Background = new SolidColorBrush(Color.Parse("#2B4BFF")),
                        Child = new Image
                        {
                            Source = bmp,
                            Stretch = Stretch.Fill
                        }
                    };
                }
                catch
                {
                    // Fallback vetorial abaixo
                }
            }
        }

        // Fallback caso a imagem PNG não seja encontrada
        return new Border
        {
            Background = new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(1, 1, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(Color.Parse("#3858FF"), 0.0),
                    new GradientStop(Color.Parse("#223CE8"), 1.0)
                }
            },
            Child = new TextBlock
            {
                Text = "VEGAS PRO 21 • CINEMA ENGINE",
                FontSize = 22,
                FontWeight = FontWeight.Black,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center
            }
        };
    }

    private void SelectLeftNavTab(int index)
    {
        for (int i = 0; i < _leftNavTiles.Length; i++)
        {
            _leftNavTiles[i].Background = new SolidColorBrush(
                Color.Parse(i == index ? "#0095FF" : "#0F1012"));
        }

        _lowerRightHost.Content = index switch
        {
            1 => BuildOpenProjectView(),
            2 => BuildGettingStartedView(),
            _ => BuildNewProjectView()
        };
    }

    private Control BuildNewProjectView()
    {
        var grid = new Grid
        {
            RowDefinitions = new RowDefinitions("78,*"),
            Background = new SolidColorBrush(Color.Parse("#18191C"))
        };

        // 1. Linha com os 5 Seletores de Proporção (Widescreen 16:9, Fullscreen 4:3, Portrait 9:16, Square 1:1, Scope 2.4:1)
        var aspectGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,*,*,*,*")
        };

        for (int i = 0; i < AspectPresets.Length; i++)
        {
            int idx = i;
            var preset = AspectPresets[i];

            var icon = new PathIcon
            {
                Data = preset.Icon,
                Width = 26,
                Height = 22,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center
            };

            var lbl = new TextBlock
            {
                Text = preset.Label,
                FontSize = 11,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center,
                Margin = new Thickness(0, 7, 0, 0)
            };

            var tile = new Border
            {
                Background = new SolidColorBrush(Color.Parse(i == _selectedAspectIndex ? "#0095FF" : "#1D1E22")),
                BorderBrush = new SolidColorBrush(Color.Parse("#121316")),
                BorderThickness = new Thickness(0, 0, 1, 1),
                Cursor = new Cursor(StandardCursorType.Hand),
                Child = new StackPanel
                {
                    VerticalAlignment = VerticalAlignment.Center,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    Children = { icon, lbl }
                }
            };
            tile.PointerPressed += (_, _) => SelectAspectTile(idx);
            _aspectTiles[i] = tile;
            Grid.SetColumn(tile, i);
            aspectGrid.Children.Add(tile);
        }

        Grid.SetRow(aspectGrid, 0);
        grid.Children.Add(aspectGrid);

        // 2. Formulário "Create a new project" (Resolution, Framerate, Drop Frame, Create Project, Advanced Settings...)
        var loc = LocalizationManager.Instance;
        var formStack = new StackPanel
        {
            Spacing = 9,
            Margin = new Thickness(16, 12, 16, 10)
        };

        formStack.Children.Add(new TextBlock
        {
            Text = loc.T("Welcome.CreateHeading", "Create a new project"),
            FontSize = 12,
            FontWeight = FontWeight.Bold,
            Foreground = Brushes.White
        });

        // Linha Resolution:
        var resRow = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("86,*")
        };
        resRow.Children.Add(new TextBlock
        {
            Text = loc.T("Welcome.Resolution", "Resolution:"),
            FontSize = 11.5,
            Foreground = new SolidColorBrush(Color.Parse("#D1D5DB")),
            VerticalAlignment = VerticalAlignment.Center,
            [Grid.ColumnProperty] = 0
        });

        _cmbResolution = new ComboBox
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Height = 27,
            FontSize = 11.5,
            Background = new SolidColorBrush(Color.Parse("#111215")),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#2E3138")),
            CornerRadius = new CornerRadius(0)
        };
        Grid.SetColumn(_cmbResolution, 1);
        resRow.Children.Add(_cmbResolution);
        formStack.Children.Add(resRow);

        // Linha Framerate:
        var fpsRow = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("86,*,8,195")
        };
        fpsRow.Children.Add(new TextBlock
        {
            Text = loc.T("Welcome.Framerate", "Framerate:"),
            FontSize = 11.5,
            Foreground = new SolidColorBrush(Color.Parse("#D1D5DB")),
            VerticalAlignment = VerticalAlignment.Center,
            [Grid.ColumnProperty] = 0
        });

        var fpsLabels = new List<string>();
        foreach (var fp in FrameratePresets)
            fpsLabels.Add(fp.Label);

        _cmbFramerate = new ComboBox
        {
            ItemsSource = fpsLabels,
            SelectedIndex = 0,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Height = 27,
            FontSize = 11.5,
            Background = new SolidColorBrush(Color.Parse("#111215")),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#2E3138")),
            CornerRadius = new CornerRadius(0)
        };
        Grid.SetColumn(_cmbFramerate, 1);
        fpsRow.Children.Add(_cmbFramerate);

        _cmbDropFrame = new ComboBox
        {
            ItemsSource = new[]
            {
                "Drop Frame (1/1.001)",
                "Non-Drop Frame (1/1.000)"
            },
            SelectedIndex = 0,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Height = 27,
            FontSize = 11.5,
            Background = new SolidColorBrush(Color.Parse("#111215")),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#2E3138")),
            CornerRadius = new CornerRadius(0)
        };
        Grid.SetColumn(_cmbDropFrame, 3);
        fpsRow.Children.Add(_cmbDropFrame);
        formStack.Children.Add(fpsRow);

        // Linha de Botões: [Create Project]  [Advanced Settings...]
        var btnRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(86, 4, 0, 0)
        };

        var btnCreate = CreateVegasDarkButton(loc.T("Welcome.CreateBtn", "Create Project"), 148);
        btnCreate.Click += (_, _) => CreateProjectFromSelection();

        var btnAdv = CreateVegasDarkButton(loc.T("Welcome.AdvancedBtn", "Advanced Settings..."), 156);
        btnAdv.Click += (_, _) =>
        {
            var advWin = new AdvancedSettingsWindow(_currentProject, _rustInterop, _ffmpegStatusBanner, _onSettingsUpdated);
            advWin.Show(this);
        };

        btnRow.Children.Add(btnCreate);
        btnRow.Children.Add(btnAdv);
        formStack.Children.Add(btnRow);

        Grid.SetRow(formStack, 1);
        grid.Children.Add(formStack);

        PopulateResolutionsForAspect(_selectedAspectIndex);
        return grid;
    }

    private void SelectAspectTile(int aspectIndex)
    {
        _selectedAspectIndex = Math.Clamp(aspectIndex, 0, AspectPresets.Length - 1);
        for (int i = 0; i < _aspectTiles.Length; i++)
        {
            if (_aspectTiles[i] != null)
            {
                _aspectTiles[i].Background = new SolidColorBrush(
                    Color.Parse(i == _selectedAspectIndex ? "#0095FF" : "#1D1E22"));
            }
        }
        PopulateResolutionsForAspect(_selectedAspectIndex);
    }

    private void PopulateResolutionsForAspect(int aspectIndex)
    {
        if (_cmbResolution == null) return;
        var resList = AspectPresets[aspectIndex].Resolutions;
        var labels = new List<string>(resList.Length);
        foreach (var r in resList)
            labels.Add(r.Text);
        _cmbResolution.ItemsSource = labels;
        _cmbResolution.SelectedIndex = 0;
    }

    private void CreateProjectFromSelection()
    {
        var aspect = AspectPresets[_selectedAspectIndex];
        int resIdx = Math.Clamp(_cmbResolution?.SelectedIndex ?? 0, 0, aspect.Resolutions.Length - 1);
        var (_, w, h) = aspect.Resolutions[resIdx];

        int fpsIdx = Math.Clamp(_cmbFramerate?.SelectedIndex ?? 0, 0, FrameratePresets.Length - 1);
        double fps = FrameratePresets[fpsIdx].Fps;

        var proj = VegasProject.CreateEmpty("VEGAS_New_Project", w, h, fps);
        _onProjectSelected(proj, $"Novo projeto criado ({aspect.Label} • {w}x{h} @{fps:0.###}fps)");
        Close();
    }

    private Control BuildOpenProjectView()
    {
        var stack = new StackPanel
        {
            Spacing = 8,
            Margin = new Thickness(18, 14)
        };

        stack.Children.Add(new TextBlock
        {
            Text = "Open an existing VEGAS Pro project (.veg)",
            FontSize = 12.5,
            FontWeight = FontWeight.Bold,
            Foreground = Brushes.White
        });

        var listStack = new StackPanel { Spacing = 6 };
        foreach (var (fileName, savedProj) in _pythonService.LoadSavedProjects())
        {
            var row = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
            var info = new StackPanel
            {
                Spacing = 2,
                Children =
                {
                    new TextBlock
                    {
                        Text = savedProj.Name,
                        FontSize = 11.5,
                        FontWeight = FontWeight.Bold,
                        Foreground = Brushes.White
                    },
                    new TextBlock
                    {
                        Text = $"{fileName} • {savedProj.Width}x{savedProj.Height} @{savedProj.Fps:0.##}fps",
                        FontSize = 10.5,
                        Foreground = new SolidColorBrush(Color.Parse("#9CA3AF"))
                    }
                }
            };
            Grid.SetColumn(info, 0);

            var btnOpen = CreateVegasDarkButton("Open Project", 125);
            btnOpen.Click += (_, _) =>
            {
                _onProjectSelected(savedProj, $"Projeto carregado: {fileName}");
                Close();
            };
            Grid.SetColumn(btnOpen, 1);

            row.Children.Add(info);
            row.Children.Add(btnOpen);

            listStack.Children.Add(new Border
            {
                Background = new SolidColorBrush(Color.Parse("#121316")),
                BorderBrush = new SolidColorBrush(Color.Parse("#2D3038")),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(10, 7),
                Child = row
            });
        }

        stack.Children.Add(new ScrollViewer
        {
            Height = 195,
            Content = listStack
        });

        return stack;
    }

    private Control BuildGettingStartedView()
    {
        var stack = new StackPanel
        {
            Spacing = 10,
            Margin = new Thickness(18, 14)
        };

        stack.Children.Add(new TextBlock
        {
            Text = "Getting Started with VEGAS Pro / Neoveg",
            FontSize = 12.5,
            FontWeight = FontWeight.Bold,
            Foreground = Brushes.White
        });

        stack.Children.Add(new TextBlock
        {
            Text = $"{_rustInterop.GetEngineBanner()} • {_ffmpegStatusBanner}",
            FontSize = 11,
            Foreground = new SolidColorBrush(Color.Parse("#38BDF8"))
        });

        var actionsRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 10,
            Margin = new Thickness(0, 6, 0, 0)
        };

        var btnDemo = CreateVegasDarkButton("Open Broadcast Demo (.veg)", 190);
        btnDemo.Click += (_, _) =>
        {
            var demo = VegasProject.CreateBroadcastDemo();
            _onProjectSelected(demo, "Projeto de Demonstração Broadcast carregado.");
            Close();
        };

        var btnAdv = CreateVegasDarkButton("Advanced Settings...", 160);
        btnAdv.Click += (_, _) =>
        {
            var advWin = new AdvancedSettingsWindow(_currentProject, _rustInterop, _ffmpegStatusBanner, _onSettingsUpdated);
            advWin.Show(this);
        };

        var btnSplash = CreateVegasDarkButton("View Vegas Pro 12 Splash...", 180);
        btnSplash.Click += (_, _) =>
        {
            var splash = new VegasSplashWindow(durationSeconds: 3.5);
            splash.Show(this);
        };

        actionsRow.Children.Add(btnDemo);
        actionsRow.Children.Add(btnAdv);
        actionsRow.Children.Add(btnSplash);
        stack.Children.Add(actionsRow);

        return stack;
    }

    private Control BuildBottomFooterBar()
    {
        var footerBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#0B0C0E")),
            BorderBrush = new SolidColorBrush(Color.Parse("#1F2126")),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(10, 0)
        };

        var loc = LocalizationManager.Instance;
        var chkStartup = new CheckBox
        {
            Content = loc.T("Welcome.ShowOnStartup", "Show Welcome Screen on Startup"),
            IsChecked = ShowWelcomeOnStartup,
            FontSize = 11.5,
            Foreground = new SolidColorBrush(Color.Parse("#D1D5DB")),
            VerticalAlignment = VerticalAlignment.Center
        };
        chkStartup.IsCheckedChanged += (_, _) =>
        {
            ShowWelcomeOnStartup = chkStartup.IsChecked ?? true;
            loc.ShowWelcomeOnStartup = ShowWelcomeOnStartup;
            loc.SaveSettings();
        };

        footerBorder.Child = chkStartup;
        return footerBorder;
    }

    private static Button CreateVegasDarkButton(string text, double width)
    {
        return new Button
        {
            Content = text,
            Width = width,
            Height = 28,
            FontSize = 11.5,
            Background = new SolidColorBrush(Color.Parse("#2B2D31")),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#3E4147")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center
        };
    }
}
