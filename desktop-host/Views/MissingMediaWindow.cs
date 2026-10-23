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
using Avalonia.Platform.Storage;
using Neoveg.Core;
using Neoveg.Interop;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.Views;

/// <summary>
/// Modal Missing Media Dialog (`MissingMediaWindow` / `MissingMediaWindow.axaml`) for Neoveg:
/// Triggered automatically when opening a `.veg` project with missing or moved media files in the `MediaPool`.
/// Displays:
/// - Nome do Arquivo (`FileName`)
/// - Caminho Original (`OriginalPath`)
/// - Duração Esperada (`ExpectedDuration`)
/// - Tipo de Mídia (`Vídeo` / `Áudio`)
/// Provides the 4 Neoveg resolution actions:
/// 1. Procurar em uma Pasta... (`Search Directory`) -> 3-stage recursive scan + Same-Directory Auto-Relink + Rust FFI `.sfk` cache regeneration
/// 2. Substituir Arquivo Especificamente... (`Specify New Location`) -> manual file picker + Same-Directory Sibling Auto-Relink
/// 3. Ignorar Este Arquivo (`Ignore`) -> marks selected clip as `IsOffline = true` (red MEDIA OFFLINE frame)
/// 4. Ignorar Todos Ausentes (`Ignore All`) -> marks all unresolved clips as `IsOffline = true` and opens the project immediately
/// </summary>
public sealed class MissingMediaWindow : Window
{
    private readonly string _rootDir;
    private readonly NeovegProject _project;
    private readonly List<MissingMediaEntry> _missingEntries;
    private readonly MediaRelinker _relinker;
    private readonly Action<string> _onProjectReadyToMount;

    private readonly StackPanel _rowsContainer = new() { Spacing = 4 };
    private readonly TextBlock _statusSummaryText;
    private int _selectedIndex;
    private bool _mounted;

    public MissingMediaWindow(
        string rootDir,
        NeovegProject project,
        List<MissingMediaEntry> missingEntries,
        MediaRelinker relinker,
        Action<string> onProjectReadyToMount)
    {
        _rootDir = rootDir;
        _project = project;
        _missingEntries = missingEntries;
        _relinker = relinker;
        _onProjectReadyToMount = onProjectReadyToMount;

        Title = $"Neoveg / Neoveg — Janela de Mídias Ausentes (Missing Media) — {_project.Name}";
        Width = 980;
        Height = 560;
        MinWidth = 820;
        MinHeight = 460;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#11151f"));
        Foreground = new SolidColorBrush(Color.Parse("#f8fafc"));

        _statusSummaryText = new TextBlock
        {
            Text = $"{_missingEntries.Count} arquivo(s) ausente(s) detectado(s) no MediaPool ao abrir \"{_project.Name}\". Selecione uma opção abaixo.",
            FontSize = 11.5,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(Color.Parse("#facc15"))
        };

        Content = BuildMainLayout();
        RebuildMissingFilesTable();

        Closed += (_, _) =>
        {
            if (!_mounted)
            {
                FinalizeAndMountTimeline("Projeto carregado com resolução de mídias ausentes.");
            }
        };
    }

    private Control BuildMainLayout()
    {
        var rootGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,Auto,Auto"),
            Margin = new Thickness(16)
        };

        // 1. Header Warning Banner
        var bannerBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#1e1b2e")),
            BorderBrush = new SolidColorBrush(Color.Parse("#dc2626")),
            BorderThickness = new Thickness(4, 1, 1, 1),
            Padding = new Thickness(12, 10),
            Margin = new Thickness(0, 0, 0, 12),
            Child = new StackPanel
            {
                Spacing = 4,
                Children =
                {
                    new TextBlock
                    {
                        Text = "⚠ MÍDIAS AUSENTES DETECTADAS NO PROJETO (MISSING MEDIA DIALOG)",
                        FontSize = 13.5,
                        FontWeight = FontWeight.Black,
                        Foreground = new SolidColorBrush(Color.Parse("#f87171"))
                    },
                    new TextBlock
                    {
                        Text = "Um ou mais arquivos listados no MediaPool foram movidos, renomeados ou estão em outro diretório. Utilize o motor Neoveg.Core.MediaRelinker (Busca Recursiva em 3 Etapas: Nome Exato, Fuzzy Match de Câmera e Validação FFprobe/Rust FFI) para reconectá-los ou marque-os como Mídia Offline.",
                        FontSize = 11,
                        Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
                        TextWrapping = TextWrapping.Wrap
                    }
                }
            }
        };
        Grid.SetRow(bannerBorder, 0);
        rootGrid.Children.Add(bannerBorder);

        // 2. Missing Files Table (Nome do Arquivo | Caminho Original | Duração Esperada | Tipo de Mídia | Status / Novo Caminho)
        var tableOuter = new Grid
        {
            RowDefinitions = new RowDefinitions("30,*")
        };

        var headerRow = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("1.2*,1.8*,0.75*,0.75*,1.6*"),
            Background = new SolidColorBrush(Color.Parse("#1e293b"))
        };
        headerRow.Children.Add(CreateHeaderCell("Nome do Arquivo", 0));
        headerRow.Children.Add(CreateHeaderCell("Caminho Original", 1));
        headerRow.Children.Add(CreateHeaderCell("Duração Esperada", 2));
        headerRow.Children.Add(CreateHeaderCell("Tipo de Mídia (Vídeo/Áudio)", 3));
        headerRow.Children.Add(CreateHeaderCell("Status / Vínculo (MediaRelinker)", 4));

        Grid.SetRow(headerRow, 0);
        tableOuter.Children.Add(headerRow);

        var scroll = new ScrollViewer
        {
            Background = new SolidColorBrush(Color.Parse("#0d1119")),
            Padding = new Thickness(4),
            Content = _rowsContainer
        };
        Grid.SetRow(scroll, 1);
        tableOuter.Children.Add(scroll);

        var tableBorder = new Border
        {
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            BorderThickness = new Thickness(1),
            Child = tableOuter
        };
        Grid.SetRow(tableBorder, 1);
        rootGrid.Children.Add(tableBorder);

        // 3. Status Summary Bar
        var statusBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#151c28")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(10, 7),
            Margin = new Thickness(0, 10, 0, 10),
            Child = _statusSummaryText
        };
        Grid.SetRow(statusBorder, 2);
        rootGrid.Children.Add(statusBorder);

        // 4. Action Buttons Bar (4 Required Neoveg Actions + Quick Workspace Scan + Finish Button)
        var actionsStack = new StackPanel
        {
            Spacing = 8
        };

        var primaryActionsRow = new WrapPanel
        {
            Orientation = Orientation.Horizontal
        };

        var btnSearchDir = CreateActionButton(
            "1. Procurar em uma Pasta... (Search Directory)",
            "#2563eb",
            "#ffffff",
            async () => await OnSearchDirectoryClickedAsync());

        var btnQuickMediaScan = CreateActionButton(
            "⚡ Varredura Automática 3 Etapas em media/ (Exact + Fuzzy + FFprobe)",
            "#0e7490",
            "#ffffff",
            async () => await RunRelinkScanOnFolderAsync(Path.Combine(_rootDir, "media")));

        var btnSpecifyFile = CreateActionButton(
            "2. Substituir Arquivo Especificamente... (Specify New Location)",
            "#1e293b",
            "#38bdf8",
            async () => await OnSpecifyNewLocationClickedAsync());

        var btnIgnoreOne = CreateActionButton(
            "3. Ignorar Este Arquivo (Ignore)",
            "#7f1d1d",
            "#fecaca",
            OnIgnoreSelectedClicked);

        var btnIgnoreAll = CreateActionButton(
            "4. Ignorar Todos Ausentes (Ignore All)",
            "#991b1b",
            "#ffffff",
            OnIgnoreAllClicked);

        var btnFinish = CreateActionButton(
            "✓ Concluir e Montar Timeline",
            "#16a34a",
            "#ffffff",
            () => FinalizeAndMountTimeline("Mídias relinkadas e cacheadas (.sfk Rust FFI) com sucesso!"));

        primaryActionsRow.Children.Add(btnSearchDir);
        primaryActionsRow.Children.Add(btnQuickMediaScan);
        primaryActionsRow.Children.Add(btnSpecifyFile);
        primaryActionsRow.Children.Add(btnIgnoreOne);
        primaryActionsRow.Children.Add(btnIgnoreAll);
        primaryActionsRow.Children.Add(btnFinish);

        actionsStack.Children.Add(primaryActionsRow);
        Grid.SetRow(actionsStack, 3);
        rootGrid.Children.Add(actionsStack);

        return rootGrid;
    }

    private void RebuildMissingFilesTable()
    {
        _rowsContainer.Children.Clear();

        for (int i = 0; i < _missingEntries.Count; i++)
        {
            int idx = i;
            var entry = _missingEntries[i];
            bool isSelected = idx == _selectedIndex;

            string statusHex = entry.Status switch
            {
                "Relinked" => "#4ade80",
                "IgnoredOffline" => "#f87171",
                _ => "#facc15"
            };

            string statusLabel = entry.Status switch
            {
                "Relinked" => $"✓ VINCULADO ({entry.MatchStage})\n→ {entry.ResolvedPath}",
                "IgnoredOffline" => "✖ MÍDIA OFFLINE (Quadro Vermelho na Timeline)",
                _ => $"⚠ AUSENTE ({entry.MatchStage})"
            };

            var rowGrid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("1.2*,1.8*,0.75*,0.75*,1.6*")
            };

            rowGrid.Children.Add(CreateCellText(entry.FileName, 0, "#f8fafc", FontWeight.Bold));
            rowGrid.Children.Add(CreateCellText(entry.OriginalPath, 1, "#94a3b8", FontWeight.Normal));
            rowGrid.Children.Add(CreateCellText($"{entry.ExpectedDuration:0.00}s", 2, "#e2e8f0", FontWeight.SemiBold));
            rowGrid.Children.Add(CreateCellText(
                entry.MediaType.Equals("audio", StringComparison.OrdinalIgnoreCase) ? "Áudio (Audio)" : "Vídeo (Video)",
                3,
                entry.MediaType.Equals("audio", StringComparison.OrdinalIgnoreCase) ? "#fda4af" : "#38bdf8",
                FontWeight.Bold));
            rowGrid.Children.Add(CreateCellText(statusLabel, 4, statusHex, FontWeight.SemiBold));

            var rowBorder = new Border
            {
                Background = new SolidColorBrush(Color.Parse(isSelected ? "#1e293b" : "#131924")),
                BorderBrush = new SolidColorBrush(Color.Parse(isSelected ? "#38bdf8" : "#263042")),
                BorderThickness = new Thickness(isSelected ? 1.5 : 1),
                Padding = new Thickness(8, 6),
                Cursor = new Cursor(StandardCursorType.Hand),
                Child = rowGrid
            };
            rowBorder.PointerPressed += (_, _) =>
            {
                _selectedIndex = idx;
                RebuildMissingFilesTable();
            };

            _rowsContainer.Children.Add(rowBorder);
        }

        int relinked = _missingEntries.Count(e => e.Status == "Relinked");
        int offline = _missingEntries.Count(e => e.Status == "IgnoredOffline");
        int pending = _missingEntries.Count - relinked - offline;
        _statusSummaryText.Text =
            $"Resumo MediaRelinker: {relinked} reconectado(s) (.sfk Rust FFI atualizado) • {offline} marcado(s) como Offline • {pending} pendente(s).";
    }

    private async Task OnSearchDirectoryClickedAsync()
    {
        var folders = await StorageProvider.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Selecione a pasta ou unidade para varredura recursiva de mídias ausentes",
            AllowMultiple = false
        });

        string? pickedDir = folders.FirstOrDefault()?.TryGetLocalPath();
        if (string.IsNullOrWhiteSpace(pickedDir))
        {
            pickedDir = Path.Combine(_rootDir, "media");
        }

        await RunRelinkScanOnFolderAsync(pickedDir);
    }

    private async Task RunRelinkScanOnFolderAsync(string folderPath)
    {
        _statusSummaryText.Text = $"Varrendo recursivamente \"{folderPath}\" (Etapa 1: Nome Exato • Etapa 2: Fuzzy Match • Etapa 3: FFprobe/Rust FFI)...";
        int found = await _relinker.SearchDirectoryAndRelinkAsync(folderPath, _missingEntries);
        RebuildMissingFilesTable();

        if (_missingEntries.All(e => e.Status == "Relinked"))
        {
            _statusSummaryText.Text =
                $"✓ Todas as {_missingEntries.Count} mídias ausentes foram localizadas, validadas via FFprobe e tiveram o cache .sfk regenerado em Rust FFI!";
        }
        else
        {
            _statusSummaryText.Text =
                $"Varredura concluída em \"{folderPath}\": {found} mídia(s) reconectada(s) (incluindo Auto-Relink de mesmo diretório).";
        }
    }

    private async Task OnSpecifyNewLocationClickedAsync()
    {
        if (_missingEntries.Count == 0) return;
        _selectedIndex = Math.Clamp(_selectedIndex, 0, _missingEntries.Count - 1);
        var currentEntry = _missingEntries[_selectedIndex];

        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = $"Substituir especificamente: {currentEntry.FileName}",
            AllowMultiple = false
        });

        string? pickedFile = files.FirstOrDefault()?.TryGetLocalPath();
        if (!string.IsNullOrWhiteSpace(pickedFile) && File.Exists(pickedFile))
        {
            int totalUpdated = _relinker.SpecifyManualLocationAndAutoRelinkSiblings(currentEntry, pickedFile, _missingEntries);
            RebuildMissingFilesTable();
            _statusSummaryText.Text =
                $"Arquivo \"{currentEntry.FileName}\" substituído manualmente + {Math.Max(0, totalUpdated - 1)} arquivo(s) do mesmo diretório relinkado(s) automaticamente!";
        }
    }

    private void OnIgnoreSelectedClicked()
    {
        if (_missingEntries.Count == 0) return;
        _selectedIndex = Math.Clamp(_selectedIndex, 0, _missingEntries.Count - 1);
        var currentEntry = _missingEntries[_selectedIndex];
        currentEntry.Status = "IgnoredOffline";
        currentEntry.MatchStage = "Ignorado pelo usuário (Mídia Offline)";
        RebuildMissingFilesTable();

        if (_missingEntries.All(e => e.Status is "Relinked" or "IgnoredOffline"))
        {
            FinalizeAndMountTimeline("Projeto aberto (mídias ignoradas marcadas como Mídia Offline na Timeline).");
        }
    }

    private void OnIgnoreAllClicked()
    {
        foreach (var entry in _missingEntries)
        {
            if (entry.Status != "Relinked")
            {
                entry.Status = "IgnoredOffline";
                entry.MatchStage = "Ignorado (Ignore All -> Mídia Offline)";
            }
        }
        RebuildMissingFilesTable();
        FinalizeAndMountTimeline("Projeto aberto com mídias ausentes marcadas como Mídia Offline (Quadro Vermelho).");
    }

    private void FinalizeAndMountTimeline(string message)
    {
        if (_mounted) return;
        _mounted = true;
        _relinker.ApplyRelinkResolutionsToProject(_project, _missingEntries);
        _onProjectReadyToMount(message);
        Close();
    }

    private static TextBlock CreateHeaderCell(string text, int col)
    {
        return new TextBlock
        {
            Text = text,
            FontSize = 10.5,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8")),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(8, 0),
            [Grid.ColumnProperty] = col
        };
    }

    private static TextBlock CreateCellText(string text, int col, string fgHex, FontWeight weight)
    {
        return new TextBlock
        {
            Text = text,
            FontSize = 10.5,
            FontWeight = weight,
            Foreground = new SolidColorBrush(Color.Parse(fgHex)),
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(6, 2),
            [Grid.ColumnProperty] = col
        };
    }

    private static Button CreateActionButton(string label, string bgHex, string fgHex, Action onClick)
    {
        var btn = new Button
        {
            Content = label,
            FontSize = 10.5,
            FontWeight = FontWeight.Bold,
            Background = new SolidColorBrush(Color.Parse(bgHex)),
            Foreground = new SolidColorBrush(Color.Parse(fgHex)),
            BorderBrush = new SolidColorBrush(Color.Parse("#475569")),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(10, 6),
            Margin = new Thickness(0, 0, 6, 6)
        };
        btn.Click += (_, _) => onClick();
        return btn;
    }
}
