<#
.SYNOPSIS
    Neoveg - Automated Cross-Platform Self-Contained Portable Build & Packaging Pipeline (scripts/build_all_platforms.ps1)
.DESCRIPTION
    Builds and packages self-contained (.NET 8) portable distributions identical in structure to
    G:\Meu Drive\vp-rev-eng\teste\teste2, bundling unpacked .NET/Avalonia assemblies, native Rust (neoveg_core)
    and C++ (neoveg_cpp_engine) FFI libraries, launcher scripts (neoveg.bat / neoveg.sh), Assets, media, projects,
    and addon prerequisites for:
      - Windows x64   : dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true
                        -> dist/Neoveg_v1.0.0_win-x64_Portable.zip & dist/Neoveg_v1.0.0_win-x64.zip
      - Linux x64     : dotnet publish desktop-host/Neoveg.csproj -c Release -r linux-x64 --self-contained true
                        -> dist/Neoveg_v1.0.0_linux-x64_Portable.tar.gz
      - macOS x64     : dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-x64 --self-contained true
                        -> dist/Neoveg_v1.0.0_osx-x64_Portable.tar.gz
      - macOS ARM64   : dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-arm64 --self-contained true
                        -> dist/Neoveg_v1.0.0_osx-arm64_Portable.tar.gz
                        -> dist/Neoveg_v1.0.0_osx-universal_Portable.tar.gz
#>

param(
    [switch]$SkipGDriveSync = $false
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RootDir = (Resolve-Path (Join-Path $ScriptDir "..")).Path
$DistDir = Join-Path $RootDir "dist"
$CsprojPath = Join-Path $RootDir "desktop-host\Neoveg.csproj"

if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
}

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host " NEOVEG - MULTI-PLATFORM SELF-CONTAINED PORTABLE RELEASE BUILDER" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan

# 1. Build / Synthesize Native Rust (neoveg_core) & C++ (neoveg_cpp_engine) FFI Libraries
Write-Host "[1/6] Building & staging Rust (neoveg_core) and C++ (neoveg_cpp_engine) FFI libraries..." -ForegroundColor Yellow
& python (Join-Path $RootDir "python-backend\native_ffi_builder.py")
if ($LASTEXITCODE -ne 0) {
    throw "Native FFI builder failed with exit code $LASTEXITCODE"
}

# 2. Define Target Platforms & Canonical dotnet publish commands
# Canonical commands:
#   dotnet publish desktop-host/Neoveg.csproj -c Release -r win-x64 --self-contained true
#   dotnet publish desktop-host/Neoveg.csproj -c Release -r linux-x64 --self-contained true
#   dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-x64 --self-contained true
#   dotnet publish desktop-host/Neoveg.csproj -c Release -r osx-arm64 --self-contained true

$Targets = @(
    @{
        Rid         = "win-x64"
        FolderName  = "Neoveg_v1.0.0_win-x64_Portable"
        ArchiveName = "Neoveg_v1.0.0_win-x64_Portable.zip"
        ArchiveType = "zip"
        NativeLibs  = @("neoveg_core.dll", "neoveg_cpp_engine.dll")
        Launcher    = "neoveg.bat"
    },
    @{
        Rid         = "linux-x64"
        FolderName  = "Neoveg_v1.0.0_linux-x64_Portable"
        ArchiveName = "Neoveg_v1.0.0_linux-x64_Portable.tar.gz"
        ArchiveType = "targz"
        NativeLibs  = @("libneoveg_core.so", "libneoveg_cpp_engine.so")
        Launcher    = "neoveg.sh"
    },
    @{
        Rid         = "osx-x64"
        FolderName  = "Neoveg_v1.0.0_osx-x64_Portable"
        ArchiveName = "Neoveg_v1.0.0_osx-x64_Portable.tar.gz"
        ArchiveType = "targz"
        NativeLibs  = @("libneoveg_core.dylib", "libneoveg_cpp_engine.dylib")
        Launcher    = "neoveg.sh"
    },
    @{
        Rid         = "osx-arm64"
        FolderName  = "Neoveg_v1.0.0_osx-arm64_Portable"
        ArchiveName = "Neoveg_v1.0.0_osx-arm64_Portable.tar.gz"
        ArchiveType = "targz"
        NativeLibs  = @("libneoveg_core.dylib", "libneoveg_cpp_engine.dylib")
        Launcher    = "neoveg.sh"
    }
)

Write-Host "[2/6] Publishing unpacked .NET 8 Self-Contained binaries for win-x64, linux-x64, osx-x64, osx-arm64..." -ForegroundColor Yellow

foreach ($Target in $Targets) {
    $Rid = $Target.Rid
    $OutDir = Join-Path $DistDir $Target.FolderName
    if (Test-Path $OutDir) {
        Remove-Item -LiteralPath $OutDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

    Write-Host "  -> Running: dotnet publish desktop-host/Neoveg.csproj -c Release -r $Rid --self-contained true" -ForegroundColor Gray
    & dotnet publish $CsprojPath -c Release -r $Rid --self-contained true -o $OutDir
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet publish failed for RID $Rid"
    }

    # 3. Bundle Native Rust/C++ Shared Libraries (.dll / .so / .dylib) in root and runtimes/<rid>/native
    $RuntimeNativeDst = Join-Path $OutDir "runtimes\$Rid\native"
    New-Item -ItemType Directory -Path $RuntimeNativeDst -Force | Out-Null
    foreach ($LibName in $Target.NativeLibs) {
        $SrcLib = Join-Path $RootDir "runtimes\$Rid\native\$LibName"
        if (Test-Path $SrcLib) {
            Copy-Item -LiteralPath $SrcLib -Destination (Join-Path $OutDir $LibName) -Force
            Copy-Item -LiteralPath $SrcLib -Destination (Join-Path $RuntimeNativeDst $LibName) -Force
        }
    }

    # Bundle Launcher Scripts (neoveg.bat / Executar_Neoveg.cmd for Windows, neoveg.sh for Linux/macOS)
    $ScriptsDst = Join-Path $OutDir "scripts"
    New-Item -ItemType Directory -Path $ScriptsDst -Force | Out-Null
    $LauncherSrc = Join-Path $RootDir "scripts\$($Target.Launcher)"
    if (Test-Path $LauncherSrc) {
        Copy-Item -LiteralPath $LauncherSrc -Destination (Join-Path $OutDir $Target.Launcher) -Force
        Copy-Item -LiteralPath $LauncherSrc -Destination (Join-Path $ScriptsDst $Target.Launcher) -Force
    }

    # Copy all scripts from scripts/
    Get-ChildItem -Path (Join-Path $RootDir "scripts") -File | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $ScriptsDst $_.Name) -Force
    }

    if ($Rid -eq "win-x64") {
        $CmdLauncher = Join-Path $OutDir "Executar_Neoveg.cmd"
        [System.IO.File]::WriteAllText($CmdLauncher, "@echo off`r`nSET SEE_MASK_NOZONECHECKS=1`r`nstart `"`" `"%~dp0Neoveg.exe`" %*`r`n", [System.Text.Encoding]::UTF8)

        $VcRedistSrc = Join-Path $RootDir "addon\vcredist2022-2015_14.36.32532"
        if (Test-Path $VcRedistSrc) {
            $VcRedistDst = Join-Path $OutDir "addon\vcredist2022-2015_14.36.32532"
            New-Item -ItemType Directory -Path $VcRedistDst -Force | Out-Null
            Copy-Item -Path "$VcRedistSrc\*" -Destination $VcRedistDst -Recurse -Force
        }
    } else {
        # Ensure LF line endings on neoveg.sh inside Linux/macOS packages
        foreach ($ShPath in @((Join-Path $OutDir "neoveg.sh"), (Join-Path $ScriptsDst "neoveg.sh"))) {
            if (Test-Path $ShPath) {
                $RawBytes = [System.IO.File]::ReadAllBytes($ShPath)
                $LfText = [System.Text.Encoding]::UTF8.GetString($RawBytes).Replace("`r`n", "`n")
                [System.IO.File]::WriteAllBytes($ShPath, [System.Text.Encoding]::UTF8.GetBytes($LfText))
            }
        }
    }

    # Bundle workspace directories matching G:\Meu Drive\vp-rev-eng\teste\teste2\dist\Neoveg_v1.0.0_win-x64
    foreach ($Folder in @("projects", "media", "renders", "extensions")) {
        $DstF = Join-Path $OutDir $Folder
        New-Item -ItemType Directory -Path $DstF -Force | Out-Null
        $SrcF = Join-Path $RootDir $Folder
        if (Test-Path $SrcF) {
            Get-ChildItem -Path $SrcF -File | Where-Object { $_.Length -lt 5MB } | ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $DstF $_.Name) -Force
            }
        }
    }
    New-Item -ItemType Directory -Path (Join-Path $OutDir "cache\prerender") -Force | Out-Null

    # Bundle Assets (i18n, luts, neoveg_presets, neoveg_history_themes, axaml)
    $AssetsDst = Join-Path $OutDir "Assets"
    New-Item -ItemType Directory -Path $AssetsDst -Force | Out-Null
    $RootAssetsSrc = Join-Path $RootDir "assets"
    if (Test-Path $RootAssetsSrc) {
        Copy-Item -Path "$RootAssetsSrc\*" -Destination $AssetsDst -Recurse -Force
    }
    $DesktopAssetsSrc = Join-Path $RootDir "desktop-host\Assets"
    if (Test-Path $DesktopAssetsSrc) {
        Copy-Item -Path "$DesktopAssetsSrc\*" -Destination $AssetsDst -Recurse -Force
    }

    foreach ($DocFile in @(
        "AUTHORS.md",
        "LICENSE",
        "REQUIREMENTS.md",
        "RELEASE_NOTES.md",
        "README.md",
        "KEEPS.md",
        "GUIA_NOVAS_FUNCIONALIDADES.md",
        "DOCUMENTACAO_SCRIPTS.md",
        "RELATORIO_REA_NEOVEG.md",
        "RELATORIO_MAPEAMENTO_UI_NEOVEG.md",
        "settings.json"
    )) {
        $DocSrc = Join-Path $RootDir $DocFile
        if (Test-Path $DocSrc) {
            Copy-Item -LiteralPath $DocSrc -Destination (Join-Path $OutDir $DocFile) -Force
        }
    }
}

# 3. Mirror win-x64 build into dist/Neoveg_v1.0.0_win-x64 and Workspace Root (identical to G:\Meu Drive\vp-rev-eng\teste\teste2)
Write-Host "[3/6] Populating dist/Neoveg_v1.0.0_win-x64 and root workspace binaries..." -ForegroundColor Yellow
$WinPortableDir = Join-Path $DistDir "Neoveg_v1.0.0_win-x64_Portable"
$WinReleaseDir = Join-Path $DistDir "Neoveg_v1.0.0_win-x64"
if (Test-Path $WinReleaseDir) {
    Remove-Item -LiteralPath $WinReleaseDir -Recurse -Force -ErrorAction SilentlyContinue
}
Copy-Item -Path $WinPortableDir -Destination $WinReleaseDir -Recurse -Force

# Copy framework-level root binaries to workspace root just like G:\Meu Drive\vp-rev-eng\teste\teste2
foreach ($RootPattern in @(
    "Neoveg.exe",
    "Neoveg.dll",
    "Neoveg.pdb",
    "Neoveg.deps.json",
    "Neoveg.runtimeconfig.json",
    "neoveg_core.dll",
    "neoveg_cpp_engine.dll",
    "WebView2Loader.dll",
    "SkiaSharp.dll",
    "HarfBuzzSharp.dll",
    "MicroCom.Runtime.dll",
    "Tmds.DBus.Protocol.dll",
    "Avalonia*.dll",
    "Microsoft.CodeAnalysis*.dll"
)) {
    Get-ChildItem -Path $WinPortableDir -Filter $RootPattern -File -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $RootDir $_.Name) -Force
    }
}

# Also run build_portable.py so dist/Neoveg-Portable-win-x64, dist/Neoveg-Portable-linux-x64, and dist/Neoveg-Portable-osx-x64 are updated
Write-Host "[4/6] Updating dist/Neoveg-Portable-* bundles via build_portable.py..." -ForegroundColor Yellow
& python (Join-Path $RootDir "build_portable.py") --framework-dependent
# Ensure unpacked win-x64 self-contained files are also in dist/Neoveg-Portable-win-x64
$LegacyWinPortable = Join-Path $DistDir "Neoveg-Portable-win-x64"
Copy-Item -Path "$WinPortableDir\*" -Destination $LegacyWinPortable -Recurse -Force

# 4. Create Portable Archives (.zip for Windows, .tar.gz for Linux & macOS)
Write-Host "[5/6] Compressing Portable Release Archives in dist/..." -ForegroundColor Yellow

$WinLegacyZip = Join-Path $DistDir "Neoveg_v1.0.0_win-x64.zip"
if (Test-Path $WinLegacyZip) { Remove-Item -LiteralPath $WinLegacyZip -Force }
& tar -a -cf $WinLegacyZip -C $DistDir "Neoveg_v1.0.0_win-x64"

foreach ($Target in $Targets) {
    $ArchivePath = Join-Path $DistDir $Target.ArchiveName
    if (Test-Path $ArchivePath) {
        Remove-Item -LiteralPath $ArchivePath -Force
    }

    if ($Target.ArchiveType -eq "zip") {
        Write-Host "  -> Creating $($Target.ArchiveName)..." -ForegroundColor Gray
        & tar -a -cf $ArchivePath -C $DistDir $Target.FolderName
    } else {
        Write-Host "  -> Creating $($Target.ArchiveName)..." -ForegroundColor Gray
        & tar -czf $ArchivePath -C $DistDir $Target.FolderName
        if ($LASTEXITCODE -ne 0) {
            throw "tar failed to create $ArchivePath"
        }
    }
}

# Create macOS universal archive (dist/Neoveg_v1.0.0_osx-universal_Portable.tar.gz)
$UniversalArchive = Join-Path $DistDir "Neoveg_v1.0.0_osx-universal_Portable.tar.gz"
if (Test-Path $UniversalArchive) {
    Remove-Item -LiteralPath $UniversalArchive -Force
}
Write-Host "  -> Creating Neoveg_v1.0.0_osx-universal_Portable.tar.gz..." -ForegroundColor Gray
& tar -czf $UniversalArchive -C $DistDir "Neoveg_v1.0.0_osx-x64_Portable" "Neoveg_v1.0.0_osx-arm64_Portable"

Get-ChildItem -Path $DistDir -File | Where-Object { $_.Name -like "Neoveg_v1.0.0_*" } |
    Select-Object Name, @{Name="SizeMB";Expression={[math]::Round($_.Length / 1MB, 2)}} |
    Format-Table -AutoSize

# 5. Synchronize with Google Drive (scripts/sync_gdrive.ps1)
if (-not $SkipGDriveSync) {
    Write-Host "[6/6] Synchronizing workspace & dist/ packages to Google Drive..." -ForegroundColor Yellow
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $RootDir "scripts\sync_gdrive.ps1")
}

Write-Host "[OK] All cross-platform portable packages built and synchronized identically to Google Drive!" -ForegroundColor Green
