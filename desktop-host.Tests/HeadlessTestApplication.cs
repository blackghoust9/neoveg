using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Headless.XUnit;
using Xunit;

[assembly: AvaloniaTestApplication(typeof(Neoveg.Tests.HeadlessTestApplication))]

namespace Neoveg.Tests;

public sealed class HeadlessTestApplication
{
    public static AppBuilder BuildAvaloniaApp() =>
        AppBuilder.Configure<HeadlessTestApp>()
            .UseHeadless(new AvaloniaHeadlessPlatformOptions());

    [AvaloniaFact]
    public void Headless_platform_can_create_controls()
    {
        var button = new Button { Content = "Neoveg" };

        Assert.Equal("Neoveg", button.Content);
    }

}

public sealed class HeadlessTestApp : Application
{
}
