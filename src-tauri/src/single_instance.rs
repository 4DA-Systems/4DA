// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Single-instance file lock — prevents two copies of 4DA racing on the SQLite WAL.
//!
//! The Tauri plugin `tauri_plugin_single_instance` handles the *in-app* case
//! (second launch notifies the first and exits via its callback). That runs
//! *inside* `tauri::Builder`, i.e. after the Tauri event loop has started and
//! after a lot of our `initialize_pre_tauri()` work — including database
//! opens — has already happened.
//!
//! This module is the **belt** to that plugin's **braces**: a file-based lock
//! in the data directory that we acquire *before* any database or heavy
//! initialization touches disk. If the lock is held by a live PID, this
//! process exits immediately with a native-friendly message.
//!
//! ## Why a separate file-based lock
//!
//! - Runs *before* Tauri. The plugin's callback fires in the original
//!   process, but the second process still executes everything up to the
//!   point Tauri rejects it — which for us has historically included DB
//!   open, scheduler init, and other shared-resource code. That's how WAL
//!   corruption happens.
//! - Survives crashes. The Drop impl removes the file, but if the process
//!   is SIGKILLed the file stays — the next launch detects the stale PID
//!   (process no longer alive) and recovers.
//! - Zero new crates. `sysinfo` would work but `std::process::Command`
//!   with `tasklist` (Windows) and `libc::kill(pid, 0)` (Unix) is enough
//!   and already in our deps.
//!
//! ## Race conditions
//!
//! We use `OpenOptions::create_new(true)` for the happy path — the OS
//! guarantees atomic "create-if-not-exists". If that fails with
//! `AlreadyExists`, we read the file, parse the PID, check liveness, and
//! either reject (live) or take over (stale). A second process racing with
//! the *same stale take-over* would see our newly-written PID and reject —
//! no damage done.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use tracing::{debug, info, warn};

/// Filename of the single-instance lock inside the data directory.
const LOCK_FILENAME: &str = ".process.lock";

/// Errors that can occur while acquiring the instance lock.
#[derive(Debug, Error)]
pub enum InstanceError {
    /// Another live 4DA process already holds the lock.
    #[error("4DA is already running (pid {0})")]
    AlreadyRunning(u32),

    /// I/O error reading or writing the lock file. The caller should treat
    /// this as non-fatal (fall through and continue startup) so a broken
    /// filesystem doesn't brick the app — single-instance is a nice-to-have,
    /// not a correctness gate.
    #[error("lock file I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// RAII guard — removes the lock file on drop (clean shutdown).
#[derive(Debug)]
pub struct InstanceLock {
    path: PathBuf,
    /// Whether the Drop impl should remove the file. We set this to false
    /// if the process is being replaced (hot reload) or if the caller
    /// explicitly wants to keep the lock.
    active: bool,
}

impl InstanceLock {
    /// Path of the lock file this guard manages.
    #[allow(dead_code)] // used by tests and future diagnostics
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Disarm the guard — on drop the lock file will NOT be removed.
    /// Use this only if something else takes ownership of the lock.
    #[allow(dead_code)]
    pub fn forget(mut self) {
        self.active = false;
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => {
                debug!(target: "4da::single_instance", path = ?self.path, "Instance lock released")
            }
            Err(e) => {
                debug!(target: "4da::single_instance", error = %e, "Failed to remove lock file (may be already gone)")
            }
        }
    }
}

/// Attempt to acquire the single-instance lock.
///
/// Happy path: creates `data_dir/.process.lock` atomically, writes our
/// PID + timestamp, returns `Ok(InstanceLock)`.
///
/// Race path: if the file already exists, parses the PID, checks liveness.
/// - Live PID: returns `Err(AlreadyRunning(pid))`.
/// - Stale PID: removes the file, re-creates with our PID, returns `Ok`.
///
/// Error path: any filesystem error other than `AlreadyExists` on
/// `create_new` is returned as `InstanceError::Io`. The caller should
/// treat I/O errors as soft — not blocking startup — because the lock is
/// a safety feature, not a correctness gate.
pub fn acquire_instance_lock(data_dir: &Path) -> Result<InstanceLock, InstanceError> {
    // Ensure the directory exists.
    std::fs::create_dir_all(data_dir)?;

    let lock_path = data_dir.join(LOCK_FILENAME);

    match try_create_lock(&lock_path) {
        Ok(lock) => Ok(lock),
        Err(InstanceError::Io(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Lock file exists — inspect PID.
            match read_lock_pid(&lock_path) {
                Some(pid) if is_process_alive(pid) => {
                    warn!(target: "4da::single_instance", pid, path = ?lock_path,
                        "Live 4DA instance detected");
                    Err(InstanceError::AlreadyRunning(pid))
                }
                Some(pid) => {
                    info!(target: "4da::single_instance", stale_pid = pid, path = ?lock_path,
                        "Stale lock file detected (PID not alive) — taking over");
                    // Remove the stale file and try again. If another process
                    // beat us here, create_new will fail AlreadyExists again
                    // and we'll recurse through the same logic — worst case
                    // we reject in favor of the other racer, which is safe.
                    let _ = std::fs::remove_file(&lock_path);
                    try_create_lock(&lock_path)
                }
                None => {
                    // Unreadable/empty lock file — treat as stale.
                    warn!(target: "4da::single_instance", path = ?lock_path,
                        "Unreadable lock file — treating as stale");
                    let _ = std::fs::remove_file(&lock_path);
                    try_create_lock(&lock_path)
                }
            }
        }
        Err(other) => Err(other),
    }
}

/// Create the lock file atomically (`create_new = true`). Returns
/// `AlreadyExists` I/O error if another process already owns it.
fn try_create_lock(lock_path: &Path) -> Result<InstanceLock, InstanceError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)?;

    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let contents = format!("{pid}\n{ts}\n");
    file.write_all(contents.as_bytes())?;
    file.sync_all().ok();

    info!(target: "4da::single_instance", pid, path = ?lock_path, "Instance lock acquired");
    Ok(InstanceLock {
        path: lock_path.to_path_buf(),
        active: true,
    })
}

/// Parse the PID out of an existing lock file. Returns `None` if the file
/// is unreadable, empty, or doesn't start with a parseable integer.
fn read_lock_pid(lock_path: &Path) -> Option<u32> {
    let mut contents = String::new();
    let mut f = File::open(lock_path).ok()?;
    f.read_to_string(&mut contents).ok()?;
    contents.lines().next()?.trim().parse::<u32>().ok()
}

/// Check whether a given PID refers to a live process on this machine.
///
/// Windows: invokes `tasklist /FI "PID eq <pid>" /FO CSV /NH`. A live PID
/// returns one CSV line; a dead PID returns an "INFO: No tasks..." message.
///
/// Unix: uses `kill(pid, 0)` via `libc`. Signal 0 is a no-op that only
/// checks the process exists and we have permission to signal it.
///
/// On error (e.g. tasklist missing, libc call fails), returns `true` as
/// a safe default — better to refuse startup than to race two instances.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        is_process_alive_windows(pid)
    }
    #[cfg(not(target_os = "windows"))]
    {
        is_process_alive_unix(pid)
    }
}

#[cfg(target_os = "windows")]
fn is_process_alive_windows(pid: u32) -> bool {
    use std::process::Command;

    // tasklist is present on every Windows SKU since XP. We prefer it to
    // OpenProcess because it doesn't require additional features on
    // windows-sys and can't accidentally keep a handle open.
    let mut cmd = Command::new("tasklist");
    cmd.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd.output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // When there's no match, tasklist prints:
            //   INFO: No tasks are running which match the specified criteria.
            // When there IS a match, it prints CSV rows starting with a quoted
            // image name. Look for the exact PID as a CSV field.
            let needle = format!("\"{pid}\"");
            stdout.contains(&needle)
        }
        Ok(_) => {
            // Non-zero exit — tasklist failed. Default to "alive" so we
            // don't accidentally stomp on a real running instance.
            warn!(target: "4da::single_instance", pid, "tasklist returned non-zero status, assuming alive");
            true
        }
        Err(e) => {
            warn!(target: "4da::single_instance", pid, error = %e,
                "tasklist invocation failed, assuming alive");
            true
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_alive_unix(pid: u32) -> bool {
    // kill(pid, 0) returns 0 if the process exists and we can signal it,
    // -1 otherwise. errno == ESRCH specifically means "no such process".
    // EPERM means the process exists but we lack permission — still alive.
    #[allow(unsafe_code)]
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return true;
    }
    // Read errno to distinguish ESRCH (dead) from EPERM (alive).
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ESRCH) => false,
        Some(libc::EPERM) => true, // Process exists, just not ours to signal.
        _ => {
            // Unexpected errno — safer to assume alive.
            warn!(target: "4da::single_instance", pid, error = %err, "Unexpected errno from kill(pid,0)");
            true
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn acquire_creates_lock_file_with_current_pid() {
        // Happy path: empty directory -> lock acquired, file exists, PID is us.
        let dir = TempDir::new().expect("tempdir");
        let lock = acquire_instance_lock(dir.path()).expect("first acquire");
        assert!(lock.path().exists(), "lock file should exist");

        let pid = read_lock_pid(lock.path()).expect("parseable pid");
        assert_eq!(pid, std::process::id(), "lock should record our PID");

        drop(lock);
        assert!(
            !dir.path().join(LOCK_FILENAME).exists(),
            "Drop must remove the lock file on clean shutdown"
        );
    }

    #[test]
    fn second_acquire_against_live_pid_returns_already_running() {
        // Simulating another live 4DA: our own PID is definitionally alive,
        // so a second acquire while holding the lock must reject.
        let dir = TempDir::new().expect("tempdir");
        let _first = acquire_instance_lock(dir.path()).expect("first acquire");

        let second = acquire_instance_lock(dir.path());
        match second {
            Err(InstanceError::AlreadyRunning(pid)) => {
                assert_eq!(
                    pid,
                    std::process::id(),
                    "AlreadyRunning must report the live PID"
                );
            }
            other => panic!("expected AlreadyRunning, got {other:?}"),
        }
    }

    #[test]
    fn stale_lock_with_dead_pid_is_taken_over() {
        // A PID from the previous boot that is no longer running must not
        // brick the app. Use a very high PID that is virtually guaranteed
        // not to be in use on either Windows or Linux. (PID 0 is the
        // "System Idle Process" on Windows and thus reports as alive —
        // we must avoid it. Max user-assigned PID on Linux defaults to
        // 4194304; 0x7FFFFFFE is safely above any realistic running PID
        // on modern Windows as well.)
        let dir = TempDir::new().expect("tempdir");
        let lock_path = dir.path().join(LOCK_FILENAME);
        let dead_pid: u32 = 0x7FFF_FFFE;
        std::fs::write(&lock_path, format!("{dead_pid}\n0\n")).expect("seed stale lock");
        assert!(lock_path.exists());
        // Sanity: the PID we chose really is not alive.
        assert!(
            !is_process_alive(dead_pid),
            "precondition: chosen stale PID {dead_pid} must not be alive on this host"
        );

        let lock = acquire_instance_lock(dir.path()).expect("should take over stale lock");
        let pid = read_lock_pid(lock.path()).expect("new pid");
        assert_eq!(
            pid,
            std::process::id(),
            "take-over must rewrite with our PID"
        );
    }

    #[test]
    fn unreadable_lock_file_is_treated_as_stale() {
        // Empty lock file -> no parseable PID -> treated as stale.
        let dir = TempDir::new().expect("tempdir");
        let lock_path = dir.path().join(LOCK_FILENAME);
        std::fs::write(&lock_path, b"").expect("seed empty lock");

        let lock = acquire_instance_lock(dir.path()).expect("should recover from empty lock");
        assert!(lock.path().exists());
    }

    #[test]
    fn forget_prevents_drop_cleanup() {
        // forget() is the escape hatch for process-replacement scenarios.
        // Verify the lock file survives the guard's destructor.
        let dir = TempDir::new().expect("tempdir");
        let lock = acquire_instance_lock(dir.path()).expect("acquire");
        let path = lock.path().to_path_buf();
        lock.forget();
        assert!(path.exists(), "forget() must leave the file intact");
        // Clean up so the test doesn't leak.
        let _ = std::fs::remove_file(path);
    }
}
