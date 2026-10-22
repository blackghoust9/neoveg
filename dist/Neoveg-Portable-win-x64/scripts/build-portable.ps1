# Neoveg Cross-Platform Portable Builder (Windows PowerShell)
# Builds portable distributions (~200MB-300MB) for Windows (.exe), Linux (.AppImage), and macOS (.app)
param(
    [string]$Targets = "all",
    [switch]$FrameworkDependent
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

if ($FrameworkDependent) {
    python "$RootDir\build_portable.py" --targets $Targets --framework-dependent
} else {
    python "$RootDir\build_portable.py" --targets $Targets
}
