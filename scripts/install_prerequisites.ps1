<#
.SYNOPSIS
    Neoveg — Unified Silent Prerequisite Installer (Visual C++ 2015–2022 & DirectX End-User Runtimes June 2010).
.DESCRIPTION
    1. Executes silent installation (`/q /norestart`) of `vcredist2022_2015` packages located in
       `addon\vcredist2022-2015_14.36.32532\` (or `addon\vcredist_all_in_one\install_all.bat`).
    2. Executes silent extraction and installation of DirectX End-User Runtimes (June 2010):
       `directx_Jun2010_redist.exe /q /t:%TEMP%\dxredist && %TEMP%\dxredist\DXSETUP.exe /silent`
       Official source: https://www.techpowerup.com/download/directx-redistributable-runtime/
    3. Legacy `.NET Framework 4.8` (`ndp48-web.exe`) is bypassed because Neoveg is published as a
       Self-Contained `.NET 8.0` (`win-x64`, `--self-contained true`) application.
#>

[CmdletBinding()]
param(
    [string]$AddonSubPath = "addon\vcredist2022-2015_14.36.32532",
    [string]$DirectXSubPath = "addon\directx_redist"
)

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = Split-Path -Parent $ScriptDir
$VcRedistDir = Join-Path $RootDir $AddonSubPath
$DxRedistDir = Join-Path $RootDir $DirectXSubPath

Write-Host "============================================================================" -ForegroundColor Cyan
Write-Host " Neoveg — Instalador Unificado Silencioso (Visual C++ & DirectX Runtimes)" -ForegroundColor Cyan
Write-Host "============================================================================" -ForegroundColor Cyan
Write-Host "[INFO] Diretorio VC++: $VcRedistDir"
Write-Host "[INFO] Diretorio DirectX: $DxRedistDir"
Write-Host "[INFO] .NET 8.0 Self-Contained ativo no Neoveg (dispensa ndp48-web.exe)."

# 1. Visual C++ Redistributable
$Installers = @()
if (Test-Path $VcRedistDir) {
    $Installers = Get-ChildItem -Path $VcRedistDir -Filter "*.exe" -File |
        Where-Object { $_.Name -match "vcredist|vc_redist" }
}

if ($Installers.Count -gt 0) {
    foreach ($Exe in $Installers) {
        Write-Host "[EXEC] Executando: $($Exe.FullName) /q /norestart" -ForegroundColor Yellow
        $Proc = Start-Process -FilePath $Exe.FullName -ArgumentList "/q /norestart" -Wait -PassThru -NoNewWindow
        switch ($Proc.ExitCode) {
            0    { Write-Host "[OK] $($Exe.Name) instalado com sucesso (ExitCode 0)." -ForegroundColor Green }
            1638 { Write-Host "[OK] Versao equivalente ou superior do VC++ Redistributable ja instalada (ExitCode 1638)." -ForegroundColor Green }
            3010 { Write-Host "[OK] $($Exe.Name) instalado (reinicializacao suprimida via /norestart, ExitCode 3010)." -ForegroundColor Green }
            default { Write-Host "[WARN] $($Exe.Name) finalizou com codigo $($Proc.ExitCode)." -ForegroundColor Yellow }
        }
    }
} else {
    $SysVcRuntime = Join-Path $env:SystemRoot "System32\vcruntime140.dll"
    if (Test-Path $SysVcRuntime) {
        Write-Host "[OK] Runtime Visual C++ 2015-2022 (vcruntime140.dll) detectada no sistema operacional." -ForegroundColor Green
    } else {
        Write-Host "[WARN] Nenhum executavel VC++ encontrado em '$AddonSubPath'." -ForegroundColor Yellow
    }
}

# 2. DirectX End-User Runtimes (June 2010): directx_Jun2010_redist.exe /q /t:%TEMP%\dxredist && %TEMP%\dxredist\DXSETUP.exe /silent
$DxExeCandidates = @(
    (Join-Path $DxRedistDir "directx_Jun2010_redist.exe"),
    (Join-Path $RootDir "addon\directx_Jun2010_redist.exe"),
    (Join-Path $VcRedistDir "directx_Jun2010_redist.exe")
)
$DxRedistExe = $DxExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($null -ne $DxRedistExe) {
    $TempDxDir = Join-Path $env:TEMP "dxredist"
    New-Item -ItemType Directory -Force -Path $TempDxDir | Out-Null
    Write-Host "[EXEC] Extraindo DirectX Runtimes: $DxRedistExe /q /t:$TempDxDir" -ForegroundColor Yellow
    $ExtractProc = Start-Process -FilePath $DxRedistExe -ArgumentList "/q /t:`"$TempDxDir`"" -Wait -PassThru -NoNewWindow
    $DxSetupExe = Join-Path $TempDxDir "DXSETUP.exe"
    if (($ExtractProc.ExitCode -eq 0) -and (Test-Path $DxSetupExe)) {
        Write-Host "[EXEC] Instalando DirectX Runtimes: $DxSetupExe /silent" -ForegroundColor Yellow
        $SetupProc = Start-Process -FilePath $DxSetupExe -ArgumentList "/silent" -Wait -PassThru -NoNewWindow
        Write-Host "[OK] DirectX End-User Runtimes instalado (ExitCode $($SetupProc.ExitCode))." -ForegroundColor Green
    }
} else {
    $D3d11Path = Join-Path $env:SystemRoot "System32\d3d11.dll"
    $XAudio27Path = Join-Path $env:SystemRoot "System32\XAudio2_7.dll"
    Write-Host "[INFO] directx_Jun2010_redist.exe nao encontrado localmente (https://www.techpowerup.com/download/directx-redistributable-runtime/)."
    Write-Host "[INFO] Status DirectX System32: d3d11.dll=$((Test-Path $D3d11Path)) | XAudio2_7.dll=$((Test-Path $XAudio27Path))"
}

exit 0
