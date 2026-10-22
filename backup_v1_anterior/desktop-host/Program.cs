using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace OpenVegasDesktop;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new VegasStudioWindow());
    }
}

public sealed class DarkMenuRenderer : ToolStripProfessionalRenderer
{
    public DarkMenuRenderer() : base(new DarkColorTable()) { }

    protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
    {
        e.TextColor = Color.FromArgb(241, 245, 249);
        base.OnRenderItemText(e);
    }
}

public sealed class DarkColorTable : ProfessionalColorTable
{
    public override Color MenuStripGradientBegin => Color.FromArgb(20, 23, 30);
    public override Color MenuStripGradientEnd => Color.FromArgb(20, 23, 30);
    public override Color ToolStripDropDownBackground => Color.FromArgb(28, 32, 41);
    public override Color MenuItemSelected => Color.FromArgb(37, 99, 235);
    public override Color MenuItemSelectedGradientBegin => Color.FromArgb(37, 99, 235);
    public override Color MenuItemSelectedGradientEnd => Color.FromArgb(29, 78, 216);
    public override Color MenuItemPressedGradientBegin => Color.FromArgb(30, 41, 59);
    public override Color MenuItemPressedGradientEnd => Color.FromArgb(30, 41, 59);
    public override Color MenuBorder => Color.FromArgb(59, 66, 82);
    public override Color MenuItemBorder => Color.FromArgb(96, 165, 250);
    public override Color ImageMarginGradientBegin => Color.FromArgb(28, 32, 41);
    public override Color ImageMarginGradientMiddle => Color.FromArgb(28, 32, 41);
    public override Color ImageMarginGradientEnd => Color.FromArgb(28, 32, 41);
    public override Color SeparatorDark => Color.FromArgb(46, 52, 64);
    public override Color SeparatorLight => Color.FromArgb(46, 52, 64);
}

public sealed class VegasStudioWindow : Form
{
    [DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

    private readonly WebView2 _webView;
    private CoreWebView2Environment? _webViewEnv;
    private Rectangle? _pendingPopoutBounds;
    private readonly Panel _splashPanel;
    private readonly Label _splashLabel;
    private Process? _backendProcess;
    private readonly string _rootDir;
    private readonly int _port = 8765;
    private bool _isFullscreen = false;
    private FormWindowState _prevWindowState;
    private FormBorderStyle _prevBorderStyle;

    public VegasStudioWindow()
    {
        _rootDir = ResolveWorkspaceRoot();

        Text = "OPEN VEGAS PRO 21 — NLE Studio (Rust • Python • TypeScript Native Executable)";
        Width = 1480;
        Height = 900;
        MinimumSize = new Size(1100, 680);
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(15, 17, 21);
        ForeColor = Color.FromArgb(241, 245, 249);
        WindowState = FormWindowState.Maximized;

        string icoPath = Path.Combine(_rootDir, "desktop-host", "OpenVegasPro21.ico");
        if (File.Exists(icoPath))
        {
            try { Icon = new Icon(icoPath); } catch { }
        }

        // Enable Windows 11 Dark Title Bar
        int useDarkMode = 1;
        try { DwmSetWindowAttribute(Handle, 20, ref useDarkMode, sizeof(int)); } catch { }

        MenuStrip menu = BuildNativeMenuStrip();
        MainMenuStrip = menu;

        var workspaceContainer = new Panel
        {
            Dock = DockStyle.Fill,
            BackColor = Color.FromArgb(15, 17, 21),
            Margin = new Padding(0),
            Padding = new Padding(0),
        };

        _splashPanel = new Panel
        {
            Dock = DockStyle.Fill,
            BackColor = Color.FromArgb(15, 17, 21),
        };
        _splashLabel = new Label
        {
            AutoSize = false,
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            Font = new Font("Segoe UI Semibold", 13f),
            ForeColor = Color.FromArgb(147, 197, 253),
            Text = "Inicializando OpenVegas Pro 21 Studio...\nCarregando Rust Core DSP + Python FFmpeg Engine + TypeScript NLE Surface...",
        };
        _splashPanel.Controls.Add(_splashLabel);

        _webView = new WebView2
        {
            Dock = DockStyle.Fill,
            DefaultBackgroundColor = Color.FromArgb(15, 17, 21),
            Visible = false,
        };

        workspaceContainer.Controls.Add(_webView);
        workspaceContainer.Controls.Add(_splashPanel);
        _splashPanel.BringToFront();

        // IMPORTANT WinForms Dock Z-Order:
        // Add Fill container FIRST (index 0), then Top MenuStrip SECOND (index 1)
        // so MenuStrip reserves its top row and workspaceContainer starts strictly below MenuStrip!
        Controls.Add(workspaceContainer);
        Controls.Add(menu);

        Load += async (_, _) => await InitializeStudioAsync();
        FormClosing += (_, _) => StopBackendServer();
    }

    private static string ResolveWorkspaceRoot()
    {
        string baseDir = AppContext.BaseDirectory;
        DirectoryInfo? dir = new DirectoryInfo(baseDir);
        while (dir != null)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "python-backend")) &&
                Directory.Exists(Path.Combine(dir.FullName, "frontend")))
            {
                return dir.FullName;
            }
            dir = dir.Parent;
        }
        return Directory.GetCurrentDirectory();
    }

    private MenuStrip BuildNativeMenuStrip()
    {
        var menu = new MenuStrip
        {
            Dock = DockStyle.Top,
            BackColor = Color.FromArgb(20, 23, 30),
            ForeColor = Color.FromArgb(241, 245, 249),
            Renderer = new DarkMenuRenderer(),
            Padding = new Padding(6, 2, 0, 2),
        };

        // ARQUIVO
        var fileMenu = new ToolStripMenuItem("Arquivo");
        var itemWelcome = new ToolStripMenuItem("Janela de Boas-Vindas / Projetos / Configurações Avançadas...", null, async (_, _) =>
        {
            await ExecJsAsync("window.openVegasApp?.openWelcomeModal();");
        });
        var itemNewEmpty = new ToolStripMenuItem("Novo Projeto Vazio", null, async (_, _) =>
        {
            await ExecJsAsync("window.openVegasApp?.createEmptyProjectFlow();");
        })
        {
            ShortcutKeys = Keys.Control | Keys.N
        };
        var itemLoadDemo = new ToolStripMenuItem("Abrir Projeto de Demonstração (OpenVegas_Broadcast_Demo.veg)", null, async (_, _) =>
        {
            await ExecJsAsync("window.openVegasApp?.loadDemoProjectFlow();");
        });
        var itemOpenVeg = new ToolStripMenuItem("Abrir Projeto (.veg)...", null, async (_, _) => await OpenProjectDialogAsync())
        {
            ShortcutKeys = Keys.Control | Keys.O
        };
        var itemSaveVeg = new ToolStripMenuItem("Salvar Projeto (.veg)...", null, async (_, _) => await SaveProjectDialogAsync())
        {
            ShortcutKeys = Keys.Control | Keys.S
        };
        var itemImportMedia = new ToolStripMenuItem("Importar Mídia (MP4 / WAV / MP3)...", null, async (_, _) => await ImportNativeMediaDialogAsync())
        {
            ShortcutKeys = Keys.Control | Keys.I
        };
        var itemRenderAs = new ToolStripMenuItem("Render As... (FFmpeg + Frei0r + LADSPA)", null, async (_, _) =>
        {
            await ExecJsAsync("window.openVegasApp?.openRenderModal();");
        })
        {
            ShortcutKeys = Keys.Control | Keys.M
        };
        var itemExit = new ToolStripMenuItem("Sair", null, (_, _) => Close())
        {
            ShortcutKeys = Keys.Alt | Keys.F4
        };
        fileMenu.DropDownItems.AddRange(new ToolStripItem[]
        {
            itemWelcome,
            itemNewEmpty,
            itemLoadDemo,
            new ToolStripSeparator(),
            itemOpenVeg,
            itemSaveVeg,
            new ToolStripSeparator(),
            itemImportMedia,
            itemRenderAs,
            new ToolStripSeparator(),
            itemExit
        });

        // EDITAR
        var editMenu = new ToolStripMenuItem("Editar");
        editMenu.DropDownItems.Add(new ToolStripMenuItem("Play / Pause (Space)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.togglePlayback();")));
        editMenu.DropDownItems.Add(new ToolStripMenuItem("Split Event at Playhead (S)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.splitAtPlayhead();")));
        editMenu.DropDownItems.Add(new ToolStripMenuItem("Excluir Evento Selecionado (Del)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.deleteSelectedClip();")));

        // INSERIR
        var insertMenu = new ToolStripMenuItem("Inserir");
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Nova Faixa de Vídeo", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.addTrack('video');")));
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Nova Faixa de Áudio", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.addTrack('audio');")));
        insertMenu.DropDownItems.Add(new ToolStripSeparator());
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Agrupar Faixas de Vídeo + Áudio (Track Group V+A)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.groupAVTracksFlow();")));
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Desagrupar Faixas de Vídeo / Áudio", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.ungroupTracksFlow();")));
        insertMenu.DropDownItems.Add(new ToolStripSeparator());
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Inserir Marker no Cursor (M)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.insertMarkerAtPlayhead();")));
        insertMenu.DropDownItems.Add(new ToolStripMenuItem("Inserir Region no Loop (R)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.insertRegionAtLoop();")));

        // JANELAS / LAYOUT & MULTI-MONITOR
        var winMenu = new ToolStripMenuItem("Janelas");
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Destacar Video Preview para Outro Monitor (Janela OS Nativa)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.popoutPanelToMonitor('pane-preview');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Destacar Media Pool & FX para Outro Monitor (Janela OS Nativa)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.popoutPanelToMonitor('pane-media');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Destacar Event Pan/Crop & Inspector para Outro Monitor", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.popoutPanelToMonitor('pane-inspector');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Destacar Master Mixer & Scopes para Outro Monitor", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.popoutPanelToMonitor('pane-mixer');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Destacar Timeline Multi-Track para Outro Monitor", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.popoutPanelToMonitor('pane-timeline');")));
        winMenu.DropDownItems.Add(new ToolStripSeparator());
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar Todas as Janelas Internas (Modo Flutuante Livre)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.floatAllPanels();")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Acoplar Todas (Restaurar Layout Padrão Qt/VEGAS)", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.resetDefaultLayout();")));
        winMenu.DropDownItems.Add(new ToolStripSeparator());
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar / Acoplar: Project Media & FX", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.toggleFloat('pane-media');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar / Acoplar: Event Pan/Crop & Inspector", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.toggleFloat('pane-inspector');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar / Acoplar: Video Preview Monitor", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.toggleFloat('pane-preview');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar / Acoplar: Master Mixer & RGB Scopes", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.toggleFloat('pane-mixer');")));
        winMenu.DropDownItems.Add(new ToolStripMenuItem("Soltar / Acoplar: Multi-Track Timeline", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.dockManager?.toggleFloat('pane-timeline');")));

        // FERRAMENTAS & CONFIGURAÇÕES
        var toolsMenu = new ToolStripMenuItem("Ferramentas");
        toolsMenu.DropDownItems.Add(new ToolStripMenuItem("Abrir Configurações Avançadas em Outra Janela...", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.openAdvancedSettingsWindow();")));
        toolsMenu.DropDownItems.Add(new ToolStripMenuItem("Aba de Configurações Avançadas do Projeto e Estúdio...", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.openSettingsTab();")));
        toolsMenu.DropDownItems.Add(new ToolStripSeparator());
        toolsMenu.DropDownItems.Add(new ToolStripMenuItem("Diagnóstico e Correção de Problemas do VEGAS + log_erros.txt...", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.openDiagnosticsModal();")));
        toolsMenu.DropDownItems.Add(new ToolStripMenuItem("Vegas Python Scripting Console...", null, async (_, _) =>
            await ExecJsAsync("window.openVegasApp?.openScriptingModal();")));
        toolsMenu.DropDownItems.Add(new ToolStripMenuItem("Abrir Pasta de Vídeos Renderizados (renders/)", null, (_, _) =>
        {
            string rendersPath = Path.Combine(_rootDir, "renders");
            Directory.CreateDirectory(rendersPath);
            Process.Start(new ProcessStartInfo("explorer.exe", rendersPath) { UseShellExecute = true });
        }));

        // EXIBIR
        var viewMenu = new ToolStripMenuItem("Exibir");
        var itemFullscreen = new ToolStripMenuItem("Tela Cheia (Fullscreen)", null, (_, _) => ToggleFullscreen())
        {
            ShortcutKeys = Keys.F11
        };
        var itemReload = new ToolStripMenuItem("Recarregar Studio UI", null, (_, _) => _webView.Reload())
        {
            ShortcutKeys = Keys.F5
        };
        viewMenu.DropDownItems.AddRange(new ToolStripItem[] { itemFullscreen, itemReload });

        menu.Items.AddRange(new ToolStripItem[] { fileMenu, editMenu, insertMenu, winMenu, toolsMenu, viewMenu });
        return menu;
    }

    private void ToggleFullscreen()
    {
        if (!_isFullscreen)
        {
            _prevWindowState = WindowState;
            _prevBorderStyle = FormBorderStyle;
            FormBorderStyle = FormBorderStyle.None;
            WindowState = FormWindowState.Maximized;
            _isFullscreen = true;
        }
        else
        {
            FormBorderStyle = _prevBorderStyle;
            WindowState = _prevWindowState;
            _isFullscreen = false;
        }
    }

    private async Task InitializeStudioAsync()
    {
        try
        {
            await EnsureBackendServerRunningAsync();

            string userDataDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "OpenVegasPro21",
                "WebView2Data"
            );
            Directory.CreateDirectory(userDataDir);

            _webViewEnv = await CoreWebView2Environment.CreateAsync(null, userDataDir);
            await _webView.EnsureCoreWebView2Async(_webViewEnv);

            _webView.CoreWebView2.Settings.AreDefaultContextMenusEnabled = false;
            _webView.CoreWebView2.Settings.IsStatusBarEnabled = false;
            _webView.CoreWebView2.Settings.IsZoomControlEnabled = false;

            _webView.CoreWebView2.WebMessageReceived += (_, msgArgs) =>
            {
                try
                {
                    using var doc = JsonDocument.Parse(msgArgs.WebMessageAsJson);
                    var root = doc.RootElement;
                    if (root.TryGetProperty("action", out var act) && act.GetString() == "prepare_popout_bounds")
                    {
                        int x = root.TryGetProperty("x", out var px) ? px.GetInt32() : 120;
                        int y = root.TryGetProperty("y", out var py) ? py.GetInt32() : 90;
                        int w = root.TryGetProperty("width", out var pw) ? pw.GetInt32() : 860;
                        int h = root.TryGetProperty("height", out var ph) ? ph.GetInt32() : 560;
                        _pendingPopoutBounds = new Rectangle(x, y, Math.Max(420, w), Math.Max(320, h));
                    }
                }
                catch { }
            };

            _webView.CoreWebView2.NewWindowRequested += async (_, winArgs) =>
            {
                var deferral = winArgs.GetDeferral();
                try
                {
                    await SpawnNativeMultiMonitorPopoutWindowAsync(winArgs);
                }
                catch { }
                finally
                {
                    deferral.Complete();
                }
            };

            _webView.NavigationCompleted += (_, _) =>
            {
                _splashPanel.Visible = false;
                _webView.Visible = true;
                _webView.Focus();
            };

            _webView.CoreWebView2.Navigate($"http://127.0.0.1:{_port}/");
        }
        catch (Exception ex)
        {
            _splashLabel.ForeColor = Color.FromArgb(248, 113, 113);
            _splashLabel.Text = $"Erro ao inicializar o motor nativo WebView2:\n{ex.Message}";
        }
    }

    private async Task SpawnNativeMultiMonitorPopoutWindowAsync(CoreWebView2NewWindowRequestedEventArgs winArgs)
    {
        if (_webViewEnv == null) return;

        var bounds = _pendingPopoutBounds ?? new Rectangle(140, 100, 880, 560);
        _pendingPopoutBounds = null;

        // If the user has a secondary monitor connected and did not drag to a specific secondary coordinate,
        // automatically place the popped-out window on the secondary monitor if available!
        var allScreens = Screen.AllScreens;
        if (allScreens.Length > 1)
        {
            var currentScreen = Screen.FromControl(this);
            foreach (var scr in allScreens)
            {
                if (scr.DeviceName != currentScreen.DeviceName)
                {
                    bool alreadyOnOtherMonitor = scr.WorkingArea.Contains(bounds.Location);
                    if (!alreadyOnOtherMonitor)
                    {
                        bounds.X = scr.WorkingArea.X + 60;
                        bounds.Y = scr.WorkingArea.Y + 60;
                    }
                    break;
                }
            }
        }

        var popForm = new Form
        {
            Text = $"OpenVegas Pro 21 — Multi-Monitor Studio Panel ({winArgs.Uri})",
            StartPosition = FormStartPosition.Manual,
            Location = new Point(bounds.X, bounds.Y),
            Size = new Size(bounds.Width, bounds.Height),
            MinimumSize = new Size(380, 260),
            BackColor = Color.FromArgb(15, 17, 21),
            ForeColor = Color.FromArgb(241, 245, 249),
            Icon = Icon,
        };

        int useDarkMode = 1;
        try { DwmSetWindowAttribute(popForm.Handle, 20, ref useDarkMode, sizeof(int)); } catch { }

        var popWebView = new WebView2
        {
            Dock = DockStyle.Fill,
            DefaultBackgroundColor = Color.FromArgb(15, 17, 21),
        };
        popForm.Controls.Add(popWebView);
        popForm.Show();

        await popWebView.EnsureCoreWebView2Async(_webViewEnv);
        popWebView.CoreWebView2.Settings.AreDefaultContextMenusEnabled = false;
        popWebView.CoreWebView2.Settings.IsStatusBarEnabled = false;
        winArgs.NewWindow = popWebView.CoreWebView2;
        winArgs.Handled = true;
    }

    private async Task EnsureBackendServerRunningAsync()
    {
        using var client = new HttpClient { Timeout = TimeSpan.FromMilliseconds(900) };
        try
        {
            var resp = await client.GetAsync($"http://127.0.0.1:{_port}/api/status");
            if (resp.IsSuccessStatusCode) return;
        }
        catch { }

        string buildMjs = Path.Combine(_rootDir, "frontend", "build.mjs");
        if (File.Exists(buildMjs))
        {
            try
            {
                var nodeProc = Process.Start(new ProcessStartInfo
                {
                    FileName = "node",
                    Arguments = $"\"{buildMjs}\"",
                    WorkingDirectory = _rootDir,
                    CreateNoWindow = true,
                    UseShellExecute = false,
                });
                if (nodeProc != null) await nodeProc.WaitForExitAsync();
            }
            catch { }
        }

        string serverPy = Path.Combine(_rootDir, "python-backend", "server.py");
        var psi = new ProcessStartInfo
        {
            FileName = "python",
            Arguments = $"\"{serverPy}\"",
            WorkingDirectory = _rootDir,
            CreateNoWindow = true,
            UseShellExecute = false,
        };
        psi.Environment["OPENVEGAS_PORT"] = _port.ToString();
        _backendProcess = Process.Start(psi);

        for (int i = 0; i < 32; i++)
        {
            await Task.Delay(250);
            try
            {
                var check = await client.GetAsync($"http://127.0.0.1:{_port}/api/status");
                if (check.IsSuccessStatusCode) return;
            }
            catch { }
        }
    }

    private void StopBackendServer()
    {
        try
        {
            if (_backendProcess != null && !_backendProcess.HasExited)
            {
                _backendProcess.Kill(entireProcessTree: true);
                _backendProcess.Dispose();
            }
        }
        catch { }
    }

    private async Task<string> ExecJsAsync(string script)
    {
        if (_webView.CoreWebView2 == null) return "";
        return await _webView.CoreWebView2.ExecuteScriptAsync(script);
    }

    private async Task SaveProjectDialogAsync()
    {
        if (_webView.CoreWebView2 == null) return;
        using var sfd = new SaveFileDialog
        {
            Title = "Salvar Projeto VEGAS Pro (.veg)",
            Filter = "VEGAS Project (*.veg)|*.veg|JSON Project (*.json)|*.json",
            FileName = "OpenVegas_Broadcast_Master.veg",
            InitialDirectory = _rootDir,
        };
        if (sfd.ShowDialog(this) == DialogResult.OK)
        {
            string rawRes = await ExecJsAsync("window.openVegasApp ? window.openVegasApp.exportProjectJson() : ''");
            string? unquoted = JsonSerializer.Deserialize<string>(rawRes);
            if (!string.IsNullOrEmpty(unquoted))
            {
                await File.WriteAllTextAsync(sfd.FileName, unquoted, Encoding.UTF8);
            }
        }
    }

    private async Task OpenProjectDialogAsync()
    {
        if (_webView.CoreWebView2 == null) return;
        using var ofd = new OpenFileDialog
        {
            Title = "Abrir Projeto VEGAS Pro (.veg)",
            Filter = "VEGAS Project (*.veg;*.json)|*.veg;*.json",
            InitialDirectory = _rootDir,
        };
        if (ofd.ShowDialog(this) == DialogResult.OK)
        {
            string content = await File.ReadAllTextAsync(ofd.FileName, Encoding.UTF8);
            string escaped = JsonSerializer.Serialize(content);
            await ExecJsAsync($"window.openVegasApp?.loadProjectJson({escaped});");
        }
    }

    private async Task ImportNativeMediaDialogAsync()
    {
        using var ofd = new OpenFileDialog
        {
            Title = "Importar Vídeo ou Áudio para o Projeto",
            Filter = "Arquivos de Mídia (*.mp4;*.webm;*.mov;*.wav;*.mp3;*.ogg)|*.mp4;*.webm;*.mov;*.wav;*.mp3;*.ogg|Todos os Arquivos (*.*)|*.*",
            InitialDirectory = Path.Combine(_rootDir, "media"),
        };
        if (ofd.ShowDialog(this) == DialogResult.OK)
        {
            using var client = new HttpClient();
            string payload = JsonSerializer.Serialize(new { filePath = ofd.FileName });
            var resp = await client.PostAsync(
                $"http://127.0.0.1:{_port}/api/media/import-path",
                new StringContent(payload, Encoding.UTF8, "application/json")
            );
            if (resp.IsSuccessStatusCode)
            {
                string body = await resp.Content.ReadAsStringAsync();
                using var doc = JsonDocument.Parse(body);
                if (doc.RootElement.TryGetProperty("asset", out var assetElem))
                {
                    string assetJson = assetElem.GetRawText();
                    await ExecJsAsync(
                        $"if(window.openVegasApp) {{ const a = {assetJson}; a.color = a.type==='audio'?'#0891b2':'#2563eb'; window.openVegasApp.mediaBin.unshift(a); window.openVegasApp.renderLeftDock(); window.openVegasApp.addMediaAssetToTimeline(a); }}"
                    );
                }
            }
        }
    }
}
