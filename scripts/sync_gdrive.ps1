$ErrorActionPreference = "Stop"

$Source = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Destination = if ($env:NEOVEG_GDRIVE_TARGET_DIR) { $env:NEOVEG_GDRIVE_TARGET_DIR } else { "G:\Meu Drive\vp-rev-eng\teste\teste2" }
$Robocopy = Join-Path $env:SystemRoot "System32\robocopy.exe"
$LogsDir = Join-Path $Source "logs"
$LogFile = Join-Path $LogsDir "neoveg.log"

$DriveRoot = [System.IO.Path]::GetPathRoot($Destination)
if (-not [string]::IsNullOrWhiteSpace($DriveRoot) -and -not (Test-Path -LiteralPath $DriveRoot)) {
    $Timestamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss.fff")
    $WarnMsg = "Google Drive volume '$DriveRoot' is not mounted or accessible; skipping cloud mirror sync to '$Destination'. Local workspace and 'dist/' packages are ready."
    Write-Host "[WARN] $WarnMsg" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $LogsDir -Force | Out-Null
    Add-Content -Path $LogFile -Value "[$Timestamp] [WARN] [PowerShell/sync_gdrive.ps1] [ABNT NBR ISO/IEC 25010 - Portability] $WarnMsg" -Encoding UTF8
    exit 0
}

if (-not (Test-Path -LiteralPath $Robocopy -PathType Leaf)) {
    throw "Robocopy was not found at '$Robocopy'."
}

$Arguments = @(
    $Source
    $Destination
    "/MIR"
    "/R:2"
    "/W:2"
    "/NFL"
    "/NDL"
    "/NP"
    "/NJH"
    "/NJS"
    "/XD"
    ".git"
    "bin"
    "obj"
    "Debug"
    "Release"
    "*TestResult*"
    "target"
    "build"
    "out"
    "CMakeFiles"
    "logs"
    "AppCache"
    "temp"
    "coverage"
    "__pycache__"
    ".venv"
    "venv"
    "env"
    ".pytest_cache"
    "*.egg-info"
    ".vs"
    ".idea"
    ".vscode"
    "dist"
    "runtimes"
    "*.dir"
    "OmniRoute"
    "node_modules"
    "/XF"
    ".env"
    ".env.*"
    "*.tmp"
    "*.log"
    "*.sfk"
    "*.sfvp0"
    ".DS_Store"
    "Thumbs.db"
    "desktop.ini"
    "*.user"
    "*.userosscache"
    "*.sln.docstates"
    "*.rs.bk"
    "CMakeCache.txt"
    "Makefile"
    "cmake_install.cmake"
    "*.VC.db"
    "*.opendb"
    "*.ipch"
    "*.idb"
    "*.ilk"
    "*.exp"
    "*.obj"
    "*.o"
    "*.lib"
    "*.a"
    "*.pyc"
    "*.pyo"
    "*.pyd"
    "*.class"
    "*`$py.class"
    ".coverage"
    "*.swp"
    "*.swo"
    "*.gslides"
    "*.gdoc"
    "*.gsheet"
)

Write-Host "Mirroring workspace '$Source' to '$Destination'..."
& $Robocopy @Arguments
$RobocopyExitCode = $LASTEXITCODE

if ($RobocopyExitCode -ge 8) {
    throw "Robocopy failed with exit code $RobocopyExitCode."
}

# Synchronize runtimes/ (including native .dll, .so, .dylib, and ffmpeg binaries)
$RuntimesSource = Join-Path $Source "runtimes"
$RuntimesDestination = Join-Path $Destination "runtimes"
if (Test-Path -LiteralPath $RuntimesSource -PathType Container) {
    & $Robocopy $RuntimesSource $RuntimesDestination "/E" "/R:2" "/W:2" "/NFL" "/NDL" "/NP" "/NJH" "/NJS"
    if ($LASTEXITCODE -ge 8) {
        throw "Robocopy failed to synchronize runtimes/ with exit code $LASTEXITCODE."
    }
}

# Synchronize dist/ (including all portable folders, .exe, .dll, .so, .dylib, .zip, and .tar.gz archives)
$DistSource = Join-Path $Source "dist"
$DistDestination = Join-Path $Destination "dist"
if (Test-Path -LiteralPath $DistSource -PathType Container) {
    New-Item -ItemType Directory -Path $DistDestination -Force | Out-Null
    & $Robocopy $DistSource $DistDestination "/E" "/R:2" "/W:2" "/NFL" "/NDL" "/NP" "/NJH" "/NJS"
    $DistExitCode = $LASTEXITCODE
    if ($DistExitCode -ge 8) {
        throw "Robocopy failed to synchronize dist/ portable packages with exit code $DistExitCode."
    }
    Write-Host "Synchronized portable release directories and archives in '$DistDestination'."
}

$AllowedVsCodeFiles = @(
    "settings.json"
    "tasks.json"
    "launch.json"
    "cmake-kits.json"
)
$VsCodeSource = Join-Path $Source ".vscode"
$VsCodeDestination = Join-Path $Destination ".vscode"

if (Test-Path -LiteralPath $VsCodeSource -PathType Container) {
    $StagingDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("neoveg-vscode-sync-" + [guid]::NewGuid())
    try {
        New-Item -ItemType Directory -Path $StagingDirectory | Out-Null
        foreach ($FileName in $AllowedVsCodeFiles) {
            $SourceFile = Join-Path $VsCodeSource $FileName
            if (Test-Path -LiteralPath $SourceFile -PathType Leaf) {
                Copy-Item -LiteralPath $SourceFile -Destination $StagingDirectory
            }
        }

        & $Robocopy $StagingDirectory $VsCodeDestination "/MIR" "/R:2" "/W:2" "/NFL" "/NDL" "/NP" "/NJH" "/NJS"
        $VsCodeRobocopyExitCode = $LASTEXITCODE
        if ($VsCodeRobocopyExitCode -ge 8) {
            throw "Robocopy failed to synchronize allowed VS Code files with exit code $VsCodeRobocopyExitCode."
        }
    }
    finally {
        Remove-Item -LiteralPath $StagingDirectory -Recurse -Force
    }
}

Write-Host "Google Drive synchronization completed (Robocopy exit code $RobocopyExitCode)."
