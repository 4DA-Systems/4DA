# Windows EV Code Signing via SSL.com eSigner / CodeSignTool
# Called by Tauri during `tauri build` via signCommand in tauri.conf.json.
# Silently skips if SSL_COM_CREDENTIAL_ID is not set (dev machines, macOS/Linux CI).
#
# Tauri captures (and on failure, suppresses) this script's stdout/stderr and
# only surfaces a generic "failed to run powershell". So we ALSO tee everything
# to a debug log that a later `if: always()` release.yml step prints, making the
# real CodeSignTool error visible.

param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath
)

$logFile = if ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP 'sign-windows-debug.log' } else { Join-Path $env:TEMP 'sign-windows-debug.log' }
function Log($msg) {
    $line = "[sign] $msg"
    Write-Host $line
    try { Add-Content -Path $logFile -Value $line -ErrorAction SilentlyContinue } catch {}
}

if (-not $env:SSL_COM_CREDENTIAL_ID) {
    Log "Skipping code signing -- SSL_COM_CREDENTIAL_ID not set"
    exit 0
}

Log "Signing: $FilePath"

# Diagnostics: confirm the signer + its Java runtime are actually resolvable in
# THIS process (PATH set via GITHUB_PATH in a prior step is inherited here).
$cst = Get-Command CodeSignTool -ErrorAction SilentlyContinue
Log ("CodeSignTool: " + $(if ($cst) { $cst.Source } else { 'NOT FOUND on PATH' }))
$java = Get-Command java -ErrorAction SilentlyContinue
Log ("java: " + $(if ($java) { $java.Source } else { 'NOT FOUND on PATH' }))

# Pass -input_file_path WITHOUT embedded quotes. PowerShell quotes array args
# containing spaces automatically when spawning; the previous
# `-input_file_path="$FilePath"` form passed LITERAL quotes through to the Java
# tool, which then looked for a path that included the quote characters.
$signArgs = @(
    "sign",
    "-credential_id=$env:SSL_COM_CREDENTIAL_ID",
    "-username=$env:SSL_COM_USERNAME",
    "-password=$env:SSL_COM_PASSWORD",
    "-totp_secret=$env:SSL_COM_TOTP_SECRET",
    "-input_file_path=$FilePath",
    "-override"
)

try {
    $output = & CodeSignTool @signArgs 2>&1
    $code = $LASTEXITCODE
    $text = ($output | Out-String)
    Write-Host $text
    try { Add-Content -Path $logFile -Value $text -ErrorAction SilentlyContinue } catch {}
    if ($code -ne 0) {
        Log "CodeSignTool FAILED with exit code $code"
        exit $code
    }
    Log "Signed successfully: $FilePath"
} catch {
    Log "Code signing EXCEPTION: $_"
    exit 1
}
