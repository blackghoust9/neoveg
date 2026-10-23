using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using Neoveg.Controls;
using Neoveg.Core;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.ProjectInterchange;
using Neoveg.Scripting;
using Neoveg.Services;
using Neoveg.ViewModels;

namespace Neoveg.Views;

/// <summary>
/// Main Neoveg Window for Neoveg (100% Cross-Platform C# Avalonia UI + Rust FFI + Python Isolated FFmpeg).
/// Preserves the classic Neoveg 21 ergonomics:
/// - Upper Dock: Project Media / FX / Generators, Event Pan/Crop & Inspector, Video Preview Monitor, Master Mixer & Scopes
/// - Lower Dock: Multi-Track Timeline with Track Headers (Inline Track Renaming, V+A Track Grouping, Individual Track Height +/-)
/// </summary>
public sealed class MainWindow : Window
{
    public RustCoreInterop RustInterop { get; }
    public CppCoreInterop CppInterop { get; }
    public FFmpegPythonService PythonService { get; }
    public NeovegThemeManager ThemeManager { get; }
    public LocalizationManager Localization { get; }
    public ScriptingEngineService ScriptingService { get; }
    public MediaRelinker Relinker { get; }
    public SelectivePrerenderService PrerenderService { get; }
    public SubtitleGeneratorService SubtitleService { get; }
    public NeovegProject Project { get; private set; }
    public List<NeovegHistoryTheme> HistoryThemes { get; }
    public NeovegHistoryTheme CurrentTheme { get; private set; }

    private readonly string _rootDir;
    private readonly VideoPreviewControl _previewControl;
    private readonly MasterScopeControl _scopeControl;
    private readonly MasterBusViewModel _masterBusViewModel;
    private readonly MasterBusView _masterBusView;
    private readonly MultiTrackTimelineControl _timelineControl;
    private readonly PlayheadOverlayCanvas _playheadOverlay;
    private readonly StackPanel _trackHeadersStack;
    private ScrollViewer? _trackHeadersScroll;
    private Avalonia.Controls.Primitives.ScrollBar? _timelineHorizontalScrollBar;
    private Avalonia.Controls.Primitives.ScrollBar? _timelineVerticalScrollBar;
    private bool _syncingTimelineVerticalScroll;
    private readonly ContentControl _inspectorBodyHost;
    private StackPanel? _projectMediaStack;
    private readonly TextBlock _bigSmpteReadout;
    private readonly TextBlock _subFormatReadout;
    private readonly TextBlock _statusFooterText;
    private readonly TextBlock _engineStatusPill;
    private readonly Button _btnPlayPause;
    private readonly DispatcherTimer _playbackTimer;
    private Menu? _topMenu;
    private Border? _mainToolbarBorder;
    private Border? _statusBarBorder;
    private TimelineClip? _clipboardClip;
    private TabControl? _leftDockTabs;
    private TabItem? _tabProjectMedia;
    private TabItem? _tabTrimmer;
    private TabItem? _tabVideoFx;
    private TabItem? _tabAudioFx;
    private TabItem? _tabTransitions;
    private TabItem? _tabGenerators;
    private double _playbackOriginPlayhead;
    private Button? _btnTlSave;
    private Button? _btnTlImport;
    private Button? _btnTlSplit;
    private Button? _btnTlGroup;
    private Button? _btnTlSettings;
    private Button? _btnTlPopout;
    private Button? _btnTlRender;
    private Button? _btnSnap;
    private Button? _btnAutoRipple;
    private Button? _btnLoop;
    private readonly List<Border> _dockHeaderBorders = new();
    private readonly List<Border> _dockPanelBorders = new();

    // Undockable Neoveg windows (drag the header out to float; close the floating window to dock back)
    private DetachableDockPanel? _dockProjectMedia;
    private DetachableDockPanel? _dockPreview;
    private DetachableDockPanel? _dockMixer;
    private DetachableDockPanel? _dockTimeline;
    private Border? _previewInfoStrip;
    private Window? _inspectorWindow;
    private bool _previewFxBypassed;

    // Track Number Box blinking indicator (one shared brush toggled white/black every 500ms)
    // Only the selected track blinks; the timer runs only while a track is selected.
    private readonly SolidColorBrush _trackBlinkBrush = new(Colors.White);
    private static readonly IBrush TrackIndicatorIdleBrush = new SolidColorBrush(Color.FromArgb(150, 15, 23, 42));
    private readonly DispatcherTimer _trackBlinkTimer;
    private readonly Dictionary<string, Avalonia.Controls.Shapes.Ellipse> _trackBlinkDots = new();

    private const string ToggleOnBg = "#0369a1";
    private const string ToggleOffBg = "#1e293b";

    private List<MediaAssetItem> _mediaBin = new();
    private bool _isPlaying;
    private string _ffmpegBanner = "Python Isolated FFmpeg Inicializando...";

    public MainWindow()
    {
        string rootDir = ResolveWorkspaceRoot();
        _rootDir = rootDir;
        RustInterop = new RustCoreInterop(rootDir);
        CppInterop = new CppCoreInterop(rootDir);
        PythonService = new FFmpegPythonService(rootDir);
        ThemeManager = new NeovegThemeManager(rootDir);
        Localization = new LocalizationManager(rootDir);
        ScriptingService = new ScriptingEngineService(rootDir, RustInterop);
        Relinker = new MediaRelinker(rootDir, RustInterop, PythonService);
        PrerenderService = new SelectivePrerenderService(rootDir, RustInterop);
        SubtitleService = new SubtitleGeneratorService(rootDir, CppInterop);
        WelcomeWindow.ShowWelcomeOnStartup = Localization.ShowWelcomeOnStartup;
        HistoryThemes = ThemeManager.Themes.ToList();
        CurrentTheme = ThemeManager.ActiveTheme;
        if (!string.IsNullOrWhiteSpace(Localization.SelectedThemeId))
        {
            var savedTheme = HistoryThemes.FirstOrDefault(t => string.Equals(t.Id, Localization.SelectedThemeId, StringComparison.OrdinalIgnoreCase));
            if (savedTheme != null)
            {
                CurrentTheme = savedTheme;
                ThemeManager.SetActiveTheme(savedTheme);
            }
        }

        // Clean start (Neoveg "Untitled" project): no tracks, no demo media. The demo stays available under File.
        Project = NeovegProject.CreateEmpty("Sem título");
        Project.AutoFadeOnImport = Localization.AutoFadeOnImport;
        _mediaBin = new List<MediaAssetItem>();

        Title = $"{Project.Name} - Neoveg";
        Width = 1460;
        Height = 900;
        MinWidth = 1080;
        MinHeight = 680;
        WindowState = WindowState.Maximized;
        Background = new SolidColorBrush(Color.Parse("#11141c"));
        Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"));

        _previewControl = new VideoPreviewControl
        {
            Project = Project,
            RustInterop = RustInterop,
            CppInterop = CppInterop
        };
        _previewControl.OnPanCropUpdated += () =>
        {
            if (_previewControl.SelectedClip != null)
            {
                PrerenderService.InvalidateCacheRange(_previewControl.SelectedClip.Start, _previewControl.SelectedClip.End, Project.Fps);
                _timelineControl?.InvalidateVisual();
            }
            RefreshInspectorUI();
        };

        _masterBusViewModel = new MasterBusViewModel();
        _scopeControl = new MasterScopeControl
        {
            Project = Project,
            RustInterop = RustInterop,
            ViewModel = _masterBusViewModel
        };
        _masterBusView = new MasterBusView(Project, RustInterop, _scopeControl, _masterBusViewModel);

        _timelineControl = new MultiTrackTimelineControl
        {
            Project = Project,
            RustInterop = RustInterop,
            PrerenderService = PrerenderService
        };
        _playheadOverlay = new PlayheadOverlayCanvas
        {
            Project = Project,
            RustInterop = RustInterop,
            PixelsPerSecond = _timelineControl.PixelsPerSecond,
            RulerHeight = MultiTrackTimelineControl.RulerHeight
        };
        _timelineControl.PlayheadOverlay = _playheadOverlay;
        _trackHeadersStack = new StackPanel { Spacing = 0 };
        _timelineControl.OnZoomChanged += RefreshTimelineScrollBars;
        _inspectorBodyHost = new ContentControl();

        _bigSmpteReadout = new TextBlock
        {
            Text = "00:00:00;00",
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 18,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8")),
            VerticalAlignment = VerticalAlignment.Center
        };

        _subFormatReadout = new TextBlock
        {
            Text = $"{Project.Width}x{Project.Height} @{Project.Fps:0.##}p",
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
            VerticalAlignment = VerticalAlignment.Center
        };

        _statusFooterText = new TextBlock
        {
            Text = Localization.T("Status.Ready", "Pronto • Motor Rust FFI + Python FFmpeg Isolado carregados."),
            FontSize = 11,
            Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
            VerticalAlignment = VerticalAlignment.Center
        };

        _engineStatusPill = new TextBlock
        {
            Text = RustInterop.GetEngineBanner(),
            FontSize = 10.5,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8")),
            VerticalAlignment = VerticalAlignment.Center
        };

        _btnPlayPause = NeovegIcons.CreateButton(NeovegIcons.Play, "Reproduzir / Pausar (Espaço)", () => TogglePlayback(), filled: true);

        _trackBlinkTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(500) };
        _trackBlinkTimer.Tick += (_, _) =>
            _trackBlinkBrush.Color = _trackBlinkBrush.Color == Colors.White ? Colors.Black : Colors.White;

        _timelineControl.OnPlayheadScrubbed += newTime =>
        {
            Project.Playhead = newTime;
            UpdateTimecodeAndPreview();
        };
        _timelineControl.OnClipSelected += clip =>
        {
            UpdateTrackSelectionIndicators();
            _previewControl.SelectedClip = clip;
            RefreshInspectorUI();
            _previewControl.InvalidateVisual();
        };
        _timelineControl.OnTimelineMutated += msg =>
        {
            SetStatus(msg);
            RebuildTrackHeadersUI();
            UpdateTimecodeAndPreview();
            RefreshTimelineScrollBars();
        };
        _timelineControl.OnGeneratedMediaRequested += OpenGeneratedMediaDialog;
        _timelineControl.OnEventPanCropRequested += FocusEventPanCropPanel;
        _timelineControl.OnEventFxRequested += OpenClipFxChainDialog;
        _timelineControl.OnClipCutRequested += CutClipFromEventHeader;
        _timelineControl.OnClipCopyRequested += CopyClipFromEventHeader;
        _timelineControl.OnClipPasteRequested += PasteClipFromEventHeader;
        _timelineControl.OnClipDeleteRequested += (clip, _) =>
        {
            _timelineControl.SelectedClipId = clip.Id;
            DeleteSelectedClip();
        };
        _timelineControl.OnClipPropertiesRequested += OpenClipPropertiesDialog;

        _playbackTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(16) // 60 FPS high-performance overlay playback
        };
        _playbackTimer.Tick += (_, _) =>
        {
            if (!_isPlaying) return;
            Project.Playhead += 0.0166667;
            double maxEnd = Math.Max(Project.LoopEnd, GetProjectDuration());
            if (Project.LoopEnabled && Project.Playhead >= Project.LoopEnd)
                Project.Playhead = Project.LoopStart;
            else if (Project.Playhead >= maxEnd)
                TogglePlayback(false);

            UpdateTimecodeAndPreview();
            _playheadOverlay.InvalidateVisual();
        };

        Content = BuildNeovegWorkspaceLayout();
        RebuildTrackHeadersUI();
        RefreshInspectorUI();
        _timelineControl.RecalculateCrossfades();
        UpdateTimecodeAndPreview();

        Localization.OnLanguageChanged += langCode =>
        {
            RefreshLocalizedUI();
            var langMeta = Localization.GetLanguageMetadata(langCode);
            SetStatus($"{Localization.T("Status.LanguageChanged", "Idioma alterado em tempo real para:")} {langMeta.DisplayLabel}");
        };

        AddHandler(KeyDownEvent, OnNeovegKeyDown, Avalonia.Interactivity.RoutingStrategies.Tunnel);
        KeyDown += OnNeovegKeyDown;

        Opened += (_, _) =>
        {
            VerifyNativeFfiLibrariesOnStartup();

            if (WelcomeWindow.ShowWelcomeOnStartup)
            {
                Dispatcher.UIThread.Post(() => OpenWelcomeDialog(), DispatcherPriority.Background);
            }
            _ = Task.Run(async () =>
            {
                string banner = await PythonService.ProvisionAndGetStatusBannerAsync();
                Dispatcher.UIThread.Post(() =>
                {
                    _ffmpegBanner = banner;
                    _engineStatusPill.Text = $"{RustInterop.GetEngineBanner()} • {_ffmpegBanner}";
                });
            });
        };
    }

    private void VerifyNativeFfiLibrariesOnStartup()
    {
        var missingMessages = new List<string>();
        if (!RustInterop.IsNativeLoaded)
        {
            missingMessages.Add(RustInterop.LoadDiagnosticMessage);
        }
        if (!CppInterop.IsNativeLoaded)
        {
            missingMessages.Add(CppInterop.LoadDiagnosticMessage);
        }

        if (missingMessages.Count == 0)
            return;

        string combinedMessage = string.Join(Environment.NewLine + Environment.NewLine, missingMessages);
        Program.WriteCrashLog(
            "MainWindow.VerifyNativeFfiLibrariesOnStartup",
            new DllNotFoundException(combinedMessage),
            isTerminating: false);

        SetStatus("Aviso: Uma ou mais bibliotecas nativas FFI não foram encontradas (executando em modo Fallback C#).");
        Dispatcher.UIThread.Post(() => ShowMissingNativeDllNoticeDialog(combinedMessage), DispatcherPriority.Background);
    }

    private void ShowMissingNativeDllNoticeDialog(string diagnosticDetails)
    {
        var dlg = new Window
        {
            Title = "Neoveg — Aviso de Biblioteca Nativa (FFI)",
            Width = 640,
            Height = 320,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            CanResize = false,
            Background = new SolidColorBrush(Color.Parse("#181c26")),
            Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"))
        };

        var root = new StackPanel
        {
            Margin = new Thickness(18),
            Spacing = 12
        };

        root.Children.Add(new TextBlock
        {
            Text = "Aviso de Inicialização — Bibliotecas Nativas FFI",
            FontSize = 15,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#facc15"))
        });

        root.Children.Add(new TextBlock
        {
            Text = "Uma ou mais DLLs nativas (neoveg_core.dll / neoveg_cpp_engine.dll) não foram encontradas na pasta do executável. O Neoveg continuará aberto utilizando o motor gerenciado de contingência (C# Fallback).",
            TextWrapping = TextWrapping.Wrap,
            FontSize = 12,
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0"))
        });

        var detailsBox = new TextBox
        {
            Text = diagnosticDetails,
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            Height = 130,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 11,
            Background = new SolidColorBrush(Color.Parse("#0f131a")),
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        };
        root.Children.Add(detailsBox);

        var closeBtn = CreateSquareTextButton("Continuar no Modo de Segurança", "#2563eb", "#ffffff");
        closeBtn.HorizontalAlignment = HorizontalAlignment.Right;
        closeBtn.Click += (_, _) => dlg.Close();
        root.Children.Add(closeBtn);

        dlg.Content = root;
        dlg.ShowDialog(this);
    }

    private static string ResolveWorkspaceRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "python-backend")) &&
                Directory.Exists(Path.Combine(dir.FullName, "rust-core")))
            {
                return dir.FullName;
            }
            if (Directory.Exists(Path.Combine(dir.FullName, "Assets")) &&
                Directory.Exists(Path.Combine(dir.FullName, "scripts")))
            {
                return dir.FullName;
            }
            dir = dir.Parent;
        }
        return AppContext.BaseDirectory;
    }

    private Control BuildNeovegWorkspaceLayout()
    {
        var mainGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,5,*,Auto,24")
        };

        // Row 0: Native Cross-Platform Menu
        var menu = BuildTopMenuBar();
        Grid.SetRow(menu, 0);
        mainGrid.Children.Add(menu);

        // Row 1: Upper Dock Workspace (Project Media/Trimmer/FX tabs | Video Preview | Master Bus Mixer)
        var upperDock = BuildUpperDockWorkspace();
        Grid.SetRow(upperDock, 1);
        mainGrid.Children.Add(upperDock);

        // Row 2: Horizontal Splitter between Upper Docks and Timeline
        var hSplitter = new GridSplitter
        {
            Background = new SolidColorBrush(Color.Parse("#263042")),
            ResizeDirection = GridResizeDirection.Rows
        };
        Grid.SetRow(hSplitter, 2);
        mainGrid.Children.Add(hSplitter);

        // Row 3: Multi-Track Timeline Workspace (Track Headers + MultiTrackTimelineControl) — undockable
        _dockTimeline = BuildTimelineWorkspace();
        Grid.SetRow(_dockTimeline, 3);
        mainGrid.Children.Add(_dockTimeline);
        BindDockCollapse(_dockTimeline, mainGrid.RowDefinitions[3], hSplitter);

        // Row 4: Main Icon Toolbar below the Timeline (File • Transport • Edit • Prerender)
        var toolbar = BuildMainIconToolbar();
        Grid.SetRow(toolbar, 4);
        mainGrid.Children.Add(toolbar);

        // Row 5: Status Bar
        _statusBarBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse(CurrentTheme.StatusBarBg)),
            BorderBrush = new SolidColorBrush(Color.Parse(CurrentTheme.DockBorder)),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(10, 0),
            Child = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("*,Auto"),
                Children =
                {
                    _statusFooterText,
                    _engineStatusPill
                }
            }
        };
        Grid.SetColumn(_engineStatusPill, 1);
        Grid.SetRow(_statusBarBorder, 5);
        mainGrid.Children.Add(_statusBarBorder);

        return mainGrid;
    }

    /// <summary>
    /// Collapses the grid slot (column/row + its splitter) while a dock window floats and restores it when docked back.
    /// </summary>
    private static void BindDockCollapse(DetachableDockPanel dock, DefinitionBase definition, Control? splitter)
    {
        GridLength saved = GridLength.Star;
        dock.DetachedChanged += detached =>
        {
            if (detached)
            {
                if (definition is ColumnDefinition col) { saved = col.Width; col.Width = new GridLength(0); }
                else if (definition is RowDefinition row) { saved = row.Height; row.Height = new GridLength(0); }
            }
            else
            {
                if (definition is ColumnDefinition col) col.Width = saved;
                else if (definition is RowDefinition row) row.Height = saved;
            }
            if (splitter != null)
                splitter.IsVisible = !detached;
        };
    }

    private Menu BuildTopMenuBar()
    {
        _topMenu = new Menu
        {
            Background = new SolidColorBrush(Color.Parse(CurrentTheme.MenuBg)),
            Foreground = new SolidColorBrush(Color.Parse(CurrentTheme.TextPrimary))
        };
        PopulateTopMenuBarItems();
        return _topMenu;
    }

    private void PopulateTopMenuBarItems()
    {
        if (_topMenu == null) return;
        var loc = Localization;

        var mFileImportSubMenu = new MenuItem
        {
            Header = loc.T("Menu.File.Import", "Importar"),
            ItemsSource = new Control[]
            {
                CreateMenuItem(loc.T("Menu.File.Import.Neoveg", "Projeto Nativo Neoveg (.neoveg)..."), async () => await OpenNeovegProjectPickerAsync()),
                CreateMenuItem(loc.T("Menu.File.Import.Xml", "Final Cut Pro / Premiere XML (.xml, .fcpxml)..."), async () => await ImportXmlOrFcpxmlProjectPickerAsync()),
                new Separator(),
                CreateMenuItem(loc.T("Menu.File.ImportMedia", "Importar Mídia (MP4 / WAV / MP3)..."), async () => await OpenImportMediaDialogAsync())
            }
        };

        var mFileExportSubMenu = new MenuItem
        {
            Header = loc.T("Menu.File.Export", "Exportar"),
            ItemsSource = new Control[]
            {
                CreateMenuItem(loc.T("Menu.File.Export.Fcp7Xml", "Final Cut Pro 7 / Premiere XML (.xml)..."), async () => await ExportFcp7XmlProjectPickerAsync()),
                CreateMenuItem(loc.T("Menu.File.Export.Fcpxml", "Final Cut Pro X XML (.fcpxml)..."), async () => await ExportFcpxmlProjectPickerAsync()),
                CreateMenuItem(loc.T("Menu.File.Export.Edl", "EDL CMX3600 (.edl)..."), async () => await ExportEdlProjectPickerAsync())
            }
        };

        var mFile = new MenuItem { Header = loc.T("Menu.File", "_Arquivo") };
        mFile.ItemsSource = new Control[]
        {
            CreateMenuItem(loc.T("Menu.File.Splash", "Exibir Tela de Carregamento (Splash Screen Neoveg 12)..."), NeovegSplashDialog),
            CreateMenuItem(loc.T("Menu.File.Welcome", "Janela de Boas-Vindas Simplificada..."), OpenWelcomeDialog),
            new Separator(),
            CreateMenuItem(loc.T("Menu.File.NewEmpty", "Novo Projeto Vazio (.neoveg)"), () => LoadProjectState(NeovegProject.CreateEmpty("Sem título"), "Novo Projeto Vazio (.neoveg) iniciado.")),
            CreateMenuItem(loc.T("Menu.File.Open", "Abrir Projeto (.neoveg)... (Ctrl+O)"), async () => await OpenNeovegProjectPickerAsync()),
            CreateMenuItem(loc.T("Menu.File.OpenDemo", "Abrir Projeto de Demonstração (Neoveg_Broadcast_Demo.neoveg)"), () => LoadProjectState(NeovegProject.CreateBroadcastDemo(), "Projeto Demo Broadcast (.neoveg) carregado.")),
            CreateMenuItem("Abrir Projeto com Mídias Movidas (Teste Search & Relink)...", () => LoadProjectState(NeovegProject.CreateMovedMediaRelinkDemo(), "Projeto com Mídias Movidas carregado (Missing Media Relinker).")),
            CreateMenuItem("Localizar e Relinkar Mídias Ausentes (Missing Media Dialog)...", () => OpenMissingMediaDialogForCurrentProject(forceDemoIfNoneMissing: false)),
            CreateMenuItem(loc.T("Menu.File.Save", "Salvar Projeto (.neoveg) (Ctrl+S)"), SaveCurrentProject),
            CreateMenuItem(loc.T("Menu.File.SaveAs", "Salvar Projeto Como (.neoveg)... (Ctrl+Shift+S)"), async () => await SaveProjectAsNeovegPickerAsync()),
            new Separator(),
            mFileImportSubMenu,
            mFileExportSubMenu,
            CreateMenuItem(loc.T("Menu.File.ImportMedia", "Importar Mídia (MP4 / WAV / MP3)..."), async () => await OpenImportMediaDialogAsync()),
            new Separator(),
            CreateMenuItem(loc.T("Menu.File.RenderAs", "Render As... (Python Isolated FFmpeg)"), async () => await RunIsolatedFFmpegRenderAsync()),
            new Separator(),
            CreateMenuItem(loc.T("Menu.File.Exit", "Sair"), Close)
        };

        var mEdit = new MenuItem { Header = loc.T("Menu.Edit", "_Editar") };
        mEdit.ItemsSource = new Control[]
        {
            CreateMenuItem(loc.T("Menu.Edit.PlayPause", "Play / Pause (Space)"), () => TogglePlayback()),
            CreateMenuItem(loc.T("Menu.Edit.Split", "Dividir Evento no Cursor via Rust FFI (S)"), SplitAtPlayheadViaRustFfi),
            CreateMenuItem(loc.T("Menu.Edit.Delete", "Excluir Evento Selecionado (Del)"), DeleteSelectedClip),
            new Separator(),
            CreateMenuItem(loc.T("Menu.Edit.Preferences", "Preferências..."), OpenAdvancedSettingsDialog)
        };

        var mInsert = new MenuItem { Header = loc.T("Menu.Insert", "_Inserir") };
        mInsert.ItemsSource = new Control[]
        {
            CreateMenuItem(loc.T("Menu.Insert.AudioTrack", "Faixa de Áudio"), () => AddTrack("audio"), "Ctrl+Q"),
            CreateMenuItem(loc.T("Menu.Insert.VideoTrack", "Faixa de Vídeo"), () => AddTrack("video"), "Ctrl+Shift+Q"),
            new Separator(),
            CreateMenuItem(loc.T("Menu.Insert.GroupAV", "Agrupar Faixas de Vídeo + Áudio (Grupo V+A)"), GroupVideoAndAudioTracks),
            CreateMenuItem(loc.T("Menu.Insert.Ungroup", "Desagrupar Faixas"), UngroupAllOrSelectedTracks),
            new Separator(),
            CreateMenuItem(loc.T("Menu.Insert.ExpandTracks", "Aumentar Altura de Todas as Faixas (+)"), () => AdjustAllTrackHeights(14)),
            CreateMenuItem(loc.T("Menu.Insert.ShrinkTracks", "Diminuir Altura de Todas as Faixas (-)"), () => AdjustAllTrackHeights(-14))
        };

        var mView = new MenuItem { Header = loc.T("Menu.View", "E_xibir") };
        var winExtensionsMenu = new MenuItem { Header = loc.T("Menu.View.WindowExtensions", "Extensões de Janela (Window Extensions)") };
        var extItems = new List<Control>
        {
            CreateMenuItem("Abrir Gerenciador de Extensões Acopláveis (VPFlow / Datamosh / Neoveg Toolkit / HOS)... [↗]", () => OpenDockableExtensionDialog("vpflow_extension")),
            new Separator()
        };
        foreach (var ext in ScriptingService.DiscoveredExtensions)
        {
            var capturedExt = ext;
            extItems.Add(CreateMenuItem($"📌 {capturedExt.Name}", () => OpenDockableExtensionDialog(capturedExt.Id)));
        }
        winExtensionsMenu.ItemsSource = extItems;

        var windowsSubMenu = new MenuItem
        {
            Header = loc.T("Menu.View.Windows", "Janelas (Destacar / Acoplar)"),
            ItemsSource = new Control[]
            {
                CreateMenuItem("Timeline", () => OpenFloatingOrDockedPanel("Timeline")),
                CreateMenuItem("Video Preview", () => OpenFloatingOrDockedPanel("Video Preview")),
                CreateMenuItem("Trimmer", () => OpenFloatingOrDockedPanel("Trimmer")),
                CreateMenuItem("Master Bus (Mixer)", () => OpenFloatingOrDockedPanel("Audio Mixer")),
                CreateMenuItem("Project Media", () => OpenFloatingOrDockedPanel("Project Media"))
            }
        };

        var overlaysSubMenu = new MenuItem { Header = loc.T("Menu.View.PreviewOverlays", "Sobreposições da Prévia (Overlays)") };
        overlaysSubMenu.SubmenuOpened += (_, _) => overlaysSubMenu.ItemsSource = BuildPreviewOverlayMenuItems();
        overlaysSubMenu.ItemsSource = BuildPreviewOverlayMenuItems();

        mView.ItemsSource = new Control[]
        {
            winExtensionsMenu,
            new Separator(),
            windowsSubMenu,
            overlaysSubMenu,
            CreateMenuItem("Track Motion...", () => OpenFloatingOrDockedPanel("Track Motion")),
            CreateMenuItem(loc.T("Menu.View.PanCrop", "Abrir Event Pan/Crop (Gizmo 'F' + Property Grid + Keyframes) [↗]"), OpenEventPanCropDialogForSelectedClip),
            CreateMenuItem(loc.T("Menu.View.PopoutPreview", "Destacar Video Preview para Outro Monitor [↗]"), PopoutVideoPreviewToMonitor),
            CreateMenuItem(loc.T("Menu.View.PopoutScopes", "Destacar Master Audio Mixer & RGB Scopes para Outro Monitor [↗]"), PopoutMixerScopesToMonitor),
            new Separator(),
            CreateMenuItem(loc.T("Menu.View.Splash", "Exibir Tela de Carregamento (Splash Screen Neoveg 12) [↗]"), NeovegSplashDialog),
            CreateMenuItem(loc.T("Menu.View.ThemesGallery", "Abrir Galeria de Temas Históricos (Classic Industrial / Classic NLE / Modern NLE / Contemporary NLE) [↗]"), OpenHistoricalThemesGalleryDialog)
        };

        var mTools = new MenuItem { Header = loc.T("Menu.Tools", "_Ferramentas") };
        var scriptsSubMenu = new MenuItem { Header = loc.T("Menu.Tools.Scripting", "Scripts (Scripting)") };
        var scriptMenuItems = new List<Control>
        {
            CreateMenuItem(loc.T("Menu.Tools.RunScript", "Executar Script... (Run Script...)"), async () => await OpenRunScriptFilePickerAsync()),
            CreateMenuItem(loc.T("Menu.Tools.RescanScripts", "Recarregar Pasta de Scripts (Rescan Scripts Folder)"), RescanScriptsAndExtensionsFolders),
            CreateMenuItem("Abrir Console Interativo Roslyn C# & Python (.cs / .py)... [↗]", () => OpenDockableExtensionDialog("console")),
            new Separator()
        };
        foreach (var scr in ScriptingService.DiscoveredScripts)
        {
            var capturedScript = scr;
            scriptMenuItems.Add(CreateMenuItem(
                $"▶ {capturedScript.FileName} — {capturedScript.ExtensionType}",
                async () => await RunDiscoveredScriptAsync(capturedScript)));
        }
        scriptsSubMenu.ItemsSource = scriptMenuItems;

        mTools.ItemsSource = new Control[]
        {
            CreateMenuItem(loc.T("Menu.Tools.Inspector", "Inspetor (Inspector & FX do Evento)..."), OpenInspectorWindow),
            new Separator(),
            scriptsSubMenu,
            new Separator(),
            CreateMenuItem("Construir Dynamic RAM Preview (Shift+B) -> Verde #2ECC71 (60 FPS)", async () => await TriggerDynamicRamPreviewAsync()),
            CreateMenuItem("Executar Pré-Renderização Seletiva em Disco (Shift+M -> cache/prerender/)", async () => await TriggerSelectiveDiskPrerenderAsync()),
            CreateMenuItem("Limpar Cache de Pré-Renderização e RAM Preview (Reverter para Vermelho #E74C3C)", () =>
            {
                PrerenderService.InvalidateAllCache(cleanDiskFolder: true);
                _timelineControl.InvalidateVisual();
                SetStatus("Cache de RAM Preview e Pré-Renderização Seletiva (cache/prerender/) limpo -> trechos complexos revertidos para Vermelho (#E74C3C).");
            }),
            new Separator(),
            CreateMenuItem("Gerar Legendas Automáticas por IA (Whisper.cpp -> Eventos de Texto na Timeline)", async () => await TriggerWhisperSubtitlesGenerationAsync()),
            CreateMenuItem("Localizar e Relinkar Mídias Ausentes (Search & Relink Missing Media)... [↗]", () => OpenMissingMediaDialogForCurrentProject(forceDemoIfNoneMissing: true)),
            CreateMenuItem(loc.T("Menu.Tools.CppSharedMem", "Testar C++ Shared Memory Frame Transfer (Out-of-Process IPC) [↗]"), async () => await RunCppSharedMemoryTransferDemoAsync())
        };

        var mThemes = new MenuItem { Header = loc.T("Menu.Themes", "_Temas Históricos") };
        var themeItems = new List<Control>
        {
            CreateMenuItem(loc.T("Menu.Themes.Gallery", "Galeria Histórica de Temas, Paletas e Ícones (1999–2026)..."), OpenHistoricalThemesGalleryDialog),
            new Separator()
        };
        foreach (var th in HistoryThemes)
        {
            var captured = th;
            themeItems.Add(CreateMenuItem($"Era {captured.EraNumber}: {captured.Name}", () => ApplyHistoricalTheme(captured)));
        }
        mThemes.ItemsSource = themeItems;

        var mHelp = new MenuItem { Header = loc.T("Menu.Help", "A_juda") };
        mHelp.ItemsSource = new Control[]
        {
            CreateMenuItem(loc.T("Menu.Help.Welcome", "Tela de Boas-Vindas e Guia Rápido..."), OpenWelcomeDialog),
            CreateMenuItem("Visualizador de Diagnóstico e Logs (logs/neoveg.log • Copiar Logs)... [↗]", OpenLogViewerWindow),
            CreateMenuItem(loc.T("Menu.Help.About", "Sobre o Neoveg (Avalonia UI + Rust + C++ + Python)..."), NeovegSplashDialog)
        };

        _topMenu.ItemsSource = new[] { mFile, mEdit, mInsert, mView, mTools, mThemes, mHelp };
    }

    public void OpenLogViewerWindow()
    {
        var logWin = new LogViewerWindow();
        logWin.Show(this);
        SetStatus("Visualizador de Diagnóstico e Logs aberto (logs/neoveg.log & logs/crash.log).");
    }

    private Border BuildMainIconToolbar()
    {
        var loc = Localization;
        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 3,
            VerticalAlignment = VerticalAlignment.Center
        };

        void AddGroupSeparator() => bar.Children.Add(new Border
        {
            Width = 1,
            Height = 20,
            Margin = new Thickness(5, 0),
            Background = new SolidColorBrush(Color.Parse("#475569"))
        });

        // File group
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.NewProject, loc.T("Toolbar.Tooltip.New", "Novo Projeto"), () => LoadProjectState(NeovegProject.CreateEmpty("Sem título"), "Novo Projeto Vazio (.neoveg) iniciado.")));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Open, loc.T("Toolbar.Tooltip.Open", "Abrir Projeto (Ctrl+O)"), async () => await OpenNeovegProjectPickerAsync()));
        _btnTlSave = NeovegIcons.CreateButton(NeovegIcons.Save, loc.T("Toolbar.Tooltip.Save", "Salvar Projeto (Ctrl+S)"), SaveCurrentProject);
        bar.Children.Add(_btnTlSave);
        _btnTlImport = NeovegIcons.CreateButton(NeovegIcons.ImportMedia, loc.T("Toolbar.Tooltip.Import", "Importar Mídia"), async () => await OpenImportMediaDialogAsync());
        bar.Children.Add(_btnTlImport);
        _btnTlRender = NeovegIcons.CreateButton(NeovegIcons.Render, loc.T("Toolbar.Tooltip.Render", "Renderizar Como... (Python Isolated FFmpeg)"), async () => await RunIsolatedFFmpegRenderAsync());
        bar.Children.Add(_btnTlRender);
        _btnTlSettings = NeovegIcons.CreateButton(NeovegIcons.Gear, loc.T("Toolbar.Tooltip.Settings", "Preferências"), OpenAdvancedSettingsDialog);
        bar.Children.Add(_btnTlSettings);
        AddGroupSeparator();

        // Transport group (Neoveg transport bar below the timeline)
        _btnLoop = NeovegIcons.CreateButton(NeovegIcons.Loop, "Reprodução em Loop", ToggleLoopPlayback, Project.LoopEnabled ? ToggleOnBg : ToggleOffBg);
        bar.Children.Add(_btnLoop);
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.PlayFromStart, "Reproduzir do Início", () =>
        {
            TogglePlayback(false);
            Project.Playhead = 0;
            TogglePlayback(true);
        }, filled: true));
        bar.Children.Add(_btnPlayPause);
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Stop, "Parar (retorna o cursor ao ponto de início)", StopPlaybackAndReturnCursor, filled: true));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.GoToStart, "Ir para o Início (Home)", () => MovePlayheadTo(0.0), filled: true));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.GoToEnd, "Ir para o Fim (End)", () => MovePlayheadTo(GetProjectDuration()), filled: true));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.PrevFrame, "Quadro Anterior (←)", () => MovePlayheadTo(Project.Playhead - 1.0 / Math.Max(1.0, Project.Fps)), filled: true));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.NextFrame, "Próximo Quadro (→)", () => MovePlayheadTo(Project.Playhead + 1.0 / Math.Max(1.0, Project.Fps)), filled: true));
        AddGroupSeparator();

        // Edit group
        _btnTlSplit = NeovegIcons.CreateButton(NeovegIcons.Split, loc.T("Toolbar.Tooltip.Split", "Dividir Evento no Cursor (S)"), SplitAtPlayheadViaRustFfi);
        bar.Children.Add(_btnTlSplit);
        _btnTlGroup = NeovegIcons.CreateButton(NeovegIcons.Group, loc.T("Toolbar.Tooltip.GroupAV", "Agrupar (G)"), GroupSelectedEventsOrTracks);
        bar.Children.Add(_btnTlGroup);
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Ungroup, "Desagrupar (U)", UngroupSelectedEventsOrTracks));
        _btnSnap = NeovegIcons.CreateButton(NeovegIcons.Snap, "Ativar Snapping (F8)", ToggleTimelineSnapping, Project.SnapEnabled ? ToggleOnBg : ToggleOffBg);
        bar.Children.Add(_btnSnap);
        _btnAutoRipple = NeovegIcons.CreateButton(NeovegIcons.AutoRipple, "Auto-Ripple (ajusta eventos adjacentes em cortes e exclusões)", () =>
        {
            Project.RippleEnabled = !Project.RippleEnabled;
            UpdateToolbarToggleStates();
            SetStatus($"Auto-Ripple {(Project.RippleEnabled ? "ATIVADO" : "DESATIVADO")}.");
        }, Project.RippleEnabled ? ToggleOnBg : ToggleOffBg);
        bar.Children.Add(_btnAutoRipple);
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Marker, "Inserir Marcador (M)", AddMarkerAtPlayhead));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Region, "Inserir Região (R)", AddRegionAtLoopOrPlayhead));
        AddGroupSeparator();

        // Track group
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.AddAudioTrack, "Inserir Faixa de Áudio (Ctrl+Q)", () => AddTrack("audio")));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.AddVideoTrack, "Inserir Faixa de Vídeo (Ctrl+Shift+Q)", () => AddTrack("video")));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackHeightShrink, "Diminuir Altura das Faixas", () => AdjustAllTrackHeights(-12)));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackHeightGrow, "Aumentar Altura das Faixas", () => AdjustAllTrackHeights(12)));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.ZoomOut, "Diminuir Zoom da Timeline", () => ZoomTimeline(1.0 / 1.25)));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.ZoomIn, "Aumentar Zoom da Timeline", () => ZoomTimeline(1.25)));
        AddGroupSeparator();

        // Prerender / timecode group
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.RamPreview, "Dynamic RAM Preview (Shift+B)", async () => await TriggerDynamicRamPreviewAsync(), fgHex: "#2ECC71"));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Prerender, "Pré-Renderização Seletiva (Shift+M)", async () => await TriggerSelectiveDiskPrerenderAsync(), fgHex: "#F1C40F"));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Timecode, "Alternar Timecode Drop-Frame / Non-Drop-Frame", ToggleDropFrameTimecode, fgHex: "#3498DB"));
        AddGroupSeparator();

        // Historical Era Quick Theme Icon Buttons (Tooltips only)
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackColor, "1: Classic Industrial", () => ApplyHistoricalTheme(ThemeManager.GetThemeByEra(1)), "#84827E", "#FFFFFF"));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackColor, "2: Classic Silver/Gold", () => ApplyHistoricalTheme(ThemeManager.GetThemeByEra(2)), "#D97706", "#FFFFFF"));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackColor, "3: Modern Carbon Dark", () => ApplyHistoricalTheme(ThemeManager.GetThemeByEra(3)), "#0078D7", "#FFFFFF"));
        bar.Children.Add(NeovegIcons.CreateButton(NeovegIcons.TrackColor, "4: Contemporary 2026", () => ApplyHistoricalTheme(ThemeManager.GetThemeByEra(4)), "#00A3FF", "#FFFFFF"));

        _mainToolbarBorder = new Border
        {
            Background = NeovegThemeManager.CreateToolbarBrush(CurrentTheme),
            BorderBrush = new SolidColorBrush(Color.Parse(CurrentTheme.DockBorder)),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(6, 3),
            Child = new ScrollViewer
            {
                HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
                Content = bar
            }
        };
        return _mainToolbarBorder;
    }

    private void UpdateToolbarToggleStates()
    {
        if (_btnSnap != null) _btnSnap.Background = new SolidColorBrush(Color.Parse(Project.SnapEnabled ? ToggleOnBg : ToggleOffBg));
        if (_btnAutoRipple != null) _btnAutoRipple.Background = new SolidColorBrush(Color.Parse(Project.RippleEnabled ? ToggleOnBg : ToggleOffBg));
        if (_btnLoop != null) _btnLoop.Background = new SolidColorBrush(Color.Parse(Project.LoopEnabled ? ToggleOnBg : ToggleOffBg));
    }

    private void ToggleLoopPlayback()
    {
        Project.LoopEnabled = !Project.LoopEnabled;
        UpdateToolbarToggleStates();
        _timelineControl.InvalidateVisual();
        SetStatus($"Reprodução em Loop {(Project.LoopEnabled ? "ATIVADA" : "DESATIVADA")}.");
    }

    private void StopPlaybackAndReturnCursor()
    {
        bool wasPlaying = _isPlaying;
        TogglePlayback(false);
        if (wasPlaying)
            Project.Playhead = _playbackOriginPlayhead;
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
    }

    private void MovePlayheadTo(double seconds)
    {
        Project.Playhead = Math.Max(0.0, seconds);
        _playheadOverlay.NotifyKeyboardCursorMove();
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
    }

    private void ZoomTimeline(double factor)
    {
        // Toolbar zoom is anchored at the edit cursor (playhead) when it is visible, otherwise at the view center
        double oldPps = Math.Max(20.0, _timelineControl.PixelsPerSecond);
        double viewWidth = Math.Max(1.0, _timelineControl.Bounds.Width);
        double cursorX = (Project.Playhead - _timelineControl.HorizontalOffsetSeconds) * oldPps;
        double anchorX = cursorX >= 0.0 && cursorX <= viewWidth ? cursorX : viewWidth * 0.5;
        double anchorTime = _timelineControl.HorizontalOffsetSeconds + anchorX / oldPps;
        _timelineControl.PixelsPerSecond = Math.Clamp(
            _timelineControl.PixelsPerSecond * factor,
            MultiTrackTimelineControl.MinPixelsPerSecond,
            MultiTrackTimelineControl.MaxPixelsPerSecond);
        _timelineControl.HorizontalOffsetSeconds = Math.Max(0.0, anchorTime - anchorX / Math.Max(20.0, _timelineControl.PixelsPerSecond));
        _playheadOverlay.PixelsPerSecond = _timelineControl.PixelsPerSecond;
        RefreshTimelineScrollBars();
        _timelineControl.InvalidateVisual();
        _playheadOverlay.InvalidateVisual();
    }

    private void AddMarkerAtPlayhead()
    {
        Project.Markers.Add(new TimelineMarker
        {
            Time = Math.Round(Project.Playhead, 3),
            Label = $"Marker {Project.Markers.Count + 1}",
            Color = "#f97316"
        });
        _timelineControl.InvalidateVisual();
        SetStatus($"Atalho Neoveg [M]: Marcador adicionado em {RustInterop.FormatSmpte(Project.Playhead, Project.Fps)}");
    }

    private void ToggleDropFrameTimecode()
    {
        bool curDf = PlayheadOverlayCanvas.ResolveIsDropFrame(Project.Fps, _playheadOverlay.ForceDropFrame);
        _playheadOverlay.ForceDropFrame = !curDf;
        UpdateTimecodeAndPreview();
        _playheadOverlay.InvalidateVisual();
        string modeLabel = _playheadOverlay.ForceDropFrame == true ? "Drop-Frame (00;00;50;00)" : "Non-Drop-Frame (00:00:50:00)";
        SetStatus($"Régua de Timecode do Playhead alterada para {modeLabel}.");
    }

    private Grid BuildUpperDockWorkspace()
    {
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("1.1*,4,1.65*,4,0.8*")
        };

        // 1. Left Dock: Project Media, Trimmer, Video FX, Audio FX, Transitions, Generators (each tab undockable)
        _dockProjectMedia = BuildLeftMediaAndFxDock();
        _dockProjectMedia.MinWidth = 220;
        Grid.SetColumn(_dockProjectMedia, 0);
        grid.Children.Add(_dockProjectMedia);

        var splitterLeft = CreateVerticalSplitter(1);
        grid.Children.Add(splitterLeft);
        BindDockCollapse(_dockProjectMedia, grid.ColumnDefinitions[0], splitterLeft);

        // 2. Video Preview (clean: overlays are optional and start disabled)
        _dockPreview = BuildVideoPreviewDock();
        _dockPreview.MinWidth = 300;
        Grid.SetColumn(_dockPreview, 2);
        grid.Children.Add(_dockPreview);
        BindDockCollapse(_dockPreview, grid.ColumnDefinitions[2], null);

        var splitterRight = CreateVerticalSplitter(3);
        grid.Children.Add(splitterRight);

        // 3. Master Bus Mixer & Scopes
        _dockMixer = BuildMasterMixerDock();
        _dockMixer.MinWidth = 175;
        Grid.SetColumn(_dockMixer, 4);
        grid.Children.Add(_dockMixer);
        BindDockCollapse(_dockMixer, grid.ColumnDefinitions[4], splitterRight);

        return grid;
    }

    private DetachableDockPanel BuildLeftMediaAndFxDock()
    {
        var tabs = new TabControl
        {
            Background = new SolidColorBrush(Color.Parse("#141923"))
        };
        _leftDockTabs = tabs;
        Neoveg.Styles.NeovegDockTabStyles.Apply(tabs);

        // Tab 1: Project Media Bin
        _projectMediaStack = new StackPanel { Spacing = 6, Margin = new Thickness(6) };
        RebuildProjectMediaBinUI();

        // Tab 2: Video FX (CorridorKey AI + Frei0r + Resolve Color + Neoveg 3D LUTs .cube)
        var fxStack = new StackPanel { Spacing = 6, Margin = new Thickness(6) };
        var fxCatalog = new (string Type, string Badge, string Name, string Desc)[]
        {
            ("corridorKey", "CORRIDORKEY", "CorridorKey AI Green Screen Unmixer", "Desmixagem neural de alpha linear e remoção de spill verde"),
            ("colorGrade", "COLOR", "Neoveg 3-Wheel Color Grade", "Lift / Gamma / Gain cinematográfico"),
            ("lut3d.teal_orange", "3D LUT", "Neoveg 3D LUT — Teal & Orange (.cube)", "Look Hollywood Blockbuster importado do kdenlive_neoveg (TEAL_ORANGE.cube)"),
            ("lut3d.cinematic", "3D LUT", "Neoveg 3D LUT — Cinematic Film (.cube)", "Curva S de contraste cinematográfico (CINEMATIC.cube)"),
            ("lut3d.blue_tint", "3D LUT", "Neoveg 3D LUT — Cold Blue Tint (.cube)", "Balanço frio anamórfico para cenas noturnas (BLUE_TINT.cube)"),
            ("lut3d.green_tint", "3D LUT", "Neoveg 3D LUT — Matrix Green Tint (.cube)", "Estética sci-fi em tons esverdeados (GREEN_TINT.cube)"),
            ("frei0r.glow", "FREI0R", "Frei0r Glow (Optical Bloom)", "Difusão óptica de altas-luzes via FFmpeg Frei0r"),
            ("frei0r.cartoon", "FREI0R", "Frei0r Cartoon (Cel-Shaded)", "Quantização tonal e contorno Sobel"),
            ("frei0r.pixeliz0r", "FREI0R", "Frei0r Pixeliz0r (Mosaic)", "Mosaico paramétrico de blocos")
        };
        foreach (var fx in fxCatalog)
        {
            var item = fx;
            fxStack.Children.Add(CreateCatalogCard(item.Badge, item.Name, item.Desc, "+ FX", () => ApplyFxToSelectedOrNewClip(item.Type, item.Name)));
        }

        // Tab 3: Audio FX (LADSPA)
        var audioFxStack = new StackPanel { Spacing = 6, Margin = new Thickness(6) };
        var ladspaCatalog = new (string Type, string Name, string Desc)[]
        {
            ("ladspa.sc4_1882", "LADSPA SC4 Stereo Compressor (1882)", "Compressor dinâmico RMS/Peak broadcast"),
            ("ladspa.fast_lookahead_limiter_1913", "LADSPA Lookahead Limiter (1913)", "Brickwall True-Peak Limiter (-1.0 dBTP)"),
            ("ladspa.mbeq_1197", "LADSPA 15-Band Graphic EQ (1197)", "Equalizador harmônico de 15 bandas"),
            ("ladspa.plate_1423", "LADSPA Neoveg Plate Reverb (1423)", "Reverberação acústica de placa metálica")
        };
        foreach (var lfx in ladspaCatalog)
        {
            var item = lfx;
            audioFxStack.Children.Add(CreateCatalogCard("LADSPA", item.Name, item.Desc, "+ Audio FX", () => ApplyFxToSelectedOrNewClip(item.Type, item.Name)));
        }

        // Tab 4: Transitions (Crossfades, Clock Wipe, Additive Dissolve, Barn Door)
        var transStack = new StackPanel { Spacing = 6, Margin = new Thickness(6) };
        var transCatalog = new (string Type, string Name, string Desc)[]
        {
            ("transition.crossEffect", "Neoveg Cross Effect (Auto-Crossfade S-Curve)", "Transição suave de sobreposição automática entre dois eventos"),
            ("transition.additiveDissolve", "Neoveg Additive Dissolve (Flash Glow)", "Dissolução aditiva de luminância estilo broadcast"),
            ("transition.clockWipe", "Neoveg Clock Wipe Radial", "Transição radial horária clássica do Classic NLE"),
            ("transition.linearWipe", "Neoveg Linear Wipe / Barn Door", "Cortina geométrica direcional com borda suave")
        };
        foreach (var trItem in transCatalog)
        {
            var item = trItem;
            transStack.Children.Add(CreateCatalogCard("TRANS", item.Name, item.Desc, "+ Apply", () => ApplyFxToSelectedOrNewClip(item.Type, item.Name)));
        }

        // Tab 5: Media Generators
        var genStack = new StackPanel { Spacing = 6, Margin = new Thickness(6) };
        var gens = new[]
        {
            new MediaAssetItem { Name = "CorridorKey Green Screen Subject", Type = "generator", GeneratorType = "greenScreenActor", Duration = 5.0, Color = "#16a34a" },
            new MediaAssetItem { Name = "Neoveg Titles & Text Overlay", Type = "title", GeneratorType = "titleText", Duration = 4.5, Color = "#6366f1" },
            new MediaAssetItem { Name = "SMPTE HD Master Color Bars", Type = "generator", GeneratorType = "smpteBars", Duration = 4.5, Color = "#2563eb" },
            new MediaAssetItem { Name = "Synthwave Horizon Cyber Grid", Type = "generator", GeneratorType = "cyberGrid", Duration = 5.0, Color = "#ec4899" }
        };
        foreach (var g in gens)
        {
            var genItem = g;
            genStack.Children.Add(CreateCatalogCard("GEN", genItem.Name, $"Gerador Procedural 60fps • {genItem.Duration:0.0}s", "+ Inserir", () => InsertMediaAssetIntoTimeline(genItem)));
        }

        var loc = Localization;
        _tabProjectMedia = new TabItem { Header = loc.T("Tab.ProjectMedia", "Project Media"), Content = new ScrollViewer { Content = _projectMediaStack } };
        _tabTrimmer = new TabItem { Header = "Trimmer", Content = new ScrollViewer { Content = BuildTrimmerDockContent() } };
        _tabVideoFx = new TabItem { Header = loc.T("Tab.VideoFX", "Video FX"), Content = new ScrollViewer { Content = fxStack } };
        _tabAudioFx = new TabItem { Header = loc.T("Tab.AudioFX", "Audio FX"), Content = new ScrollViewer { Content = audioFxStack } };
        _tabTransitions = new TabItem { Header = loc.T("Tab.Transitions", "Transições"), Content = new ScrollViewer { Content = transStack } };
        _tabGenerators = new TabItem { Header = loc.T("Tab.Generators", "Geradores"), Content = new ScrollViewer { Content = genStack } };

        foreach (var tab in new[] { _tabProjectMedia, _tabTrimmer, _tabVideoFx, _tabAudioFx, _tabTransitions, _tabGenerators })
        {
            tabs.Items.Add(tab);
        }
        // Neoveg docking: drag any tab header down/out of the tab strip to float it as an independent window
        DetachableDockPanel.EnableTabUndocking(tabs, t => t.Header?.ToString() ?? "Neoveg");

        return WrapInDockPanel(loc.T("Dock.LeftHeader", "Project Media"), tabs);
    }

    private Control BuildTrimmerDockContent()
    {
        var stack = new StackPanel { Spacing = 8, Margin = new Thickness(8) };
        var txtIn = new TextBox { Text = "0.000", Watermark = "In Point (s)" };
        var txtOut = new TextBox { Text = "4.000", Watermark = "Out Point (s)" };

        var btnRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        var btnMarkIn = NeovegIcons.CreateButton(NeovegIcons.MarkIn, "Marcar In (I)", fgHex: "#facc15");
        btnMarkIn.Click += (_, _) => txtIn.Text = Project.Playhead.ToString("0.000", System.Globalization.CultureInfo.InvariantCulture);

        var btnMarkOut = NeovegIcons.CreateButton(NeovegIcons.MarkOut, "Marcar Out (O)", fgHex: "#facc15");
        btnMarkOut.Click += (_, _) => txtOut.Text = Math.Max(Project.Playhead + 0.5, 1.0).ToString("0.000", System.Globalization.CultureInfo.InvariantCulture);

        var btnInsertSubclip = NeovegIcons.CreateButton(NeovegIcons.InsertToTimeline, "Inserir do Trimmer na Timeline", bgHex: "#2563eb", fgHex: "#ffffff");
        btnInsertSubclip.Click += (_, _) =>
        {
            double.TryParse(txtIn.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double inSec);
            double.TryParse(txtOut.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double outSec);
            double dur = Math.Max(0.25, outSec - inSec);
            var firstAsset = _mediaBin.FirstOrDefault() ?? new MediaAssetItem
            {
                Name = "Subclip Trimmer Master",
                Type = "video",
                GeneratorType = "smpteBars",
                Duration = dur
            };
            InsertMediaAssetIntoTimeline(new MediaAssetItem
            {
                Name = $"{firstAsset.Name} [Trimmer {inSec:0.0}s-{outSec:0.0}s]",
                Type = firstAsset.Type,
                GeneratorType = firstAsset.GeneratorType,
                Duration = dur,
                Color = firstAsset.Color,
                Peaks = firstAsset.Peaks
            });
        };

        btnRow.Children.Add(btnMarkIn);
        btnRow.Children.Add(btnMarkOut);
        btnRow.Children.Add(btnInsertSubclip);

        stack.Children.Add(new TextBlock { Text = "Ponto de Entrada (In Point - Segundos):", FontSize = 10, Foreground = new SolidColorBrush(Color.Parse("#94a3b8")) });
        stack.Children.Add(txtIn);
        stack.Children.Add(new TextBlock { Text = "Ponto de Saída (Out Point - Segundos):", FontSize = 10, Foreground = new SolidColorBrush(Color.Parse("#94a3b8")) });
        stack.Children.Add(txtOut);
        stack.Children.Add(btnRow);
        return stack;
    }

    private DetachableDockPanel BuildVideoPreviewDock()
    {
        var grid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,Auto")
        };

        // Compact Classic NLE preview controls: project setup, quality, FX, snapshots, and monitors.
        var previewBar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            Margin = new Thickness(4, 3)
        };

        previewBar.Children.Add(NeovegIcons.CreateButton(
            NeovegIcons.Gear,
            "Propriedades de Vídeo do Projeto (resolução, FPS e proporção)",
            OpenProjectVideoPropertiesDialog,
            size: 22));

        // Neoveg preview quality: one list (Draft, Preview, Good, Best, Auto); each quality implies its
        // preview resolution (Draft = 1/4, Preview = 1/2, Good/Best = full, Auto = fit to the preview window)
        _previewControl.PreviewResolution = PreviewResolutionForQuality(_previewControl.PreviewQuality);
        var qualitySelector = new ComboBox
        {
            ItemsSource = new[] { "Draft", "Preview", "Good", "Best", "Auto" },
            SelectedItem = _previewControl.PreviewQuality,
            Width = 78,
            Height = 22,
            FontSize = 10,
            Padding = new Thickness(4, 0),
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#1e293b")),
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0")),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569"))
        };
        ToolTip.SetTip(qualitySelector, "Qualidade da Prévia (Draft, Preview, Good, Best, Auto)");
        qualitySelector.SelectionChanged += (_, _) =>
        {
            if (qualitySelector.SelectedItem is string quality)
            {
                _previewControl.PreviewQuality = quality;
                _previewControl.PreviewResolution = PreviewResolutionForQuality(quality);
                UpdatePreviewStatus();
                _previewControl.InvalidateVisual();
            }
        };
        previewBar.Children.Add(qualitySelector);

        var btnFxBypass = NeovegIcons.CreateButton(
            NeovegIcons.Fx,
            "Ignorar / Ativar Efeitos de Vídeo (FX Bypass)",
            bgHex: _previewFxBypassed ? ToggleOnBg : ToggleOffBg,
            size: 22);
        btnFxBypass.Click += (_, _) =>
        {
            _previewFxBypassed = !_previewFxBypassed;
            _previewControl.VideoFxBypassed = _previewFxBypassed;
            btnFxBypass.Background = new SolidColorBrush(Color.Parse(_previewFxBypassed ? ToggleOnBg : ToggleOffBg));
            _previewControl.InvalidateVisual();
            SetStatus($"Efeitos de vídeo da prévia {(_previewFxBypassed ? "ignorados" : "ativados")}.");
        };
        previewBar.Children.Add(btnFxBypass);

        var btnSplitScreen = NeovegIcons.CreateButton(
            NeovegIcons.SplitScreen,
            "Visualização Split-Screen (FX Bypass)",
            bgHex: Project.SplitScreenBypass ? ToggleOnBg : ToggleOffBg,
            size: 22);
        btnSplitScreen.Click += (_, _) =>
        {
            Project.SplitScreenBypass = !Project.SplitScreenBypass;
            btnSplitScreen.Background = new SolidColorBrush(Color.Parse(Project.SplitScreenBypass ? ToggleOnBg : ToggleOffBg));
            _previewControl.InvalidateVisual();
        };
        previewBar.Children.Add(btnSplitScreen);

        var overlaysFlyout = new MenuFlyout();
        overlaysFlyout.Opening += (_, _) => overlaysFlyout.ItemsSource = BuildPreviewOverlayMenuItems();
        var btnOverlays = NeovegIcons.CreateButton(NeovegIcons.Overlays, "Sobreposições (Overlays): Grade, Áreas Seguras, Timecode, Pan/Crop...", size: 22);
        btnOverlays.Flyout = overlaysFlyout;
        previewBar.Children.Add(btnOverlays);

        previewBar.Children.Add(NeovegIcons.CreateButton(
            NeovegIcons.Snapshot,
            "Salvar Snapshot do Frame Atual (PNG/JPEG)",
            async () => await SavePreviewSnapshotAsync(),
            size: 22));
        previewBar.Children.Add(NeovegIcons.CreateButton(
            NeovegIcons.CopyImage,
            "Copiar Snapshot do Frame Atual para a Área de Transferência",
            async () => await CopyPreviewSnapshotAsync(),
            size: 22));

        _btnTlPopout = NeovegIcons.CreateButton(NeovegIcons.Popout, Localization.T("Toolbar.Tooltip.Popout", "Visualizar em Monitor Externo"), PopoutVideoPreviewToMonitor, size: 22);
        previewBar.Children.Add(_btnTlPopout);
        previewBar.Children.Add(NeovegIcons.CreateButton(
            NeovegIcons.PreviewBeside,
            "Abrir Prévia em Tela Cheia (pressione Esc para fechar)",
            OpenFullscreenVideoPreview,
            size: 22));

        var previewToolbarScroll = new ScrollViewer
        {
            HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
            Content = previewBar
        };
        Grid.SetRow(previewToolbarScroll, 0);
        grid.Children.Add(previewToolbarScroll);

        Grid.SetRow(_previewControl, 1);
        grid.Children.Add(_previewControl);
        _previewControl.SizeChanged += (_, _) => UpdatePreviewStatus();
        UpdatePreviewStatus();

        // Project and effective preview resolution / frame rate status.
        _previewInfoStrip = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#151a24")),
            Padding = new Thickness(8, 3),
            IsVisible = true,
            Child = _subFormatReadout
        };
        Grid.SetRow(_previewInfoStrip, 2);
        grid.Children.Add(_previewInfoStrip);

        return WrapInDockPanel("Video Preview", grid);
    }

    private void OpenProjectVideoPropertiesDialog()
    {
        var widthBox = new TextBox { Text = Project.Width.ToString(System.Globalization.CultureInfo.InvariantCulture), Width = 130 };
        var heightBox = new TextBox { Text = Project.Height.ToString(System.Globalization.CultureInfo.InvariantCulture), Width = 130 };
        var fpsBox = new TextBox { Text = Project.Fps.ToString("0.###", System.Globalization.CultureInfo.InvariantCulture), Width = 130 };
        var pixelAspectOptions = new List<string>
        {
            "1.0000 - Square pixels",
            "1.2121 - NTSC DV widescreen",
            "1.4568 - PAL DV widescreen"
        };
        int selectedAspectIndex = Project.PixelAspectRatio switch
        {
            >= 0.99 and <= 1.01 => 0,
            >= 1.19 and <= 1.23 => 1,
            >= 1.43 and <= 1.48 => 2,
            _ => -1
        };
        if (selectedAspectIndex < 0)
        {
            selectedAspectIndex = pixelAspectOptions.Count;
            pixelAspectOptions.Add($"{Project.PixelAspectRatio:0.####} - Current custom");
        }
        var pixelAspectSelector = new ComboBox
        {
            ItemsSource = pixelAspectOptions,
            Width = 250,
            SelectedIndex = selectedAspectIndex
        };
        var displayAspect = new TextBlock
        {
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
            FontSize = 11
        };
        void UpdateAspectPreview()
        {
            if (int.TryParse(widthBox.Text, out int width) &&
                int.TryParse(heightBox.Text, out int height) &&
                width > 0 &&
                height > 0 &&
                pixelAspectSelector.SelectedItem is string selectedPixelAspect &&
                double.TryParse(selectedPixelAspect.Split(' ')[0], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double pixelAspect))
            {
                displayAspect.Text = $"Proporção de exibição: {width * pixelAspect / height:0.####}:1";
            }
            else
            {
                displayAspect.Text = "Informe largura e altura válidas.";
            }
        }
        widthBox.TextChanged += (_, _) => UpdateAspectPreview();
        heightBox.TextChanged += (_, _) => UpdateAspectPreview();
        pixelAspectSelector.SelectionChanged += (_, _) => UpdateAspectPreview();
        UpdateAspectPreview();

        var form = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            RowDefinitions = new RowDefinitions("Auto,Auto,Auto,Auto,Auto"),
            Margin = new Thickness(18)
        };
        void AddPropertyRow(int row, string label, Control input)
        {
            var text = new TextBlock
            {
                Text = label,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, 12, 8),
                Foreground = new SolidColorBrush(Color.Parse("#cbd5e1"))
            };
            Grid.SetRow(text, row);
            form.Children.Add(text);
            Grid.SetRow(input, row);
            Grid.SetColumn(input, 1);
            input.Margin = new Thickness(0, 0, 0, 8);
            form.Children.Add(input);
        }

        AddPropertyRow(0, "Largura do projeto (pixels):", widthBox);
        AddPropertyRow(1, "Altura do projeto (pixels):", heightBox);
        AddPropertyRow(2, "Frame rate (FPS):", fpsBox);
        AddPropertyRow(3, "Proporção do pixel:", pixelAspectSelector);
        Grid.SetRow(displayAspect, 4);
        Grid.SetColumnSpan(displayAspect, 2);
        form.Children.Add(displayAspect);

        var dialog = new Window
        {
            Title = "Propriedades de Vídeo do Projeto",
            Width = 460,
            Height = 330,
            MinWidth = 420,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#12161f")),
            Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"))
        };
        var cancelButton = CreateSquareTextButton("Cancelar", "#1e293b", "#cbd5e1");
        cancelButton.Click += (_, _) => dialog.Close();
        var applyButton = CreateSquareTextButton("Aplicar", "#2563eb", "#ffffff");
        applyButton.Click += (_, _) =>
        {
            int width = 0;
            int height = 0;
            double fps = 0;
            double pixelAspect = 0;
            bool valid =
                int.TryParse(widthBox.Text, System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out width) &&
                int.TryParse(heightBox.Text, System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out height) &&
                double.TryParse(fpsBox.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out fps) &&
                pixelAspectSelector.SelectedItem is string selectedPixelAspect &&
                double.TryParse(selectedPixelAspect.Split(' ')[0], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out pixelAspect) &&
                width is >= 16 and <= 16384 &&
                height is >= 16 and <= 16384 &&
                fps is >= 1 and <= 240 &&
                pixelAspect > 0;
            if (!valid)
            {
                SetStatus("Propriedades de vídeo inválidas: use resolução entre 16 e 16384 px e FPS entre 1 e 240.");
                return;
            }

            Project.Width = width;
            Project.Height = height;
            Project.Fps = fps;
            Project.PixelAspectRatio = pixelAspect;
            UpdateTimecodeAndPreview();
            _previewControl.InvalidateVisual();
            SetStatus($"Propriedades de vídeo atualizadas: {width}x{height}, {fps:0.###} fps, pixel aspect {pixelAspect:0.####}.");
            dialog.Close();
        };
        var footer = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 8,
            Margin = new Thickness(18, 0, 18, 16),
            Children = { cancelButton, applyButton }
        };
        var content = new DockPanel();
        DockPanel.SetDock(footer, Dock.Bottom);
        content.Children.Add(footer);
        content.Children.Add(form);
        dialog.Content = content;
        dialog.Show(this);
    }

    private async Task SavePreviewSnapshotAsync()
    {
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Salvar Snapshot da Prévia",
            SuggestedFileName = $"Neoveg_Frame_{Project.Playhead:0.000}.png",
            DefaultExtension = "png",
            FileTypeChoices = new[]
            {
                new FilePickerFileType("PNG Image") { Patterns = new[] { "*.png" } },
                new FilePickerFileType("JPEG Image") { Patterns = new[] { "*.jpg", "*.jpeg" } }
            }
        });
        if (file == null)
            return;

        try
        {
            using var snapshot = RenderPreviewSnapshot();
            string extension = System.IO.Path.GetExtension(file.Name);
            if (extension.Equals(".jpg", StringComparison.OrdinalIgnoreCase) ||
                extension.Equals(".jpeg", StringComparison.OrdinalIgnoreCase))
            {
                string? localPath = file.TryGetLocalPath();
                if (string.IsNullOrEmpty(localPath))
                    throw new NotSupportedException("O provedor de arquivos atual não permite codificar snapshots JPEG.");
                snapshot.Save(localPath, 95);
            }
            else
            {
                await using var output = await file.OpenWriteAsync();
                snapshot.Save(output);
            }
            SetStatus($"Snapshot do frame salvo: {file.Name}");
        }
        catch (Exception ex)
        {
            Program.LogEvent("ERROR", "MainWindow.SavePreviewSnapshot", ex.Message, ex);
            SetStatus($"Falha ao salvar snapshot: {ex.Message}");
        }
    }

    private async Task CopyPreviewSnapshotAsync()
    {
        try
        {
            using var snapshot = RenderPreviewSnapshot();
            var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
            if (clipboard == null)
                throw new InvalidOperationException("A área de transferência não está disponível nesta janela.");

            using var pngData = new MemoryStream();
            snapshot.Save(pngData);
            var dataObject = new DataObject();
            dataObject.Set("image/png", pngData.ToArray());
            await clipboard.SetDataObjectAsync(dataObject);
            SetStatus("Snapshot do frame copiado para a área de transferência.");
        }
        catch (Exception ex)
        {
            Program.LogEvent("ERROR", "MainWindow.CopyPreviewSnapshot", ex.Message, ex);
            SetStatus($"Falha ao copiar snapshot: {ex.Message}");
        }
    }

    private RenderTargetBitmap RenderPreviewSnapshot()
    {
        var bounds = _previewControl.Bounds;
        if (bounds.Width < 1 || bounds.Height < 1)
            throw new InvalidOperationException("A prévia não possui tamanho válido para captura.");

        double scaling = TopLevel.GetTopLevel(_previewControl)?.RenderScaling ?? 1.0;
        var pixelSize = new PixelSize(
            Math.Max(1, (int)Math.Ceiling(bounds.Width * scaling)),
            Math.Max(1, (int)Math.Ceiling(bounds.Height * scaling)));
        var bitmap = new RenderTargetBitmap(pixelSize, new Vector(96 * scaling, 96 * scaling));
        bitmap.Render(_previewControl);
        return bitmap;
    }

    private void OpenFullscreenVideoPreview()
    {
        var fullscreenPreview = new VideoPreviewControl
        {
            Project = Project,
            SelectedClip = _previewControl.SelectedClip,
            RustInterop = RustInterop,
            CppInterop = CppInterop,
            VideoFxBypassed = _previewFxBypassed,
            PreviewQuality = _previewControl.PreviewQuality,
            PreviewResolution = _previewControl.PreviewResolution
        };
        var fullscreenWindow = new Window
        {
            Title = "Neoveg — Video Preview (Esc para fechar)",
            WindowState = WindowState.FullScreen,
            Background = new SolidColorBrush(Color.Parse("#080a0f")),
            Content = fullscreenPreview
        };
        fullscreenWindow.KeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape)
                fullscreenWindow.Close();
        };
        fullscreenWindow.Show();
    }

    private void UpdatePreviewStatus()
    {
        if (_previewControl == null || _subFormatReadout == null)
            return;

        int projectWidth = Math.Max(1, Project.Width);
        int projectHeight = Math.Max(1, Project.Height);
        double previewWidth;
        double previewHeight;
        switch (_previewControl.PreviewResolution)
        {
            case "Full":
                previewWidth = projectWidth;
                previewHeight = projectHeight;
                break;
            case "Half":
                previewWidth = projectWidth / 2.0;
                previewHeight = projectHeight / 2.0;
                break;
            case "Quarter":
                previewWidth = projectWidth / 4.0;
                previewHeight = projectHeight / 4.0;
                break;
            default:
                double scaling = TopLevel.GetTopLevel(_previewControl)?.RenderScaling ?? 1.0;
                double availableWidth = Math.Max(1.0, _previewControl.Bounds.Width * scaling);
                double availableHeight = Math.Max(1.0, _previewControl.Bounds.Height * scaling);
                double aspect = projectWidth * Math.Max(0.01, Project.PixelAspectRatio) / projectHeight;
                previewWidth = Math.Min(projectWidth, Math.Min(availableWidth, availableHeight * aspect));
                previewHeight = previewWidth / aspect;
                break;
        }

        int bitsPerPixel = Project.PixelFormat.Contains("Floating", StringComparison.OrdinalIgnoreCase) ? 128 : 32;
        _subFormatReadout.Text =
            $"Project: {projectWidth}x{projectHeight}x{bitsPerPixel}, {Project.Fps:0.000} fps  |  " +
            $"Preview: {(int)Math.Round(previewWidth)}x{(int)Math.Round(previewHeight)} " +
            $"({_previewControl.PreviewQuality})";
    }

    private static string PreviewResolutionForQuality(string quality) => quality switch
    {
        "Draft" => "Quarter",
        "Preview" => "Half",
        "Good" or "Best" => "Full",
        _ => "Auto"
    };

    /// <summary>
    /// Preview overlay toggles (all disabled on startup): shared by the Preview toolbar dropdown and View menu.
    /// </summary>
    private List<Control> BuildPreviewOverlayMenuItems()
    {
        MenuItem Toggle(string header, bool isChecked, Action toggle)
        {
            var item = new MenuItem
            {
                Header = header,
                ToggleType = MenuItemToggleType.CheckBox,
                IsChecked = isChecked
            };
            item.Click += (_, _) =>
            {
                toggle();
                _previewControl.InvalidateVisual();
            };
            return item;
        }

        return new List<Control>
        {
            Toggle("Grade de Terços", Project.ShowGrid, () => Project.ShowGrid = !Project.ShowGrid),
            Toggle("Áreas Seguras (Safe Areas)", Project.ShowSafeAreas, () => Project.ShowSafeAreas = !Project.ShowSafeAreas),
            Toggle("Timecode e Resolução (HUD)", _previewControl.ShowHudOverlay || Project.ShowPreviewHud, () =>
            {
                bool next = !(_previewControl.ShowHudOverlay || Project.ShowPreviewHud);
                _previewControl.ShowHudOverlay = next;
                Project.ShowPreviewHud = next;
            }),
            Toggle("Quadro Interativo Pan/Crop ('F')", _previewControl.ShowPanCropGizmo, () => _previewControl.ShowPanCropGizmo = !_previewControl.ShowPanCropGizmo),
            Toggle("Rótulos de Geradores e FX", _previewControl.ShowClipOverlayLabels, () => _previewControl.ShowClipOverlayLabels = !_previewControl.ShowClipOverlayLabels),
            Toggle("Split-Screen (FX Bypass)", Project.SplitScreenBypass, () => Project.SplitScreenBypass = !Project.SplitScreenBypass),
            new Separator(),
            Toggle("Barra de Informações do Vídeo", _previewInfoStrip?.IsVisible == true, () =>
            {
                if (_previewInfoStrip != null)
                    _previewInfoStrip.IsVisible = !_previewInfoStrip.IsVisible;
            })
        };
    }

    private DetachableDockPanel BuildMasterMixerDock()
    {
        _masterBusView.Project = Project;
        _masterBusView.RustInterop = RustInterop;
        _masterBusView.IsPlaying = _isPlaying;
        return WrapInDockPanel("Master Bus", _masterBusView);
    }

    private DetachableDockPanel BuildTimelineWorkspace()
    {
        // Split between Left Track Headers and Right Timeline Canvas (*)
        var splitGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("340,4,*")
        };

        var leftCol = new Grid
        {
            RowDefinitions = new RowDefinitions($"{MultiTrackTimelineControl.TotalHeaderHeight},*,24"),
            Background = new SolidColorBrush(Color.Parse("#151a24"))
        };

        // Neoveg time display above the track list (cursor position)
        var hdrTopBanner = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#0b0f17")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2e3846")),
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(8, 0),
            Child = _bigSmpteReadout
        };
        ToolTip.SetTip(hdrTopBanner, "Posição do Cursor (Timecode SMPTE)");
        Grid.SetRow(hdrTopBanner, 0);
        leftCol.Children.Add(hdrTopBanner);

        var trackScroll = new ScrollViewer
        {
            Content = _trackHeadersStack,
            HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Hidden
        };
        _trackHeadersScroll = trackScroll;
        trackScroll.PropertyChanged += (_, e) =>
        {
            if (e.Property.Name == nameof(ScrollViewer.Offset) && !_syncingTimelineVerticalScroll)
                SetTimelineVerticalOffset(trackScroll.Offset.Y);
        };
        trackScroll.SizeChanged += (_, _) => RefreshTimelineScrollBars();
        Grid.SetRow(trackScroll, 1);
        leftCol.Children.Add(trackScroll);
        var trackHeaderFooter = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#263042")),
            BorderThickness = new Thickness(0, 1, 0, 0)
        };
        Grid.SetRow(trackHeaderFooter, 2);
        leftCol.Children.Add(trackHeaderFooter);

        Grid.SetColumn(leftCol, 0);
        splitGrid.Children.Add(leftCol);

        splitGrid.Children.Add(CreateVerticalSplitter(1));

        // Dedicated 60 FPS Playhead Overlay Canvas stacked over MultiTrackTimelineControl
        var timelineCanvasOverlayHost = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,14"),
            RowDefinitions = new RowDefinitions($"{MultiTrackTimelineControl.TotalHeaderHeight},*,24")
        };
        var canvasHost = new Grid();
        Grid.SetRowSpan(canvasHost, 2);
        Grid.SetColumn(canvasHost, 0);
        timelineCanvasOverlayHost.Children.Add(canvasHost);
        canvasHost.Children.Add(_timelineControl);
        canvasHost.Children.Add(_playheadOverlay);

        var verticalScrollBar = new Avalonia.Controls.Primitives.ScrollBar
        {
            Orientation = Orientation.Vertical,
            Minimum = 0,
            Maximum = 0,
            ViewportSize = 0,
            SmallChange = 24,
            LargeChange = 100,
            Width = 14,
            Background = new SolidColorBrush(Color.Parse("#111620")),
            Foreground = new SolidColorBrush(Color.Parse("#64748b")),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch
        };
        _timelineVerticalScrollBar = verticalScrollBar;
        verticalScrollBar.ValueChanged += (_, e) => SetTimelineVerticalOffset(e.NewValue);
        var verticalScrollBarHost = new Border
        {
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#263042")),
            BorderThickness = new Thickness(1, 0, 0, 0),
            Child = verticalScrollBar
        };
        Grid.SetRow(verticalScrollBarHost, 1);
        Grid.SetColumn(verticalScrollBarHost, 1);
        timelineCanvasOverlayHost.Children.Add(verticalScrollBarHost);

        var horizontalControls = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("24,*,24"),
            Background = new SolidColorBrush(Color.Parse("#111620"))
        };
        var zoomOutButton = CreateTimelineScrollButton("-", "Diminuir zoom da linha do tempo", () => ZoomTimeline(1.0 / 1.25));
        Grid.SetColumn(zoomOutButton, 0);
        horizontalControls.Children.Add(zoomOutButton);

        var horizontalScrollBar = new Avalonia.Controls.Primitives.ScrollBar
        {
            Orientation = Orientation.Horizontal,
            Minimum = 0,
            Maximum = 0,
            ViewportSize = 0,
            SmallChange = 1,
            LargeChange = 10,
            Height = 16,
            Background = new SolidColorBrush(Color.Parse("#111620")),
            Foreground = new SolidColorBrush(Color.Parse("#64748b")),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Center
        };
        _timelineHorizontalScrollBar = horizontalScrollBar;
        horizontalScrollBar.ValueChanged += (_, e) => SetTimelineHorizontalOffset(e.NewValue);
        var horizontalScrollBarHost = new Border
        {
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#263042")),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = horizontalScrollBar
        };
        Grid.SetColumn(horizontalScrollBarHost, 1);
        horizontalControls.Children.Add(horizontalScrollBarHost);

        var zoomInButton = CreateTimelineScrollButton("+", "Aumentar zoom da linha do tempo", () => ZoomTimeline(1.25));
        Grid.SetColumn(zoomInButton, 2);
        horizontalControls.Children.Add(zoomInButton);
        Grid.SetRow(horizontalControls, 2);
        timelineCanvasOverlayHost.Children.Add(horizontalControls);

        timelineCanvasOverlayHost.SizeChanged += (_, _) => RefreshTimelineScrollBars();
        Grid.SetColumn(timelineCanvasOverlayHost, 2);
        splitGrid.Children.Add(timelineCanvasOverlayHost);

        RefreshTimelineScrollBars();
        return WrapInDockPanel("Timeline", splitGrid);
    }

    private Button CreateTimelineScrollButton(string text, string tooltip, Action onClick)
    {
        var button = new Button
        {
            Content = text,
            Width = 23,
            Height = 21,
            Padding = new Thickness(0),
            FontSize = 13,
            FontWeight = FontWeight.Bold,
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#1b2431")),
            Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
            BorderBrush = new SolidColorBrush(Color.Parse("#263042")),
            BorderThickness = new Thickness(1),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch
        };
        ToolTip.SetTip(button, tooltip);
        button.Click += (_, _) => onClick();
        return button;
    }

    private void RefreshTimelineScrollBars()
    {
        if (_timelineHorizontalScrollBar == null ||
            _timelineVerticalScrollBar == null ||
            _trackHeadersScroll == null)
            return;

        double duration = GetProjectDuration();
        double visibleSeconds = _timelineControl.Bounds.Width / Math.Max(20.0, _timelineControl.PixelsPerSecond);
        double maxHorizontalOffset = Math.Max(0.0, duration - visibleSeconds);
        _timelineHorizontalScrollBar.ViewportSize = Math.Min(duration, visibleSeconds);
        _timelineHorizontalScrollBar.Maximum = maxHorizontalOffset;
        SetTimelineHorizontalOffset(_timelineControl.HorizontalOffsetSeconds);

        double totalTrackHeight = Project.Tracks.Sum(track => Math.Clamp(track.Height, 46.0, 220.0));
        double visibleTrackHeight = _trackHeadersScroll.Viewport.Height;
        _timelineVerticalScrollBar.ViewportSize = Math.Min(totalTrackHeight, visibleTrackHeight);
        _timelineVerticalScrollBar.Maximum = Math.Max(0.0, totalTrackHeight - visibleTrackHeight);
        SetTimelineVerticalOffset(_timelineControl.VerticalOffset);
    }

    private void SetTimelineHorizontalOffset(double offsetSeconds)
    {
        if (_timelineHorizontalScrollBar == null)
            return;

        double offset = Math.Clamp(offsetSeconds, 0.0, _timelineHorizontalScrollBar.Maximum);
        _timelineHorizontalScrollBar.Value = offset;
        _timelineControl.HorizontalOffsetSeconds = offset;
        _playheadOverlay.HorizontalOffsetSeconds = offset;
        _timelineControl.InvalidateVisual();
        _playheadOverlay.InvalidateVisual();
    }

    private void SetTimelineVerticalOffset(double offset)
    {
        if (_timelineVerticalScrollBar == null || _trackHeadersScroll == null || _syncingTimelineVerticalScroll)
            return;

        _syncingTimelineVerticalScroll = true;
        try
        {
            double value = Math.Clamp(offset, 0.0, _timelineVerticalScrollBar.Maximum);
            _timelineVerticalScrollBar.Value = value;
            _trackHeadersScroll.Offset = new Vector(_trackHeadersScroll.Offset.X, value);
            _timelineControl.VerticalOffset = value;
            _timelineControl.InvalidateVisual();
        }
        finally
        {
            _syncingTimelineVerticalScroll = false;
        }
    }

    /// <summary>
    /// Track Header Indicator: only the selected track's dot blinks (white ⇄ black, 500ms).
    /// Unselected tracks keep a static neutral dot; with no selected track the blink timer is stopped.
    /// </summary>
    private void UpdateTrackSelectionIndicators()
    {
        string? selectedId = _timelineControl.SelectedTrackId;
        bool hasSelection = selectedId != null && _trackBlinkDots.ContainsKey(selectedId);

        foreach (var (trackId, dot) in _trackBlinkDots)
        {
            dot.Fill = hasSelection && trackId == selectedId ? _trackBlinkBrush : TrackIndicatorIdleBrush;
        }

        if (hasSelection)
        {
            if (!_trackBlinkTimer.IsEnabled)
            {
                _trackBlinkBrush.Color = Colors.White;
                _trackBlinkTimer.Start();
            }
        }
        else
        {
            _trackBlinkTimer.Stop();
            _trackBlinkBrush.Color = Colors.White;
        }
    }

    public void RebuildTrackHeadersUI()
    {
        _trackHeadersStack.Children.Clear();
        _trackBlinkDots.Clear();

        string? prevAccentHex = null;
        for (int idx = 0; idx < Project.Tracks.Count; idx++)
        {
            var track = Project.Tracks[idx];
            int capturedIdx = idx;

            if (string.IsNullOrWhiteSpace(track.TrackAccentColor))
            {
                track.TrackAccentColor = TrackColorPaletteService.Instance.GetNextTrackColor(idx, prevAccentHex);
            }
            string accentHex = track.TrackAccentColor!;
            prevAccentHex = accentHex;
            var accentCol = TrackColorPaletteService.ParseAccentColor(accentHex);

            double th = Math.Clamp(track.Height, 46.0, 220.0);
            bool hasGroup = !string.IsNullOrEmpty(track.GroupId);
            var grpCol = Color.Parse(track.GroupColor ?? "#f59e0b");

            var rowBorder = new Border
            {
                Height = th,
                Background = new SolidColorBrush(Color.Parse(track.Type == "video" ? CurrentTheme.TrackVideoBg : CurrentTheme.TrackAudioBg)),
                BorderBrush = new SolidColorBrush(Color.Parse(CurrentTheme.DockBorder)),
                BorderThickness = new Thickness(0, 0, 0, 1),
                Padding = new Thickness(0)
            };

            // Outer Grid: [4px Left Accent Bar | Optional Group Indicator | Main Track Header Content]
            var outerHeaderGrid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions(hasGroup ? "4,3,*" : "4,0,*")
            };

            // 4px Left Accent Bar painted with TrackAccentColor
            var accentBar = new Border
            {
                Name = "PART_TrackAccentBar",
                Width = 4,
                HorizontalAlignment = HorizontalAlignment.Left,
                VerticalAlignment = VerticalAlignment.Stretch,
                Background = new SolidColorBrush(accentCol)
            };
            ToolTip.SetTip(accentBar, $"Cor de Destaque da Faixa ({accentHex}) — Clique com o botão direito para alterar (Cor da Faixa / Track Color)");
            Grid.SetColumn(accentBar, 0);
            outerHeaderGrid.Children.Add(accentBar);

            if (hasGroup)
            {
                var grpBar = new Border
                {
                    Width = 3,
                    VerticalAlignment = VerticalAlignment.Stretch,
                    Background = new SolidColorBrush(grpCol)
                };
                Grid.SetColumn(grpBar, 1);
                outerHeaderGrid.Children.Add(grpBar);
            }

            var stack = new StackPanel
            {
                Spacing = 4,
                Margin = new Thickness(6, 4)
            };

            // Top Row: Index Badge + Inline Rename TextBox + [🎨] + [✥ Motion] + [FX] + [✎] + [G] + [-] + [+] + [M] + [S]
            var topGrid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("32,*,Auto,Auto,Auto,Auto,Auto,Auto,Auto,Auto,Auto")
            };

            // Track Number Box: indicator dot (blinks white ⇄ black only on the selected track) + track number
            var blinkDot = new Avalonia.Controls.Shapes.Ellipse
            {
                Width = 5,
                Height = 5,
                Fill = TrackIndicatorIdleBrush,
                VerticalAlignment = VerticalAlignment.Center
            };
            _trackBlinkDots[track.Id] = blinkDot;

            var numBadge = new Border
            {
                Background = new SolidColorBrush(accentCol),
                Width = 28,
                Height = 20,
                CornerRadius = new CornerRadius(2),
                HorizontalAlignment = HorizontalAlignment.Left,
                Child = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 3,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                    Children =
                    {
                        blinkDot,
                        new TextBlock
                        {
                            Text = (idx + 1).ToString(),
                            FontSize = 10,
                            FontWeight = FontWeight.Bold,
                            Foreground = Brushes.White,
                            VerticalAlignment = VerticalAlignment.Center
                        }
                    }
                }
            };
            Grid.SetColumn(numBadge, 0);
            topGrid.Children.Add(numBadge);

            var txtName = new TextBox
            {
                Text = track.Name,
                FontSize = 11,
                Height = 22,
                MinHeight = 22,
                Padding = new Thickness(4, 2),
                Background = new SolidColorBrush(Color.Parse("#0f141d")),
                Foreground = new SolidColorBrush(Color.Parse("#f8fafc")),
                BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
                CornerRadius = new CornerRadius(0)
            };
            txtName.LostFocus += (_, _) =>
            {
                if (!string.IsNullOrWhiteSpace(txtName.Text))
                {
                    track.Name = txtName.Text.Trim();
                    SetStatus($"Faixa renomeada para: \"{track.Name}\"");
                }
            };
            txtName.KeyDown += (_, ke) =>
            {
                if (ke.Key == Key.Enter && !string.IsNullOrWhiteSpace(txtName.Text))
                {
                    track.Name = txtName.Text.Trim();
                    SetStatus($"Faixa renomeada para: \"{track.Name}\"");
                }
            };
            Grid.SetColumn(txtName, 1);
            topGrid.Children.Add(txtName);

            var btnColor = CreateMiniTrackButton(NeovegIcons.TrackColor, $"Cor da Faixa / Track Color ({accentHex})", "#1e293b", accentHex);
            btnColor.Click += (_, _) => OpenTrackColorPickerDialog(track, capturedIdx);
            Grid.SetColumn(btnColor, 2);
            topGrid.Children.Add(btnColor);

            var btnTrackMotion = CreateMiniTrackButton(NeovegIcons.TrackMotion, "Track Motion (Movimento 2D/3D da Faixa)", "#1e293b", "#38bdf8");
            btnTrackMotion.Click += (_, _) => OpenTrackMotionDialog(track);
            Grid.SetColumn(btnTrackMotion, 3);
            topGrid.Children.Add(btnTrackMotion);

            var btnTrackFx = CreateMiniTrackButton(NeovegIcons.Fx, $"Track FX ({track.Fx.Count} ativo(s) — Media FX -> Take FX -> Event FX -> Track FX -> Master FX)", track.Fx.Count > 0 ? "#065f46" : "#1e293b", "#4ade80");
            btnTrackFx.Click += (_, _) =>
            {
                var firstClip = track.Clips.FirstOrDefault() ?? new TimelineClip { Name = track.Name };
                OpenClipFxChainDialog(firstClip, track);
            };
            Grid.SetColumn(btnTrackFx, 4);
            topGrid.Children.Add(btnTrackFx);

            var btnRename = CreateMiniTrackButton(NeovegIcons.Rename, "Renomear esta faixa de áudio/vídeo", "#1e293b", "#38bdf8");
            btnRename.Click += (_, _) =>
            {
                txtName.Focus();
                txtName.SelectAll();
                SetStatus($"Editando nome da faixa \"{track.Name}\" — digite e pressione Enter.");
            };
            Grid.SetColumn(btnRename, 5);
            topGrid.Children.Add(btnRename);

            var btnGroup = CreateMiniTrackButton(NeovegIcons.Group, hasGroup ? $"Agrupada ({track.GroupName}) — Clique para desagrupar" : "Agrupar faixa V+A", hasGroup ? "#78350f" : "#1e293b", hasGroup ? "#facc15" : "#cbd5e1");
            btnGroup.Click += (_, _) => ToggleSingleTrackGroup(track);
            Grid.SetColumn(btnGroup, 6);
            topGrid.Children.Add(btnGroup);

            var btnDecH = CreateMiniTrackButton(NeovegIcons.TrackHeightShrink, "Diminuir altura desta faixa", "#1e293b", "#cbd5e1");
            btnDecH.Click += (_, _) =>
            {
                track.Height = Math.Clamp(track.Height - 14.0, 46.0, 220.0);
                RebuildTrackHeadersUI();
                _timelineControl.InvalidateVisual();
            };
            Grid.SetColumn(btnDecH, 7);
            topGrid.Children.Add(btnDecH);

            var btnIncH = CreateMiniTrackButton(NeovegIcons.TrackHeightGrow, "Aumentar altura desta faixa", "#1e293b", "#cbd5e1");
            btnIncH.Click += (_, _) =>
            {
                track.Height = Math.Clamp(track.Height + 14.0, 46.0, 220.0);
                RebuildTrackHeadersUI();
                _timelineControl.InvalidateVisual();
            };
            Grid.SetColumn(btnIncH, 8);
            topGrid.Children.Add(btnIncH);

            var btnMute = CreateMiniTrackButton(NeovegIcons.Mute, "Mute Track (Sincronizado no Grupo V+A)", track.Muted ? "#dc2626" : "#1e293b", "#ffffff");
            btnMute.Click += (_, _) =>
            {
                bool nextMute = !track.Muted;
                track.Muted = nextMute;
                if (!string.IsNullOrEmpty(track.GroupId))
                {
                    foreach (var member in Project.Tracks.Where(t => t.GroupId == track.GroupId))
                        member.Muted = nextMute;
                }
                RebuildTrackHeadersUI();
                UpdateTimecodeAndPreview();
            };
            Grid.SetColumn(btnMute, 9);
            topGrid.Children.Add(btnMute);

            var btnSolo = CreateMiniTrackButton(NeovegIcons.Solo, "Solo Track (Sincronizado no Grupo V+A)", track.Solo ? "#d97706" : "#1e293b", "#ffffff");
            btnSolo.Click += (_, _) =>
            {
                bool nextSolo = !track.Solo;
                track.Solo = nextSolo;
                if (!string.IsNullOrEmpty(track.GroupId))
                {
                    foreach (var member in Project.Tracks.Where(t => t.GroupId == track.GroupId))
                        member.Solo = nextSolo;
                }
                RebuildTrackHeadersUI();
                UpdateTimecodeAndPreview();
            };
            Grid.SetColumn(btnSolo, 10);
            topGrid.Children.Add(btnSolo);

            stack.Children.Add(topGrid);

            // Bottom Row: Group Pill (if grouped) + Interactive Opacity/Volume Slider & Pan Slider
            var botStack = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 6,
                VerticalAlignment = VerticalAlignment.Center
            };

            if (hasGroup)
            {
                botStack.Children.Add(new Border
                {
                    BorderBrush = new SolidColorBrush(grpCol),
                    BorderThickness = new Thickness(1),
                    Background = new SolidColorBrush(Color.FromArgb(40, grpCol.R, grpCol.G, grpCol.B)),
                    Padding = new Thickness(4, 1),
                    Child = new TextBlock
                    {
                        Text = track.GroupName ?? "Grupo V+A",
                        FontSize = 9,
                        FontWeight = FontWeight.Bold,
                        Foreground = new SolidColorBrush(grpCol)
                    }
                });
            }

            var lblLevel = new TextBlock
            {
                Text = track.Type == "video" ? $"Opac: {(int)(track.Level * 100)}%" : $"Vol: {(int)(track.Level * 100)}%",
                FontSize = 9.5,
                Width = 58,
                Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
                VerticalAlignment = VerticalAlignment.Center
            };
            var sldLevel = NeovegPrecisionSlider.Create(
                minimum: 0.0,
                maximum: 1.0,
                initialValue: Math.Clamp(track.Level, 0.0, 1.0),
                defaultValue: 1.0,
                parameterLabel: track.Type == "video" ? "Opacidade da Faixa (%)" : "Volume da Faixa (%)",
                width: 74,
                displayAsPercentage: true);
            sldLevel.PropertyChanged += (_, pe) =>
            {
                if (pe.Property.Name == nameof(Slider.Value))
                {
                    track.Level = sldLevel.Value;
                    lblLevel.Text = track.Type == "video" ? $"Opac: {(int)(track.Level * 100)}%" : $"Vol: {(int)(track.Level * 100)}%";
                    _timelineControl.InvalidateVisual();
                    _previewControl.InvalidateVisual();
                    _masterBusView.RefreshTelemetry();
                }
            };

            botStack.Children.Add(lblLevel);
            botStack.Children.Add(sldLevel);

            if (track.Type == "audio")
            {
                var lblPan = new TextBlock
                {
                    Text = $"Pan: {track.Pan:+0.0;-0.0;0.0}",
                    FontSize = 9.5,
                    Width = 52,
                    Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
                    VerticalAlignment = VerticalAlignment.Center
                };
                var sldPan = NeovegPrecisionSlider.Create(
                    minimum: -1.0,
                    maximum: 1.0,
                    initialValue: Math.Clamp(track.Pan, -1.0, 1.0),
                    defaultValue: 0.0,
                    parameterLabel: "Balanço Estéreo (Pan)",
                    width: 60,
                    displayAsPercentage: false);
                sldPan.PropertyChanged += (_, pe) =>
                {
                    if (pe.Property.Name == nameof(Slider.Value))
                    {
                        track.Pan = sldPan.Value;
                        lblPan.Text = $"Pan: {track.Pan:+0.0;-0.0;0.0}";
                        _masterBusView.RefreshTelemetry();
                    }
                };
                botStack.Children.Add(lblPan);
                botStack.Children.Add(sldPan);
            }

            stack.Children.Add(botStack);
            Grid.SetColumn(stack, 2);
            outerHeaderGrid.Children.Add(stack);

            rowBorder.Child = outerHeaderGrid;
            rowBorder.ContextMenu = BuildTrackHeaderContextMenu(track, capturedIdx);
            // Clicking anywhere on the header (including its buttons/name box) selects the track
            rowBorder.AddHandler(PointerPressedEvent, (_, _) =>
            {
                if (_timelineControl.SelectedTrackId == track.Id) return;
                _timelineControl.SelectedTrackId = track.Id;
                UpdateTrackSelectionIndicators();
                _timelineControl.InvalidateVisual();
            }, Avalonia.Interactivity.RoutingStrategies.Tunnel, handledEventsToo: true);
            _trackHeadersStack.Children.Add(rowBorder);
        }

        UpdateTrackSelectionIndicators();
        RefreshTimelineScrollBars();
    }

    private ContextMenu BuildTrackHeaderContextMenu(TimelineTrack track, int trackIndex)
    {
        var menu = new ContextMenu();
        var rootItems = new List<object>();

        var colorSubMenu = new MenuItem
        {
            Header = "🎨 Cor da Faixa / Track Color"
        };

        var subItems = new List<object>();
        foreach (var entry in TrackColorPaletteService.CuratedPalette)
        {
            var capturedHex = entry.HexCode;
            var swatchItem = new MenuItem
            {
                Header = $"{entry.Name} — {entry.HexCode}",
                Icon = new Border
                {
                    Width = 12,
                    Height = 12,
                    CornerRadius = new CornerRadius(2),
                    Background = new SolidColorBrush(TrackColorPaletteService.ParseAccentColor(entry.HexCode))
                }
            };
            swatchItem.Click += (_, _) => ApplyCustomTrackAccentColor(track, capturedHex);
            subItems.Add(swatchItem);
        }

        subItems.Add(new Separator());

        var miCustomPicker = new MenuItem
        {
            Header = "Escolher Cor Personalizada (Color Picker)..."
        };
        miCustomPicker.Click += (_, _) => OpenTrackColorPickerDialog(track, trackIndex);
        subItems.Add(miCustomPicker);

        var miToggleClipTint = new MenuItem
        {
            Header = track.PropagateTintToClips
                ? "✓ Propagar Tom Suave aos Clipes (Clip Tint Ativo)"
                : "Propagar Tom Suave aos Clipes (Clip Tint Desativado)"
        };
        miToggleClipTint.Click += (_, _) =>
        {
            track.PropagateTintToClips = !track.PropagateTintToClips;
            RebuildTrackHeadersUI();
            _timelineControl.InvalidateVisual();
            SetStatus($"Propagação de cor para clipes da faixa \"{track.Name}\": {(track.PropagateTintToClips ? "Ativada" : "Desativada")}.");
        };
        subItems.Add(miToggleClipTint);

        colorSubMenu.ItemsSource = subItems;
        rootItems.Add(colorSubMenu);

        var miCycleColor = new MenuItem
        {
            Header = "Próxima Cor Automática da Paleta"
        };
        miCycleColor.Click += (_, _) =>
        {
            string nextHex = TrackColorPaletteService.Instance.GetNextTrackColor(trackIndex + 1, track.TrackAccentColor ?? track.Color);
            ApplyCustomTrackAccentColor(track, nextHex);
        };
        rootItems.Add(miCycleColor);

        menu.ItemsSource = rootItems;
        return menu;
    }

    private void ApplyCustomTrackAccentColor(TimelineTrack track, string hexColor)
    {
        if (string.IsNullOrWhiteSpace(hexColor)) return;
        string normalized = hexColor.Trim();
        if (!normalized.StartsWith('#'))
            normalized = "#" + normalized;

        track.TrackAccentColor = normalized;
        track.Color = normalized;
        foreach (var clip in track.Clips)
        {
            clip.TrackAccentColor = normalized;
        }

        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        SetStatus($"Cor de destaque da faixa \"{track.Name}\" alterada para {normalized}.");
    }

    public void OpenTrackColorPickerDialog(TimelineTrack track, int trackIndex)
    {
        var win = new Window
        {
            Title = $"Seletor de Cor da Faixa (Track Color Picker) — {track.Name}",
            Width = 440,
            Height = 360,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White
        };

        var root = new StackPanel { Spacing = 12, Margin = new Thickness(16) };
        root.Children.Add(new TextBlock
        {
            Text = $"COR DE DESTAQUE DA FAIXA — {track.Name}",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        root.Children.Add(new TextBlock
        {
            Text = "Selecione uma cor da paleta curada para o tema escuro ou informe um código HEX personalizado (#RRGGBB):",
            FontSize = 10.5,
            TextWrapping = TextWrapping.Wrap,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        });

        var txtHex = new TextBox
        {
            Text = track.TrackAccentColor ?? track.Color,
            Watermark = "#3498DB",
            FontSize = 12,
            Background = new SolidColorBrush(Color.Parse("#0f141d")),
            Foreground = Brushes.White
        };

        var swatchWrap = new WrapPanel
        {
            Orientation = Orientation.Horizontal
        };

        foreach (var entry in TrackColorPaletteService.CuratedPalette)
        {
            var capturedHex = entry.HexCode;
            var swatchBtn = new Button
            {
                Content = $"{entry.Name} ({entry.HexCode})",
                Background = new SolidColorBrush(TrackColorPaletteService.ParseAccentColor(entry.HexCode)),
                Foreground = Brushes.White,
                FontSize = 10.5,
                FontWeight = FontWeight.Bold,
                Padding = new Thickness(10, 6),
                Margin = new Thickness(0, 0, 6, 6)
            };
            swatchBtn.Click += (_, _) =>
            {
                txtHex.Text = capturedHex;
                ApplyCustomTrackAccentColor(track, capturedHex);
            };
            swatchWrap.Children.Add(swatchBtn);
        }

        root.Children.Add(swatchWrap);
        root.Children.Add(txtHex);

        var chkPropagate = new CheckBox
        {
            Content = "Propagar tom suave aos clipes/eventos desta faixa (Clip Header / Border Tint)",
            IsChecked = track.PropagateTintToClips,
            Foreground = Brushes.White,
            FontSize = 11
        };
        root.Children.Add(chkPropagate);

        var btnRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right
        };

        var btnApply = CreateSquareTextButton("Aplicar Cor", "#2563eb", "#ffffff");
        btnApply.Click += (_, _) =>
        {
            track.PropagateTintToClips = chkPropagate.IsChecked ?? true;
            ApplyCustomTrackAccentColor(track, txtHex.Text ?? "#3498DB");
            win.Close();
        };
        btnRow.Children.Add(btnApply);

        root.Children.Add(btnRow);
        win.Content = root;
        win.Show(this);
    }

    /// <summary>
    /// Opens the Inspector (active event info + FX chain) as a tool window from the Tools menu.
    /// Event Pan/Crop is no longer embedded beside the preview: it opens only from the video event header button.
    /// </summary>
    public void OpenInspectorWindow()
    {
        if (_inspectorWindow != null)
        {
            _inspectorWindow.Activate();
            return;
        }

        RefreshInspectorUI();
        var win = new Window
        {
            Title = Localization.T("Tab.Inspector", "Inspector"),
            Width = 380,
            Height = 520,
            MinWidth = 280,
            MinHeight = 260,
            ShowInTaskbar = false,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse(CurrentTheme.DockPanelBg)),
            Foreground = new SolidColorBrush(Color.Parse(CurrentTheme.TextPrimary)),
            Content = _inspectorBodyHost
        };
        win.Closing += (_, _) =>
        {
            win.Content = null;
            _inspectorWindow = null;
        };
        _inspectorWindow = win;
        win.Show(this);
    }

    private void RefreshInspectorUI()
    {
        var loc = Localization;
        var clip = _previewControl.SelectedClip ?? Project.Tracks.SelectMany(t => t.Clips).FirstOrDefault();
        var stack = new StackPanel { Spacing = 8, Margin = new Thickness(8) };

        if (clip == null)
        {
            stack.Children.Add(new TextBlock
            {
                Text = loc.T("Inspector.NoSelection", "Nenhum evento selecionado na timeline."),
                FontSize = 11,
                Foreground = new SolidColorBrush(Color.Parse("#94a3b8")),
                TextWrapping = TextWrapping.Wrap
            });
            _inspectorBodyHost.Content = stack;
            return;
        }

        stack.Children.Add(new TextBlock
        {
            Text = $"{loc.T("Inspector.ActiveEvent", "EVENTO ATIVO:")} {clip.Name}",
            FontWeight = FontWeight.Bold,
            FontSize = 11.5,
            TextWrapping = TextWrapping.Wrap,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        stack.Children.Add(new TextBlock
        {
            Text = $"{clip.Start:0.000}s – {clip.End:0.000}s • {clip.Duration:0.000}s • Fade In {clip.FadeIn:0.00}s / Out {clip.FadeOut:0.00}s",
            FontSize = 10.5,
            TextWrapping = TextWrapping.Wrap,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        });

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        bool isVideoClip = clip.MediaType != "audio";
        if (isVideoClip)
        {
            actions.Children.Add(NeovegIcons.CreateButton(NeovegIcons.PanCrop, loc.T("Inspector.OpenFullPanCrop", "Event Pan/Crop..."), () => OpenEventPanCropWindow(clip)));
            actions.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Restore, loc.T("Inspector.ResetCrop", "Restaurar Quadro 'F' Padrão"), () =>
            {
                clip.PanCrop.Width = clip.PanCrop.SourceWidth;
                clip.PanCrop.Height = clip.PanCrop.SourceHeight;
                clip.PanCrop.XCenter = clip.PanCrop.SourceWidth * 0.5;
                clip.PanCrop.YCenter = clip.PanCrop.SourceHeight * 0.5;
                clip.PanCrop.Rotation = 0.0;
                clip.PanCrop.SyncNormalizedFromPixels();
                _previewControl.InvalidateVisual();
                RefreshInspectorUI();
            }));
        }
        var ownerTrack = Project.Tracks.FirstOrDefault(t => t.Clips.Contains(clip));
        if (ownerTrack != null)
        {
            actions.Children.Add(NeovegIcons.CreateButton(NeovegIcons.Fx, ownerTrack.Type == "audio" ? "Audio Event FX..." : "Video Event FX...", () => OpenClipFxChainDialog(clip, ownerTrack)));
        }
        stack.Children.Add(actions);

        stack.Children.Add(new TextBlock
        {
            Text = $"{loc.T("Inspector.ActiveFxCount", "Efeitos Ativos no Clipe")} ({clip.Fx.Count}):",
            FontWeight = FontWeight.Bold,
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#f59e0b")),
            Margin = new Thickness(0, 6, 0, 0)
        });

        foreach (var fx in clip.Fx)
        {
            stack.Children.Add(new Border
            {
                Background = new SolidColorBrush(Color.Parse("#18202c")),
                BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(6, 4),
                Child = new TextBlock
                {
                    Text = $"✓ {fx.Name}",
                    FontSize = 10.5,
                    Foreground = new SolidColorBrush(Color.Parse("#4ade80"))
                }
            });
        }

        _inspectorBodyHost.Content = new ScrollViewer { Content = stack };
    }

    public void LoadProjectState(NeovegProject newProject, string statusMsg)
    {
        TogglePlayback(false);

        // Automatic Missing Media Detection on Project Open (Neoveg .neoveg MediaPool Check)
        var missingEntries = Relinker.DetectMissingMediaInProject(newProject);
        if (missingEntries.Count > 0)
        {
            // Interrupt visual timeline mounting while the user resolves or ignores missing media
            SetStatus($"Atenção: {missingEntries.Count} arquivo(s) ausente(s) detectado(s) em \"{newProject.Name}\". Montagem da timeline interrompida — aguardando Missing Media Dialog...");

            var missingDialog = new MissingMediaWindow(
                _rootDir,
                newProject,
                missingEntries,
                Relinker,
                resolutionMsg =>
                {
                    MountProjectToTimelineAndWorkspace(newProject, $"{statusMsg} • {resolutionMsg}");
                });
            missingDialog.Show(this);
            return;
        }

        MountProjectToTimelineAndWorkspace(newProject, statusMsg);
    }

    private void MountProjectToTimelineAndWorkspace(NeovegProject newProject, string statusMsg)
    {
        Project = newProject;
        PrerenderService.InvalidateAllCache();
        _previewControl.Project = Project;
        _previewControl.SelectedClip = Project.Tracks.SelectMany(t => t.Clips).FirstOrDefault();
        _scopeControl.Project = Project;
        _masterBusView.Project = Project;
        _timelineControl.Project = Project;
        _timelineControl.SelectedClipId = _previewControl.SelectedClip?.Id;
        _timelineControl.RecalculateCrossfades();
        RebuildProjectMediaBinUI();
        RebuildTrackHeadersUI();
        RefreshInspectorUI();
        UpdateToolbarToggleStates();
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
        Title = $"{Project.Name} - Neoveg";
        SetStatus(statusMsg);
    }

    public void OpenMissingMediaDialogForCurrentProject(bool forceDemoIfNoneMissing = false)
    {
        var missing = Relinker.DetectMissingMediaInProject(Project);
        if (missing.Count == 0 && forceDemoIfNoneMissing)
        {
            // Load the Moved Media Relink Demo project so the user can immediately test Search & Relink
            LoadProjectState(
                NeovegProject.CreateMovedMediaRelinkDemo(),
                "Projeto de Teste 'Moved Media Relink Demo' carregado.");
            return;
        }

        var dialog = new MissingMediaWindow(
            _rootDir,
            Project,
            missing,
            Relinker,
            resolutionMsg =>
            {
                MountProjectToTimelineAndWorkspace(Project, resolutionMsg);
            });
        dialog.Show(this);
    }

    private void RebuildProjectMediaBinUI()
    {
        if (_projectMediaStack == null) return;
        _projectMediaStack.Children.Clear();

        // Project Media toolbar (icon-only): Import Media • Search & Relink Missing Media
        _projectMediaStack.Children.Add(new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            Children =
            {
                NeovegIcons.CreateButton(NeovegIcons.ImportMedia, Localization.T("Toolbar.Tooltip.Import", "Importar Mídia"), async () => await OpenImportMediaDialogAsync(), size: 22),
                NeovegIcons.CreateButton(NeovegIcons.Search, "Localizar / Relinkar Mídias Ausentes...", () => OpenMissingMediaDialogForCurrentProject(forceDemoIfNoneMissing: false), size: 22)
            }
        });

        // 1. Show Project.MediaPool entries first (including online/offline status & updated paths)
        if (Project?.MediaPool != null && Project.MediaPool.Count > 0)
        {
            foreach (var poolAsset in Project.MediaPool)
            {
                var m = poolAsset;
                string badge = m.IsOffline ? "OFFLINE" : m.Type.ToUpperInvariant();
                string sub = m.IsOffline
                    ? $"[MÍDIA AUSENTE] {m.FilePath}"
                    : $"{Path.GetFileName(m.FilePath)} • {m.Duration:0.0}s";
                _projectMediaStack.Children.Add(CreateCatalogCard(
                    badge,
                    m.Name,
                    sub,
                    m.IsOffline ? "Relink..." : "+ Track",
                    () =>
                    {
                        if (m.IsOffline)
                            OpenMissingMediaDialogForCurrentProject(forceDemoIfNoneMissing: false);
                        else
                            InsertMediaAssetIntoTimeline(m);
                    }));
            }
        }

        // 2. Show Workspace Media Bin items
        foreach (var asset in _mediaBin)
        {
            var m = asset;
            _projectMediaStack.Children.Add(CreateCatalogCard(
                m.Type.ToUpperInvariant(),
                m.Name,
                m.Type == "audio"
                    ? $"{m.SampleRate}Hz • Peak {m.PeakDb:0.0} dB • {m.Duration:0.0}s"
                    : $"{m.Width}x{m.Height} @{m.Fps:0.##}fps • {m.Duration:0.0}s",
                "+ Track",
                () => InsertMediaAssetIntoTimeline(m)));
        }
    }

    private void InsertMediaAssetIntoTimeline(MediaAssetItem asset)
    {
        string targetType = asset.Type == "audio" ? "audio" : "video";
        var track = Project.Tracks.FirstOrDefault(t => t.Type == targetType && !t.Locked);
        if (track == null)
        {
            // Empty project (no tracks): Neoveg creates the matching track when media is added
            AddTrack(targetType);
            track = Project.Tracks.FirstOrDefault(t => t.Type == targetType && !t.Locked);
            if (track == null) return;
        }

        // Imported media (audio/video/image) gets automatic fades only when enabled in Edit > Preferences
        bool isImportedMedia = asset.Type is not ("generator" or "title");
        bool autoFade = isImportedMedia && (Localization.AutoFadeOnImport || Project.AutoFadeOnImport);
        double autoFadeSec = autoFade ? Math.Min(0.35, asset.Duration * 0.25) : 0.0;

        var clip = new TimelineClip
        {
            Name = asset.Name,
            MediaId = asset.Id,
            // Full path so the Rust FFI waveform/.sfk analysis and the Missing Media relinker can find the file
            MediaFile = !string.IsNullOrWhiteSpace(asset.FilePath) ? asset.FilePath : asset.FileName,
            MediaType = asset.Type,
            GeneratorType = asset.GeneratorType,
            Color = asset.Color,
            TrackAccentColor = track.TrackAccentColor ?? track.Color,
            Start = Math.Round(Project.Playhead, 3),
            Duration = asset.Duration,
            FadeIn = autoFadeSec,
            FadeOut = autoFadeSec,
            Peaks = asset.Peaks
        };
        track.Clips.Add(clip);
        _previewControl.SelectedClip = clip;
        _timelineControl.SelectedClipId = clip.Id;
        _timelineControl.RecalculateCrossfades();
        RebuildTrackHeadersUI();
        RefreshInspectorUI();
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
        SetStatus($"Inserido \"{clip.Name}\" na faixa \"{track.Name}\".");
    }

    private void ApplyFxToSelectedOrNewClip(string fxType, string fxName)
    {
        var clip = _previewControl.SelectedClip ?? Project.Tracks.SelectMany(t => t.Clips).FirstOrDefault();
        if (clip == null)
        {
            InsertMediaAssetIntoTimeline(new MediaAssetItem
            {
                Name = fxType == "corridorKey" ? "CorridorKey Green Screen Actor" : "SMPTE HD Master Bars",
                Type = "generator",
                GeneratorType = fxType == "corridorKey" ? "greenScreenActor" : "smpteBars",
                Duration = 5.0,
                Color = fxType == "corridorKey" ? "#16a34a" : "#2563eb"
            });
            clip = _previewControl.SelectedClip;
        }
        if (clip == null) return;

        clip.Fx.Add(new VideoFxInstance
        {
            Type = fxType,
            Name = fxName,
            Enabled = true
        });
        // Automatically invalidate prerender/RAM cache on this clip's region -> turns status bar back to Red (#E74C3C)
        PrerenderService.InvalidateCacheRange(clip.Start, clip.End, Project.Fps);
        RefreshInspectorUI();
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
        SetStatus($"Efeito \"{fxName}\" aplicado a \"{clip.Name}\" (Cache invalidado no trecho -> Barra Vermelha #E74C3C).");
    }

    public void SplitAtPlayheadViaRustFfi()
    {
        int splits = 0;
        foreach (var track in Project.Tracks)
        {
            if (track.Locked) continue;
            var toAdd = new List<TimelineClip>();
            foreach (var clip in track.Clips)
            {
                if (RustInterop.TrySplitClip(clip, Project.Playhead, out var rightHalf))
                {
                    toAdd.Add(rightHalf);
                    _previewControl.SelectedClip = rightHalf;
                    _timelineControl.SelectedClipId = rightHalf.Id;
                    PrerenderService.InvalidateCacheRange(clip.Start, rightHalf.End, Project.Fps);
                    splits++;
                }
            }
            track.Clips.AddRange(toAdd);
        }

        if (splits > 0)
        {
            _timelineControl.RecalculateCrossfades();
            _timelineControl.InvalidateVisual();
            UpdateTimecodeAndPreview();
            SetStatus($"Rust FFI Split: {splits} evento(s) dividido(s) em {RustInterop.FormatSmpte(Project.Playhead, Project.Fps)} (Cache invalidado -> Vermelho #E74C3C)");
        }
        else
        {
            SetStatus("Posicione a agulha sobre um clipe para dividir com o motor Rust FFI (Atalho: S).");
        }
    }

    private void DeleteSelectedClip()
    {
        string? selId = _timelineControl.SelectedClipId;
        if (string.IsNullOrEmpty(selId)) return;
        foreach (var track in Project.Tracks)
        {
            var found = track.Clips.FirstOrDefault(c => c.Id == selId);
            if (found != null)
            {
                double removedStart = found.Start;
                double removedDuration = found.Duration;
                PrerenderService.InvalidateCacheRange(found.Start, found.End, Project.Fps);
                track.Clips.Remove(found);
                if (Project.RippleEnabled)
                {
                    ApplyAutoRippleShift(track, removedStart, -removedDuration);
                }
                _timelineControl.SelectedClipId = null;
                _previewControl.SelectedClip = null;
                _timelineControl.RecalculateCrossfades();
                RefreshInspectorUI();
                UpdateTimecodeAndPreview();
                RefreshTimelineScrollBars();
                _timelineControl.InvalidateVisual();
                SetStatus($"Evento \"{found.Name}\" removido{(Project.RippleEnabled ? " com ajuste Auto-Ripple" : "")} (Cache de pré-renderização invalidado no trecho).");
                return;
            }
        }
    }

    public void ApplyAutoRippleShift(TimelineTrack track, double rippleStartSec, double shiftDeltaSec)
    {
        foreach (var subsequent in track.Clips.Where(c => !c.Locked && c.Start >= rippleStartSec - 0.001))
        {
            subsequent.Start = Math.Max(0.0, Math.Round(subsequent.Start + shiftDeltaSec, 6));
        }
    }

    public void AddTrack(string type)
    {
        int count = Project.Tracks.Count(t => t.Type == type) + 1;
        int newTrackIdx = type == "video" ? 0 : Project.Tracks.Count;
        string? neighborColor = null;
        if (Project.Tracks.Count > 0)
        {
            var neighbor = type == "video" ? Project.Tracks[0] : Project.Tracks[^1];
            neighborColor = neighbor.TrackAccentColor ?? neighbor.Color;
        }

        var newModel = new TimelineTrack
        {
            Name = type == "video" ? $"Video Track {count}" : $"Audio Track {count}",
            Type = type,
            Height = 78.0
        };

        TrackViewModelBase trackVm = type == "video"
            ? new VideoTrackViewModel(newModel, Project.Tracks.Count, neighborColor)
            : new AudioTrackViewModel(newModel, Project.Tracks.Count, neighborColor);

        var tr = trackVm.ToTimelineTrack();
        if (type == "video")
            Project.Tracks.Insert(0, tr);
        else
            Project.Tracks.Add(tr);

        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        SetStatus($"Nova faixa adicionada: {tr.Name} (Cor Automática: {tr.TrackAccentColor})");
    }

    private void FocusEventPanCropPanel(TimelineClip clip, TimelineTrack track)
    {
        _timelineControl.SelectedClipId = clip.Id;
        _timelineControl.SelectedTrackId = track.Id;
        _previewControl.SelectedClip = clip;
        RefreshInspectorUI();
        _previewControl.InvalidateVisual();
        _timelineControl.InvalidateVisual();

        // Event Pan/Crop opens exclusively as its own pop-up window (Neoveg event header button)
        OpenEventPanCropWindow(clip);
    }

    private void OpenEventPanCropDialogForSelectedClip()
    {
        var clip = _previewControl.SelectedClip ?? Project.Tracks.Where(t => t.Type == "video").SelectMany(t => t.Clips).FirstOrDefault();
        if (clip != null)
        {
            OpenEventPanCropWindow(clip);
        }
    }

    private void OpenEventPanCropWindow(TimelineClip clip)
    {
        var panCropWin = new EventPanCropWindow(
            Project,
            clip,
            RustInterop,
            CppInterop,
            () =>
            {
                RefreshInspectorUI();
                UpdateTimecodeAndPreview();
                _timelineControl.InvalidateVisual();
            });
        panCropWin.Show(this);
        SetStatus($"Event Pan/Crop aberto para \"{clip.Name}\" (Quadro 'F': {clip.PanCrop.Width:0.0}×{clip.PanCrop.Height:0.0}px • Rot: {clip.PanCrop.Rotation:0.0}°).");
    }

    private void OpenGeneratedMediaDialog(TimelineClip clip, TimelineTrack track)
    {
        var stack = new StackPanel { Spacing = 10, Margin = new Thickness(16) };

        stack.Children.Add(new TextBlock
        {
            Text = $"GENERATED MEDIA — {clip.Name}",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        var txtTitle = new TextBox
        {
            Text = clip.TitleText ?? clip.Name,
            Watermark = "Texto Principal / Nome do Gerador"
        };
        var txtSubtitle = new TextBox
        {
            Text = clip.SubtitleText ?? "Gerador Procedural em Tempo Real (60fps)",
            Watermark = "Subtítulo / Parâmetro do Gerador"
        };
        var cmbGenType = new ComboBox
        {
            ItemsSource = new[] { "titleText", "smpteBars", "greenScreenActor", "cyberGrid", "solidColor" },
            SelectedItem = string.IsNullOrWhiteSpace(clip.GeneratorType) ? "titleText" : clip.GeneratorType,
            HorizontalAlignment = HorizontalAlignment.Stretch
        };
        var txtColor = new TextBox
        {
            Text = clip.Color,
            Watermark = "#0e7490"
        };

        stack.Children.Add(new TextBlock { Text = "Texto / Título do Gerador:", FontSize = 11 });
        stack.Children.Add(txtTitle);
        stack.Children.Add(new TextBlock { Text = "Subtítulo / Descrição:", FontSize = 11 });
        stack.Children.Add(txtSubtitle);
        stack.Children.Add(new TextBlock { Text = "Tipo de Media Generator:", FontSize = 11 });
        stack.Children.Add(cmbGenType);
        stack.Children.Add(new TextBlock { Text = "Cor Sólida / Hex:", FontSize = 11 });
        stack.Children.Add(txtColor);

        var win = new Window
        {
            Title = $"Neoveg Generated Media — {clip.Name}",
            Width = 460,
            Height = 360,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White
        };

        var btnApply = CreateSquareTextButton("Aplicar ao Gerador", "#2563eb", "#ffffff");
        btnApply.Click += (_, _) =>
        {
            if (!string.IsNullOrWhiteSpace(txtTitle.Text))
            {
                clip.TitleText = txtTitle.Text.Trim();
                clip.Name = txtTitle.Text.Trim();
            }
            clip.SubtitleText = txtSubtitle.Text?.Trim();
            if (cmbGenType.SelectedItem is string gType)
                clip.GeneratorType = gType;
            if (!string.IsNullOrWhiteSpace(txtColor.Text))
                clip.Color = txtColor.Text.Trim();

            RefreshInspectorUI();
            UpdateTimecodeAndPreview();
            _timelineControl.InvalidateVisual();
            SetStatus($"Generated Media atualizado para \"{clip.Name}\".");
            win.Close();
        };

        stack.Children.Add(btnApply);
        win.Content = new ScrollViewer { Content = stack };
        win.Show(this);
    }

    private void OpenClipFxChainDialog(TimelineClip clip, TimelineTrack track)
    {
        bool isAudio = track.Type == "audio";
        var rootStack = new StackPanel { Spacing = 10, Margin = new Thickness(16) };

        rootStack.Children.Add(new TextBlock
        {
            Text = isAudio
                ? $"AUDIO EVENT FX CHAIN — {clip.Name}"
                : $"VIDEO EVENT FX CHAIN — {clip.Name}",
            FontSize = 13.5,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        rootStack.Children.Add(new TextBlock
        {
            Text = $"Faixa: {track.Name} • Posição: {clip.Start:0.00}s – {clip.End:0.00}s • Hierarquia: {HierarchicalFxPipeline.CanonicalPipelineSignature}",
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        });

        FxHierarchyStage activeStage = FxHierarchyStage.EventFx;
        var stageRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        stageRow.Children.Add(new TextBlock
        {
            Text = "Estágio da Pipeline FX:",
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });
        var cmbStage = new ComboBox
        {
            ItemsSource = new[]
            {
                FxHierarchyStage.MediaFx,
                FxHierarchyStage.TakeFx,
                FxHierarchyStage.EventFx,
                FxHierarchyStage.TrackFx,
                FxHierarchyStage.MasterFx
            },
            SelectedItem = activeStage,
            Width = 180
        };
        stageRow.Children.Add(cmbStage);
        rootStack.Children.Add(stageRow);

        var activeListStack = new StackPanel { Spacing = 6 };
        void RebuildActiveFxList()
        {
            activeListStack.Children.Clear();
            var targetList = HierarchicalFxPipeline.GetMutableListForStage(Project, track, clip, activeStage);
            if (targetList.Count == 0)
            {
                activeListStack.Children.Add(new TextBlock
                {
                    Text = $"Nenhum efeito no estágio {HierarchicalFxPipeline.GetStageDisplayName(activeStage)}. Adicione um plugin abaixo.",
                    FontSize = 10.5,
                    Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
                });
                return;
            }

            foreach (var fx in targetList.ToList())
            {
                var capturedFx = fx;
                var row = new Grid
                {
                    ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto")
                };

                var chk = new CheckBox
                {
                    IsChecked = capturedFx.Enabled,
                    VerticalAlignment = VerticalAlignment.Center,
                    Margin = new Thickness(0, 0, 8, 0)
                };
                chk.IsCheckedChanged += (_, _) =>
                {
                    capturedFx.Enabled = chk.IsChecked ?? true;
                    RefreshInspectorUI();
                    UpdateTimecodeAndPreview();
                    _timelineControl.InvalidateVisual();
                };
                Grid.SetColumn(chk, 0);
                row.Children.Add(chk);

                row.Children.Add(new TextBlock
                {
                    Text = $"[{HierarchicalFxPipeline.GetStageDisplayName(activeStage)}] {capturedFx.Name} ({capturedFx.Type})",
                    FontSize = 11,
                    FontWeight = FontWeight.SemiBold,
                    Foreground = new SolidColorBrush(Color.Parse("#4ade80")),
                    VerticalAlignment = VerticalAlignment.Center,
                    [Grid.ColumnProperty] = 1
                });

                var btnRem = NeovegIcons.CreateButton(NeovegIcons.Delete, "Remover efeito da cadeia", bgHex: "#7f1d1d", fgHex: "#ffffff", size: 22);
                btnRem.Click += (_, _) =>
                {
                    targetList.Remove(capturedFx);
                    RebuildActiveFxList();
                    RefreshInspectorUI();
                    UpdateTimecodeAndPreview();
                    _timelineControl.InvalidateVisual();
                    SetStatus($"FX \"{capturedFx.Name}\" removido do estágio {HierarchicalFxPipeline.GetStageDisplayName(activeStage)}.");
                };
                Grid.SetColumn(btnRem, 2);
                row.Children.Add(btnRem);

                activeListStack.Children.Add(new Border
                {
                    Background = new SolidColorBrush(Color.Parse("#18202c")),
                    BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
                    BorderThickness = new Thickness(1),
                    Padding = new Thickness(8, 5),
                    Child = row
                });
            }
        }

        cmbStage.SelectionChanged += (_, _) =>
        {
            if (cmbStage.SelectedItem is FxHierarchyStage selStage)
            {
                activeStage = selStage;
                RebuildActiveFxList();
            }
        };

        RebuildActiveFxList();
        rootStack.Children.Add(new TextBlock
        {
            Text = "Cadeia de Efeitos Ativa (Plug-In Chain):",
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#f59e0b"))
        });
        rootStack.Children.Add(activeListStack);

        // Catalog to add new FX directly into this clip's FX Chain
        var availableFx = isAudio
            ? new (string Type, string Name)[]
            {
                ("ladspa.sc4_1882", "LADSPA SC4 Stereo Compressor (1882)"),
                ("ladspa.fast_lookahead_limiter_1913", "LADSPA Lookahead Limiter (1913)"),
                ("ladspa.mbeq_1197", "LADSPA 15-Band Graphic EQ (1197)"),
                ("ladspa.plate_1423", "LADSPA Neoveg Plate Reverb (1423)")
            }
            : new (string Type, string Name)[]
            {
                ("corridorKey", "CorridorKey AI Green Screen Unmixer"),
                ("colorGrade", "Neoveg 3-Wheel Color Grade"),
                ("lut3d.teal_orange", "Neoveg 3D LUT — Teal & Orange (.cube)"),
                ("lut3d.cinematic", "Neoveg 3D LUT — Cinematic Film (.cube)"),
                ("frei0r.glow", "Frei0r Glow (Optical Bloom)"),
                ("frei0r.cartoon", "Frei0r Cartoon (Cel-Shaded)"),
                ("frei0r.pixeliz0r", "Frei0r Pixeliz0r (Mosaic)")
            };

        rootStack.Children.Add(new TextBlock
        {
            Text = "Adicionar Plug-in à Cadeia (Add FX):",
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8")),
            Margin = new Thickness(0, 6, 0, 0)
        });

        foreach (var catalogItem in availableFx)
        {
            var item = catalogItem;
            rootStack.Children.Add(CreateCatalogCard(
                isAudio ? "AUDIO FX" : "VIDEO FX",
                item.Name,
                item.Type,
                "+ Add to Chain",
                () =>
                {
                    var stageList = HierarchicalFxPipeline.GetMutableListForStage(Project, track, clip, activeStage);
                    stageList.Add(new VideoFxInstance
                    {
                        Type = item.Type,
                        Name = item.Name,
                        Enabled = true
                    });
                    RebuildActiveFxList();
                    RefreshInspectorUI();
                    UpdateTimecodeAndPreview();
                    _timelineControl.InvalidateVisual();
                    SetStatus($"Plug-in \"{item.Name}\" adicionado ao estágio {HierarchicalFxPipeline.GetStageDisplayName(activeStage)} ({HierarchicalFxPipeline.CanonicalPipelineSignature}).");
                }));
        }

        var fxWin = new Window
        {
            Title = isAudio
                ? $"Audio Event FX — {clip.Name}"
                : $"Video Event FX — {clip.Name}",
            Width = 560,
            Height = 520,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White,
            Content = new ScrollViewer { Content = rootStack }
        };
        fxWin.Show(this);
        SetStatus($"Janela {(isAudio ? "Audio" : "Video")} Event FX Chain aberta para \"{clip.Name}\".");
    }

    private void CutClipFromEventHeader(TimelineClip clip, TimelineTrack track)
    {
        double cutStart = clip.Start;
        double cutDur = clip.Duration;
        _clipboardClip = clip.Clone();
        track.Clips.Remove(clip);
        if (Project.RippleEnabled)
        {
            ApplyAutoRippleShift(track, cutStart, -cutDur);
        }
        if (_timelineControl.SelectedClipId == clip.Id)
        {
            _timelineControl.SelectedClipId = null;
            _previewControl.SelectedClip = null;
        }
        _timelineControl.RecalculateCrossfades();
        RefreshInspectorUI();
        UpdateTimecodeAndPreview();
        _timelineControl.InvalidateVisual();
        SetStatus($"Evento \"{clip.Name}\" recortado para a área de transferência{(Project.RippleEnabled ? " (Auto-Ripple aplicado)" : "")}.");
    }

    private void CopyClipFromEventHeader(TimelineClip clip, TimelineTrack track)
    {
        _clipboardClip = clip.Clone();
        SetStatus($"Evento \"{clip.Name}\" copiado para a área de transferência.");
    }

    private void PasteClipFromEventHeader(TimelineClip referenceClip, TimelineTrack track)
    {
        var source = _clipboardClip ?? referenceClip;
        var pasted = source.Clone();
        pasted.Start = Math.Round(Project.Playhead > 0.01 ? Project.Playhead : referenceClip.End, 3);
        track.Clips.Add(pasted);
        _timelineControl.SelectedClipId = pasted.Id;
        _previewControl.SelectedClip = pasted;
        _timelineControl.RecalculateCrossfades();
        RefreshInspectorUI();
        UpdateTimecodeAndPreview();
        RefreshTimelineScrollBars();
        _timelineControl.InvalidateVisual();
        SetStatus($"Evento \"{pasted.Name}\" colado na faixa \"{track.Name}\" em {pasted.Start:0.00}s.");
    }

    private void OpenClipPropertiesDialog(TimelineClip clip, TimelineTrack track)
    {
        var stack = new StackPanel { Spacing = 8, Margin = new Thickness(16) };

        stack.Children.Add(new TextBlock
        {
            Text = $"PROPRIEDADES DO EVENTO — {clip.Name}",
            FontSize = 13,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        var txtName = new TextBox { Text = clip.Name };
        var txtStart = new TextBox { Text = clip.Start.ToString("0.000", System.Globalization.CultureInfo.InvariantCulture) };
        var txtDur = new TextBox { Text = clip.Duration.ToString("0.000", System.Globalization.CultureInfo.InvariantCulture) };
        var txtRate = new TextBox { Text = clip.PlaybackRate.ToString("0.00", System.Globalization.CultureInfo.InvariantCulture) };
        var chkMute = new CheckBox { Content = "Mute Event (Silenciar Evento)", IsChecked = clip.Muted };
        var chkLock = new CheckBox { Content = "Lock Event (Travar Evento na Timeline)", IsChecked = clip.Locked };

        stack.Children.Add(new TextBlock { Text = "Nome do Arquivo / Mídia no Event Header:", FontSize = 10.5 });
        stack.Children.Add(txtName);
        stack.Children.Add(new TextBlock { Text = "Início na Timeline (segundos):", FontSize = 10.5 });
        stack.Children.Add(txtStart);
        stack.Children.Add(new TextBlock { Text = "Duração (segundos):", FontSize = 10.5 });
        stack.Children.Add(txtDur);
        stack.Children.Add(new TextBlock { Text = "Playback Rate (Velocidade):", FontSize = 10.5 });
        stack.Children.Add(txtRate);
        stack.Children.Add(chkMute);
        stack.Children.Add(chkLock);

        var propWin = new Window
        {
            Title = $"Propriedades do Evento — {clip.Name}",
            Width = 430,
            Height = 410,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White
        };

        var btnSave = CreateSquareTextButton("Salvar Propriedades", "#2563eb", "#ffffff");
        btnSave.Click += (_, _) =>
        {
            if (!string.IsNullOrWhiteSpace(txtName.Text))
                clip.Name = txtName.Text.Trim();
            if (double.TryParse(txtStart.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double st))
                clip.Start = Math.Max(0.0, st);
            if (double.TryParse(txtDur.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double dur))
                clip.Duration = Math.Max(0.15, dur);
            if (double.TryParse(txtRate.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double rate))
                clip.PlaybackRate = Math.Clamp(rate, 0.1, 4.0);

            clip.Muted = chkMute.IsChecked ?? false;
            clip.Locked = chkLock.IsChecked ?? false;

            _timelineControl.RecalculateCrossfades();
            RefreshInspectorUI();
            UpdateTimecodeAndPreview();
            RefreshTimelineScrollBars();
            _timelineControl.InvalidateVisual();
            SetStatus($"Propriedades do evento \"{clip.Name}\" atualizadas.");
            propWin.Close();
        };

        stack.Children.Add(btnSave);
        propWin.Content = new ScrollViewer { Content = stack };
        propWin.Show(this);
    }

    public void AdjustAllTrackHeights(double delta)
    {
        foreach (var tr in Project.Tracks)
            tr.Height = Math.Clamp(tr.Height + delta, 46.0, 220.0);
        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
    }

    public void GroupVideoAndAudioTracks()
    {
        string[] palette = { "#f59e0b", "#10b981", "#a855f7", "#ec4899" };
        var vTrack = Project.Tracks.FirstOrDefault(t => t.Type == "video" && string.IsNullOrEmpty(t.GroupId))
                     ?? Project.Tracks.FirstOrDefault(t => t.Type == "video");
        var aTrack = Project.Tracks.FirstOrDefault(t => t.Type == "audio" && string.IsNullOrEmpty(t.GroupId))
                     ?? Project.Tracks.FirstOrDefault(t => t.Type == "audio");

        int grpNum = Project.Tracks.Select(t => t.GroupId).Where(g => !string.IsNullOrEmpty(g)).Distinct().Count() + 1;
        string gid = $"grp_av_{grpNum}";
        string gname = $"Grupo V+A {grpNum}";
        string gcol = palette[(grpNum - 1) % palette.Length];

        if (vTrack != null) { vTrack.GroupId = gid; vTrack.GroupName = gname; vTrack.GroupColor = gcol; }
        if (aTrack != null) { aTrack.GroupId = gid; aTrack.GroupName = gname; aTrack.GroupColor = gcol; }

        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        SetStatus($"Faixas de Vídeo e Áudio agrupadas em \"{gname}\" (Mute/Solo sincronizados).");
    }

    public void UngroupAllOrSelectedTracks()
    {
        foreach (var tr in Project.Tracks)
        {
            tr.GroupId = null;
            tr.GroupName = null;
            tr.GroupColor = null;
        }
        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        SetStatus("Todas as faixas foram desagrupadas.");
    }

    private void ToggleSingleTrackGroup(TimelineTrack track)
    {
        if (!string.IsNullOrEmpty(track.GroupId))
        {
            string gid = track.GroupId;
            foreach (var m in Project.Tracks.Where(t => t.GroupId == gid))
            {
                m.GroupId = null;
                m.GroupName = null;
                m.GroupColor = null;
            }
            RebuildTrackHeadersUI();
            _timelineControl.InvalidateVisual();
            SetStatus($"Grupo removido da faixa \"{track.Name}\".");
        }
        else
        {
            GroupVideoAndAudioTracks();
        }
    }

    private void TogglePlayback(bool? force = null)
    {
        bool nextState = force ?? !_isPlaying;
        if (nextState && !_isPlaying)
        {
            _playbackOriginPlayhead = Project.Playhead;
        }
        _isPlaying = nextState;
        _scopeControl.IsPlaying = _isPlaying;
        _masterBusView.IsPlaying = _isPlaying;
        _playheadOverlay.IsPlaying = _isPlaying;
        NeovegIcons.SetButtonIcon(_btnPlayPause, _isPlaying ? NeovegIcons.Pause : NeovegIcons.Play, filled: true);
        if (_isPlaying) _playbackTimer.Start();
        else _playbackTimer.Stop();
        _playheadOverlay.InvalidateVisual();
        _masterBusView.RefreshTelemetry();
    }

    public void PlayOrStopAndReturnCursor()
    {
        if (_isPlaying)
        {
            TogglePlayback(false);
            Project.Playhead = _playbackOriginPlayhead;
            UpdateTimecodeAndPreview();
            _timelineControl.InvalidateVisual();
            SetStatus($"Atalho [Enter]: Play/Stop com retorno do cursor para {RustInterop.FormatSmpte(Project.Playhead, Project.Fps)}.");
        }
        else
        {
            _playbackOriginPlayhead = Project.Playhead;
            TogglePlayback(true);
            SetStatus($"Atalho [Enter]: Reprodução iniciada a partir de {RustInterop.FormatSmpte(_playbackOriginPlayhead, Project.Fps)} (pressione Enter novamente para parar e retornar).");
        }
    }

    public void ToggleTimelineSnapping()
    {
        Project.SnapEnabled = !Project.SnapEnabled;
        UpdateToolbarToggleStates();
        _timelineControl.InvalidateVisual();
        SetStatus($"Atalho Neoveg [F8]: Snapping Magnético {(Project.SnapEnabled ? "ATIVADO" : "DESATIVADO")}.");
    }

    public void AddRegionAtLoopOrPlayhead()
    {
        double start = Project.LoopEnd > Project.LoopStart ? Project.LoopStart : Math.Round(Project.Playhead, 3);
        double end = Project.LoopEnd > Project.LoopStart ? Project.LoopEnd : Math.Round(Project.Playhead + 3.0, 3);
        var region = new TimelineRegion
        {
            Start = start,
            End = end,
            Label = $"Region {Project.Regions.Count + 1}"
        };
        Project.Regions.Add(region);
        _timelineControl.InvalidateVisual();
        SetStatus($"Atalho Neoveg [R]: Região \"{region.Label}\" criada ({start:0.00}s – {end:0.00}s).");
    }

    public void GroupSelectedEventsOrTracks()
    {
        GroupVideoAndAudioTracks();
        string gid = $"evt_grp_{Guid.NewGuid():N}"[..11];
        foreach (var tr in Project.Tracks)
        {
            foreach (var c in tr.Clips.Where(c => Math.Abs(c.Start - Project.Playhead) <= 2.5 || c.Id == _timelineControl.SelectedClipId))
            {
                c.GroupId = gid;
            }
        }
        _timelineControl.InvalidateVisual();
        SetStatus("Atalho Neoveg [G]: Eventos e faixas V+A agrupados.");
    }

    public void UngroupSelectedEventsOrTracks()
    {
        UngroupAllOrSelectedTracks();
        foreach (var tr in Project.Tracks)
        {
            foreach (var c in tr.Clips)
            {
                c.GroupId = null;
            }
        }
        _timelineControl.InvalidateVisual();
        SetStatus("Atalho Neoveg [U]: Eventos e faixas desagrupados.");
    }

    /// <summary>
    /// Toggles a Neoveg dock window between docked and floating (Timeline, Video Preview, Mixer, Trimmer, Project Media).
    /// </summary>
    public void OpenFloatingOrDockedPanel(string panelName)
    {
        DetachableDockPanel? dock = panelName.ToLowerInvariant() switch
        {
            "timeline" => _dockTimeline,
            "video preview" => _dockPreview,
            "audio mixer" or "mixer de áudio" or "master bus" or "mixer" => _dockMixer,
            "project media" => _dockProjectMedia,
            _ => null
        };
        if (dock != null)
        {
            dock.Toggle();
            SetStatus($"Janela \"{dock.PanelTitle}\" {(dock.IsDetached ? "destacada (feche a janela flutuante para acoplar de volta)" : "acoplada")}.");
            return;
        }
        if (string.Equals(panelName, "Trimmer", StringComparison.OrdinalIgnoreCase) && _leftDockTabs != null && _tabTrimmer != null)
        {
            if (_leftDockTabs.Items.Contains(_tabTrimmer))
            {
                DetachableDockPanel.UndockTab(_leftDockTabs, _tabTrimmer, "Trimmer", null, null);
                SetStatus("Trimmer destacado em janela flutuante (feche-a para acoplar de volta).");
            }
            return;
        }
        if (string.Equals(panelName, "Event Pan/Crop", StringComparison.OrdinalIgnoreCase))
        {
            OpenEventPanCropDialogForSelectedClip();
            return;
        }
        if (string.Equals(panelName, "Track Motion", StringComparison.OrdinalIgnoreCase))
        {
            OpenTrackMotionDialog(Project.Tracks.FirstOrDefault(t => t.Type == "video"));
            return;
        }

        Control content = string.Equals(panelName, "Trimmer", StringComparison.OrdinalIgnoreCase)
            ? BuildTrimmerDockContent()
            : new TextBlock
            {
                Text = $"Painel Acoplável / Flutuante: {panelName} (Sistema de Docking Neoveg 13/18/21)",
                Margin = new Thickness(14),
                Foreground = Brushes.White
            };

        var floatWin = new Window
        {
            Title = $"Neoveg Dockable Panel — {panelName}",
            Width = 540,
            Height = 400,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White,
            Content = content
        };
        floatWin.Show(this);
        SetStatus($"Painel acoplável/flutuante \"{panelName}\" aberto.");
    }

    public void OpenTrackMotionDialog(TimelineTrack? track = null)
    {
        var targetTrack = track ?? Project.Tracks.FirstOrDefault(t => t.Type == "video") ?? Project.Tracks.FirstOrDefault();
        if (targetTrack == null) return;

        var stack = new StackPanel { Spacing = 10, Margin = new Thickness(16) };
        stack.Children.Add(new TextBlock
        {
            Text = $"NEOVEG TRACK MOTION — {targetTrack.Name}",
            FontSize = 13.5,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });

        var txtX = new TextBox { Text = targetTrack.TrackMotion.XCenter.ToString("0.0", System.Globalization.CultureInfo.InvariantCulture) };
        var txtY = new TextBox { Text = targetTrack.TrackMotion.YCenter.ToString("0.0", System.Globalization.CultureInfo.InvariantCulture) };
        var txtRot = new TextBox { Text = targetTrack.TrackMotion.Rotation.ToString("0.0", System.Globalization.CultureInfo.InvariantCulture) };

        stack.Children.Add(new TextBlock { Text = "Posição X (Center X):", FontSize = 10.5 });
        stack.Children.Add(txtX);
        stack.Children.Add(new TextBlock { Text = "Posição Y (Center Y):", FontSize = 10.5 });
        stack.Children.Add(txtY);
        stack.Children.Add(new TextBlock { Text = "Rotação 2D/3D (Graus):", FontSize = 10.5 });
        stack.Children.Add(txtRot);

        var win = new Window
        {
            Title = $"Track Motion — {targetTrack.Name}",
            Width = 440,
            Height = 340,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White
        };

        var btnApply = CreateSquareTextButton("Aplicar Track Motion", "#2563eb", "#ffffff");
        btnApply.Click += (_, _) =>
        {
            if (double.TryParse(txtX.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double cx))
                targetTrack.TrackMotion.XCenter = cx;
            if (double.TryParse(txtY.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double cy))
                targetTrack.TrackMotion.YCenter = cy;
            if (double.TryParse(txtRot.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double rot))
                targetTrack.TrackMotion.Rotation = rot;
            _previewControl.InvalidateVisual();
            SetStatus($"Track Motion atualizado na faixa \"{targetTrack.Name}\".");
            win.Close();
        };
        stack.Children.Add(btnApply);
        win.Content = stack;
        win.Show(this);
    }

    private double GetProjectDuration()
    {
        double max = 8.0;
        foreach (var tr in Project.Tracks)
            foreach (var c in tr.Clips)
                if (c.End > max) max = c.End;
        return max;
    }

    private void UpdateTimecodeAndPreview()
    {
        _bigSmpteReadout.Text = PlayheadOverlayCanvas.FormatPlayheadSmpte(
            Project.Playhead,
            Project.Fps,
            RustInterop,
            _playheadOverlay.ForceDropFrame);
        UpdatePreviewStatus();
        _playheadOverlay.Project = Project;
        _playheadOverlay.PixelsPerSecond = _timelineControl.PixelsPerSecond;
        _playheadOverlay.InvalidateVisual();
        _previewControl.InvalidateVisual();
        _masterBusView.RefreshTelemetry();
    }

    private void NeovegSplashDialog()
    {
        var splash = new NeovegSplashWindow(durationSeconds: 4.0);
        splash.Show(this);
        SetStatus("Exibindo Tela de Carregamento Clássica (Neoveg 12.0 Build 563 64-bit • Updating plug-in cache...).");
    }

    private async Task RunCppSharedMemoryTransferDemoAsync()
    {
        var res = await CppInterop.TransferFrameViaSharedMemoryAsync(Project.Width, Project.Height);
        SetStatus($"{CppInterop.GetCppExtensionBanner()} • {res.Summary}");
    }

    private void RescanScriptsAndExtensionsFolders()
    {
        var scripts = ScriptingService.RescanScriptsFolder();
        var exts = ScriptingService.RescanExtensionsFolder();
        PopulateTopMenuBarItems();
        SetStatus($"Pasta scripts/ e extensions/ recarregada: {scripts.Count} script(s) (.cs/.py/.dll) e {exts.Count} extensão(ões) de janela encontradas.");
    }

    private async Task OpenRunScriptFilePickerAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Executar Script ou Extensão (.cs / .py / .dll)",
            AllowMultiple = false
        });

        var picked = files.FirstOrDefault();
        string? localPath = picked?.TryGetLocalPath();
        if (!string.IsNullOrEmpty(localPath))
        {
            var res = await ScriptingService.ExecuteScriptFileAsync(
                localPath,
                Project,
                _timelineControl.SelectedClipId,
                _timelineControl.SelectedTrackId,
                () =>
                {
                    _timelineControl.RecalculateCrossfades();
                    RebuildTrackHeadersUI();
                    RefreshInspectorUI();
                    UpdateTimecodeAndPreview();
                    _timelineControl.InvalidateVisual();
                });
            SetStatus($"[{res.EngineUsed}] {res.SummaryMessage}");
        }
    }

    private async Task RunDiscoveredScriptAsync(ScriptFileEntry entry)
    {
        var res = await ScriptingService.ExecuteScriptFileAsync(
            entry.FullPath,
            Project,
            _timelineControl.SelectedClipId,
            _timelineControl.SelectedTrackId,
            () =>
            {
                _timelineControl.RecalculateCrossfades();
                RebuildTrackHeadersUI();
                RefreshInspectorUI();
                UpdateTimecodeAndPreview();
                _timelineControl.InvalidateVisual();
            });
        SetStatus($"[{res.EngineUsed}] {res.SummaryMessage}");
    }

    private void OpenDockableExtensionDialog(string extensionId)
    {
        var extWin = new DockableExtensionWindow(
            Project,
            RustInterop,
            ScriptingService,
            _timelineControl.SelectedClipId,
            _timelineControl.SelectedTrackId,
            extensionId,
            statusMsg =>
            {
                _timelineControl.RecalculateCrossfades();
                RebuildTrackHeadersUI();
                RefreshInspectorUI();
                UpdateTimecodeAndPreview();
                _timelineControl.InvalidateVisual();
                SetStatus(statusMsg);
            },
            dockControl =>
            {
                OpenInspectorWindow();
                _inspectorBodyHost.Content = dockControl;
                SetStatus($"Extensão de Janela '{dockControl.DisplayName}' acoplada à janela Inspector do Neoveg.");
            });
        extWin.Show(this);
        SetStatus($"Extensão de Janela aberta: {extensionId} (VPFlow / Datamosh / Neoveg Toolkit / HOS).");
    }

    private void OpenWelcomeDialog()
    {
        var win = new WelcomeWindow(
            Project,
            PythonService,
            RustInterop,
            _ffmpegBanner,
            (selectedProj, msg) => LoadProjectState(selectedProj, msg),
            () =>
            {
                UpdateTimecodeAndPreview();
                SetStatus($"Configurações Avançadas aplicadas: {Project.Width}x{Project.Height} @{Project.Fps:0.##}fps • {Project.ColorSpace}");
            });
        win.Show(this);
    }

    private void OpenAdvancedSettingsDialog()
    {
        var adv = new AdvancedSettingsWindow(
            Project,
            RustInterop,
            _ffmpegBanner,
            () =>
            {
                UpdateTimecodeAndPreview();
                SetStatus($"Configurações Avançadas aplicadas: {Project.Width}x{Project.Height} @{Project.Fps:0.##}fps • {Project.ColorSpace}");
            });
        adv.Show(this);
    }

    private void PopoutVideoPreviewToMonitor()
    {
        var popMonitor = new VideoPreviewControl
        {
            Project = Project,
            SelectedClip = _previewControl.SelectedClip,
            RustInterop = RustInterop,
            CppInterop = CppInterop,
            VideoFxBypassed = _previewFxBypassed,
            PreviewQuality = _previewControl.PreviewQuality,
            PreviewResolution = _previewControl.PreviewResolution
        };
        var popWin = new Window
        {
            Title = "NEOVEG — Secondary Monitor Video Preview Out (Avalonia Multi-Monitor)",
            Width = 860,
            Height = 520,
            Background = new SolidColorBrush(Color.Parse("#080a0f")),
            Content = popMonitor
        };
        popWin.Show();
        SetStatus("Video Preview destacado em uma nova janela multiplataforma para o segundo monitor!");
    }

    private void PopoutMixerScopesToMonitor()
    {
        var popMasterBus = new MasterBusView(Project, RustInterop)
        {
            IsPlaying = _isPlaying
        };
        var popWin = new Window
        {
            Title = "NEOVEG — Secondary Monitor Master Bus Mixer (Avalonia Multi-Monitor)",
            Width = 340,
            Height = 520,
            Background = new SolidColorBrush(Color.Parse(CurrentTheme.DockPanelBg)),
            Content = popMasterBus
        };
        popWin.Show();
        SetStatus("Master Bus Mixer destacado em uma nova janela para múltiplos monitores!");
    }

    private void SaveCurrentProject()
    {
        string savedPath = PythonService.SaveProjectToDisk(Project, Project.Name);
        SetStatus($"Projeto nativo (.neoveg: project.json + media_pool.json + history.json) salvo em: {savedPath}");
    }

    private async Task SaveProjectAsNeovegPickerAsync()
    {
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Salvar Projeto Como (.neoveg)",
            SuggestedFileName = $"{Project.Name}.neoveg",
            DefaultExtension = "neoveg",
            FileTypeChoices = new[]
            {
                new FilePickerFileType("Projeto Nativo Neoveg (*.neoveg)") { Patterns = new[] { "*.neoveg" } }
            }
        });

        string? localPath = file?.TryGetLocalPath();
        if (!string.IsNullOrEmpty(localPath))
        {
            string saved = NeovegProjectInterchangeEngine.SaveNativeNeovegContainer(Project, localPath, _rootDir);
            SetStatus($"Projeto salvo como contêiner nativo .neoveg em: {saved}");
        }
    }

    private async Task OpenNeovegProjectPickerAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Abrir Projeto (.neoveg)",
            AllowMultiple = false,
            FileTypeFilter = new[]
            {
                new FilePickerFileType("Projeto Nativo Neoveg (*.neoveg)") { Patterns = new[] { "*.neoveg" } }
            }
        });

        string? localPath = files.FirstOrDefault()?.TryGetLocalPath();
        if (!string.IsNullOrEmpty(localPath) && File.Exists(localPath))
        {
            var loaded = NeovegProjectInterchangeEngine.LoadNativeNeovegContainer(localPath);
            LoadProjectState(loaded, $"Projeto nativo .neoveg carregado: {Path.GetFileName(localPath)}");
        }
    }

    private async Task ImportLegacyVegProjectPickerAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Abrir Projeto Nativo Neoveg (.neoveg)",
            AllowMultiple = false,
            FileTypeFilter = new[]
            {
                new FilePickerFileType("Projeto Nativo Neoveg (*.neoveg)") { Patterns = new[] { "*.neoveg" } }
            }
        });

        string? localPath = files.FirstOrDefault()?.TryGetLocalPath();
        if (!string.IsNullOrEmpty(localPath) && File.Exists(localPath))
        {
            var proj = NeovegProjectInterchangeEngine.LoadNativeNeovegContainer(localPath);
            LoadProjectState(proj, $"Projeto nativo .neoveg carregado ({Path.GetFileName(localPath)})");
        }
    }

    private async Task ImportXmlOrFcpxmlProjectPickerAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Importar Final Cut Pro / Premiere XML (.xml, .fcpxml)",
            AllowMultiple = false,
            FileTypeFilter = new[]
            {
                new FilePickerFileType("Final Cut Pro / Premiere XML (*.xml, *.fcpxml)") { Patterns = new[] { "*.xml", "*.fcpxml" } }
            }
        });

        string? localPath = files.FirstOrDefault()?.TryGetLocalPath();
        if (!string.IsNullOrEmpty(localPath) && File.Exists(localPath))
        {
            string ext = Path.GetExtension(localPath).ToLowerInvariant();
            NeovegProject imported = ext == ".fcpxml" || File.ReadAllText(localPath).Contains("<fcpxml", StringComparison.OrdinalIgnoreCase)
                ? NeovegProjectInterchangeEngine.ImportFcpxml(localPath)
                : NeovegProjectInterchangeEngine.ImportFcp7Xml(localPath);
            LoadProjectState(imported, $"Timeline XML importada com sucesso de: {Path.GetFileName(localPath)} ({imported.Tracks.Count} faixas)");
        }
    }

    private async Task ExportFcp7XmlProjectPickerAsync()
    {
        string defaultOut = Path.Combine(PythonService.ProjectsDir, $"{Project.Name}_FCP7.xml");
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Exportar Final Cut Pro 7 / Premiere XML (.xml)",
            SuggestedFileName = Path.GetFileName(defaultOut),
            DefaultExtension = "xml",
            FileTypeChoices = new[]
            {
                new FilePickerFileType("Final Cut Pro 7 / Premiere XML (*.xml)") { Patterns = new[] { "*.xml" } }
            }
        });

        string targetPath = file?.TryGetLocalPath() ?? defaultOut;
        string saved = NeovegProjectInterchangeEngine.ExportFcp7Xml(Project, targetPath);
        SetStatus($"Projeto exportado em FCP7 / Premiere XML (.xml): {saved}");
    }

    private async Task ExportFcpxmlProjectPickerAsync()
    {
        string defaultOut = Path.Combine(PythonService.ProjectsDir, $"{Project.Name}_FCPX.fcpxml");
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Exportar Final Cut Pro X / DaVinci Resolve XML (.fcpxml)",
            SuggestedFileName = Path.GetFileName(defaultOut),
            DefaultExtension = "fcpxml",
            FileTypeChoices = new[]
            {
                new FilePickerFileType("Final Cut Pro X XML (*.fcpxml)") { Patterns = new[] { "*.fcpxml" } }
            }
        });

        string targetPath = file?.TryGetLocalPath() ?? defaultOut;
        string saved = NeovegProjectInterchangeEngine.ExportFcpxml(Project, targetPath);
        SetStatus($"Projeto exportado em Final Cut Pro X / DaVinci Resolve (.fcpxml): {saved}");
    }

    private async Task ExportEdlProjectPickerAsync()
    {
        string defaultOut = Path.Combine(PythonService.ProjectsDir, $"{Project.Name}_CMX3600.edl");
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Exportar EDL CMX3600 (.edl)",
            SuggestedFileName = Path.GetFileName(defaultOut),
            DefaultExtension = "edl",
            FileTypeChoices = new[]
            {
                new FilePickerFileType("EDL CMX3600 (*.edl)") { Patterns = new[] { "*.edl" } }
            }
        });

        string targetPath = file?.TryGetLocalPath() ?? defaultOut;
        string saved = NeovegProjectInterchangeEngine.ExportCmx3600Edl(Project, targetPath);
        SetStatus($"Projeto exportado em EDL CMX3600 (.edl): {saved}");
    }

    private async Task OpenImportMediaDialogAsync()
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Importar Vídeo ou Áudio (MP4 / WAV / MP3)",
            AllowMultiple = true
        });

        foreach (var f in files)
        {
            string? localPath = f.TryGetLocalPath();
            if (!string.IsNullOrEmpty(localPath))
            {
                var asset = await PythonService.ProbeAndImportMediaAsync(localPath, RustInterop);
                if (asset != null)
                {
                    _mediaBin.Insert(0, asset);
                    InsertMediaAssetIntoTimeline(asset);
                }
            }
        }
    }

    private async Task RunIsolatedFFmpegRenderAsync()
    {
        var loc = Localization;
        SetStatus(loc.T("Dialog.Export.Rendering", "Renderizando projeto multi-track via Python Isolated FFmpeg..."));
        var (ok, outPath, _) = await PythonService.RenderProjectViaPythonFFmpegAsync(Project, "Neoveg_Master_Output.mp4");
        if (ok)
            SetStatus($"{loc.T("Dialog.Export.Success", "Render FFmpeg concluído com sucesso:")} {outPath}");
        else
            SetStatus(loc.T("Dialog.Export.Error", "Erro durante a exportação FFmpeg. Verifique o log de renderização."));
    }

    private void OnNeovegKeyDown(object? sender, KeyEventArgs e)
    {
        // Global Ctrl shortcuts (Ctrl+Q / Ctrl+Shift+Q / Ctrl+S / Ctrl+O) also work while a track name TextBox has focus;
        // plain letter shortcuts are left to the TextBox.
        bool isGlobalCtrlShortcut = e.KeyModifiers.HasFlag(KeyModifiers.Control) && e.Key is Key.Q or Key.S or Key.O;
        if (e.Source is TextBox && !isGlobalCtrlShortcut) return;
        if (e.Key == Key.S && e.KeyModifiers.HasFlag(KeyModifiers.Control) && e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            e.Handled = true;
            _ = SaveProjectAsNeovegPickerAsync();
        }
        else if (e.Key == Key.S && e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            e.Handled = true;
            SaveCurrentProject();
        }
        else if (e.Key == Key.O && e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            e.Handled = true;
            _ = OpenNeovegProjectPickerAsync();
        }
        else if (e.Key == Key.Q && e.KeyModifiers.HasFlag(KeyModifiers.Control) && e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            e.Handled = true;
            AddTrack("video");
        }
        else if (e.Key == Key.Q && e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            e.Handled = true;
            AddTrack("audio");
        }
        else if (e.Key == Key.F8)
        {
            e.Handled = true;
            ToggleTimelineSnapping();
        }
        else if (e.Key == Key.Space)
        {
            e.Handled = true;
            TogglePlayback();
        }
        else if (e.Key == Key.Enter)
        {
            e.Handled = true;
            PlayOrStopAndReturnCursor();
        }
        else if (e.Key == Key.S)
        {
            e.Handled = true;
            SplitAtPlayheadViaRustFfi();
        }
        else if (e.Key == Key.R)
        {
            e.Handled = true;
            AddRegionAtLoopOrPlayhead();
        }
        else if (e.Key == Key.G)
        {
            e.Handled = true;
            GroupSelectedEventsOrTracks();
        }
        else if (e.Key == Key.U)
        {
            e.Handled = true;
            UngroupSelectedEventsOrTracks();
        }
        else if (e.Key is Key.Delete or Key.Back or Key.Z)
        {
            e.Handled = true;
            DeleteSelectedClip();
        }
        else if (e.Key == Key.Left)
        {
            e.Handled = true;
            int stepFrames = e.KeyModifiers.HasFlag(KeyModifiers.Shift) ? 10 : 1;
            double frameSec = stepFrames / Math.Max(1.0, Project.Fps);
            Project.Playhead = Math.Max(0.0, Project.Playhead - frameSec);
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            SetStatus($"Atalho [←]: -{stepFrames} quadro(s) -> {PlayheadOverlayCanvas.FormatPlayheadSmpte(Project.Playhead, Project.Fps, RustInterop, _playheadOverlay.ForceDropFrame)} (Blink Timer resetado -> Opacity 1.0)");
        }
        else if (e.Key == Key.Right)
        {
            e.Handled = true;
            int stepFrames = e.KeyModifiers.HasFlag(KeyModifiers.Shift) ? 10 : 1;
            double frameSec = stepFrames / Math.Max(1.0, Project.Fps);
            Project.Playhead = Math.Min(GetProjectDuration(), Project.Playhead + frameSec);
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            SetStatus($"Atalho [→]: +{stepFrames} quadro(s) -> {PlayheadOverlayCanvas.FormatPlayheadSmpte(Project.Playhead, Project.Fps, RustInterop, _playheadOverlay.ForceDropFrame)} (Blink Timer resetado -> Opacity 1.0)");
        }
        else if (e.Key == Key.Home)
        {
            e.Handled = true;
            Project.Playhead = 0.0;
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            SetStatus($"Atalho [Home]: Início da Timeline -> {PlayheadOverlayCanvas.FormatPlayheadSmpte(Project.Playhead, Project.Fps, RustInterop, _playheadOverlay.ForceDropFrame)}");
        }
        else if (e.Key == Key.End)
        {
            e.Handled = true;
            Project.Playhead = GetProjectDuration();
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            SetStatus($"Atalho [End]: Fim da Timeline -> {PlayheadOverlayCanvas.FormatPlayheadSmpte(Project.Playhead, Project.Fps, RustInterop, _playheadOverlay.ForceDropFrame)}");
        }
        else if (e.Key == Key.J)
        {
            e.Handled = true;
            Project.Playhead = Math.Max(0, Project.Playhead - 1.0);
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            _timelineControl.InvalidateVisual();
            SetStatus($"Atalho Neoveg [J]: Retroceder 1.0s -> {RustInterop.FormatSmpte(Project.Playhead, Project.Fps)}");
        }
        else if (e.Key == Key.L)
        {
            e.Handled = true;
            Project.Playhead = Math.Min(GetProjectDuration(), Project.Playhead + 1.0);
            _playheadOverlay.NotifyKeyboardCursorMove();
            UpdateTimecodeAndPreview();
            _timelineControl.InvalidateVisual();
            SetStatus($"Atalho Neoveg [L]: Avançar 1.0s -> {RustInterop.FormatSmpte(Project.Playhead, Project.Fps)}");
        }
        else if (e.Key == Key.I)
        {
            e.Handled = true;
            Project.LoopStart = Math.Round(Project.Playhead, 3);
            if (Project.LoopEnd <= Project.LoopStart)
                Project.LoopEnd = Math.Round(Project.LoopStart + 2.5, 3);
            _timelineControl.InvalidateVisual();
            SetStatus($"Atalho Neoveg [I]: In Point marcado em {RustInterop.FormatSmpte(Project.LoopStart, Project.Fps)}");
        }
        else if (e.Key == Key.O)
        {
            e.Handled = true;
            Project.LoopEnd = Math.Max(Project.LoopStart + 0.25, Math.Round(Project.Playhead, 3));
            _timelineControl.InvalidateVisual();
            SetStatus($"Atalho Neoveg [O]: Out Point marcado em {RustInterop.FormatSmpte(Project.LoopEnd, Project.Fps)}");
        }
        else if (e.Key == Key.B && e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            e.Handled = true;
            _ = TriggerDynamicRamPreviewAsync();
        }
        else if (e.Key == Key.M && e.KeyModifiers.HasFlag(KeyModifiers.Shift))
        {
            e.Handled = true;
            _ = TriggerSelectiveDiskPrerenderAsync();
        }
        else if (e.Key == Key.M)
        {
            e.Handled = true;
            AddMarkerAtPlayhead();
        }
    }

    public async Task TriggerDynamicRamPreviewAsync()
    {
        await PrerenderService.BuildDynamicRamPreviewAsync(
            Project,
            msg => SetStatus(msg),
            () => _timelineControl.InvalidateVisual());
    }

    public async Task TriggerSelectiveDiskPrerenderAsync()
    {
        await PrerenderService.RunSelectiveDiskPrerenderAsync(
            Project,
            PythonService,
            msg => SetStatus(msg),
            () => _timelineControl.InvalidateVisual());
    }

    public async Task TriggerWhisperSubtitlesGenerationAsync()
    {
        var res = await SubtitleService.TranscribeAndGenerateTimelineEventsAsync(
            Project,
            Localization.CurrentLanguageCode,
            "ggml-base.bin",
            (_, msg) => Dispatcher.UIThread.Post(() => SetStatus(msg)));

        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        _previewControl.InvalidateVisual();
        SetStatus(res.Summary);
    }

    public void ApplyHistoricalTheme(NeovegHistoryTheme theme)
    {
        CurrentTheme = theme;
        ThemeManager.SetActiveTheme(theme);
        _timelineControl.ActiveTheme = theme;

        Background = new SolidColorBrush(Color.Parse(theme.WindowBg));
        Foreground = new SolidColorBrush(Color.Parse(theme.TextPrimary));

        if (_topMenu != null)
        {
            _topMenu.Background = new SolidColorBrush(Color.Parse(theme.MenuBg));
            _topMenu.Foreground = new SolidColorBrush(Color.Parse(theme.TextPrimary));
            _topMenu.Padding = NeovegThemeManager.GetMenuPadding(theme);
        }
        if (_mainToolbarBorder != null)
        {
            _mainToolbarBorder.Background = NeovegThemeManager.CreateToolbarBrush(theme);
            _mainToolbarBorder.BorderBrush = new SolidColorBrush(Color.Parse(theme.DockBorder));
            _mainToolbarBorder.BorderThickness = theme.EraNumber == 1 ? new Thickness(0, 1, 0, 2) : new Thickness(0, 0, 0, 1);
        }
        if (_statusBarBorder != null)
        {
            _statusBarBorder.Background = new SolidColorBrush(Color.Parse(theme.StatusBarBg));
            _statusBarBorder.BorderBrush = new SolidColorBrush(Color.Parse(theme.DockBorder));
        }
        _bigSmpteReadout.Foreground = new SolidColorBrush(Color.Parse(theme.TimecodeFg));
        _engineStatusPill.Foreground = new SolidColorBrush(Color.Parse(theme.AccentSecondary));
        _statusFooterText.Foreground = new SolidColorBrush(Color.Parse(theme.TextPrimary));

        foreach (var hdr in _dockHeaderBorders)
        {
            hdr.Background = NeovegThemeManager.CreateDockHeaderBrush(theme);
            hdr.BorderBrush = new SolidColorBrush(Color.Parse(theme.DockBorder));
        }

        foreach (var pnl in _dockPanelBorders)
        {
            pnl.Background = new SolidColorBrush(Color.Parse(theme.DockPanelBg));
            pnl.BorderBrush = new SolidColorBrush(Color.Parse(theme.DockBorder));
            pnl.BorderThickness = NeovegThemeManager.GetDockBorderThickness(theme);
            if (pnl is DetachableDockPanel dock)
                dock.HeaderTitle.Foreground = new SolidColorBrush(Color.Parse(theme.TextPrimary));
        }

        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        _previewControl.InvalidateVisual();
        _scopeControl.InvalidateVisual();
        Localization.SelectedThemeId = theme.Id;
        Localization.SaveSettings();
        SetStatus($"Tema Histórico aplicado: {theme.Name} ({theme.Vendor} • {theme.Years})");
    }

    public void RefreshLocalizedUI()
    {
        var loc = Localization;
        Title = $"{Project.Name} - Neoveg";

        PopulateTopMenuBarItems();

        if (_tabProjectMedia != null) _tabProjectMedia.Header = loc.T("Tab.ProjectMedia", "Project Media");
        if (_tabVideoFx != null) _tabVideoFx.Header = loc.T("Tab.VideoFX", "Video FX");
        if (_tabAudioFx != null) _tabAudioFx.Header = loc.T("Tab.AudioFX", "Audio FX");
        if (_tabTransitions != null) _tabTransitions.Header = loc.T("Tab.Transitions", "Transições");
        if (_tabGenerators != null) _tabGenerators.Header = loc.T("Tab.Generators", "Geradores");

        if (_btnTlSave != null) ToolTip.SetTip(_btnTlSave, loc.T("Toolbar.Tooltip.Save", "Salvar Projeto (Ctrl+S)"));
        if (_btnTlImport != null) ToolTip.SetTip(_btnTlImport, loc.T("Toolbar.Tooltip.Import", "Importar Mídia"));
        if (_btnTlSplit != null) ToolTip.SetTip(_btnTlSplit, loc.T("Toolbar.Tooltip.Split", "Dividir Evento no Cursor (S)"));
        if (_btnTlGroup != null) ToolTip.SetTip(_btnTlGroup, loc.T("Toolbar.Tooltip.GroupAV", "Agrupar (G)"));
        if (_btnTlSettings != null) ToolTip.SetTip(_btnTlSettings, loc.T("Toolbar.Tooltip.Settings", "Preferências"));
        if (_btnTlPopout != null) ToolTip.SetTip(_btnTlPopout, loc.T("Toolbar.Tooltip.Popout", "Visualizar em Monitor Externo"));
        if (_btnTlRender != null) ToolTip.SetTip(_btnTlRender, loc.T("Toolbar.Tooltip.Render", "Renderizar Como... (Python Isolated FFmpeg)"));

        RefreshInspectorUI();
        RebuildTrackHeadersUI();
        _timelineControl.InvalidateVisual();
        _previewControl.InvalidateVisual();
    }

    private void OpenHistoricalThemesGalleryDialog()
    {
        var stack = new StackPanel { Spacing = 10, Margin = new Thickness(14) };
        stack.Children.Add(new TextBlock
        {
            Text = "ACERVO HISTÓRICO DE TEMAS, PALETAS E ÍCONES DO NEOVEG (1999–2026)",
            FontSize = 14,
            FontWeight = FontWeight.Black,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });
        stack.Children.Add(new TextBlock
        {
            Text = $"Diretório de Ativos: {Path.Combine(_rootDir, "assets", "neoveg_history_themes")}",
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 10.5,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        });

        foreach (var th in HistoryThemes)
        {
            var captured = th;
            var cardGrid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto")
            };

            var swatchStack = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Margin = new Thickness(0, 0, 12, 0),
                VerticalAlignment = VerticalAlignment.Center
            };
            foreach (string hex in new[] { captured.WindowBg, captured.TrackVideoBg, captured.TrackAudioBg, captured.AccentPrimary, captured.AccentSecondary, captured.TimecodeFg })
            {
                swatchStack.Children.Add(new Border
                {
                    Width = 22,
                    Height = 28,
                    Background = new SolidColorBrush(Color.Parse(hex)),
                    BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
                    BorderThickness = new Thickness(1)
                });
            }
            Grid.SetColumn(swatchStack, 0);
            cardGrid.Children.Add(swatchStack);

            var infoStack = new StackPanel
            {
                Spacing = 2,
                Children =
                {
                    new TextBlock
                    {
                        Text = $"Era {captured.EraNumber} ({captured.Years}): {captured.Name}",
                        FontSize = 11.5,
                        FontWeight = FontWeight.Bold,
                        Foreground = new SolidColorBrush(Color.Parse("#f8fafc"))
                    },
                    new TextBlock
                    {
                        Text = string.IsNullOrWhiteSpace(captured.Description)
                            ? $"{captured.Vendor} • Pasta: assets/neoveg_history_themes/{captured.EraFolder}"
                            : captured.Description,
                        FontSize = 10,
                        Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
                        TextWrapping = TextWrapping.Wrap
                    },
                    new TextBlock
                    {
                        Text = $"Ativos: {captured.LogoSvgPath} • {captured.IconsSvgPath} • {captured.SwatchPngPath}",
                        FontFamily = new FontFamily("Consolas, monospace"),
                        FontSize = 9.5,
                        Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
                    }
                }
            };
            Grid.SetColumn(infoStack, 1);
            cardGrid.Children.Add(infoStack);

            var btnApply = CreateSquareTextButton("Aplicar Tema", captured.AccentPrimary, "#ffffff");
            btnApply.VerticalAlignment = VerticalAlignment.Center;
            btnApply.Margin = new Thickness(10, 0, 0, 0);
            btnApply.Click += (_, _) => ApplyHistoricalTheme(captured);
            Grid.SetColumn(btnApply, 2);
            cardGrid.Children.Add(btnApply);

            stack.Children.Add(new Border
            {
                Background = new SolidColorBrush(Color.Parse("#161d2a")),
                BorderBrush = new SolidColorBrush(Color.Parse(captured.AccentPrimary)),
                BorderThickness = new Thickness(3, 1, 1, 1),
                Padding = new Thickness(10, 8),
                Child = cardGrid
            });
        }

        var galleryWin = new Window
        {
            Title = "Galeria Histórica do Neoveg (1999–2026) — Classic Industrial • Classic NLE • Modern NLE • Contemporary NLE",
            Width = 880,
            Height = 620,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#0f141d")),
            Content = new ScrollViewer { Content = stack }
        };
        galleryWin.Show(this);
    }

    private void SetStatus(string message) => _statusFooterText.Text = message;

    private DetachableDockPanel WrapInDockPanel(string title, Control body)
    {
        var dock = new DetachableDockPanel(title, body)
        {
            Background = new SolidColorBrush(Color.Parse(CurrentTheme.DockPanelBg)),
            BorderBrush = new SolidColorBrush(Color.Parse(CurrentTheme.DockBorder)),
            BorderThickness = NeovegThemeManager.GetDockBorderThickness(CurrentTheme)
        };
        dock.HeaderBorder.Background = NeovegThemeManager.CreateDockHeaderBrush(CurrentTheme);
        dock.HeaderBorder.BorderBrush = new SolidColorBrush(Color.Parse(CurrentTheme.DockBorder));
        dock.HeaderTitle.Foreground = new SolidColorBrush(Color.Parse(CurrentTheme.TextPrimary));
        _dockHeaderBorders.Add(dock.HeaderBorder);
        _dockPanelBorders.Add(dock);
        return dock;
    }

    private static GridSplitter CreateVerticalSplitter(int col)
    {
        var sp = new GridSplitter
        {
            Background = new SolidColorBrush(Color.Parse("#263042")),
            ResizeDirection = GridResizeDirection.Columns
        };
        Grid.SetColumn(sp, col);
        return sp;
    }

    private static Border CreateCatalogCard(string badge, string title, string subtitle, string actionLabel, Action onClick)
    {
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto")
        };

        var badgeBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#1e3a8a")),
            Padding = new Thickness(5, 2),
            Margin = new Thickness(0, 0, 8, 0),
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock
            {
                Text = badge,
                FontSize = 8.5,
                FontWeight = FontWeight.Bold,
                Foreground = Brushes.White
            }
        };
        Grid.SetColumn(badgeBorder, 0);

        var info = new StackPanel
        {
            Spacing = 1,
            Children =
            {
                new TextBlock { Text = title, FontSize = 10.5, FontWeight = FontWeight.Bold, Foreground = new SolidColorBrush(Color.Parse("#f8fafc")), TextTrimming = TextTrimming.CharacterEllipsis },
                new TextBlock { Text = subtitle, FontSize = 9.5, Foreground = new SolidColorBrush(Color.Parse("#94a3b8")), TextTrimming = TextTrimming.CharacterEllipsis }
            }
        };
        Grid.SetColumn(info, 1);

        var btn = NeovegIcons.CreateButton(NeovegIcons.Plus, actionLabel, onClick, "#2563eb", "#ffffff", size: 22);
        btn.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(btn, 2);

        grid.Children.Add(badgeBorder);
        grid.Children.Add(info);
        grid.Children.Add(btn);

        return new Border
        {
            Background = new SolidColorBrush(Color.Parse("#181f2c")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2d3748")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(7, 5),
            Child = grid
        };
    }

    private static Button CreateSquareTextButton(string text, string bgHex, string fgHex)
    {
        StreamGeometry glyph = text.Contains("Remover", StringComparison.OrdinalIgnoreCase) || text.Contains("Delete", StringComparison.OrdinalIgnoreCase)
            ? NeovegIcons.Delete
            : text.Contains("Cor", StringComparison.OrdinalIgnoreCase) || text.Contains("Tema", StringComparison.OrdinalIgnoreCase)
                ? NeovegIcons.TrackColor
                : NeovegIcons.Prerender;
        return NeovegIcons.CreateButton(glyph, text, null, bgHex, fgHex, size: 24);
    }

    private static Button CreateMiniTrackButton(StreamGeometry icon, string tooltip, string bgHex, string fgHex)
    {
        var btn = NeovegIcons.CreateButton(icon, tooltip, null, bgHex, fgHex, size: 20);
        btn.Width = 22;
        return btn;
    }

    private static MenuItem CreateMenuItem(string header, Action action, string? inputGesture = null)
    {
        var item = new MenuItem { Header = header };
        if (inputGesture != null)
            item.InputGesture = KeyGesture.Parse(inputGesture);
        item.Click += (_, _) => action();
        return item;
    }
}
