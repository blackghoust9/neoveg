namespace ScriptPortal.Neoveg;

/// <summary>
/// Namespace compatibility shim so classic Neoveg scripts with `using ScriptPortal.Neoveg;` compile unmodified.
/// </summary>
public sealed class ScriptPortalBridge
{
    public static string CompatibilityVersion => "ScriptPortal.Neoveg 21.0 Bridge -> Neoveg.Scripting";
}
