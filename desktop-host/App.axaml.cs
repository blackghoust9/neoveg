using System;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Styling;
using Avalonia.Themes.Fluent;
using Avalonia.Threading;
using Neoveg.Views;

namespace Neoveg;

/// <summary>
/// Reactive exception handler bridge for Avalonia UI (`RxApp.DefaultExceptionHandler`).
/// Intercepts unhandled UI/reactive pipeline exceptions and routes them to `logs/neoveg.log` and `logs/crash.log`.
/// </summary>
public static class RxApp
{
    public static IObserver<Exception> DefaultExceptionHandler { get; set; } = new NeovegReactiveExceptionObserver();

    private sealed class NeovegReactiveExceptionObserver : IObserver<Exception>
    {
        public void OnNext(Exception value)
        {
            Program.LogEvent("ERROR", "RxApp.DefaultExceptionHandler", $"Unhandled reactive UI exception: {value.Message}", value);
            Program.WriteCrashLog("RxApp.DefaultExceptionHandler", value, isTerminating: false);
        }

        public void OnError(Exception error)
        {
            Program.LogEvent("FATAL", "RxApp.DefaultExceptionHandler.OnError", error.Message, error);
            Program.WriteCrashLog("RxApp.DefaultExceptionHandler.OnError", error, isTerminating: false);
        }

        public void OnCompleted()
        {
        }
    }
}

public sealed class App : Application
{
    public override void Initialize()
    {
        RegisterAvaloniaExceptionHandlers();
        Styles.Add(new FluentTheme());
        Styles.Add(Neoveg.Styles.NeovegDockTabStyles.Create());
        Styles.Add(Neoveg.Controls.NeovegPrecisionSlider.CreateStyles());
        RequestedThemeVariant = ThemeVariant.Dark;
    }

    private static void RegisterAvaloniaExceptionHandlers()
    {
        RxApp.DefaultExceptionHandler = RxApp.DefaultExceptionHandler;

        Dispatcher.UIThread.UnhandledException += (_, e) =>
        {
            Program.LogEvent("ERROR", "Avalonia.Dispatcher.UnhandledException", e.Exception.Message, e.Exception);
            Program.WriteCrashLog("Avalonia.Dispatcher.UnhandledException", e.Exception, isTerminating: false);
            RxApp.DefaultExceptionHandler.OnNext(e.Exception);
            e.Handled = true;
        };
    }

    public override void OnFrameworkInitializationCompleted()
    {
        try
        {
            Program.LogEvent("INFO", "App.OnFrameworkInitializationCompleted", "Initializing Neoveg MainWindow and Splash");
            if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
            {
                var mw = new MainWindow();
                var splash = new NeovegSplashWindow(() =>
                {
                    desktop.MainWindow = mw;
                    mw.Show();
                    mw.Activate();
                }, durationSeconds: 2.6);
                desktop.MainWindow = splash;
            }
            base.OnFrameworkInitializationCompleted();
        }
        catch (Exception ex)
        {
            RxApp.DefaultExceptionHandler.OnNext(ex);
            Program.WriteCrashLog("App.OnFrameworkInitializationCompleted", ex, isTerminating: true);
            throw;
        }
    }
}
