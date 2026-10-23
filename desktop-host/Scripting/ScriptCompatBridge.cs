namespace Neoveg.Scripting.Compat;

/// <summary>
/// Namespace compatibility shim so classic Neoveg scripts with `using Neoveg.Scripting.Compat;` compile unmodified.
/// </summary>
public sealed class ScriptCompatBridge
{
    public static string CompatibilityVersion => "Neoveg.Scripting.Compat Bridge -> Neoveg.Scripting";
}
