// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use super::*;

const FAST: Duration = Duration::from_millis(1);

#[test]
fn a_free_profile_never_waits() {
    let mut polls = 0;
    let outcome = wait_until_released(Duration::from_secs(5), FAST, || {
        polls += 1;
        false
    });
    assert_eq!(outcome, ProfileWait::Free);
    assert_eq!(polls, 1);
}

#[test]
fn a_profile_released_mid_wait_reports_released() {
    let mut polls = 0;
    let outcome = wait_until_released(Duration::from_secs(5), FAST, || {
        polls += 1;
        polls < 4
    });
    assert!(matches!(outcome, ProfileWait::Released(_)), "{outcome:?}");
    assert_eq!(polls, 4);
}

#[test]
fn a_profile_held_past_the_timeout_gives_up() {
    let outcome = wait_until_released(Duration::from_millis(20), FAST, || true);
    match outcome {
        ProfileWait::StillBusy(waited) => assert!(waited >= Duration::from_millis(20)),
        other => panic!("expected StillBusy, got {other:?}"),
    }
}

#[test]
fn the_lockfile_sits_in_the_apps_webview2_profile() {
    let local = Path::new("C:/Users/u/AppData/Local");
    assert_eq!(
        profile_lockfile(local, "com.4da.app"),
        Some(local.join("com.4da.app").join("EBWebView").join("lockfile"))
    );
    assert_eq!(profile_lockfile(local, "../escape"), None);
    assert_eq!(profile_lockfile(local, ""), None);
}

#[test]
fn an_absent_or_released_lockfile_is_free() {
    let dir = tempfile::tempdir().unwrap();
    let lockfile = dir.path().join("lockfile");
    assert!(!is_profile_busy(&lockfile));
    std::fs::write(&lockfile, b"").unwrap();
    assert!(!is_profile_busy(&lockfile));
}

/// WebView2's browser process opens the lockfile without write sharing.
#[cfg(windows)]
#[test]
fn a_lockfile_held_without_write_sharing_is_busy() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let dir = tempfile::tempdir().unwrap();
    let lockfile = dir.path().join("lockfile");
    let held = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .share_mode(FILE_SHARE_READ)
        .open(&lockfile)
        .unwrap();
    assert!(is_profile_busy(&lockfile));
    drop(held);
    assert!(!is_profile_busy(&lockfile));
}
