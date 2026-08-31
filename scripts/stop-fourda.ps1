# Stop fourda.exe instances belonging to ONE tree only.
#
# Why this exists (2026-08-31): `taskkill /F /IM fourda.exe` kills EVERY
# fourda instance on the machine by image name. On this multi-lane fleet that
# includes other worktrees' dev apps and the scheduled background-refresh
# engine mid-cycle — two engine runs were killed as collateral by a peer
# lane's dev restart in one afternoon (task result 0xFFFFFFFF, judgments
# lost, ghost tray icons). Kill by executable PATH, not image name.
#
# Usage:
#   pwsh scripts/stop-fourda.ps1            # stop instances from THIS tree's target/
#   pwsh scripts/stop-fourda.ps1 -Root D:\some\worktree
#   pwsh scripts/stop-fourda.ps1 -DryRun    # show what would be stopped
param(
    [string]$Root = (Join-Path (Split-Path $PSScriptRoot -Parent) 'src-tauri\target'),
    [switch]$DryRun
)

$prefix = (Resolve-Path $Root -ErrorAction SilentlyContinue)?.Path
if (-not $prefix) {
    Write-Host "No such target root: $Root — nothing to stop."
    exit 0
}

$mine = Get-CimInstance Win32_Process -Filter "Name = 'fourda.exe'" |
    Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) }

if (-not $mine) {
    Write-Host "No fourda.exe running from $prefix"
    exit 0
}

foreach ($p in $mine) {
    if ($DryRun) {
        Write-Host "[dry-run] would stop PID $($p.ProcessId): $($p.ExecutablePath)"
    } else {
        Write-Host "Stopping PID $($p.ProcessId): $($p.ExecutablePath)"
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

$others = Get-CimInstance Win32_Process -Filter "Name = 'fourda.exe'" |
    Where-Object { $_.ExecutablePath -and -not $_.ExecutablePath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) }
if ($others) {
    Write-Host "Left running (other trees/lanes):"
    $others | ForEach-Object { Write-Host "  PID $($_.ProcessId): $($_.ExecutablePath)" }
}
