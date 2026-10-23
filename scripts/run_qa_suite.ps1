$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rootDir = Split-Path -Parent $scriptDir
$cppDir = Join-Path $rootDir "cpp-extensions"
$buildDir = Join-Path $cppDir "build-qa"
$coverageDir = Join-Path $rootDir "coverage\qa"
$dotnetResults = Join-Path $coverageDir "dotnet"
$pythonCoverage = Join-Path $coverageDir "python\coverage.xml"
$cppCoverage = Join-Path $coverageDir "cpp\coverage.xml"

if (-not (Get-Command "ninja" -ErrorAction SilentlyContinue)) {
    throw "Required QA tool 'ninja' was not found on PATH."
}

function Invoke-QACommand {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )

    if (-not (Get-Command $Executable -ErrorAction SilentlyContinue)) {
        throw "Required QA tool '$Executable' for '$Name' was not found on PATH."
    }

    Write-Host "`n== $Name =="
    Push-Location $WorkingDirectory
    try {
        & $Executable @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "'$Name' failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

New-Item -ItemType Directory -Force -Path $coverageDir, (Split-Path -Parent $pythonCoverage), (Split-Path -Parent $cppCoverage) | Out-Null

Invoke-QACommand "Configure instrumented C++ QA build" "cmake" @(
    "-S", $cppDir, "-B", $buildDir, "-G", "Ninja",
    "-DCMAKE_BUILD_TYPE=Debug",
    "-DNEOVEG_ENABLE_SANITIZERS=ON",
    "-DNEOVEG_ENABLE_COVERAGE=ON"
) $rootDir

Invoke-QACommand ".NET formatting" "dotnet" @("format", (Join-Path $rootDir "Neoveg.sln"), "--verify-no-changes") $rootDir
Invoke-QACommand "Rust Clippy" "cargo" @("clippy", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--all-targets", "--all-features") $rootDir
Invoke-QACommand "C++ clang-tidy" "clang-tidy" @(
    (Join-Path $cppDir "src\neoveg_cpp_engine.cpp"),
    (Join-Path $cppDir "whisper.cpp\src\neoveg_whisper_bridge.cpp"),
    "-p", $buildDir
) $rootDir

Invoke-QACommand ".NET tests with coverage collection" "dotnet" @(
    "test", (Join-Path $rootDir "Neoveg.sln"),
    "--collect:XPlat Code Coverage",
    "--results-directory", $dotnetResults
) $rootDir
Invoke-QACommand "Rust tests" "cargo" @("test", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--all-targets", "--all-features") $rootDir
Invoke-QACommand "Criterion waveform benchmark" "cargo" @("bench", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--bench", "audio_peaks") $rootDir
Invoke-QACommand "Build sanitized C++ QA targets" "cmake" @("--build", $buildDir, "--config", "Debug") $rootDir
Invoke-QACommand "C++ tests" "ctest" @("--test-dir", $buildDir, "-C", "Debug", "--output-on-failure") $rootDir
Invoke-QACommand "Python tests with coverage" "pytest" @(
    "tests", "python-backend",
    "--cov=python-backend",
    "--cov=tests",
    "--cov-report=term-missing",
    "--cov-report=xml:$pythonCoverage"
) $rootDir

Invoke-QACommand "Rust coverage" "cargo" @(
    "llvm-cov", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"),
    "--all-targets", "--all-features", "--html",
    "--output-dir", (Join-Path $coverageDir "rust")
) $rootDir
Invoke-QACommand "C++ coverage report" "gcovr" @(
    "--root", $rootDir,
    "--object-directory", $buildDir,
    "--filter", (Join-Path $cppDir "src"),
    "--xml-pretty", "--xml", $cppCoverage
) $rootDir

$dotnetCoverage = Get-ChildItem -Path $dotnetResults -Filter "coverage.cobertura.xml" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $dotnetCoverage) {
    throw "The .NET coverage collector did not produce a coverage.cobertura.xml report."
}
foreach ($report in @($pythonCoverage, $cppCoverage, (Join-Path $coverageDir "rust\index.html"))) {
    if (-not (Test-Path $report -PathType Leaf)) {
        throw "Expected coverage report was not generated: $report"
    }
}
Write-Host "`nQA suite and coverage verification completed successfully."
