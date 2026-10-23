using System;
using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;

namespace Neoveg.Controls;

/// <summary>
/// Precision NLE Fader / Slider control (<see cref="NeovegPrecisionSlider"/>) for Opacity, Volume, Pan, and Master Bus:
/// <list type="bullet">
///   <item><description><b>Flat Rectangular Thumb (<c>CornerRadius = 0</c>):</b> Compact width (<c>8px</c>) and solid fill color.</description></item>
///   <item><description><b>Double-Click Reset:</b> Double-clicking the thumb or slider resets <see cref="RangeBase.Value"/> to <see cref="DefaultValue"/> (1.0 / 100% for opacity/volume, 0.0 for pan).</description></item>
///   <item><description><b>Fine Adjustment (<c>Ctrl</c> Drag):</b> Holding <c>Ctrl</c> while dragging reduces cursor sensitivity to 1/10th (<see cref="FineStepFactor"/> = 0.1) for decimal precision.</description></item>
///   <item><description><b>Right-Click Context Menu:</b> Provides <c>'Restaurar Padrão'</c> and <c>'Digitar Valor...'</c> actions.</description></item>
/// </list>
/// </summary>
public sealed class NeovegPrecisionSlider : Slider
{
    public const string PrecisionSliderClass = "neoveg-precision-slider";
    public const double FineStepFactor = 0.1; // 1/10 sensitivity when Ctrl is held

    protected override Type StyleKeyOverride => typeof(Slider);

    public double DefaultValue { get; set; } = 1.0;
    public string ParameterLabel { get; set; } = "Valor";
    public bool DisplayAsPercentage { get; set; }

    private bool _isFineDragging;
    private Point _lastPointerPos;
    private double _fineAnchorValue;
    private Thumb? _templateThumb;

    public NeovegPrecisionSlider()
    {
        Classes.Add(PrecisionSliderClass);
        CornerRadius = new CornerRadius(0);
        MinHeight = 18;
        Height = 20;

        ContextMenu = BuildPrecisionContextMenu();

        AddHandler(DoubleTappedEvent, OnSliderDoubleTapped, RoutingStrategies.Tunnel | RoutingStrategies.Bubble, handledEventsToo: true);
        AddHandler(PointerPressedEvent, OnSliderPointerPressed, RoutingStrategies.Tunnel, handledEventsToo: true);
        AddHandler(PointerMovedEvent, OnSliderPointerMoved, RoutingStrategies.Tunnel, handledEventsToo: true);
        AddHandler(PointerReleasedEvent, OnSliderPointerReleased, RoutingStrategies.Tunnel, handledEventsToo: true);
    }

    public static NeovegPrecisionSlider Create(
        double minimum,
        double maximum,
        double initialValue,
        double defaultValue,
        string parameterLabel,
        double width = double.NaN,
        bool displayAsPercentage = false)
    {
        var slider = new NeovegPrecisionSlider
        {
            Minimum = minimum,
            Maximum = maximum,
            DefaultValue = Math.Clamp(defaultValue, minimum, maximum),
            Value = Math.Clamp(initialValue, minimum, maximum),
            ParameterLabel = parameterLabel,
            DisplayAsPercentage = displayAsPercentage,
            VerticalAlignment = VerticalAlignment.Center
        };
        if (!double.IsNaN(width) && width > 0)
        {
            slider.Width = width;
        }
        return slider;
    }

    protected override void OnApplyTemplate(TemplateAppliedEventArgs e)
    {
        base.OnApplyTemplate(e);

        if (e.NameScope.Find<Track>("PART_Track") is { } track && track.Thumb is { } thumb)
        {
            _templateThumb = thumb;
            ApplyFlatSquareThumbAppearance(thumb);
            thumb.RemoveHandler(DoubleTappedEvent, OnSliderDoubleTapped);
            thumb.AddHandler(DoubleTappedEvent, OnSliderDoubleTapped, RoutingStrategies.Tunnel | RoutingStrategies.Bubble, handledEventsToo: true);
        }
    }

    private static void ApplyFlatSquareThumbAppearance(Thumb thumb)
    {
        thumb.Width = 8;
        thumb.Height = 16;
        thumb.MinWidth = 8;
        thumb.MinHeight = 14;
        thumb.CornerRadius = new CornerRadius(0);
        thumb.Background = new SolidColorBrush(Color.Parse("#38bdf8"));
        thumb.BorderBrush = new SolidColorBrush(Color.Parse("#e2e8f0"));
        thumb.BorderThickness = new Thickness(1);
        thumb.Template = new FuncControlTemplate<Thumb>((_, _) => new Border
        {
            Width = 8,
            Height = 16,
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#38bdf8")),
            BorderBrush = new SolidColorBrush(Color.Parse("#e2e8f0")),
            BorderThickness = new Thickness(1)
        });
    }

    /// <summary>
    /// Resets the slider value to <see cref="DefaultValue"/> (e.g. 1.0 / 100% for Volume/Opacity or 0.0 for Pan).
    /// </summary>
    public void ResetToDefault()
    {
        Value = Math.Clamp(DefaultValue, Minimum, Maximum);
    }

    private void OnSliderDoubleTapped(object? sender, TappedEventArgs e)
    {
        ResetToDefault();
        e.Handled = true;
    }

    private void OnSliderPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        var point = e.GetCurrentPoint(this);
        if (point.Properties.IsRightButtonPressed)
        {
            return;
        }

        // Detect double-click on the slider or thumb to reset to DefaultValue
        if (e.ClickCount >= 2 && point.Properties.IsLeftButtonPressed)
        {
            ResetToDefault();
            _isFineDragging = false;
            e.Handled = true;
            return;
        }

        if (point.Properties.IsLeftButtonPressed)
        {
            _lastPointerPos = e.GetPosition(this);
            _fineAnchorValue = Value;
            _isFineDragging = e.KeyModifiers.HasFlag(KeyModifiers.Control);
            if (_isFineDragging)
            {
                e.Pointer.Capture(this);
                e.Handled = true;
            }
        }
    }

    private void OnSliderPointerMoved(object? sender, PointerEventArgs e)
    {
        var point = e.GetCurrentPoint(this);
        if (!point.Properties.IsLeftButtonPressed)
        {
            _isFineDragging = false;
            return;
        }

        var currentPos = e.GetPosition(this);
        bool ctrlHeld = e.KeyModifiers.HasFlag(KeyModifiers.Control);

        if (ctrlHeld)
        {
            if (!_isFineDragging)
            {
                _isFineDragging = true;
                _lastPointerPos = currentPos;
                _fineAnchorValue = Value;
                e.Pointer.Capture(this);
            }

            double trackWidth = Math.Max(24.0, Bounds.Width - 8.0);
            double deltaPx = currentPos.X - _lastPointerPos.X;
            ApplyFineDragDelta(deltaPx, trackWidth);
            _lastPointerPos = currentPos;
            e.Handled = true;
        }
        else
        {
            _isFineDragging = false;
            _lastPointerPos = currentPos;
            _fineAnchorValue = Value;
        }
    }

    /// <summary>
    /// Applies a fine-adjustment drag delta at 1/10th (<see cref="FineStepFactor"/>) of normal cursor speed.
    /// </summary>
    public double ApplyFineDragDelta(double deltaPx, double trackWidth)
    {
        if (!_isFineDragging)
        {
            _fineAnchorValue = Value;
            _isFineDragging = true;
        }
        double effectiveWidth = Math.Max(24.0, trackWidth);
        double range = Math.Max(1e-6, Maximum - Minimum);
        double deltaValue = (deltaPx / effectiveWidth) * range * FineStepFactor;
        _fineAnchorValue = Math.Clamp(_fineAnchorValue + deltaValue, Minimum, Maximum);
        Value = Math.Round(_fineAnchorValue, 4);
        return Value;
    }

    private void OnSliderPointerReleased(object? sender, PointerReleasedEventArgs e)
    {
        if (_isFineDragging)
        {
            _isFineDragging = false;
            if (ReferenceEquals(e.Pointer.Captured, this))
            {
                e.Pointer.Capture(null);
            }
            e.Handled = true;
        }
    }

    private ContextMenu BuildPrecisionContextMenu()
    {
        var menu = new ContextMenu();

        var miReset = new MenuItem
        {
            Header = "Restaurar Padrão"
        };
        miReset.Click += (_, _) => ResetToDefault();

        var miTypeValue = new MenuItem
        {
            Header = "Digitar Valor..."
        };
        miTypeValue.Click += (_, _) => OpenTypeValueDialog();

        menu.ItemsSource = new object[]
        {
            miReset,
            new Separator(),
            miTypeValue
        };
        return menu;
    }

    public void OpenTypeValueDialog()
    {
        double currentDisplay = DisplayAsPercentage ? Math.Round(Value * 100.0, 2) : Math.Round(Value, 3);
        double minDisplay = DisplayAsPercentage ? Math.Round(Minimum * 100.0, 2) : Math.Round(Minimum, 3);
        double maxDisplay = DisplayAsPercentage ? Math.Round(Maximum * 100.0, 2) : Math.Round(Maximum, 3);
        string unitSuffix = DisplayAsPercentage ? "%" : "";

        var txtInput = new TextBox
        {
            Text = currentDisplay.ToString("0.###", CultureInfo.InvariantCulture),
            FontFamily = new FontFamily("Consolas, monospace"),
            FontSize = 12,
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#0f141d")),
            Foreground = Brushes.White,
            BorderBrush = new SolidColorBrush(Color.Parse("#38bdf8"))
        };

        var dlg = new Window
        {
            Title = $"Digitar Valor — {ParameterLabel}",
            Width = 340,
            Height = 175,
            CanResize = false,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = new SolidColorBrush(Color.Parse("#111621")),
            Foreground = Brushes.White
        };

        void ApplyTypedValue()
        {
            string raw = (txtInput.Text ?? "").Trim().TrimEnd('%').Replace(',', '.');
            if (double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out double parsed))
            {
                double normalized = DisplayAsPercentage ? parsed / 100.0 : parsed;
                Value = Math.Clamp(normalized, Minimum, Maximum);
            }
            dlg.Close();
        }

        txtInput.KeyDown += (_, ke) =>
        {
            if (ke.Key == Key.Enter)
            {
                ke.Handled = true;
                ApplyTypedValue();
            }
            else if (ke.Key == Key.Escape)
            {
                ke.Handled = true;
                dlg.Close();
            }
        };

        var btnOk = new Button
        {
            Content = "Aplicar",
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#2563eb")),
            Foreground = Brushes.White,
            FontWeight = FontWeight.Bold,
            Padding = new Thickness(12, 5)
        };
        btnOk.Click += (_, _) => ApplyTypedValue();

        var btnDefault = new Button
        {
            Content = "Restaurar Padrão",
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(Color.Parse("#1e293b")),
            Foreground = new SolidColorBrush(Color.Parse("#cbd5e1")),
            Padding = new Thickness(10, 5)
        };
        btnDefault.Click += (_, _) =>
        {
            ResetToDefault();
            dlg.Close();
        };

        var root = new StackPanel
        {
            Spacing = 10,
            Margin = new Thickness(14),
            Children =
            {
                new TextBlock
                {
                    Text = $"{ParameterLabel} ({minDisplay:0.##}{unitSuffix} a {maxDisplay:0.##}{unitSuffix}):",
                    FontSize = 11,
                    FontWeight = FontWeight.SemiBold,
                    Foreground = new SolidColorBrush(Color.Parse("#e2e8f0"))
                },
                txtInput,
                new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 8,
                    HorizontalAlignment = HorizontalAlignment.Right,
                    Children = { btnDefault, btnOk }
                }
            }
        };

        dlg.Content = root;
        if (TopLevel.GetTopLevel(this) is Window ownerWin)
        {
            dlg.Show(ownerWin);
        }
        else
        {
            dlg.Show();
        }
        txtInput.Focus();
        txtInput.SelectAll();
    }

    /// <summary>
    /// Application-level styles enforcing a rectangular/square Flat Thumb (<c>CornerRadius = 0</c>) on all sliders.
    /// </summary>
    public static Avalonia.Styling.Styles CreateStyles()
    {
        Selector sliderThumb = default(Selector).OfType<Slider>().Template().OfType<Thumb>();
        Selector precisionThumb = default(Selector).OfType<Slider>().Class(PrecisionSliderClass).Template().OfType<Thumb>();

        return new Avalonia.Styling.Styles
        {
            new Style(_ => sliderThumb)
            {
                Setters =
                {
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                    new Setter(Layoutable.WidthProperty, 8.0),
                    new Setter(Layoutable.HeightProperty, 16.0),
                    new Setter(TemplatedControl.BackgroundProperty, new SolidColorBrush(Color.Parse("#38bdf8"))),
                    new Setter(TemplatedControl.BorderBrushProperty, new SolidColorBrush(Color.Parse("#e2e8f0"))),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(1))
                }
            },
            new Style(_ => precisionThumb)
            {
                Setters =
                {
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                    new Setter(Layoutable.WidthProperty, 8.0),
                    new Setter(Layoutable.HeightProperty, 16.0),
                    new Setter(TemplatedControl.BackgroundProperty, new SolidColorBrush(Color.Parse("#38bdf8"))),
                    new Setter(TemplatedControl.BorderBrushProperty, new SolidColorBrush(Color.Parse("#e2e8f0"))),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(1))
                }
            }
        };
    }
}
