<#
.SYNOPSIS
    Runs the heavy Neoveg tasks (headless screenshots, benchmark, icon conversion) in a background job.

.DESCRIPTION
    Starts the job "NeovegBackgroundPipeline" and returns at once, so the terminal stays free:

      1. cargo test --test headless_screenshot   renders docs/screenshots/*.png offscreen
                                                  (NEOVEG_SCREENSHOTS=1; needs a graphics adapter)
      2. cargo bench --bench playback_bench      only when such a benchmark exists in the workspace
      3. python scripts/generate_icon_formats.py converts the SVG icons to PNG / BMP / JPG
                                                  (needs Pillow plus cairosvg or resvg)

    A step that cannot run (missing benchmark, missing Python modules) is reported and skipped; the
    others still run. The job works in the repository root (the folder above scripts/), and sends
    Cargo's output to D:\ambiente_neoveg\target only when the D: drive exists.

.PARAMETER Status
    Shows the job state (Get-Job) and what it has printed so far (Receive-Job -Keep), then returns.

.PARAMETER Wait
    After starting (or with an already running job), waits for the job to finish and prints its output.

.PARAMETER Stop
    Stops and removes the job.

.PARAMETER EnvDir
    The environment drive folder (default D:\ambiente_neoveg). It is only used when it exists.

.EXAMPLE
    powershell -File scripts\run_background_agent.ps1          # start and return
    powershell -File scripts\run_background_agent.ps1 -Status  # look at the progress
    powershell -File scripts\run_background_agent.ps1 -Wait    # start and wait for the result

    A background job lives only as long as the PowerShell session that started it: run the script in
    a session that stays open (dot-source or call it with `&`), not through `powershell -File`,
    when the job must outlive the command.
#>
[CmdletBinding()]
param(
    [switch]$Status,
    [switch]$Wait,
    [switch]$Stop,
    [string]$EnvDir = 'D:\ambiente_neoveg'
)

$JobName = 'NeovegBackgroundPipeline'
$RepoRoot = Split-Path -Parent $PSScriptRoot

function Show-Job {
    $job = Get-Job -Name $JobName -ErrorAction SilentlyContinue
    if (-not $job) {
        Write-Host "Nenhum job '$JobName' nesta sessão."
        return
    }
    Get-Job -Name $JobName | Format-Table Id, Name, State, HasMoreData, PSBeginTime -AutoSize
    Receive-Job -Name $JobName -Keep
}

if ($Stop) {
    Get-Job -Name $JobName -ErrorAction SilentlyContinue | Stop-Job -PassThru | Remove-Job -Force
    Write-Host "Job '$JobName' encerrado."
    return
}

if ($Status) {
    Show-Job
    return
}

$existing = Get-Job -Name $JobName -ErrorAction SilentlyContinue
if ($existing -and $existing.State -eq 'Running') {
    Write-Host "O job '$JobName' já está em execução; use -Status para acompanhar."
} else {
    if ($existing) { Remove-Job -Name $JobName -Force }

    # Inicia a suíte de renderização e benchmarks em segundo plano
    Start-Job -Name $JobName -ArgumentList $RepoRoot, $EnvDir -ScriptBlock {
        param($Repo, $Env)
        $ErrorActionPreference = 'Continue'
        Set-Location $Repo
        $summary = [ordered]@{}

        # Cargo's output goes to the environment drive when there is one
        if (Test-Path -LiteralPath (Split-Path -Qualifier $Env) -ErrorAction SilentlyContinue) {
            $env:CARGO_TARGET_DIR = Join-Path $Env 'target'
            "CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR"
        } else {
            "A pasta de ambiente '$Env' não está disponível: usando a pasta target do repositório."
        }

        function Invoke-Step([string]$Name, [scriptblock]$Body) {
            "=== $Name ==="
            $global:LASTEXITCODE = 0
            $global:StepNote = $null
            & $Body
            $code = $global:LASTEXITCODE
            $summary[$Name] = if ($global:StepNote) { $global:StepNote } elseif ($code -eq 0) { 'ok' } else { "falhou (código $code)" }
        }

        Invoke-Step 'capturas de tela headless' {
            $env:NEOVEG_SCREENSHOTS = '1'
            cargo test -p neoveg_ui --test headless_screenshot -- --nocapture 2>&1 | ForEach-Object { "$_" }
        }

        Invoke-Step 'benchmark playback_bench' {
            $bench = Get-ChildItem -Path (Join-Path $Repo 'crates') -Recurse -Filter 'playback_bench.rs' -ErrorAction SilentlyContinue |
                Select-Object -First 1
            if ($bench) {
                cargo bench --bench playback_bench 2>&1 | ForEach-Object { "$_" }
            } else {
                'Não existe um benchmark playback_bench no workspace: etapa ignorada.'
                $global:StepNote = 'ignorado (o benchmark não existe)'
            }
        }

        Invoke-Step 'conversão de ícones' {
            if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
                'Python não encontrado: etapa ignorada.'
                $global:StepNote = 'ignorado (sem Python)'
                return
            }
            if (Test-Path -LiteralPath (Split-Path -Qualifier $Env) -ErrorAction SilentlyContinue) {
                $out = Join-Path $Env 'assets\icons'
            } else {
                $out = Join-Path $Repo 'target\icon_formats'
            }
            python scripts/generate_icon_formats.py --out $out 2>&1 | ForEach-Object { "$_" }
        }

        '=== resumo ==='
        foreach ($key in $summary.Keys) { '{0}: {1}' -f $key, $summary[$key] }
    } | Out-Null
    Write-Host "Job '$JobName' iniciado em segundo plano (repositório: $RepoRoot)."
}

# Acompanhamento do job
if ($Wait) {
    Wait-Job -Name $JobName | Out-Null
    Receive-Job -Name $JobName
    Get-Job -Name $JobName | Format-Table Id, Name, State -AutoSize
    Remove-Job -Name $JobName
} else {
    Show-Job
    Write-Host "Acompanhe com:  Get-Job -Name $JobName;  Receive-Job -Name $JobName -Keep"
}
