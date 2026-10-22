using System;
using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;

namespace Neoveg.Views;

/// <summary>
/// Tela de Carregamento (Splash Screen) inspirada fielmente no clássico Sony Vegas Pro 12 (Build 563 64-bit).
/// Renderiza via Skia/Avalonia:
/// - Textura lateral direita em fibra de carbono escura com o emblema serifado no topo direito
/// - Cunha geométrica azul-ciano translúcida com facetas diagonais e padrão micro-halftone
/// - Triângulo "Play" 3D central com cantos arredondados e brilho especular
/// - Tipografia "Vegas™ Pro 12" à esquerda e bloco "Version 12.0 (Build 563) 64-bit / Licensed to / Serial" à direita
/// - Status dinâmico ("Updating plug-in cache...") e barra de progresso clássica branca/cinza na base esquerda
/// </summary>
public sealed class VegasSplashWindow : Window
{
    private readonly VegasSplashCanvas _canvas;
    private readonly DispatcherTimer _timer;
    private readonly Action? _onCompleted;
    private double _progress;

    private static readonly string[] StartupStages =
    {
        "Updating plug-in cache...",
        "Initializing Rust FFI Core (neoveg_core.dll 64-bit)...",
        "Updating plug-in cache... (OpenFX / Frei0r / 3D LUT .cube)",
        "Scanning 24-bit/96kHz WAV .sfk Peak Cache & Audio DSP...",
        "Provisioning Isolated Python FFmpeg & SO4 Reader...",
        "Starting Vegas Pro / Neoveg Workspace..."
    };

    public VegasSplashWindow(Action? onCompleted = null, double durationSeconds = 2.6)
    {
        _onCompleted = onCompleted;

        Title = "Vegas Pro 12.0 — Updating plug-in cache...";
        Width = 602;
        Height = 365;
        CanResize = false;
        SystemDecorations = SystemDecorations.None;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        TransparencyLevelHint = new[] { WindowTransparencyLevel.Transparent };
        Background = Brushes.Transparent;
        Topmost = true;

        _canvas = new VegasSplashCanvas
        {
            Progress = 0.45, // Começa já exibindo a barra parcial idêntica à referência e avança até 100%
            StatusText = StartupStages[0],
            LicensedUser = Environment.UserName is { Length: > 0 } u ? u : "David Johns"
        };

        Content = _canvas;

        // Permite fechar ao clicar se aberta manualmente
        PointerPressed += (_, e) =>
        {
            if (e.GetCurrentPoint(this).Properties.IsLeftButtonPressed && _progress > 0.65)
            {
                CompleteAndClose();
            }
        };

        double step = 1.0 / Math.Max(15.0, (durationSeconds * 1000.0) / 40.0);
        _progress = 0.18;

        _timer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(40)
        };
        _timer.Tick += (_, _) =>
        {
            _progress += step;
            double clamped = Math.Clamp(_progress, 0.0, 1.0);
            _canvas.Progress = clamped;

            int stageIdx = Math.Clamp((int)(clamped * StartupStages.Length), 0, StartupStages.Length - 1);
            // Mantém "Updating plug-in cache..." em destaque na maior parte do carregamento
            _canvas.StatusText = clamped < 0.55 ? "Updating plug-in cache..." : StartupStages[stageIdx];
            _canvas.InvalidateVisual();

            if (_progress >= 1.05)
            {
                CompleteAndClose();
            }
        };

        Opened += (_, _) => _timer.Start();
    }

    private void CompleteAndClose()
    {
        if (!_timer.IsEnabled) return;
        _timer.Stop();
        _onCompleted?.Invoke();
        Close();
    }
}

public sealed class VegasSplashCanvas : Control
{
    public double Progress { get; set; } = 0.45;
    public string StatusText { get; set; } = "Updating plug-in cache...";
    public string LicensedUser { get; set; } = "David Johns";
    public string SerialNumber { get; set; } = "1TF-RLFN-7JPY-DYHS";

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        double w = Bounds.Width;
        double h = Bounds.Height;
        if (w < 50 || h < 50) return;

        var fullRect = new Rect(0, 0, w, h);
        var roundedClip = new RoundedRect(fullRect, new CornerRadius(14, 14, 3, 3));

        using (context.PushClip(roundedClip))
        {
            // 1. FUNDO DIREITO: Textura de Fibra de Carbono Escura (#0A0C0F com trama diagonal metálica)
            context.FillRectangle(new SolidColorBrush(Color.Parse("#0A0C0F")), fullRect);
            DrawCarbonFiberWeave(context, w, h);

            // 2. EMBLEMA SUPERIOR DIREITO ("SONY®" no estilo clássico serifado branco)
            DrawTopRightSonyBrand(context, w, h);

            // 3. SOMBRA PROJETADA DA CUNHA PRINCIPAL SOBRE A FIBRA DE CARBONO
            var shadowWedge = BuildMainWedgeGeometry(w + 6, h + 4);
            context.DrawGeometry(new SolidColorBrush(Color.FromArgb(145, 0, 0, 0)), null, shadowWedge);

            // 4. CUNHA PRINCIPAL AZUL-CIANO (Varrendo da esquerda até a ponta arredondada à direita)
            var mainWedge = BuildMainWedgeGeometry(w, h);
            var mainWedgeGrad = new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0.0, 0.05, RelativeUnit.Relative),
                EndPoint = new RelativePoint(0.85, 0.95, RelativeUnit.Relative),
                GradientStops = new GradientStops
                {
                    new GradientStop(Color.Parse("#14E0FF"), 0.0),
                    new GradientStop(Color.Parse("#00C0F7"), 0.36),
                    new GradientStop(Color.Parse("#008EE0"), 0.70),
                    new GradientStop(Color.Parse("#0254A8"), 1.0)
                }
            };
            context.DrawGeometry(mainWedgeGrad, new Pen(new SolidColorBrush(Color.Parse("#3CE8FF")), 1.2), mainWedge);

            using (context.PushGeometryClip(mainWedge))
            {
                // 4.1 Facetas Geométricas Diagonais Translúcidas (Feixes de luz característicos do Vegas Pro 12)
                // Faceta triangular inferior esquerda (Azul Cobalto mais profundo)
                var lowerLeftFacet = new PolylineGeometry(new[]
                {
                    new Point(0, h * 0.56),
                    new Point(w * 0.76, h),
                    new Point(0, h)
                }, true);
                context.DrawGeometry(
                    new LinearGradientBrush
                    {
                        StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                        EndPoint = new RelativePoint(1, 1, RelativeUnit.Relative),
                        GradientStops = new GradientStops
                        {
                            new GradientStop(Color.FromArgb(165, 2, 74, 156), 0.0),
                            new GradientStop(Color.FromArgb(210, 3, 105, 198), 1.0)
                        }
                    },
                    null,
                    lowerLeftFacet);

                // Faceta diagonal luminosa superior-esquerda -> centro-direita
                var upperRayFacet = new PolylineGeometry(new[]
                {
                    new Point(0, 0),
                    new Point(w * 0.42, 0),
                    new Point(w * 0.90, h * 0.72),
                    new Point(0, h * 0.24)
                }, true);
                context.DrawGeometry(
                    new SolidColorBrush(Color.FromArgb(38, 125, 249, 255)),
                    null,
                    upperRayFacet);

                // Faceta diagonal secundária cruzada
                var midCrossFacet = new PolylineGeometry(new[]
                {
                    new Point(w * 0.16, h),
                    new Point(w * 0.68, h * 0.12),
                    new Point(w * 0.95, h * 0.48),
                    new Point(w * 0.58, h)
                }, true);
                context.DrawGeometry(
                    new SolidColorBrush(Color.FromArgb(42, 0, 58, 128)),
                    null,
                    midCrossFacet);

                // 4.2 Padrão Micro-Halftone Pontilhado Sutil dentro do Azul
                DrawCyanHalftoneDots(context, w, h);

                // 5. TRIÂNGULO "PLAY" CENTRAL 3D COM CANTOS ARREDONDADOS
                DrawInnerRoundedPlayTriangle(context, w, h);
            }

            // 6. TIPOGRAFIA PRINCIPAL À ESQUERDA: "Vegas™ Pro 12"
            DrawLeftVegasTitle(context, w, h);

            // 7. BLOCO DE INFORMAÇÕES DE VERSÃO E LICENÇA À DIREITA (Dentro do Triângulo Play)
            DrawRightVersionAndLicenseBlock(context, w, h);

            // 8. STATUS DE CARREGAMENTO ("Updating plug-in cache..."), BARRA DE PROGRESSO E COPYRIGHT
            DrawBottomLeftLoadingStatus(context, w, h);
        }
    }

    private static void DrawCarbonFiberWeave(DrawingContext context, double w, double h)
    {
        // Desenha a trama diagonal de fibra de carbono visível nos cantos direitos
        var dotLight = new SolidColorBrush(Color.Parse("#242932"));
        var dotMid = new SolidColorBrush(Color.Parse("#151920"));
        double startX = w * 0.56;
        for (double y = 0; y < h; y += 6)
        {
            int row = (int)(y / 6);
            for (double x = startX; x < w; x += 8)
            {
                double shift = (row % 2) * 4.0;
                context.FillRectangle(dotLight, new Rect(x + shift, y, 4.2, 2.6));
                context.FillRectangle(dotMid, new Rect(x + shift + 2.0, y + 3.0, 4.2, 2.5));
            }
        }
    }

    private static void DrawTopRightSonyBrand(DrawingContext context, double w, double h)
    {
        // Renderiza "SONY" no canto superior direito com tipografia serifada larga clássica + símbolo ®
        var sonyText = new FormattedText(
            "SONY",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Georgia, Times New Roman, serif", FontStyle.Normal, FontWeight.Bold),
            21.5,
            Brushes.White);

        double sx = w - sonyText.Width - 22;
        double sy = 15;

        // Sutil espaçamento expandido simulando o logotipo original
        context.DrawText(sonyText, new Point(sx, sy));

        var regText = new FormattedText(
            "®",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Segoe UI, Inter, sans-serif", FontStyle.Normal, FontWeight.Normal),
            7.5,
            Brushes.White);
        context.DrawText(regText, new Point(w - 19, sy + 12));
    }

    private static StreamGeometry BuildMainWedgeGeometry(double w, double h)
    {
        // Cunha azul principal com ponta arredondada à direita (x ~ 0.918*w, y ~ 0.50*h)
        var geo = new StreamGeometry();
        using (var ctx = geo.Open())
        {
            ctx.BeginFigure(new Point(0, 0), true);
            ctx.LineTo(new Point(w * 0.648, 0));
            ctx.LineTo(new Point(w * 0.902, h * 0.445));
            ctx.CubicBezierTo(
                new Point(w * 0.932, h * 0.485),
                new Point(w * 0.932, h * 0.518),
                new Point(w * 0.902, h * 0.558));
            ctx.LineTo(new Point(w * 0.612, h));
            ctx.LineTo(new Point(0, h));
            ctx.EndFigure(true);
        }
        return geo;
    }

    private static void DrawCyanHalftoneDots(DrawingContext context, double w, double h)
    {
        var dotBrush = new SolidColorBrush(Color.FromArgb(18, 255, 255, 255));
        for (double y = 12; y < h - 12; y += 11)
        {
            int r = (int)(y / 11);
            for (double x = 18; x < w * 0.88; x += 11)
            {
                double ox = (r % 2) * 5.5;
                context.FillRectangle(dotBrush, new Rect(x + ox, y, 2.2, 2.2));
            }
        }
    }

    private static void DrawInnerRoundedPlayTriangle(DrawingContext context, double w, double h)
    {
        // Triângulo Play central com cantos arredondados (idêntico à arte do Vegas Pro 12)
        double leftX = w * 0.453;
        double topY = h * 0.088;
        double botY = h * 0.885;
        double tipX = w * 0.858;
        double tipY = h * 0.490;

        var playGeo = new StreamGeometry();
        using (var ctx = playGeo.Open())
        {
            // Começa abaixo do canto superior esquerdo do triângulo
            ctx.BeginFigure(new Point(leftX, topY + 22), true);
            // Curva superior esquerda
            ctx.CubicBezierTo(
                new Point(leftX, topY - 2),
                new Point(leftX + 16, topY - 6),
                new Point(leftX + 34, topY + 8));
            // Reta superior até antes da ponta direita
            ctx.LineTo(new Point(tipX - 22, tipY - 20));
            // Ponta direita arredondada
            ctx.CubicBezierTo(
                new Point(tipX + 6, tipY - 4),
                new Point(tipX + 6, tipY + 6),
                new Point(tipX - 22, tipY + 22));
            // Reta inferior até o canto inferior esquerdo
            ctx.LineTo(new Point(leftX + 34, botY - 8));
            // Curva inferior esquerda
            ctx.CubicBezierTo(
                new Point(leftX + 16, botY + 6),
                new Point(leftX, botY + 2),
                new Point(leftX, botY - 22));
            ctx.EndFigure(true);
        }

        // Sombra suave externa do triângulo Play
        var shadowGeo = new StreamGeometry();
        using (var sctx = shadowGeo.Open())
        {
            sctx.BeginFigure(new Point(leftX - 3, topY + 24), true);
            sctx.LineTo(new Point(tipX + 2, tipY + 2));
            sctx.LineTo(new Point(leftX - 3, botY - 18));
            sctx.EndFigure(true);
        }
        context.DrawGeometry(new SolidColorBrush(Color.FromArgb(40, 0, 24, 68)), null, shadowGeo);

        var playGrad = new LinearGradientBrush
        {
            StartPoint = new RelativePoint(0.0, 0.0, RelativeUnit.Relative),
            EndPoint = new RelativePoint(0.9, 0.95, RelativeUnit.Relative),
            GradientStops = new GradientStops
            {
                new GradientStop(Color.FromArgb(230, 38, 228, 255), 0.0),
                new GradientStop(Color.FromArgb(215, 0, 184, 242), 0.45),
                new GradientStop(Color.FromArgb(225, 2, 118, 204), 1.0)
            }
        };

        var borderPen = new Pen(new SolidColorBrush(Color.FromArgb(140, 120, 244, 255)), 1.3);
        context.DrawGeometry(playGrad, borderPen, playGeo);

        // Brilho secundário curvo no terço superior do triângulo Play
        var innerGloss = new StreamGeometry();
        using (var gctx = innerGloss.Open())
        {
            gctx.BeginFigure(new Point(leftX + 16, topY + 24), true);
            gctx.LineTo(new Point(w * 0.695, h * 0.255));
            gctx.CubicBezierTo(
                new Point(w * 0.725, h * 0.285),
                new Point(w * 0.715, h * 0.340),
                new Point(w * 0.675, h * 0.365));
            gctx.LineTo(new Point(leftX + 16, h * 0.56));
            gctx.EndFigure(true);
        }
        context.DrawGeometry(new SolidColorBrush(Color.FromArgb(35, 185, 250, 255)), null, innerGloss);
    }

    private static void DrawLeftVegasTitle(DrawingContext context, double w, double h)
    {
        double baseX = 24;
        double baseY = h * 0.425;

        // "Vegas"
        var ftVegas = new FormattedText(
            "Vegas",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Segoe UI, Inter, sans-serif", FontStyle.Normal, FontWeight.Light),
            44,
            Brushes.White);
        context.DrawText(ftVegas, new Point(baseX, baseY));

        // "TM"
        var ftTm = new FormattedText(
            "TM",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Segoe UI, Inter, sans-serif", FontStyle.Normal, FontWeight.SemiBold),
            7.5,
            Brushes.White);
        context.DrawText(ftTm, new Point(baseX + ftVegas.Width + 1, baseY + 14));

        // "Pro"
        double proX = baseX + ftVegas.Width + 18;
        var ftPro = new FormattedText(
            "Pro",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Segoe UI, Inter, sans-serif", FontStyle.Normal, FontWeight.Light),
            44,
            Brushes.White);
        context.DrawText(ftPro, new Point(proX, baseY));

        // "12"
        double numX = proX + ftPro.Width + 9;
        var ftNum = new FormattedText(
            "12",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface("Segoe UI, Inter, sans-serif", FontStyle.Normal, FontWeight.ExtraLight),
            37,
            Brushes.White);
        context.DrawText(ftNum, new Point(numX, baseY + 7));
    }

    private void DrawRightVersionAndLicenseBlock(DrawingContext context, double w, double h)
    {
        double rx = w * 0.568;
        double ry = h * 0.402;

        DrawShadowedText(context, "Vegas Pro 12.0", new Point(rx, ry), 12.5, true);
        DrawShadowedText(context, "Version 12.0 (Build 563) 64-bit", new Point(rx, ry + 17), 12.0, false);

        DrawShadowedText(context, "Licensed to:", new Point(rx, ry + 41), 12.5, true);
        DrawShadowedText(context, LicensedUser, new Point(rx, ry + 58), 12.0, false);

        DrawShadowedText(context, SerialNumber, new Point(rx, ry + 93), 12.0, false);
    }

    private void DrawBottomLeftLoadingStatus(DrawingContext context, double w, double h)
    {
        double lx = 24;
        double statusY = h * 0.805;

        // "Updating plug-in cache..."
        DrawShadowedText(context, StatusText, new Point(lx, statusY), 13.0, false);

        // Barra de Progresso Clássica do Vegas Pro 12 (Fundo branco #FFFFFF + Preenchimento cinza-ardósia #686E78)
        double barY = h * 0.882;
        double barW = w * 0.336;
        double barH = 7.0;
        var outerBarRect = new Rect(lx, barY, barW, barH);
        context.FillRectangle(Brushes.White, outerBarRect);

        double fillW = Math.Max(4.0, (barW - 2.0) * Math.Clamp(Progress, 0.0, 1.0));
        var innerFillRect = new Rect(lx + 1.0, barY + 1.0, fillW, barH - 2.0);
        context.FillRectangle(new SolidColorBrush(Color.Parse("#686E78")), innerFillRect);

        // Copyright Footer
        double copyY = h * 0.922;
        DrawShadowedText(
            context,
            "Copyright © 2012 Sony Creative Software Inc. All Rights Reserved.",
            new Point(lx, copyY),
            10.5,
            false);
    }

    private static void DrawShadowedText(DrawingContext context, string text, Point pt, double fontSize, bool bold)
    {
        var tf = new Typeface("Tahoma, Segoe UI, Inter, sans-serif", FontStyle.Normal, bold ? FontWeight.Bold : FontWeight.Normal);
        var shadow = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            tf,
            fontSize,
            new SolidColorBrush(Color.FromArgb(65, 0, 25, 60)));
        context.DrawText(shadow, new Point(pt.X + 0.8, pt.Y + 0.8));

        var fg = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            tf,
            fontSize,
            Brushes.White);
        context.DrawText(fg, pt);
    }
}
