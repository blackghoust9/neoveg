using System;
using System.Collections.Generic;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

namespace Neoveg.Controls;

/// <summary>
/// VEGAS Pro style docking window (Timeline, Video Preview, Trimmer, Master Bus...).
/// Dragging the header grip (or a tab header, see <see cref="EnableTabUndocking"/>) tears the panel
/// out into an independent floating window; closing the floating window docks it back into its slot.
/// </summary>
public sealed class DetachableDockPanel : Border
{
    private const double DragThresholdPx = 8.0;

    private readonly Decorator _bodyHost;
    private readonly Control _body;
    private Window? _floatingWindow;
    private Point _pressPoint;
    private PointerPressedEventArgs? _pressArgs;

    public string PanelTitle { get; }
    public Border HeaderBorder { get; }
    public TextBlock HeaderTitle { get; }
    public bool IsDetached => _floatingWindow != null;

    /// <summary>Raised when the panel is torn out (`true`) or docked back (`false`), so the host grid can collapse/restore the slot.</summary>
    public event Action<bool>? DetachedChanged;

    public DetachableDockPanel(string title, Control body)
    {
        PanelTitle = title;
        _body = body;
        _bodyHost = new Decorator { Child = body };

        HeaderTitle = new TextBlock
        {
            Text = title,
            FontSize = 10,
            FontWeight = FontWeight.Bold,
            VerticalAlignment = VerticalAlignment.Center
        };

        var grip = NeovegIcons.Create(StreamGeometry.Parse("M 3,4 L 3,4.5 M 3,8 L 3,8.5 M 3,12 L 3,12.5 M 7,4 L 7,4.5 M 7,8 L 7,8.5 M 7,12 L 7,12.5"), 12);
        var btnUndock = NeovegIcons.CreateButton(NeovegIcons.Popout, $"Destacar janela \"{title}\" (ou arraste o cabeçalho)", () => Detach(null), "#00000000", size: 18);
        btnUndock.BorderThickness = new Thickness(0);

        var headerGrid = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto") };
        grip.Margin = new Thickness(0, 0, 4, 0);
        Grid.SetColumn(grip, 0);
        Grid.SetColumn(HeaderTitle, 1);
        Grid.SetColumn(btnUndock, 2);
        headerGrid.Children.Add(grip);
        headerGrid.Children.Add(HeaderTitle);
        headerGrid.Children.Add(btnUndock);

        HeaderBorder = new Border
        {
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(6, 0, 2, 0),
            Cursor = new Cursor(StandardCursorType.SizeAll),
            Child = headerGrid
        };
        ToolTip.SetTip(HeaderBorder, $"Arraste para destacar \"{title}\" em uma janela flutuante • Duplo clique para destacar/acoplar");

        HeaderBorder.PointerPressed += OnHeaderPointerPressed;
        HeaderBorder.PointerMoved += OnHeaderPointerMoved;
        HeaderBorder.PointerReleased += (_, _) => _pressArgs = null;
        HeaderBorder.DoubleTapped += (_, _) => Toggle();

        var grid = new Grid { RowDefinitions = new RowDefinitions("22,*") };
        Grid.SetRow(HeaderBorder, 0);
        Grid.SetRow(_bodyHost, 1);
        grid.Children.Add(HeaderBorder);
        grid.Children.Add(_bodyHost);
        Child = grid;
    }

    public void Toggle()
    {
        if (IsDetached) Redock();
        else Detach(null);
    }

    private void OnHeaderPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (!e.GetCurrentPoint(HeaderBorder).Properties.IsLeftButtonPressed) return;
        _pressPoint = e.GetPosition(this);
        _pressArgs = e;
    }

    private void OnHeaderPointerMoved(object? sender, PointerEventArgs e)
    {
        if (_pressArgs == null || IsDetached) return;
        if (!e.GetCurrentPoint(HeaderBorder).Properties.IsLeftButtonPressed)
        {
            _pressArgs = null;
            return;
        }

        var pos = e.GetPosition(this);
        if (Math.Abs(pos.X - _pressPoint.X) < DragThresholdPx && Math.Abs(pos.Y - _pressPoint.Y) < DragThresholdPx)
            return;

        var args = _pressArgs;
        _pressArgs = null;
        Detach(args, this.PointToScreen(pos));
    }

    /// <summary>
    /// Tears the panel body out into a floating window. When <paramref name="dragArgs"/> is provided the
    /// new window keeps following the mouse (native window move-drag) until the button is released.
    /// </summary>
    public void Detach(PointerPressedEventArgs? dragArgs, PixelPoint? screenPoint = null)
    {
        if (IsDetached) return;
        var owner = TopLevel.GetTopLevel(this) as Window;
        double w = Math.Max(360, Bounds.Width);
        double h = Math.Max(240, Bounds.Height);

        _bodyHost.Child = null;
        var win = CreateFloatingWindow(PanelTitle, _body, w, h, Background);
        if (screenPoint is { } sp)
        {
            win.WindowStartupLocation = WindowStartupLocation.Manual;
            win.Position = new PixelPoint(sp.X - (int)Math.Min(_pressPoint.X, w * 0.5), Math.Max(0, sp.Y - 12));
        }

        win.Closing += (_, _) =>
        {
            win.Content = null;
            _bodyHost.Child = _body;
            _floatingWindow = null;
            IsVisible = true;
            DetachedChanged?.Invoke(false);
        };

        _floatingWindow = win;
        IsVisible = false;
        DetachedChanged?.Invoke(true);

        if (owner != null) win.Show(owner);
        else win.Show();

        if (dragArgs != null)
        {
            Dispatcher.UIThread.Post(() =>
            {
                try { win.BeginMoveDrag(dragArgs); }
                catch { /* Platform without native move-drag: window stays where it was opened */ }
            }, DispatcherPriority.Input);
        }
    }

    public void Redock() => _floatingWindow?.Close();

    internal static Window CreateFloatingWindow(string title, Control content, double width, double height, IBrush? background)
    {
        return new Window
        {
            Title = title,
            Width = width,
            Height = height,
            MinWidth = 220,
            MinHeight = 140,
            ShowInTaskbar = false,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = background ?? new SolidColorBrush(Color.Parse("#141923")),
            Foreground = new SolidColorBrush(Color.Parse("#f1f5f9")),
            Content = content
        };
    }

    /// <summary>
    /// Makes every tab of <paramref name="tabs"/> undockable: dragging a tab header out of the tab strip
    /// opens it as a floating window; closing that window puts the tab back at its original position.
    /// Tabs must be added through <see cref="ItemsControl.Items"/> (not ItemsSource).
    /// </summary>
    public static void EnableTabUndocking(TabControl tabs, Func<TabItem, string> titleResolver)
    {
        foreach (var obj in new List<object?>(tabs.Items))
        {
            if (obj is TabItem tab)
                EnableTabUndocking(tabs, tab, titleResolver);
        }
    }

    public static void EnableTabUndocking(TabControl tabs, TabItem tab, Func<TabItem, string> titleResolver)
    {
        Point press = default;
        PointerPressedEventArgs? pressArgs = null;

        tab.AddHandler(PointerPressedEvent, (_, e) =>
        {
            if (!e.GetCurrentPoint(tab).Properties.IsLeftButtonPressed) return;
            press = e.GetPosition(tab);
            pressArgs = e;
        }, RoutingStrategies.Tunnel | RoutingStrategies.Bubble, handledEventsToo: true);

        tab.AddHandler(PointerReleasedEvent, (_, _) => pressArgs = null, RoutingStrategies.Tunnel | RoutingStrategies.Bubble, handledEventsToo: true);

        tab.AddHandler(PointerMovedEvent, (_, e) =>
        {
            if (pressArgs == null) return;
            if (!e.GetCurrentPoint(tab).Properties.IsLeftButtonPressed)
            {
                pressArgs = null;
                return;
            }
            var pos = e.GetPosition(tab);
            // Tear out only when the pointer leaves the tab strip vertically (horizontal moves just change the tab)
            if (Math.Abs(pos.Y - press.Y) < tab.Bounds.Height + DragThresholdPx)
                return;

            var args = pressArgs;
            pressArgs = null;
            UndockTab(tabs, tab, titleResolver(tab), args, tab.PointToScreen(pos));
        }, RoutingStrategies.Tunnel | RoutingStrategies.Bubble, handledEventsToo: true);
    }

    public static void UndockTab(TabControl tabs, TabItem tab, string title, PointerPressedEventArgs? dragArgs, PixelPoint? screenPoint)
    {
        int index = tabs.Items.IndexOf(tab);
        if (index < 0 || tab.Content is not Control content) return;

        var owner = TopLevel.GetTopLevel(tabs) as Window;
        double w = Math.Max(360, tabs.Bounds.Width);
        double h = Math.Max(260, tabs.Bounds.Height);

        tab.Content = null;
        tabs.Items.Remove(tab);
        if (tabs.SelectedIndex < 0 && tabs.Items.Count > 0)
            tabs.SelectedIndex = 0;

        var win = CreateFloatingWindow(title, content, w, h, tabs.Background);
        if (screenPoint is { } sp)
        {
            win.WindowStartupLocation = WindowStartupLocation.Manual;
            win.Position = new PixelPoint(sp.X - 40, Math.Max(0, sp.Y - 12));
        }
        win.Closing += (_, _) =>
        {
            win.Content = null;
            tab.Content = content;
            tabs.Items.Insert(Math.Min(index, tabs.Items.Count), tab);
            tabs.SelectedItem = tab;
        };

        if (owner != null) win.Show(owner);
        else win.Show();

        if (dragArgs != null)
        {
            Dispatcher.UIThread.Post(() =>
            {
                try { win.BeginMoveDrag(dragArgs); }
                catch { /* Platform without native move-drag */ }
            }, DispatcherPriority.Input);
        }
    }
}
