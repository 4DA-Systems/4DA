# Windows Authenticode signing via Azure Artifact Signing (artifact-signing-cli).
#
# Tauri calls this once per file through bundle.windows.signCommand, but ONLY
# when the release build passes --config src-tauri/tauri.artifact-signing.conf.json.
# release.yml does that only when the ARTIFACT_SIGNING_* repository variables
# are configured. The default tauri.conf.json never references this script, so
# dev builds and unconfigured releases never reach it.
#
# Because this script runs only when signing was explicitly configured, it
# NEVER skips silently: a missing input is a failure. (The SSL.com script,
# sign-windows.ps1, skips when credentials are absent; that behaviour is what
# lets an unsigned build through, and the verify step exists to catch it.)
#
# Every file Tauri hands over is signed, including the NSIS uninstaller, which
# arrives as a temporary file with a .tmp extension. signtool recognises a PE
# file by its content, not its extension, so no extension filter is applied.
#
# tauri-bundler swallows a failing signCommand's output and reports only that
# the command failed. Everything is therefore also appended to
# sign-windows-debug.log, which release.yml prints in an always() step.

param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath
)

$logFile = if ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP 'sign-windows-debug.log' } else { Join-Path $env:TEMP 'sign-windows-debug.log' }
function Log($msg) {
    $line = "[artifact-sign] $msg"
    Write-Host $line
    try { Add-Content -Path $logFile -Value $line -ErrorAction SilentlyContinue } catch {}
}

Log "Signing: $FilePath"

# Names only, never values: the secrets are masked by GitHub, but a log line
# that prints them would leak them anywhere else this script is run.
$required = @(
    'ARTIFACT_SIGNING_ENDPOINT',
    'ARTIFACT_SIGNING_ACCOUNT',
    'ARTIFACT_SIGNING_PROFILE',
    'AZURE_CLIENT_ID',
    'AZURE_CLIENT_SECRET',
    'AZURE_TENANT_ID'
)
$missing = @($required | Where-Object { -not [Environment]::GetEnvironmentVariable($_) })
if ($missing.Count -gt 0) {
    Log ("Cannot sign: missing " + ($missing -join ', ') + ". Artifact Signing was selected, so this is a failure, not a skip.")
    exit 1
}

if (-not (Test-Path -LiteralPath $FilePath)) {
    Log "Cannot sign: file not found: $FilePath"
    exit 1
}

# Resolve the CLI by ABSOLUTE path (exported by the install step). A PATH
# addition made earlier in the job did not reliably reach Tauri's signCommand
# subprocess for CodeSignTool; the same lesson applies here. Fall back to PATH.
$cli = $env:ARTIFACT_SIGNING_CLI
if (-not $cli -or -not (Test-Path -LiteralPath $cli)) {
    $found = Get-Command artifact-signing-cli -ErrorAction SilentlyContinue
    if ($found) { $cli = $found.Source } else { $cli = $null }
}
if (-not $cli) {
    Log "Cannot sign: artifact-signing-cli not found (ARTIFACT_SIGNING_CLI unset and not on PATH)"
    exit 1
}
Log "artifact-signing-cli: $cli"
Log ("signtool: " + $(if ($env:SIGNTOOL_PATH) { $env:SIGNTOOL_PATH } else { '(CLI default)' }))
Log ("azure cli: " + $(if ($env:AZURE_CLI_PATH) { $env:AZURE_CLI_PATH } else { '(CLI default)' }))

# The CLI reads AZURE_CLIENT_ID / AZURE_CLIENT_SECRET / AZURE_TENANT_ID from the
# environment. Endpoint, account and certificate profile are passed explicitly.
# -d sets the signature description, which Windows shows as the program name in
# the UAC prompt for an .msi and otherwise displays as a random string.
$signArgs = @(
    '-e', $env:ARTIFACT_SIGNING_ENDPOINT,
    '-a', $env:ARTIFACT_SIGNING_ACCOUNT,
    '-c', $env:ARTIFACT_SIGNING_PROFILE,
    '-d', '4DA',
    $FilePath
)

try {
    $output = & $cli @signArgs 2>&1
    $code = $LASTEXITCODE
    $text = ($output | Out-String)
    Write-Host $text
    try { Add-Content -Path $logFile -Value $text -ErrorAction SilentlyContinue } catch {}
    if ($code -ne 0) {
        Log "artifact-signing-cli FAILED (exit=$code) for $FilePath"
        exit 1
    }
} catch {
    Log "artifact-signing-cli EXCEPTION: $_"
    exit 1
}

# Trust the result, but check it. A signing tool that exits 0 without signing
# is exactly what happened with CodeSignTool; ask Windows directly.
#
# The module is imported by ABSOLUTE path from this edition's own $PSHOME.
# Measured 2026-10-12: under Tauri's signCommand, Windows PowerShell 5.1
# inherited a PSModulePath that listed PowerShell 7's module directory first,
# and Get-AuthenticodeSignature failed with "the module
# 'Microsoft.PowerShell.Security' could not be loaded". (pwsh repairs that
# path when IT launches powershell.exe; a node -> tauri -> powershell chain
# gets no such repair.) Never depend on module autoload here.
try {
    $securityModule = Join-Path $PSHOME 'Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1'
    Import-Module $securityModule -ErrorAction Stop
    $sig = Get-AuthenticodeSignature -FilePath $FilePath -ErrorAction Stop
} catch {
    Log "Could not read the signature of ${FilePath}: $_"
    exit 1
}
if ($sig.Status -ne 'Valid') {
    Log "Signed by the CLI but Windows reports status '$($sig.Status)' for $FilePath"
    exit 1
}
if (-not $sig.TimeStamperCertificate) {
    Log "Signature on $FilePath has NO timestamp. Artifact Signing certificates live for days, so an untimestamped signature expires with them."
    exit 1
}
Log "Signed successfully: $FilePath ($($sig.SignerCertificate.Subject))"
exit 0
