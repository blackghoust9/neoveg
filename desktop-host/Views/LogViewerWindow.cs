using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Neoveg.ViewModels;

namespace Neoveg.Views;

/// <summary>
/// Centralized Diagnostic & Crash Log Viewer Window (`LogViewerWindow.axaml` / `LogViewerWindow.cs`).
/// Bound to `LogViewerViewModel` with support for partial user text selection in `TextBox` / `SelectableTextBlock`
/// and one-click `Copiar Logs` (`CopyToClipboardCommand` -> `"Copiado!"` for 2 seconds).
/// </summary>
public sealed class LogViewerWindow : Window
{
    public LogViewerViewModel ViewModel { get; }
    private readonly TextBox _logTextBox;
    private readonly Button _toolbarCopyBtn;
    private readonly Button _footerCopyBtn;
    private readonly SelectableTextBlock _statusTextBlock;

    public LogViewerWindow()
    {
        ViewModel = new LogViewerViewModel(this);
        DataContext = ViewModel;

        Title = "Neoveg — Visualizador Centralizado de Diagnóstico e Logs (logs/neoveg.log & logs/crash.log)";
        Width = 920;
        Height = 620;
        MinWidth = 680;
        MinHeight = 420;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = new SolidColorBrush(Color.Parse("#0f141d"));
        Foreground = new SolidColorBrush(Color.Parse("#f1f5f9"));

        var rootGrid = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*,Auto"),
            Margin = new Thickness(14)
        };

        // Top Header & Toolbar
        var headerGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto")
        };

        var titleStack = new StackPanel { Spacing = 2 };
        titleStack.Children.Add(new SelectableTextBlock
        {
            Text = "DIAGNÓSTICO E LOGS EM TEMPO DE EXECUÇÃO (C# • Rust FFI • C++ • Python)",
            FontSize = 13,
            FontWeight = FontWeight.Bold,
            Foreground = new SolidColorBrush(Color.Parse("#38bdf8"))
        });
        titleStack.Children.Add(new SelectableTextBlock
        {
            Text = "Selecione qualquer trecho de texto abaixo ou clique em 'Copiar Logs' para copiar todo o conteúdo para a Área de Transferência.",
            FontSize = 11,
            Foreground = new SolidColorBrush(Color.Parse("#94a3b8"))
        });
        Grid.SetColumn(titleStack, 0);
        headerGrid.Children.Add(titleStack);

        var toolbarButtons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center
        };

        _toolbarCopyBtn = new Button
        {
            Content = ViewModel.CopyButtonLabel,
            Background = new SolidColorBrush(Color.Parse("#2563eb")),
            Foreground = Brushes.White,
            FontWeight = FontWeight.Bold,
            Padding = new Thickness(12, 6)
        };

        var refreshBtn = new Button
        {
            Content = "Atualizar Logs",
            Background = new SolidColorBrush(Color.Parse("#1e293b")),
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0")),
            Padding = new Thickness(10, 6)
        };
        refreshBtn.Click += (_, _) =>
        {
            ViewModel.ReloadLogsFromDisk();
            _logTextBox!.Text = ViewModel.LogText;
        };

        toolbarButtons.Children.Add(_toolbarCopyBtn);
        toolbarButtons.Children.Add(refreshBtn);
        Grid.SetColumn(toolbarButtons, 1);
        headerGrid.Children.Add(toolbarButtons);

        var headerBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#161d2a")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2563eb")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(12, 8),
            Margin = new Thickness(0, 0, 0, 10),
            Child = headerGrid
        };
        Grid.SetRow(headerBorder, 0);
        rootGrid.Children.Add(headerBorder);

        // Main Selectable Log TextBox
        _logTextBox = new TextBox
        {
            Text = ViewModel.LogText,
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.NoWrap,
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 11.5,
            Background = new SolidColorBrush(Color.Parse("#0b0f17")),
            Foreground = new SolidColorBrush(Color.Parse("#e2e8f0")),
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(10)
        };
        _logTextBox.PropertyChanged += (_, e) =>
        {
            if (e.Property == TextBox.SelectionStartProperty || e.Property == TextBox.SelectionEndProperty)
            {
                ViewModel.SelectedLogText = _logTextBox.SelectedText ?? "";
            }
        };
        Grid.SetRow(_logTextBox, 1);
        rootGrid.Children.Add(_logTextBox);

        // Footer Status & Action Buttons
        var footerGrid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto")
        };

        _statusTextBlock = new SelectableTextBlock
        {
            Text = ViewModel.StatusMessage,
            VerticalAlignment = VerticalAlignment.Center,
            FontSize = 11,
            Foreground = new SolidColorBrush(Color.Parse("#cbd5e1"))
        };
        Grid.SetColumn(_statusTextBlock, 0);
        footerGrid.Children.Add(_statusTextBlock);

        var footerButtons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8
        };

        _footerCopyBtn = new Button
        {
            Content = ViewModel.CopyButtonLabel,
            Background = new SolidColorBrush(Color.Parse("#2563eb")),
            Foreground = Brushes.White,
            FontWeight = FontWeight.Bold,
            Padding = new Thickness(14, 6)
        };

        var closeBtn = new Button
        {
            Content = "Fechar",
            Background = new SolidColorBrush(Color.Parse("#334155")),
            Foreground = Brushes.White,
            Padding = new Thickness(14, 6)
        };
        closeBtn.Click += (_, _) => Close();

        footerButtons.Children.Add(_footerCopyBtn);
        footerButtons.Children.Add(closeBtn);
        Grid.SetColumn(footerButtons, 1);
        footerGrid.Children.Add(footerButtons);

        var footerBorder = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#161d2a")),
            BorderBrush = new SolidColorBrush(Color.Parse("#334155")),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(10, 8),
            Margin = new Thickness(0, 10, 0, 0),
            Child = footerGrid
        };
        Grid.SetRow(footerBorder, 2);
        rootGrid.Children.Add(footerBorder);

        _toolbarCopyBtn.Click += async (_, _) => await TriggerCopyAsync();
        _footerCopyBtn.Click += async (_, _) => await TriggerCopyAsync();

        ViewModel.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(LogViewerViewModel.CopyButtonLabel) ||
                e.PropertyName == nameof(LogViewerViewModel.IsCopyConfirmed))
            {
                _toolbarCopyBtn.Content = ViewModel.CopyButtonLabel;
                _footerCopyBtn.Content = ViewModel.CopyButtonLabel;
                var bgBrush = new SolidColorBrush(Color.Parse(ViewModel.IsCopyConfirmed ? "#16a34a" : "#2563eb"));
                _toolbarCopyBtn.Background = bgBrush;
                _footerCopyBtn.Background = bgBrush;
            }
            else if (e.PropertyName == nameof(LogViewerViewModel.StatusMessage))
            {
                _statusTextBlock.Text = ViewModel.StatusMessage;
            }
            else if (e.PropertyName == nameof(LogViewerViewModel.LogText))
            {
                _logTextBox.Text = ViewModel.LogText;
            }
        };

        Content = rootGrid;
    }

    private async System.Threading.Tasks.Task TriggerCopyAsync()
    {
        string? sel = _logTextBox.SelectedText;
        await ViewModel.CopyLogsToClipboardAsync(this, sel);
    }
}
