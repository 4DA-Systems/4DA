# Emergency D: reclaim — delete cargo's regenerable `target/debug/incremental`
# caches, per recipe-reclaim-cargo-incremental-cache. NEVER touches `deps`
# (real artifacts) and NEVER runs `cargo clean`.
#
# Safety: skips any worktree whose target/ was written in the last $SkipMinutes
# minutes, so a lane that is mid-build is left alone.
param([int]$SkipMinutes = 60, [switch]$Execute)

$cutoff = (Get-Date).AddMinutes(-$SkipMinutes)
$roots = @('D:\4DA\src-tauri\target\debug\incremental')
$roots += (Get-ChildItem 'D:\4DA\.claude\worktrees' -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path $_.FullName 'src-tauri\target\debug\incremental' })

$freed = 0
$skipped = @()
foreach ($p in $roots) {
    if (-not (Test-Path $p)) { continue }
    $targetDir = Split-Path (Split-Path $p -Parent) -Parent   # ...\target
    $lastWrite = (Get-Item $targetDir -ErrorAction SilentlyContinue).LastWriteTime
    if ($lastWrite -and $lastWrite -gt $cutoff) {
        $skipped += "$p (active: $lastWrite)"
        continue
    }
    $sz = (Get-ChildItem $p -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum
    if (-not $sz) { $sz = 0 }
    if ($Execute) {
        Remove-Item $p -Recurse -Force -ErrorAction SilentlyContinue
        if (-not (Test-Path $p)) { $freed += $sz }
    } else {
        $freed += $sz
    }
}
Write-Host ("{0}: {1} GB across {2} cache dirs" -f $(if ($Execute) { 'FREED' } else { 'WOULD FREE' }), [math]::Round($freed / 1GB, 2), ($roots.Count - $skipped.Count))
if ($skipped.Count -gt 0) { Write-Host "Skipped (recently active):"; $skipped | ForEach-Object { Write-Host "  $_" } }
Write-Host ("D: free now: {0} GB" -f [math]::Round((Get-PSDrive D).Free / 1GB, 2))
