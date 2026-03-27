# Windows EV Code Signing via SSL.com eSigner / CodeSignTool
# Called by Tauri during `tauri build` via signCommand in tauri.conf.json.
# Silently skips if SSL_COM_CREDENTIAL_ID is not set (dev machines, macOS/Linux CI).

param(
    [Parameter(Mandatory=$true)]
    [string]$FilePath
)

if (-not $env:SSL_COM_CREDENTIAL_ID) {
    Write-Host "  [sign] Skipping code signing — SSL_COM_CREDENTIAL_ID not set"
    exit 0
}

Write-Host "  [sign] Signing: $FilePath"

$signArgs = @(
    "sign"
    "-credential_id=$env:SSL_COM_CREDENTIAL_ID"
    "-username=$env:SSL_COM_USERNAME"
    "-password=$env:SSL_COM_PASSWORD"
    "-totp_secret=$env:SSL_COM_TOTP_SECRET"
    "-input_file_path=`"$FilePath`""
    "-override"
)

try {
    & CodeSignTool @signArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "CodeSignTool failed with exit code $LASTEXITCODE"
        exit $LASTEXITCODE
    }
    Write-Host "  [sign] Signed successfully: $FilePath"
} catch {
    Write-Error "Code signing failed: $_"
    exit 1
}
