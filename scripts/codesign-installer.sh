#!/usr/bin/env bash
# Manual local code signing for Windows installers via SSL.com CodeSignTool.
# Called from the release runbook (step 4) when signing outside CI.
#
# Prerequisites:
#   - CodeSignTool on PATH (download from https://www.ssl.com/esigner/)
#   - Environment variables: SSL_COM_USERNAME, SSL_COM_PASSWORD,
#     SSL_COM_CREDENTIAL_ID, SSL_COM_TOTP_SECRET
#
# Usage:
#   ./scripts/codesign-installer.sh path/to/4DA_1.0.0_x64-setup.exe

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <path-to-installer>"
    exit 1
fi

FILE="$1"

if [[ ! -f "$FILE" ]]; then
    echo "ERROR: File not found: $FILE"
    exit 1
fi

for VAR in SSL_COM_USERNAME SSL_COM_PASSWORD SSL_COM_CREDENTIAL_ID SSL_COM_TOTP_SECRET; do
    if [[ -z "${!VAR:-}" ]]; then
        echo "ERROR: $VAR is not set"
        exit 1
    fi
done

if ! command -v CodeSignTool &> /dev/null; then
    echo "ERROR: CodeSignTool not found on PATH"
    echo "Download from https://www.ssl.com/esigner/ and add to PATH"
    exit 1
fi

echo "Signing: $FILE"

CodeSignTool sign \
    -credential_id="$SSL_COM_CREDENTIAL_ID" \
    -username="$SSL_COM_USERNAME" \
    -password="$SSL_COM_PASSWORD" \
    -totp_secret="$SSL_COM_TOTP_SECRET" \
    -input_file_path="$FILE" \
    -override

echo ""
echo "Signed: $FILE"
echo "SHA-256: $(sha256sum "$FILE" | awk '{print $1}')"
echo ""
echo "Next: node scripts/verify-installer.cjs --path \"$FILE\""
