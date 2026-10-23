using System;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;
using Neoveg.Controls;

namespace Neoveg.Styles;

/// <summary>
/// VEGAS Pro docked-window tabs for every Neoveg dock panel (Project Media, Trimmer, Video FX, Transitions...):
/// - Tab strip in the footer of the window (`TabStripPlacement = Bottom`).
/// - Compact flat tab headers (`FontSize = 11`, `Padding = 6,2,6,2`, 22px height).
/// - Horizontal scrolling tab strip with left/right `RepeatButton` arrows, shown only when the tabs overflow.
/// Opt in with <see cref="Apply"/> (adds the <see cref="DockTabsClass"/> style class).
/// </summary>
public static class NeovegDockTabStyles
{
    public const string DockTabsClass = "neoveg-dock-tabs";
    public const string DockTabItemClass = "neoveg-dock-tab";
    private const double StripHeight = 22.0;
    private const double ScrollStepPx = 60.0;

    private static readonly StreamGeometry ArrowLeft = StreamGeometry.Parse("M 10,3.5 L 5.5,8 L 10,12.5 Z");
    private static readonly StreamGeometry ArrowRight = StreamGeometry.Parse("M 6,3.5 L 10.5,8 L 6,12.5 Z");

    /// <summary>Marks a dock panel <see cref="TabControl"/> to use the VEGAS compact bottom tab strip.</summary>
    public static void Apply(TabControl tabs)
    {
        tabs.Classes.Add(DockTabsClass);
        tabs.TabStripPlacement = Dock.Bottom;

        // The Fluent TabItem theme (TabItemHeaderFontSize / TabItemMinHeight) wins over the class style on the
        // header, so the compact metrics are also set as local values on every tab, including tabs added or
        // re-docked later.
        foreach (var item in tabs.Items)
        {
            if (item is TabItem tab) ApplyCompactHeader(tab);
        }
        tabs.Items.CollectionChanged += (_, e) =>
        {
            if (e.NewItems == null) return;
            foreach (var item in e.NewItems)
            {
                if (item is TabItem tab) ApplyCompactHeader(tab);
            }
        };
    }

    public static void ApplyCompactHeader(TabItem tab)
    {
        if (!tab.Classes.Contains(DockTabItemClass))
            tab.Classes.Add(DockTabItemClass);
        tab.FontSize = 11.0;
        tab.FontWeight = FontWeight.Normal;
        tab.Padding = new Thickness(6, 2, 6, 2);
        tab.MinHeight = 0.0;
        tab.Height = StripHeight;
        tab.Margin = new Thickness(0);
        tab.VerticalContentAlignment = VerticalAlignment.Center;

        // The Fluent TabItem theme re-enables its accent underline (PART_SelectedPipe) and selected background
        // through its own pseudo-class rules, so the flat VEGAS look is enforced as local values on the template parts.
        tab.TemplateApplied -= OnDockTabTemplateApplied;
        tab.TemplateApplied += OnDockTabTemplateApplied;
    }

    private static readonly IBrush SelectedTabBrush = new SolidColorBrush(Color.Parse("#2a3446"));

    private static void OnDockTabTemplateApplied(object? sender, TemplateAppliedEventArgs e)
    {
        if (sender is not TabItem tab) return;
        if (e.NameScope.Find<Border>("PART_SelectedPipe") is { } pipe)
            pipe.IsVisible = false;
        if (e.NameScope.Find<Border>("PART_LayoutRoot") is not { } root)
            return;

        void Sync() => root.Background = tab.IsSelected ? SelectedTabBrush : Brushes.Transparent;
        Sync();
        tab.PropertyChanged += (_, args) =>
        {
            if (args.Property == TabItem.IsSelectedProperty)
                Sync();
        };
    }

    /// <summary>Application-level styles (registered in <c>App.Initialize</c> after the Fluent theme).</summary>
    public static Avalonia.Styling.Styles Create()
    {
        Selector dockTabs = default(Selector).OfType<TabControl>().Class(DockTabsClass);
        // Tabs carry their own class (added in ApplyCompactHeader) so the selector never depends on tree traversal
        Selector tabItem = default(Selector).OfType<TabItem>().Class(DockTabItemClass);

        return new Avalonia.Styling.Styles
        {
            new Style(_ => dockTabs)
            {
                Setters =
                {
                    new Setter(TabControl.TabStripPlacementProperty, Dock.Bottom),
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(TemplatedControl.TemplateProperty, new FuncControlTemplate<TabControl>(BuildTabControlTemplate))
                }
            },
            new Style(_ => tabItem)
            {
                Setters =
                {
                    new Setter(TemplatedControl.FontSizeProperty, 11.0),
                    new Setter(TemplatedControl.FontWeightProperty, FontWeight.Normal),
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(6, 2, 6, 2)),
                    new Setter(Layoutable.MinHeightProperty, 0.0),
                    new Setter(Layoutable.HeightProperty, StripHeight),
                    new Setter(Layoutable.MarginProperty, new Thickness(0)),
                    new Setter(TemplatedControl.ForegroundProperty, new SolidColorBrush(Color.Parse("#94a3b8"))),
                    new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent)
                }
            },
            // Flat VEGAS look: the selected tab is a lighter "card" joined to the content, no Fluent underline pipe
            new Style(_ => tabItem.Class(":selected").Template().OfType<Border>().Name("PART_LayoutRoot"))
            {
                Setters = { new Setter(Border.BackgroundProperty, new SolidColorBrush(Color.Parse("#2a3446"))) }
            },
            new Style(_ => tabItem.Class(":selected"))
            {
                Setters = { new Setter(TemplatedControl.ForegroundProperty, new SolidColorBrush(Color.Parse("#f8fafc"))) }
            },
            new Style(_ => tabItem.Class(":pointerover").Template().OfType<Border>().Name("PART_LayoutRoot"))
            {
                Setters = { new Setter(Border.BackgroundProperty, new SolidColorBrush(Color.Parse("#1f2836"))) }
            },
            new Style(_ => tabItem.Template().OfType<Border>().Name("PART_SelectedPipe"))
            {
                Setters = { new Setter(Visual.IsVisibleProperty, false) }
            }
        };
    }

    private static Control BuildTabControlTemplate(TabControl tc, INameScope scope)
    {
        var contentHost = new ContentPresenter
        {
            Name = "PART_SelectedContentHost",
            [!ContentPresenter.ContentProperty] = tc[!TabControl.SelectedContentProperty],
            [!ContentPresenter.ContentTemplateProperty] = tc[!TabControl.SelectedContentTemplateProperty]
        }.RegisterInNameScope(scope);

        var itemsPresenter = new ItemsPresenter
        {
            Name = "PART_ItemsPresenter",
            ItemsPanel = new FuncTemplate<Panel?>(() => new StackPanel { Orientation = Orientation.Horizontal })
        }.RegisterInNameScope(scope);

        var stripScroll = new ScrollViewer
        {
            HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Content = itemsPresenter
        };

        var btnLeft = CreateArrowButton(ArrowLeft, "Aba anterior");
        var btnRight = CreateArrowButton(ArrowRight, "Próxima aba");
        btnLeft.Click += (_, _) => ScrollStrip(stripScroll, -ScrollStepPx);
        btnRight.Click += (_, _) => ScrollStrip(stripScroll, ScrollStepPx);

        // Mouse wheel over the tab strip scrolls it horizontally
        stripScroll.AddHandler(InputElement.PointerWheelChangedEvent, (_, e) =>
        {
            double delta = Math.Abs(e.Delta.X) > Math.Abs(e.Delta.Y) ? e.Delta.X : e.Delta.Y;
            ScrollStrip(stripScroll, -delta * ScrollStepPx);
            e.Handled = true;
        }, Avalonia.Interactivity.RoutingStrategies.Tunnel);

        // Arrows appear only when the tabs do not fit the panel width
        void UpdateArrows()
        {
            bool overflow = stripScroll.Extent.Width > stripScroll.Viewport.Width + 0.5;
            btnLeft.IsVisible = overflow;
            btnRight.IsVisible = overflow;
            btnLeft.IsEnabled = stripScroll.Offset.X > 0.5;
            btnRight.IsEnabled = stripScroll.Offset.X < stripScroll.Extent.Width - stripScroll.Viewport.Width - 0.5;
        }
        stripScroll.PropertyChanged += (_, e) =>
        {
            if (e.Property == ScrollViewer.ExtentProperty ||
                e.Property == ScrollViewer.ViewportProperty ||
                e.Property == ScrollViewer.OffsetProperty)
            {
                UpdateArrows();
            }
        };

        // Keep the selected tab visible when it is changed by keyboard, code or after a resize
        tc.SelectionChanged += (_, _) =>
        {
            if (tc.ContainerFromIndex(tc.SelectedIndex) is Control selected)
                selected.BringIntoView();
        };

        var stripGrid = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto") };
        Grid.SetColumn(btnLeft, 0);
        Grid.SetColumn(stripScroll, 1);
        Grid.SetColumn(btnRight, 2);
        stripGrid.Children.Add(btnLeft);
        stripGrid.Children.Add(stripScroll);
        stripGrid.Children.Add(btnRight);

        var strip = new Border
        {
            Height = StripHeight + 1,
            Background = new SolidColorBrush(Color.Parse("#111620")),
            BorderBrush = new SolidColorBrush(Color.Parse("#2a3446")),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = stripGrid
        };
        DockPanel.SetDock(strip, Dock.Bottom);

        var root = new DockPanel { LastChildFill = true };
        root.Children.Add(strip);
        root.Children.Add(contentHost);

        return new Border
        {
            [!Border.BackgroundProperty] = tc[!TemplatedControl.BackgroundProperty],
            Child = root
        };
    }

    private static void ScrollStrip(ScrollViewer scroll, double deltaPx)
    {
        double maxX = Math.Max(0.0, scroll.Extent.Width - scroll.Viewport.Width);
        scroll.Offset = new Vector(Math.Clamp(scroll.Offset.X + deltaPx, 0.0, maxX), scroll.Offset.Y);
    }

    private static RepeatButton CreateArrowButton(StreamGeometry arrow, string tooltip)
    {
        var btn = new RepeatButton
        {
            Width = 16,
            Height = StripHeight,
            Padding = new Thickness(0),
            Delay = 300,
            Interval = 60,
            IsVisible = false,
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            Content = NeovegIcons.Create(arrow, 10, filled: true)
        };
        ToolTip.SetTip(btn, tooltip);
        return btn;
    }
}
