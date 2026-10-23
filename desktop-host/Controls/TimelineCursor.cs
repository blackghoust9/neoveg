using System;
using Avalonia.Controls;
using Avalonia.Threading;

namespace Neoveg.Controls;

/// <summary>
/// Classic Neoveg Edit Cursor Blink Controller (`TimelineCursor.cs` / `PlayheadOverlay.axaml`).
/// Manages the 3 animation states of the timeline Playhead / Edit Cursor:
/// 1. Estado Parado / Pausado (`IsPlaying == false && !IsScrubbing`):
///    - Runs a `DispatcherTimer` with an interval of ~`500ms` (`BlinkIntervalMs = 500`) that alternates
///      the opacity of the vertical cursor line (`VerticalLineOpacity`) between `1.0` and `0.0`
///      (`IsVerticalLineVisible` visible/invisible), making the vertical guide line blink smoothly at its fixed X coordinate.
/// 2. Estado em Reprodução (`IsPlaying == true`) ou Arrasto (`IsScrubbing == true`):
///    - Immediately stops the blink `DispatcherTimer` (`_blinkTimer.Stop()`),
///    - Locks `VerticalLineOpacity = 1.0` (solid `1.0`),
///    - Updates the cursor's `X` coordinate in real time at `60 FPS`.
/// 3. Sincronização com Entrada de Teclado (`Left` / `Right` Arrow Keys — Frame-by-Frame Navigation):
///    - `ResetBlinkTimerOnKeyPress()` stops the blink timer, locks `VerticalLineOpacity = 1.0` during movement,
///      and restarts the `500ms` timer so the cursor only resumes blinking `500ms` after the last key press.
/// </summary>
public sealed class TimelineCursor
{
    public const int BlinkIntervalMs = 500;

    private readonly DispatcherTimer _blinkTimer;
    private bool _isPlaying;
    private bool _isScrubbing;

    /// <summary>
    /// Current opacity of the vertical 1px guide line across the timeline tracks (`1.0` when visible/solid, `0.0` when hidden in blink phase).
    /// </summary>
    public double VerticalLineOpacity { get; private set; } = 1.0;

    /// <summary>
    /// True when the vertical cursor line is in its visible phase (`VerticalLineOpacity >= 0.5`).
    /// </summary>
    public bool IsVerticalLineVisible => VerticalLineOpacity >= 0.5;

    /// <summary>
    /// Current horizontal pixel coordinate (`X`) of the playhead on the timeline overlay.
    /// </summary>
    public double CursorX { get; private set; }

    /// <summary>
    /// Current timeline position in seconds.
    /// </summary>
    public double TimeSeconds { get; private set; }

    /// <summary>
    /// True when the timeline is actively playing (`IsPlaying == true`).
    /// When true, the blink timer is stopped immediately and `VerticalLineOpacity` is locked to `1.0` at 60 FPS.
    /// </summary>
    public bool IsPlaying
    {
        get => _isPlaying;
        set
        {
            if (_isPlaying == value) return;
            _isPlaying = value;
            ApplyAnimationState();
        }
    }

    /// <summary>
    /// True when the user is dragging/scrubbing the playhead on the timecode ruler (`IsScrubbing == true`).
    /// When true, the blink timer is stopped immediately and `VerticalLineOpacity` is locked to `1.0` at 60 FPS.
    /// </summary>
    public bool IsScrubbing
    {
        get => _isScrubbing;
        set
        {
            if (_isScrubbing == value) return;
            _isScrubbing = value;
            ApplyAnimationState();
        }
    }

    /// <summary>
    /// Raised whenever the cursor's blink opacity (`1.0` / `0.0`) or `X` position changes.
    /// </summary>
    public event Action? OnCursorVisualChanged;

    public TimelineCursor()
    {
        _blinkTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(BlinkIntervalMs)
        };
        _blinkTimer.Tick += OnBlinkTimerTick;
        _blinkTimer.Start();
    }

    private void OnBlinkTimerTick(object? sender, EventArgs e)
    {
        if (_isPlaying || _isScrubbing)
        {
            _blinkTimer.Stop();
            if (Math.Abs(VerticalLineOpacity - 1.0) > 1e-4)
            {
                VerticalLineOpacity = 1.0;
                OnCursorVisualChanged?.Invoke();
            }
            return;
        }

        // Toggle vertical cursor line opacity strictly between 1.0 and 0.0 every ~500ms while paused/stopped
        VerticalLineOpacity = VerticalLineOpacity >= 0.5 ? 0.0 : 1.0;
        OnCursorVisualChanged?.Invoke();
    }

    private void ApplyAnimationState()
    {
        if (_isPlaying || _isScrubbing)
        {
            // Estado em Reprodução (IsPlaying == true) ou Arrasto (Scrubbing):
            // Interrompe o temporizador de piscada imediatamente e trava a opacidade da linha em 1.0 (sólida).
            _blinkTimer.Stop();
            VerticalLineOpacity = 1.0;
            OnCursorVisualChanged?.Invoke();
        }
        else
        {
            // Estado Parado / Pausado (IsPlaying == false):
            // Inicia visível (1.0) e ativa o temporizador de ~500ms para alternar entre 1.0 e 0.0.
            VerticalLineOpacity = 1.0;
            _blinkTimer.Stop();
            _blinkTimer.Start();
            OnCursorVisualChanged?.Invoke();
        }
    }

    /// <summary>
    /// Updates the real-time 60 FPS horizontal position (`X`) and time (`seconds`) of the cursor.
    /// </summary>
    public void UpdatePosition(double timeSeconds, double pixelsPerSecond, double horizontalOffsetSeconds = 0.0)
    {
        TimeSeconds = Math.Max(0.0, timeSeconds);
        CursorX = (TimeSeconds - horizontalOffsetSeconds) * Math.Max(20.0, pixelsPerSecond);
    }

    /// <summary>
    /// Sincronização com Entrada de Teclado (Setas Esquerda / Direita para navegar por quadros):
    /// Reseta o temporizador da piscada para manter a linha visível (`VerticalLineOpacity = 1.0`)
    /// durante o movimento, voltando a piscar `500ms` após a última tecla ser pressionada.
    /// </summary>
    public void ResetBlinkTimerOnKeyPress()
    {
        _blinkTimer.Stop();
        VerticalLineOpacity = 1.0;
        if (!_isPlaying && !_isScrubbing)
        {
            _blinkTimer.Start();
        }
        OnCursorVisualChanged?.Invoke();
    }
}

/// <summary>
/// Backing UserControl for `PlayheadOverlay.axaml` hosting `PlayheadOverlayCanvas` and `TimelineCursor`.
/// </summary>
public partial class PlayheadOverlay : UserControl
{
    public PlayheadOverlayCanvas OverlayCanvas { get; }

    public TimelineCursor CursorController => OverlayCanvas.EditCursor;

    public PlayheadOverlay()
    {
        IsHitTestVisible = false;
        ClipToBounds = true;
        OverlayCanvas = new PlayheadOverlayCanvas();
        Content = OverlayCanvas;
    }
}
