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

# Tauri invokes signCommand for EVERY file in the bundle pipeline, including NSIS
# plugin DLLs and transient .tmp files. CodeSignTool errors on non-PE inputs
# ("Unsupported file format - tmp"); skip anything not a signable PE artifact.
$ext = [System.IO.Path]::GetExtension($FilePath).ToLowerInvariant()
$signable = @('.exe', '.dll', '.msi', '.msix', '.appx', '.cab', '.sys', '.ocx', '.ps1')
if ($signable -notcontains $ext) {
    Log "Skipping non-signable file ($ext): $FilePath"
    exit 0
}

# Resolve CodeSignTool by ABSOLUTE path (CODESIGNTOOL_BAT, set by the install
# step) — the signCommand subprocess does NOT reliably inherit the GITHUB_PATH
# addition, so a bare `CodeSignTool` was "not recognized". Fall back to PATH.
$tool = $env:CODESIGNTOOL_BAT
if (-not $tool -or -not (Test-Path $tool)) {
    $cmd = Get-Command CodeSignTool -ErrorAction SilentlyContinue
    if ($cmd) { $tool = $cmd.Source }
}
Log ("CodeSignTool: " + $(if ($tool) { $tool } else { 'NOT FOUND (CODESIGNTOOL_BAT unset and not on PATH)' }))
$java = Get-Command java -ErrorAction SilentlyContinue
Log ("java: " + $(if ($java) { $java.Source } else { 'NOT FOUND on PATH' }))
if (-not $tool) { Log "Cannot sign: CodeSignTool unavailable"; exit 1 }

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

# Run from CodeSignTool's own directory: the .bat resolves its jar/conf relative
# to itself, and some versions assume the working dir is the tool dir.
$toolDir = Split-Path -Parent $tool
try {
    Push-Location $toolDir
    $output = & $tool @signArgs 2>&1
    $code = $LASTEXITCODE
    Pop-Location
    $text = ($output | Out-String)
    Write-Host $text
    try { Add-Content -Path $logFile -Value $text -ErrorAction SilentlyContinue } catch {}
    # CodeSignTool exits 0 EVEN ON FAILURE (e.g. eSigner auth rejection prints
    # "Error: The provided authorization grant is invalid..." then returns 0).
    # Treat any "Error:" line as failure so the build stops at the real cause
    # instead of producing an unsigned binary that only the later verify catches.
    if ($code -ne 0 -or ($text -match '(?im)^\s*Error:')) {
        Log "CodeSignTool FAILED (exit=$code) for $FilePath"
        if ($code -eq 0) { Log "  (exit code was 0 but output contained an Error: line)" }
        exit 1
    }
    Log "Signed successfully: $FilePath"
} catch {
    try { Pop-Location } catch {}
    Log "Code signing EXCEPTION: $_"
    exit 1
}
