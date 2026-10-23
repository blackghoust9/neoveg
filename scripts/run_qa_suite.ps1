$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rootDir = Split-Path -Parent $scriptDir
$cppDir = Join-Path $rootDir "cpp-extensions"
$buildDir = Join-Path $cppDir "build-qa"
$coverageDir = Join-Path $rootDir "coverage\qa"
$dotnetResults = Join-Path $coverageDir "dotnet"
$pythonCoverage = Join-Path $coverageDir "python\coverage.xml"
$cppCoverage = Join-Path $coverageDir "cpp\coverage.xml"
$qaReportJson = Join-Path $coverageDir "qa_report.json"
$qaReportJs = Join-Path $rootDir "docs\qa_report.js"
$logsDir = Join-Path $rootDir "logs"
$qaLogPath = Join-Path $logsDir "neoveg.log"

# ISO/IEC 25010 & ISO/IEC 29119 quality pillars used to categorize every QA step (ABNT NBR ISO/IEC 25010 / ABNT NBR ISO/IEC 29119).
$pillars = [ordered]@{
    Performance     = "Eficiencia de Desempenho"
    Reliability     = "Confiabilidade"
    Usability       = "Usabilidade"
    Portability     = "Portabilidade"
    Maintainability = "Manutenibilidade"
}
$results = New-Object System.Collections.Generic.List[object]

function Write-QALog {
    param(
        [Parameter(Mandatory = $true)][string]$Level,
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Message
    )

    $line = "[{0}] [{1}] [QA/{2}] {3}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"), $Level, $Source, $Message
    Write-Host $line
    New-Item -ItemType Directory -Force -Path $logsDir | Out-Null
    Add-Content -Path $qaLogPath -Value $line -Encoding UTF8
}

# Ensure user-level WinGet, Cargo, and Python Scripts directories are on PATH on Windows
$candidateToolDirs = @(
    (Join-Path $env:USERPROFILE ".cargo\bin")
)
if ($env:APPDATA) {
    $pyRoaming = Join-Path $env:APPDATA "Python"
    if (Test-Path $pyRoaming) {
        Get-ChildItem -Path $pyRoaming -Directory -Filter "Python*" -ErrorAction SilentlyContinue | ForEach-Object {
            $s = Join-Path $_.FullName "Scripts"
            if (Test-Path $s) { $candidateToolDirs += $s }
        }
    }
}
if ($env:LOCALAPPDATA) {
    $wingetPkgs = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
    if (Test-Path $wingetPkgs) {
        Get-ChildItem -Path $wingetPkgs -Recurse -Filter "ninja.exe" -ErrorAction SilentlyContinue | ForEach-Object {
            $candidateToolDirs += $_.DirectoryName
        }
        Get-ChildItem -Path $wingetPkgs -Recurse -Filter "clang-tidy.exe" -ErrorAction SilentlyContinue | ForEach-Object {
            $candidateToolDirs += $_.DirectoryName
        }
        Get-ChildItem -Path $wingetPkgs -Recurse -Filter "x86_64-w64-mingw32-clang++.exe" -ErrorAction SilentlyContinue | ForEach-Object {
            $candidateToolDirs += $_.DirectoryName
        }
    }
}
foreach ($td in ($candidateToolDirs | Select-Object -Unique)) {
    if ($td -and (Test-Path $td) -and ($env:PATH -notlike "*$td*")) {
        $env:PATH = "$td;$env:PATH"
    }
}
if (-not $env:LLVM_COV) {
    $llvmCovCmd = Get-Command "llvm-cov" -ErrorAction SilentlyContinue
    if ($llvmCovCmd) { $env:LLVM_COV = $llvmCovCmd.Source }
}
if (-not $env:LLVM_PROFDATA) {
    $llvmProfCmd = Get-Command "llvm-profdata" -ErrorAction SilentlyContinue
    if ($llvmProfCmd) { $env:LLVM_PROFDATA = $llvmProfCmd.Source }
}

if (-not (Get-Command "ninja" -ErrorAction SilentlyContinue)) {
    throw "Required QA tool 'ninja' was not found on PATH."
}

function Invoke-QACommand {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][ValidateSet("Performance", "Reliability", "Usability", "Portability", "Maintainability")][string]$Pillar,
        [Parameter(Mandatory = $true)][string]$Standard
    )

    $entry = [ordered]@{ name = $Name; pillar = $Pillar; standard = $Standard; status = "running"; duration_ms = 0 }
    $results.Add($entry)
    $timer = [System.Diagnostics.Stopwatch]::StartNew()

    Write-Host "`n== $Name =="
    Write-QALog "INFO" $Pillar "START '$Name' | Requisito: $Standard | Pilar ISO/IEC 25010: $($pillars[$Pillar])"
    try {
        if (-not (Get-Command $Executable -ErrorAction SilentlyContinue)) {
            throw "Required QA tool '$Executable' for '$Name' was not found on PATH."
        }

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
        $entry.status = "pass"
        Write-QALog "INFO" $Pillar "PASS '$Name' ($($timer.ElapsedMilliseconds) ms) | Requisito: $Standard"
    }
    catch {
        $entry.status = "fail"
        $entry.error = $_.Exception.Message
        Write-QALog "ERROR" $Pillar "FAIL '$Name' | Requisito: $Standard | $($_.Exception.Message)"
        throw
    }
    finally {
        $entry.duration_ms = $timer.ElapsedMilliseconds
    }
}

function Assert-QACondition {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Check,
        [Parameter(Mandatory = $true)][string]$Pillar,
        [Parameter(Mandatory = $true)][string]$Standard
    )

    $entry = [ordered]@{ name = $Name; pillar = $Pillar; standard = $Standard; status = "running"; duration_ms = 0 }
    $results.Add($entry)
    try {
        & $Check
        $entry.status = "pass"
        Write-QALog "INFO" $Pillar "PASS '$Name' | Requisito: $Standard"
    }
    catch {
        $entry.status = "fail"
        $entry.error = $_.Exception.Message
        Write-QALog "ERROR" $Pillar "FAIL '$Name' | Requisito: $Standard | $($_.Exception.Message)"
        throw
    }
}

function Get-DockerEnvironment {
    # Real Docker state for the dashboard (never hard-coded): CLI presence + daemon reachability.
    $docker = Get-Command "docker" -ErrorAction SilentlyContinue
    if (-not $docker) {
        return [ordered]@{ installed = $false; daemon_running = $false; client_version = $null; server_version = $null; status = "Not Installed / Optional" }
    }
    $clientVersion = (& docker version --format "{{.Client.Version}}" 2>$null | Select-Object -First 1)
    $serverVersion = (& docker info --format "{{.ServerVersion}}" 2>$null | Select-Object -First 1)
    $running = ($LASTEXITCODE -eq 0) -and -not [string]::IsNullOrWhiteSpace($serverVersion)
    return [ordered]@{
        installed      = $true
        daemon_running = $running
        client_version = $clientVersion
        server_version = $(if ($running) { $serverVersion } else { $null })
        status         = $(if ($running) { "Running" } else { "Not Running / Optional" })
    }
}

function Write-QAReport {
    param([Parameter(Mandatory = $true)][string]$Outcome)

    $byPillar = [ordered]@{}
    foreach ($key in $pillars.Keys) {
        $steps = @($results | Where-Object { $_.pillar -eq $key })
        $status = "not_run"
        if (@($steps | Where-Object { $_.status -eq "fail" }).Count -gt 0) { $status = "fail" }
        elseif ($steps.Count -gt 0 -and @($steps | Where-Object { $_.status -ne "pass" }).Count -eq 0) { $status = "pass" }
        $byPillar[$key] = [ordered]@{
            label  = $pillars[$key]
            status = $status
            passed = @($steps | Where-Object { $_.status -eq "pass" }).Count
            total  = $steps.Count
        }
    }

    $report = [ordered]@{
        generated_at = (Get-Date).ToString("o")
        platform     = "windows"
        outcome      = $Outcome
        standards    = @(
            "ABNT NBR ISO/IEC 25010",
            "ABNT NBR ISO/IEC/IEEE 29119",
            "ABNT NBR ISO/IEC 12207",
            "ABNT NBR ISO/IEC 14764",
            "ABNT NBR ISO/IEC 27001"
        )
        pillars      = $byPillar
        steps        = $results
        environment  = [ordered]@{ docker = (Get-DockerEnvironment) }
    }
    $json = $report | ConvertTo-Json -Depth 6
    Set-Content -Path $qaReportJson -Value $json -Encoding UTF8
    # docs/index.html is usually opened via file://, where fetch() is blocked; a script include still loads.
    Set-Content -Path $qaReportJs -Value "window.NEOVEG_QA_REPORT = $json;" -Encoding UTF8
    Write-QALog "INFO" "Report" "QA report ($Outcome) written to $qaReportJson and $qaReportJs"
}

New-Item -ItemType Directory -Force -Path $coverageDir, (Split-Path -Parent $pythonCoverage), (Split-Path -Parent $cppCoverage) | Out-Null
Write-QALog "INFO" "Suite" "Neoveg QA suite started (ABNT NBR ISO/IEC 25010 / 29119 / 12207 / 14764 / 27001)."

$outcome = "fail"
try {
    Invoke-QACommand "Configure instrumented C++ QA build" "cmake" @(
        "-S", $cppDir, "-B", $buildDir, "-G", "Ninja",
        "-DCMAKE_BUILD_TYPE=Debug",
        "-DNEOVEG_ENABLE_SANITIZERS=ON",
        "-DNEOVEG_ENABLE_COVERAGE=ON"
    ) $rootDir "Portability" "ABNT NBR ISO/IEC 25010 (Portabilidade) - build CMake multiplataforma"

    Invoke-QACommand ".NET formatting" "dotnet" @("format", (Join-Path $rootDir "Neoveg.sln"), "--verify-no-changes") $rootDir `
        "Maintainability" "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764"
    Invoke-QACommand "Rust Clippy" "cargo" @("clippy", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--all-targets", "--all-features", "--", "-D", "warnings") $rootDir `
        "Maintainability" "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764"
    Invoke-QACommand "C++ clang-tidy" "clang-tidy" @(
        (Join-Path $cppDir "src\neoveg_cpp_engine.cpp"),
        (Join-Path $cppDir "whisper.cpp\src\neoveg_whisper_bridge.cpp"),
        "-p", $buildDir
    ) $rootDir "Maintainability" "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764"

    # The .NET project bundles rust-core/target/release/neoveg_core.dll: build it from source first
    Invoke-QACommand "Build Rust native library (release)" "cargo" @("build", "--release", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml")) $rootDir `
        "Portability" "ABNT NBR ISO/IEC 25010 (Portabilidade) - biblioteca nativa compilada a partir do codigo-fonte"
    Invoke-QACommand ".NET tests with coverage collection (incl. Avalonia.Headless UI)" "dotnet" @(
        "test", (Join-Path $rootDir "Neoveg.sln"),
        "--collect:XPlat Code Coverage",
        "--results-directory", $dotnetResults
    ) $rootDir "Usability" "ABNT NBR ISO/IEC/IEEE 29119 (testes de UI headless) / ISO/IEC 25010 (Usabilidade)"
    Invoke-QACommand "Rust tests" "cargo" @("test", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--all-targets", "--all-features") $rootDir `
        "Reliability" "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)"
    Invoke-QACommand "Criterion waveform benchmark" "cargo" @("bench", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"), "--bench", "audio_peaks") $rootDir `
        "Performance" "ABNT NBR ISO/IEC 25010 (Eficiencia de Desempenho)"
    Invoke-QACommand "Build sanitized C++ QA targets" "cmake" @("--build", $buildDir, "--config", "Debug") $rootDir `
        "Reliability" "ABNT NBR ISO/IEC 27001 (seguranca de memoria: ASan/UBSan)"
    Invoke-QACommand "C++ tests" "ctest" @("--test-dir", $buildDir, "-C", "Debug", "--output-on-failure") $rootDir `
        "Reliability" "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 27001 (FFI sob ASan/UBSan)"
    Invoke-QACommand "Python tests with coverage" "pytest" @(
        "tests", "python-backend",
        "--cov=python-backend",
        "--cov=tests",
        "--cov-report=term-missing",
        "--cov-report=xml:$pythonCoverage"
    ) $rootDir "Reliability" "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)"

    Invoke-QACommand "Rust coverage" "cargo" @(
        "llvm-cov", "--manifest-path", (Join-Path $rootDir "rust-core\Cargo.toml"),
        "--all-targets", "--all-features", "--html",
        "--output-dir", (Join-Path $coverageDir "rust")
    ) $rootDir "Maintainability" "ABNT NBR ISO/IEC/IEEE 29119 (cobertura) / ISO/IEC 12207"
    $rustHtmlNested = Join-Path $coverageDir "rust\html\index.html"
    $rustHtmlRoot = Join-Path $coverageDir "rust\index.html"
    if ((Test-Path $rustHtmlNested -PathType Leaf) -and -not (Test-Path $rustHtmlRoot -PathType Leaf)) {
        Copy-Item -Path $rustHtmlNested -Destination $rustHtmlRoot -Force
    }
    # Clear LLVM_PROFDATA before gcovr so gcovr uses GcovHandler (.gcno/.gcda) instead of LlvmHandler (.profraw)
    Remove-Item Env:\LLVM_PROFDATA -ErrorAction SilentlyContinue
    $gcovrFilter = ($cppDir -replace '\\', '/')
    $gcovrArgs = @(
        ($buildDir -replace '\\', '/'),
        "--root", ($rootDir -replace '\\', '/'),
        "--object-directory", ($buildDir -replace '\\', '/'),
        "--filter", $gcovrFilter,
        "--xml-pretty", "--xml", $cppCoverage
    )
    if (-not (Get-Command "gcov" -ErrorAction SilentlyContinue) -and (Get-Command "llvm-cov" -ErrorAction SilentlyContinue)) {
        $gcovrArgs += @("--gcov-executable", "llvm-cov gcov")
    }
    Invoke-QACommand "C++ coverage report" "gcovr" $gcovrArgs $rootDir "Maintainability" "ABNT NBR ISO/IEC/IEEE 29119 (cobertura) / ISO/IEC 12207"

    Assert-QACondition "Coverage report verification" {
        $dotnetCoverage = Get-ChildItem -Path $dotnetResults -Filter "coverage.cobertura.xml" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $dotnetCoverage) {
            throw "The .NET coverage collector did not produce a coverage.cobertura.xml report."
        }
        foreach ($report in @($pythonCoverage, $cppCoverage, (Join-Path $coverageDir "rust\index.html"))) {
            if (-not (Test-Path $report -PathType Leaf)) {
                throw "Expected coverage report was not generated: $report"
            }
        }
    } "Maintainability" "ABNT NBR ISO/IEC/IEEE 29119 (evidencia de cobertura)"

    $outcome = "pass"
}
finally {
    Write-QAReport $outcome
}
Write-QALog "INFO" "Suite" "QA suite and coverage verification completed successfully."
Write-Host "`nQA suite and coverage verification completed successfully."
