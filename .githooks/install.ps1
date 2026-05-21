# Install the voltex git hooks by pointing core.hooksPath at .githooks\.
# Idempotent — safe to re-run.

$ErrorActionPreference = 'Stop'

$repoRoot = & git rev-parse --show-toplevel 2>$null
if (-not $repoRoot) {
    Write-Error "not inside a git repository"
    exit 1
}

Set-Location $repoRoot

git config core.hooksPath .githooks

Write-Host "✓ git hooks installed." -ForegroundColor Green
Write-Host "  core.hooksPath = $(git config --get core.hooksPath)"
Write-Host ""
Write-Host "  Disable temporarily: git commit --no-verify"
Write-Host "  Disable permanently: git config --unset core.hooksPath"
