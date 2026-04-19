// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Binary integrity verification.
//!
//! On startup, checks that the running binary hasn't been tampered with
//! by verifying code signature status (Windows/macOS) or file permissions (Linux).
//! Logs security events but does NOT block execution (could be running unsigned in dev).

use tracing::{info, warn};

/// Run integrity checks on the current binary.
/// Returns true if all checks pass, false if any concern is detected.
/// Never blocks execution — only logs warnings.
pub(crate) fn verify_integrity() -> bool {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            warn!(target: "4da::integrity", error = %e, "Cannot determine executable path");
            return false;
        }
    };

    info!(target: "4da::integrity", path = %exe_path.display(), "Verifying binary integrity");

    let mut ok = true;

    // Check 1: Executable exists and is a regular file
    if !exe_path.is_file() {
        warn!(target: "4da::integrity", "Executable path is not a regular file");
        ok = false;
    }

    // Check 2: File size sanity (4DA binary should be > 10MB, < 500MB)
    if let Ok(meta) = std::fs::metadata(&exe_path) {
        let size = meta.len();
        if size < 10_000_000 {
            warn!(target: "4da::integrity", size, "Binary suspiciously small");
            ok = false;
        }
        if size > 500_000_000 {
            warn!(target: "4da::integrity", size, "Binary suspiciously large");
            ok = false;
        }
    }

    // Check 3: Platform-specific signature verification
    #[cfg(target_os = "windows")]
    {
        ok = verify_windows_signature(&exe_path) && ok;
    }

    #[cfg(target_os = "macos")]
    {
        ok = verify_macos_signature(&exe_path) && ok;
    }

    #[cfg(target_os = "linux")]
    {
        ok = verify_linux_permissions(&exe_path) && ok;
    }

    if ok {
        info!(target: "4da::integrity", "Binary integrity check passed");
    } else {
        warn!(target: "4da::integrity", "Binary integrity concerns detected — this may be a development build");
        // Log to security audit if database is available
        if let Ok(db) = crate::get_database() {
            db.log_security_event(
                "integrity_warning",
                &exe_path.display().to_string(),
                "warning",
            );
        }
    }

    ok
}

/// Windows: Check if binary has a valid Authenticode signature.
/// Uses PowerShell's `Get-AuthenticodeSignature` as a simple check.
#[cfg(target_os = "windows")]
fn verify_windows_signature(exe_path: &std::path::Path) -> bool {
    // In development/debug builds, binaries won't be signed — that's OK
    if cfg!(debug_assertions) {
        return true;
    }

    match std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-AuthenticodeSignature '{}').Status",
                exe_path.display()
            ),
        ])
        .output()
    {
        Ok(output) => {
            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if status == "Valid" {
                info!(target: "4da::integrity", "Windows Authenticode signature valid");
                true
            } else {
                warn!(target: "4da::integrity", status = %status, "Binary not signed or signature invalid");
                false
            }
        }
        Err(_) => {
            // PowerShell not available — can't verify, assume OK
            true
        }
    }
}

/// macOS: Check codesign status.
#[cfg(target_os = "macos")]
fn verify_macos_signature(exe_path: &std::path::Path) -> bool {
    if cfg!(debug_assertions) {
        return true;
    }

    match std::process::Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(exe_path)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                info!(target: "4da::integrity", "macOS code signature valid");
                true
            } else {
                warn!(target: "4da::integrity", "macOS code signature verification failed");
                false
            }
        }
        Err(_) => true, // codesign not available
    }
}

/// Linux: Check file permissions (no world-writable).
/// Uses `std::os::unix` — no unsafe required.
#[cfg(target_os = "linux")]
fn verify_linux_permissions(exe_path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(exe_path) {
        Ok(meta) => {
            let mode = meta.permissions().mode();
            // Check not world-writable (others write bit)
            if mode & 0o002 != 0 {
                warn!(target: "4da::integrity", mode = format!("{mode:o}"), "Binary is world-writable");
                return false;
            }
            true
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_integrity_does_not_block() {
        // The function should always return without panicking,
        // even in test environments where the binary is small/unsigned.
        let _result = verify_integrity();
    }
}
