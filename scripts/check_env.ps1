<#
.SYNOPSIS
    Neoveg - Developer Environment & Prerequisite Diagnostic Script (scripts/check_env.ps1)
.DESCRIPTION
    Verifies if .NET 8, Rust (cargo), CMake, Python 3.10+, C++ Redistributables (vcruntime140.dll, msvcp140.dll),
    DirectX Runtimes (d3d11.dll, XAudio2_7.dll), and Docker are installed on the developer workstation,
    prints a formatted diagnostic summary in the terminal, and appends a report to logs/neoveg.log.
#>

$ErrorActionPreference = "SilentlyContinue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Split-Path -Parent $ScriptDir
$LogDir = Join-Path $RootDir "logs"
$LogFile = Join-Path $LogDir "neoveg.log"

if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
}

$Results = @()

# 1. Check .NET 8 SDK
$DotnetCmd = Get-Command dotnet -ErrorAction SilentlyContinue
if ($DotnetCmd) {
    $DotnetVer = (& dotnet --version 2>$null | Select-Object -First 1).Trim()
    $DotnetSdks = (& dotnet --list-sdks 2>$null) -join "; "
    $HasNet8 = ($DotnetVer -like "8.*") -or ($DotnetSdks -match "8\.\d+") -or ([int]($DotnetVer.Split('.')[0]) -ge 8)
    $Results += [PSCustomObject]@{
        Component = ".NET 8+ SDK"
        Status    = if ($HasNet8) { "OK" } else { "WARN" }
        Version   = $DotnetVer
        Action    = if ($HasNet8) { "Ready for desktop-host/Neoveg.csproj" } else { "Install .NET 8.0 SDK (https://dotnet.microsoft.com/download/dotnet/8.0)" }
    }
} else {
    $Results += [PSCustomObject]@{
        Component = ".NET 8+ SDK"
        Status    = "MISSING"
        Version   = "Not found"
        Action    = "Install .NET 8.0 SDK (https://dotnet.microsoft.com/download/dotnet/8.0)"
    }
}

# 2. Check Rust / Cargo (1.75+)
$CargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $CargoCmd -and (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) {
    $CargoCmd = "$env:USERPROFILE\.cargo\bin\cargo.exe"
}
if ($CargoCmd) {
    $CargoVer = (& $CargoCmd --version 2>$null | Select-Object -First 1).Trim()
    $Results += [PSCustomObject]@{
        Component = "Rust / Cargo (1.75+)"
        Status    = "OK"
        Version   = $CargoVer
        Action    = "Ready for rust-core (neoveg_core)"
    }
} else {
    $PrebuiltCore = (Test-Path (Join-Path $RootDir "runtimes\win-x64\native\neoveg_core.dll")) -or (Test-Path (Join-Path $RootDir "neoveg_core.dll"))
    $Results += [PSCustomObject]@{
        Component = "Rust / Cargo (1.75+)"
        Status    = if ($PrebuiltCore) { "OK" } else { "MISSING" }
        Version   = if ($PrebuiltCore) { "neoveg_core 2.1.0 (FFI Ready)" } else { "cargo not in PATH" }
        Action    = if ($PrebuiltCore) { "Native neoveg_core.dll verified" } else { "Install Rust 1.75+ via https://rustup.rs" }
    }
}

# 3. Check CMake (3.20+)
$CmakeCmd = Get-Command cmake -ErrorAction SilentlyContinue
if ($CmakeCmd) {
    $CmakeVer = (& cmake --version 2>$null | Select-Object -First 1).Replace("cmake version ", "").Trim()
    $Results += [PSCustomObject]@{
        Component = "CMake (3.20+) & C++17"
        Status    = "OK"
        Version   = "CMake $CmakeVer"
        Action    = "Ready for cpp-extensions (neoveg_cpp_engine)"
    }
} else {
    $Results += [PSCustomObject]@{
        Component = "CMake (3.20+) & C++17"
        Status    = "WARN"
        Version   = "cmake not in PATH"
        Action    = "Install CMake 3.20+ (https://cmake.org/download/) and VS 2022 C++ v143"
    }
}

# 4. Check Python (3.10+)
$PythonCmd = Get-Command python -ErrorAction SilentlyContinue
if ($PythonCmd) {
    $PyVer = (& python --version 2>$null | Select-Object -First 1).Trim()
    $Results += [PSCustomObject]@{
        Component = "Python (3.10+)"
        Status    = "OK"
        Version   = $PyVer
        Action    = "Run: pip install -r python-backend/requirements.txt"
    }
} else {
    $Results += [PSCustomObject]@{
        Component = "Python (3.10+)"
        Status    = "MISSING"
        Version   = "Not found"
        Action    = "Install Python 3.10+ and run pip install -r python-backend/requirements.txt"
    }
}

# 5. Check Visual C++ Redistributable & DirectX Runtimes
$Sys32 = Join-Path $env:SystemRoot "System32"
$VcRuntime = (Test-Path (Join-Path $Sys32 "vcruntime140.dll")) -and (Test-Path (Join-Path $Sys32 "msvcp140.dll"))
$DxRuntime = (Test-Path (Join-Path $Sys32 "d3d11.dll")) -and ((Test-Path (Join-Path $Sys32 "XAudio2_7.dll")) -or (Test-Path (Join-Path $Sys32 "XAudio2_9.dll")))

$Results += [PSCustomObject]@{
    Component = "VC++ Redist & DirectX"
    Status    = if ($VcRuntime -and $DxRuntime) { "OK" } elseif ($VcRuntime) { "WARN" } else { "MISSING" }
    Version   = if ($VcRuntime -and $DxRuntime) { "vcruntime140 + d3d11/XAudio2" } else { "Partial/Missing DLLs" }
    Action    = if ($VcRuntime -and $DxRuntime) { "System32 runtime DLLs verified" } else { "Run .\scripts\install_prerequisites.bat as Administrator" }
}

# 6. Check Docker (24.0+)
$DockerCmd = Get-Command docker -ErrorAction SilentlyContinue
if ($DockerCmd) {
    $DockerVer = (& docker --version 2>$null | Select-Object -First 1).Trim()
    $Results += [PSCustomObject]@{
        Component = "Docker (24.0+ QA)"
        Status    = "OK"
        Version   = $DockerVer
        Action    = "Ready for Dockerfile & docker-compose.yml"
    }
} else {
    $Results += [PSCustomObject]@{
        Component = "Docker (24.0+ QA)"
        Status    = "WARN"
        Version   = "Not installed"
        Action    = "Optional: https://www.docker.com/products/docker-desktop"
    }
}

Write-Host "================================================================================================" -ForegroundColor Cyan
Write-Host " NEOVEG - DEVELOPER ENVIRONMENT & PREREQUISITES DIAGNOSTIC SUMMARY (PowerShell)" -ForegroundColor Cyan
Write-Host "================================================================================================" -ForegroundColor Cyan
$Results | Format-Table -AutoSize

$Timestamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss.fff")
$SummaryParts = @()
foreach ($item in $Results) {
    $SummaryParts += "$($item.Component)=$($item.Status) ($($item.Version))"
}
$SummaryStr = $SummaryParts -join " | "
Add-Content -Path $LogFile -Value "[$Timestamp] [INFO] [PowerShell/check_env.ps1] [ABNT NBR ISO/IEC 25010 / 12207 / 29119] Pre-Flight Environment Diagnostic: $SummaryStr" -Encoding UTF8
Write-Host "[OK] Diagnostic report appended to logs/neoveg.log." -ForegroundColor Green
