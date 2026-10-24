<#
.SYNOPSIS
Runs the whole Neoveg Rust workspace through the isolated Docker environment (docker/).

.DESCRIPTION
1. Checks that Docker is installed and running.
2. Builds the images and runs rustfmt + clippy + cargo test under Xvfb (Linux container).
3. Verifies that every test passed.
4. Builds the optimized Linux binaries into dist/linux and, with -Macos, the macOS ones
   (Intel and Apple Silicon, cross-compiled) into dist/macos.
Works with Windows PowerShell 5.1 and PowerShell 7 on Windows, Linux and macOS.

.PARAMETER Macos      Also cross-compile for macOS (needs the SDK package in docker/sdk, see docker/sdk/README.md).
.PARAMETER MacosNative  Also build and test natively inside a macOS guest (Docker-OSX, needs KVM and the disk image, see docker/osx/README.md).
.PARAMETER Ffmpeg     Test and build with the `ffmpeg` feature.
.PARAMETER NoCache    Rebuild the images from scratch.
.PARAMETER TestsOnly  Skip the release builds.
#>
[CmdletBinding()]
param(
    [switch]$Macos,
    [switch]$MacosNative,
    [switch]$Ffmpeg,
    [switch]$NoCache,
    [switch]$TestsOnly
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

# ---- 0. D:\ambiente_neoveg ------------------------------------------------------------
# Heavy files (Cargo target, Docker caches, artifacts, macOS disk image) go to the environment drive.
# Override with NEOVEG_ENV_DIR; elsewhere than Windows the default is a folder inside the checkout.
if (-not $env:NEOVEG_ENV_DIR) {
    $env:NEOVEG_ENV_DIR = if ($IsLinux -or $IsMacOS) { Join-Path $Root '.neoveg_env' } else { 'D:\ambiente_neoveg' }
}
$EnvDir = $env:NEOVEG_ENV_DIR
$Dist = Join-Path $EnvDir 'dist'
$env:NEOVEG_DIST = $Dist
$env:CARGO_TARGET_DIR = Join-Path $EnvDir 'target'

function Fail([string]$Message) {
    Write-Host "[ERROR] $Message" -ForegroundColor Red
    exit 1
}
function Info([string]$Message) { Write-Host "[INFO] $Message" }

if (-not (Test-Path (Split-Path -Qualifier $EnvDir -ErrorAction SilentlyContinue))) {
    if ($EnvDir -match '^[A-Za-z]:') { Fail "drive $(Split-Path -Qualifier $EnvDir) is not available for $EnvDir" }
}
foreach ($dir in 'target', 'dist', 'docker', 'docker\cargo-registry', 'docker\target-linux-test',
                 'docker\target-linux-release', 'docker\target-macos', 'docker-osx') {
    New-Item -ItemType Directory -Force -Path (Join-Path $EnvDir $dir) | Out-Null
}
Info "environment directory: $EnvDir"

# Runs a native command; a non-zero exit code stops the script with $Message.
function Invoke-Native([string]$Message, [string[]]$Command) {
    $exe = $Command[0]
    $rest = @()
    if ($Command.Count -gt 1) { $rest = $Command[1..($Command.Count - 1)] }
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'   # docker writes progress to stderr
    & $exe @rest
    $code = $LASTEXITCODE
    $ErrorActionPreference = $previous
    if ($code -ne 0) { Fail "$Message (exit code $code)" }
}

# ---- 1. Docker ---------------------------------------------------------------------------------
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Fail 'Docker is not installed. Install Docker Desktop or Docker Engine: https://docs.docker.com/get-docker/'
}
$previous = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
& docker info *> $null
$daemonOk = ($LASTEXITCODE -eq 0)
& docker compose version *> $null
$composeV2 = ($LASTEXITCODE -eq 0)
$ErrorActionPreference = $previous
if (-not $daemonOk) { Fail 'Docker is installed but its daemon is not running. Start Docker and try again.' }
if ($composeV2) {
    $Compose = @('docker', 'compose')
} elseif (Get-Command docker-compose -ErrorAction SilentlyContinue) {
    $Compose = @('docker-compose')
} else {
    Fail "Docker Compose was not found (neither 'docker compose' nor 'docker-compose')."
}
$Compose += @('-f', 'docker/docker-compose.yml')
if ($Macos) {
    $Compose += @('--profile', 'macos')
    if (-not (Get-ChildItem 'docker/sdk' -Filter 'MacOSX*.sdk.tar.*' -ErrorAction SilentlyContinue)) {
        Fail 'no macOS SDK package in docker/sdk/ (see docker/sdk/README.md)'
    }
}
if ($MacosNative) {
    $Compose += @('--profile', 'macos-native')
    if (-not (Test-Path (Join-Path $EnvDir 'docker-osx\mac_hdd.img'))) {
        Fail "no macOS disk image at $EnvDir\docker-osx\mac_hdd.img (see docker/osx/README.md)"
    }
}
if ($Ffmpeg) { $env:NEOVEG_FFMPEG = '1' }

# The containers run as root on Docker Desktop; HOST_UID only matters on Linux hosts
if (Get-Command id -ErrorAction SilentlyContinue) {
    $env:HOST_UID = (& id -u)
    $env:HOST_GID = (& id -g)
}
foreach ($dir in "$Dist\reports", "$Dist\linux", 'logs') {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}
Remove-Item -Force -ErrorAction SilentlyContinue "$Dist\reports\test-linux.txt", "$Dist\reports\test-macos-native.txt"

# ---- 2. images -----------------------------------------------------------------------------------
Info 'building the images'
$services = @('test-rust-linux')
if (-not $TestsOnly) { $services += 'build-rust-linux-release' }
if ($Macos -and -not $TestsOnly) { $services += 'build-rust-macos-release' }
$build = $Compose + @('build')
if ($NoCache) { $build += '--no-cache' }
Invoke-Native 'the image build failed' ($build + $services)

# ---- 3. tests --------------------------------------------------------------------------------------
Info 'running rustfmt, clippy and cargo test in the Linux container'
Invoke-Native 'the quality gates failed' ($Compose + @('run', '--rm', '--no-deps', 'test-rust-linux'))
$report = Join-Path $Dist 'reports/test-linux.txt'
if (-not (Test-Path $report)) { Fail "the container did not write $report" }
$values = @{}
foreach ($line in Get-Content $report) {
    if ($line -match '^([^=]+)=(.*)$') { $values[$Matches[1]] = $Matches[2] }
}
if ($values['fmt'] -ne 'ok' -or $values['clippy'] -ne 'ok' -or $values['tests'] -ne 'ok' -or $values['failed'] -ne '0') {
    Fail "tests are not 100% green; see $report"
}
if ([int]$values['passed'] -le 0) { Fail 'no test ran' }
Info ("Linux: {0} tests passed, 0 failed (rustfmt and clippy clean)" -f $values['passed'])

# ---- 4. release builds --------------------------------------------------------------------------------
if (-not $TestsOnly) {
    Info 'building the optimized Linux binaries'
    Invoke-Native 'the Linux release build failed' ($Compose + @('run', '--rm', '--no-deps', 'build-rust-linux-release'))
    if ($Macos) {
        New-Item -ItemType Directory -Force -Path (Join-Path $Dist 'macos') | Out-Null
        Info 'cross-compiling for macOS (Intel and Apple Silicon)'
        Invoke-Native 'the macOS cross build failed' ($Compose + @('run', '--rm', '--no-deps', 'build-rust-macos-release'))
    }
}

# ---- 4b. native macOS (Docker-OSX) ------------------------------------------------------------------
if ($MacosNative) {
    Info 'booting the macOS guest and testing natively (the first boot takes several minutes)'
    Invoke-Native 'the native macOS test failed' ($Compose + @('up', '--abort-on-container-exit', '--exit-code-from', 'test-rust-macos-native', 'docker-osx', 'test-rust-macos-native'))
    $macReport = Join-Path $Dist 'reports/test-macos-native.txt'
    if (-not (Test-Path $macReport)) { Fail "the macOS guest did not write $macReport" }
    $mac = @{}
    foreach ($line in Get-Content $macReport) { if ($line -match '^([^=]+)=(.*)$') { $mac[$Matches[1]] = $Matches[2] } }
    if ($mac['tests'] -ne 'ok' -or $mac['failed'] -ne '0' -or [int]$mac['passed'] -le 0) { Fail "macOS tests are not 100% green; see $macReport" }
    Info ("macOS (native): {0} tests passed, 0 failed" -f $mac['passed'])
    & $Compose[0] $Compose[1..($Compose.Count - 1)] stop docker-osx *> $null
}

# ---- 5. artifacts --------------------------------------------------------------------------------------
Info "artifacts in ${Dist}:"
$missing = $false
function Show-Artifacts([string]$Folder, [string]$Filter) {
    $Folder = Join-Path $Dist $Folder
    $files = Get-ChildItem $Folder -Filter $Filter -File -ErrorAction SilentlyContinue
    if (-not $files) {
        Write-Host "[ERROR] missing $Folder/$Filter" -ForegroundColor Red
        return $false
    }
    foreach ($f in $files) {
        $hash = (Get-FileHash -Algorithm SHA256 $f.FullName).Hash.ToLower()
        Write-Host ("{0}  {1}" -f $hash, (Join-Path $Folder $f.Name))
    }
    return $true
}
if (-not $TestsOnly) {
    if (-not (Show-Artifacts 'linux' 'neoveg_v*_linux-x64')) { $missing = $true }
    if (-not (Show-Artifacts 'linux' 'Neoveg_v*_linux-x64_Portable.tar.gz')) { $missing = $true }
    if ($MacosNative -and -not (Show-Artifacts 'macos-native' 'neoveg_macos_*')) { $missing = $true }
    if ($Macos) {
        foreach ($rid in 'osx-x64', 'osx-arm64') {
            if (-not (Show-Artifacts 'macos' "Neoveg_v*_${rid}_Portable.tar.gz")) { $missing = $true }
        }
    }
}
if ($missing) { Fail 'some artifacts were not produced' }
Info 'done'
