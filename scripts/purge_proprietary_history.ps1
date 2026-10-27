<#
.SYNOPSIS
    Clean-Room Git history purge for proprietary/binary assets in Neoveg.

.DESCRIPTION
    Runs in DRY-RUN mode by default (lists historical paths detected in `git log --all` and prints
    the exact `git-filter-repo` and `bfg` commands). Pass `-Execute` on a clean mirror backup
    when ready to rewrite history prior to a coordinated `git push --force-with-lease`.
#>
param(
    [switch]$Execute
)

$ErrorActionPreference = "Stop"

Write-Host "=== Git History Audit (Clean-Room Compliance) ===" -ForegroundColor Cyan
$histFiles = git log --all --pretty=format: --name-only |
    Where-Object { $_ -match '^(results_assets/|.*\.(ico|bmp|dll|exe|veg)$)' } |
    Sort-Object -Unique

Write-Host ("Historical binary/proprietary paths detected: {0}" -f $histFiles.Count)
$histFiles | Select-Object -First 25 | ForEach-Object { Write-Host "  - $_" }
if ($histFiles.Count -gt 25) {
    Write-Host ("  ... (+{0} additional paths)" -f ($histFiles.Count - 25))
}

$filterRepoCmd = @(
    "git filter-repo --invert-paths",
    "  --path results_assets/",
    "  --path-glob '*.ico'",
    "  --path-glob '*.bmp'",
    "  --path-glob '*.dll'",
    "  --path-glob '*.exe'",
    "  --path-glob '*.veg'"
) -join " ```n"

$bfgCmd = @(
    "bfg --delete-folders results_assets --delete-files '*.{ico,bmp,dll,exe,veg}'",
    "git reflog expire --expire=now --all",
    "git gc --prune=now --aggressive"
) -join "`n"

Write-Host "`n[Option A -- git-filter-repo (recommended)]:`n$filterRepoCmd" -ForegroundColor Green
Write-Host "`n[Option B -- BFG Repo-Cleaner]:`n$bfgCmd" -ForegroundColor Yellow

if (-not $Execute) {
    Write-Host "`nDRY-RUN complete (no historical commits rewritten). Pass -Execute after backing up the repository." -ForegroundColor Cyan
    exit 0
}

if (-not (Get-Command git-filter-repo -ErrorAction SilentlyContinue)) {
    throw "git-filter-repo is not installed on PATH (install with: pip install git-filter-repo)."
}

git filter-repo --invert-paths `
    --path results_assets/ `
    --path-glob '*.ico' `
    --path-glob '*.bmp' `
    --path-glob '*.dll' `
    --path-glob '*.exe' `
    --path-glob '*.veg'
