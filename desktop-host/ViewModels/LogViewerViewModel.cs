using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Input;
using Avalonia.Controls;

namespace Neoveg.ViewModels;

/// <summary>
/// MVVM ViewModel for `LogViewerWindow` (`LogViewerWindow.axaml` / `LogViewerWindow.cs`).
/// Implements `CopyToClipboardCommand` using `TopLevel.GetTopLevel(window)?.Clipboard?.SetTextAsync(...)`:
/// - Copies only the user's selected text when a selection is active, or the entire log content otherwise.
/// - Displays visual confirmation feedback by setting `CopyButtonLabel` to `"Copiado!"` for 2 seconds.
/// </summary>
public sealed class LogViewerViewModel : INotifyPropertyChanged
{
    private string _logText = "";
    private string _selectedLogText = "";
    private string _copyButtonLabel = "Copiar Logs";
    private bool _isCopyConfirmed;
    private string _statusMessage = "Pronto • Selecione um trecho ou clique em 'Copiar Logs' para copiar todo o diagnóstico.";
    private Window? _attachedWindow;

    public event PropertyChangedEventHandler? PropertyChanged;

    public string LogText
    {
        get => _logText;
        set
        {
            if (_logText != value)
            {
                _logText = value;
                OnPropertyChanged();
            }
        }
    }

    public string SelectedLogText
    {
        get => _selectedLogText;
        set
        {
            if (_selectedLogText != value)
            {
                _selectedLogText = value;
                OnPropertyChanged();
            }
        }
    }

    public string CopyButtonLabel
    {
        get => _copyButtonLabel;
        set
        {
            if (_copyButtonLabel != value)
            {
                _copyButtonLabel = value;
                OnPropertyChanged();
            }
        }
    }

    public bool IsCopyConfirmed
    {
        get => _isCopyConfirmed;
        set
        {
            if (_isCopyConfirmed != value)
            {
                _isCopyConfirmed = value;
                OnPropertyChanged();
            }
        }
    }

    public string StatusMessage
    {
        get => _statusMessage;
        set
        {
            if (_statusMessage != value)
            {
                _statusMessage = value;
                OnPropertyChanged();
            }
        }
    }

    public ICommand CopyToClipboardCommand { get; }
    public ICommand RefreshLogsCommand { get; }

    public LogViewerViewModel(Window? window = null)
    {
        _attachedWindow = window;
        CopyToClipboardCommand = new AsyncRelayCommand(async param =>
        {
            var targetWin = param as Window ?? _attachedWindow;
            await CopyLogsToClipboardAsync(targetWin);
        });
        RefreshLogsCommand = new AsyncRelayCommand(_ =>
        {
            ReloadLogsFromDisk();
            return Task.CompletedTask;
        });

        ReloadLogsFromDisk();
    }

    public void AttachWindow(Window window)
    {
        _attachedWindow = window;
    }

    public void ReloadLogsFromDisk()
    {
        var sb = new StringBuilder();
        sb.AppendLine("================================================================================");
        sb.AppendLine($"NEOVEG — CENTRALIZED DIAGNOSTIC LOG VIEWER ({DateTime.Now:yyyy-MM-dd HH:mm:ss})");
        sb.AppendLine($"Active Log : {Program.ActiveLogPath}");
        sb.AppendLine($"Crash Log  : {Program.CrashLogPath}");
        sb.AppendLine($"AppCache   : {Program.LocalAppCacheDirectory}");
        sb.AppendLine("================================================================================");
        sb.AppendLine();

        AppendFileSection(sb, "1. ACTIVE RUNTIME LOG (logs/neoveg.log)", Program.ActiveLogPath);
        AppendFileSection(sb, "2. CRASH & UNHANDLED EXCEPTIONS LOG (logs/crash.log)", Program.CrashLogPath);

        string rootErrLog = Path.Combine(Program.GoogleDriveSyncRoot, "log_erros.txt");
        if (File.Exists(rootErrLog))
        {
            AppendFileSection(sb, "3. HISTORICAL DIAGNOSTICS (log_erros.txt)", rootErrLog);
        }

        LogText = sb.ToString();
        StatusMessage = $"Logs carregados ({LogText.Length:N0} caracteres).";
    }

    private static void AppendFileSection(StringBuilder sb, string header, string filePath)
    {
        sb.AppendLine($"--- {header} ---");
        try
        {
            if (File.Exists(filePath))
            {
                string content = File.ReadAllText(filePath, Encoding.UTF8);
                sb.AppendLine(string.IsNullOrWhiteSpace(content) ? "(Arquivo vazio — nenhuma falha crítica registrada)" : content.TrimEnd());
            }
            else
            {
                sb.AppendLine($"(Arquivo não encontrado em: {filePath})");
            }
        }
        catch (Exception ex)
        {
            sb.AppendLine($"(Erro ao ler '{filePath}': {ex.Message})");
        }
        sb.AppendLine();
    }

    /// <summary>
    /// Copies either the currently selected text (`SelectedLogText`) or the full log (`LogText`)
    /// to the system clipboard via `TopLevel.GetTopLevel(window)?.Clipboard?.SetTextAsync(...)`
    /// and updates `CopyButtonLabel` to `"Copiado!"` for 2 seconds.
    /// </summary>
    public async Task<bool> CopyLogsToClipboardAsync(Window? window = null, string? explicitSelection = null)
    {
        Window? targetWindow = window ?? _attachedWindow;
        string selection = !string.IsNullOrEmpty(explicitSelection) ? explicitSelection : SelectedLogText;
        bool isPartialSelection = !string.IsNullOrEmpty(selection);
        string payload = isPartialSelection ? selection : LogText;

        if (string.IsNullOrEmpty(payload))
        {
            StatusMessage = "Nenhum conteúdo de log disponível para copiar.";
            return false;
        }

        try
        {
            var clipboard = targetWindow != null ? TopLevel.GetTopLevel(targetWindow)?.Clipboard : null;
            if (clipboard != null)
            {
                await clipboard.SetTextAsync(payload);
            }
        }
        catch (Exception ex)
        {
            Program.LogEvent("WARN", "LogViewerViewModel.CopyToClipboard", $"Clipboard warning: {ex.Message}", ex);
        }

        CopyButtonLabel = "Copiado!";
        IsCopyConfirmed = true;
        StatusMessage = isPartialSelection
            ? $"Copiado trecho selecionado ({payload.Length:N0} caracteres) para a área de transferência!"
            : $"Copiado log completo ({payload.Length:N0} caracteres) para a área de transferência!";

        Program.LogEvent(
            "INFO",
            "LogViewerViewModel",
            $"Logs copied to clipboard (partialSelection={isPartialSelection}, chars={payload.Length}).");

        _ = ResetCopyButtonLabelAfterDelayAsync();
        return true;
    }

    private async Task ResetCopyButtonLabelAfterDelayAsync()
    {
        await Task.Delay(2000);
        CopyButtonLabel = "Copiar Logs";
        IsCopyConfirmed = false;
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }

    private sealed class AsyncRelayCommand : ICommand
    {
        private readonly Func<object?, Task> _executeAsync;

        public AsyncRelayCommand(Func<object?, Task> executeAsync)
        {
            _executeAsync = executeAsync;
        }

        public event EventHandler? CanExecuteChanged
        {
            add { }
            remove { }
        }

        public bool CanExecute(object? parameter) => true;

        public async void Execute(object? parameter)
        {
            await _executeAsync(parameter);
        }
    }
}
