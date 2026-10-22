namespace ScriptPortal.Vegas;

/// <summary>
/// Namespace compatibility shim so classic VEGAS Pro scripts with `using ScriptPortal.Vegas;` compile unmodified.
/// </summary>
public sealed class ScriptPortalBridge
{
    public static string CompatibilityVersion => "ScriptPortal.Vegas 21.0 Bridge -> Neoveg.Scripting";
}
