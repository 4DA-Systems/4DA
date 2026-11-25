<#
.SYNOPSIS
  After an NSIS upgrade, check that the install directory holds only what the
  candidate installer ships, plus the one file set the app reads although no
  installer ships it any more.

.DESCRIPTION
  An NSIS upgrade writes the candidate's files over the previous release's and
  deletes nothing, so a file only an older installer shipped stays unless the
  candidate's own hook removes it (src-tauri/windows/hooks.nsh,
  FOURDA_REMOVE_UNREAD_LEFTOVERS, at install). What the candidate ships is read
  from the installer itself (7-Zip lists an NSIS payload), so this needs no
  list of its own beyond $ReadLeftovers.

  Used by .github/workflows/installer-smoke.yml (Windows upgrade leg).

.PARAMETER Installer
  The candidate setup.exe that was just run over the previous release.

.PARAMETER InstallDir
  The install directory.

.PARAMETER Strict
  Fail on a leftover. Without it a leftover is a warning: installers up to
  1.0.4 have no hook that removes them.
#>
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [Parameter(Mandatory = $true)][string]$InstallDir,
  [switch]$Strict
)
$ErrorActionPreference = 'Stop'

# 1.0.3's bundled embedding model. From 1.0.4 no installer ships it, but the
# app loads it before its download cache (embeddings_providers/model_store.rs),
# so an upgrade keeps it; only an uninstall removes it (hooks.nsh,
# FOURDA_REMOVE_READ_LEFTOVERS). Keep the two lists equal.
$ReadLeftovers = @(
  'models\embeddings\nomic-embed-text-v1.5-fp16\config.json',
  'models\embeddings\nomic-embed-text-v1.5-fp16\model_fp16.onnx',
  'models\embeddings\nomic-embed-text-v1.5-fp16\special_tokens_map.json',
  'models\embeddings\nomic-embed-text-v1.5-fp16\tokenizer.json',
  'models\embeddings\nomic-embed-text-v1.5-fp16\tokenizer_config.json'
)

# The candidate's payload: every entry after 7-Zip's "----------" line, less
# the NSIS-internal ones ($TEMP\..., $PLUGINSDIR\...).
$listing = & 7z l -slt -- $Installer
if ($LASTEXITCODE -ne 0) {
  Write-Host "::error title=Upgrade check::7-Zip could not list $Installer (exit $LASTEXITCODE)"
  exit 1
}
$shipped = @{}
$inEntries = $false
foreach ($line in $listing) {
  if (-not $inEntries) { if ($line -eq '----------') { $inEntries = $true }; continue }
  if ($line -match '^Path = (.+)$' -and -not $Matches[1].StartsWith('$')) {
    $shipped[$Matches[1].ToLowerInvariant()] = $true
  }
}
if (-not $shipped.ContainsKey('uninstall.exe') -or $shipped.Count -lt 2) {
  Write-Host "::error title=Upgrade check::the listing of $Installer does not look like a 4DA installer ($($shipped.Count) entries)"
  exit 1
}

$root = (Resolve-Path -LiteralPath $InstallDir).Path.TrimEnd('\')
$present = @(Get-ChildItem -LiteralPath $root -Recurse -File -Force)
$kept = @()
$left = @()
foreach ($f in $present) {
  $rel = $f.FullName.Substring($root.Length + 1)
  if ($shipped.ContainsKey($rel.ToLowerInvariant())) { continue }
  if ($ReadLeftovers -contains $rel) { $kept += $f; continue }
  $left += $f
}
Write-Host "the candidate installer ships $($shipped.Count) file(s); $root holds $($present.Count)"
if ($kept.Count -gt 0) {
  $mb = [math]::Round((($kept | Measure-Object -Property Length -Sum).Sum) / 1MB, 1)
  Write-Host "kept, because the app reads them: $($kept.Count) file(s), $mb MB"
  $kept | ForEach-Object { Write-Host "  $($_.FullName.Substring($root.Length + 1))" }
}
if ($left.Count -eq 0) {
  Write-Host 'no file only an older release shipped is left'
  exit 0
}
$mb = [math]::Round((($left | Measure-Object -Property Length -Sum).Sum) / 1MB, 1)
$left | ForEach-Object { Write-Host "  left: $($_.FullName.Substring($root.Length + 1))" }
$msg = "$($left.Count) file(s), $mb MB, that only an older release shipped are still in $root after the upgrade"
if ($Strict) {
  Write-Host "::error title=Upgrade::$msg"
  exit 1
}
Write-Host "::warning title=Upgrade::$msg"
exit 0
