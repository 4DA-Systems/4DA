// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Consent-first project discovery.
//!
//! Nothing here reads a file's contents until the user presses Scan:
//! - [`ace_preview_discovery_dirs`] lists the home folders discovery would
//!   use (existence checks only).
//! - [`ace_candidate_dev_roots`] is opt-in: it looks for common project roots
//!   on the machine's fixed drives (existence checks and one directory listing
//!   per drive root) and returns them for the user to tick. It never adds them.
//! - [`ace_auto_discover`] scans, given the folders the user confirmed.
//!
//! Discovery is single-flight: a second caller (React StrictMode mounts twice;
//! Settings and onboarding can overlap) waits for the running scan and gets
//! its result instead of a "nothing found" answer.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::error::Result;
use crate::get_settings_manager;

use super::scanning::{ace_full_scan, is_system_directory, strip_extended_prefix};

/// A finished discovery is reused by callers that asked for the same folders
/// within this window, so a double mount or a double click scans once.
const RESULT_TTL: Duration = Duration::from_mins(1);

/// Common project-root folder names checked at depth 1 under each drive root.
const ROOT_NAMES: &[&str] = &[
    "dev",
    "code",
    "projects",
    "repos",
    "src",
    "source/repos",
    "GitHub",
    "work",
];

/// Drive-root entries never offered as project roots.
const SKIP_ROOT_ENTRIES: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "users",
    "recovery",
    "perflogs",
    "system volume information",
];

/// Cap on drive-root entries examined, so a huge root cannot stall the step.
const MAX_ROOT_ENTRIES: usize = 500;

struct Finished {
    at: Instant,
    key: Option<Vec<String>>,
    value: serde_json::Value,
}

static DISCOVERY: Lazy<Mutex<Option<Finished>>> = Lazy::new(|| Mutex::new(None));

/// Run `run` at most once at a time. A caller that arrives while a run is in
/// flight waits for it; a caller asking for the same folders within
/// [`RESULT_TTL`] of a finished run gets that run's result. Errors are not
/// cached, so a failed scan can be retried at once.
async fn single_flight<F, Fut>(
    slot: &Mutex<Option<Finished>>,
    key: Option<Vec<String>>,
    run: F,
) -> Result<serde_json::Value>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<serde_json::Value>>,
{
    let mut guard = slot.lock().await;
    if let Some(done) = guard.as_ref() {
        if done.key == key && done.at.elapsed() < RESULT_TTL {
            debug!(target: "4da::ace", "Discovery result reused for a concurrent caller");
            return Ok(done.value.clone());
        }
    }
    let value = run().await?;
    *guard = Some(Finished {
        at: Instant::now(),
        key,
        value: value.clone(),
    });
    Ok(value)
}

/// The home folders discovery would scan. Existence checks only.
#[tauri::command]
pub async fn ace_preview_discovery_dirs() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(crate::settings::discover_dev_directories)
        .await
        .map_err(|e| format!("Folder preview failed: {e}").into())
}

/// Opt-in: likely project roots on this machine's fixed drives, for the user
/// to tick. Never adds anything to the scan list.
#[tauri::command]
pub async fn ace_candidate_dev_roots() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(|| candidates_under(&search_roots()))
        .await
        .map_err(|e| format!("Drive search failed: {e}").into())
}

/// Scan for projects. `dirs` is the list the user confirmed; `None` keeps the
/// legacy behaviour (the home folders [`ace_preview_discovery_dirs`] lists),
/// used by explicit "Auto-discover" buttons elsewhere in the app.
#[tauri::command]
pub async fn ace_auto_discover(dirs: Option<Vec<String>>) -> Result<serde_json::Value> {
    let key = dirs.as_ref().map(|d| {
        let mut sorted = d.clone();
        sorted.sort();
        sorted
    });
    single_flight(&DISCOVERY, key, || run_discovery(dirs)).await
}

async fn run_discovery(dirs: Option<Vec<String>>) -> Result<serde_json::Value> {
    info!(target: "4da::ace", confirmed = dirs.is_some(), "Starting context discovery");
    let base_dirs = tokio::task::spawn_blocking(move || match dirs {
        Some(list) => sanitize_confirmed_dirs(list),
        None => crate::settings::discover_dev_directories(),
    })
    .await
    .map_err(|e| format!("Discovery failed: {e}"))?;

    if base_dirs.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "status": "no_directories",
            "message": "No folders to scan",
            "directories_found": 0,
            "projects_found": 0
        }));
    }

    let walk_dirs = base_dirs.clone();
    let project_dirs = tokio::task::spawn_blocking(move || {
        crate::settings::find_project_directories(&walk_dirs, 3)
    })
    .await
    .map_err(|e| format!("Discovery failed: {e}"))?;
    let dirs_to_add = choose_dirs_to_add(&base_dirs, &project_dirs);

    {
        let mut settings = get_settings_manager().lock();
        if let Err(e) = settings.add_context_dirs(dirs_to_add.clone()) {
            return Err(format!("Failed to save discovered directories: {e}").into());
        }
        if let Err(e) = settings.mark_auto_discovery_completed() {
            tracing::warn!("Failed to mark state: {e}");
        }
    }

    info!(target: "4da::ace", dirs = dirs_to_add.len(), "Running full scan on directories");
    let scan_result = ace_full_scan(dirs_to_add.clone()).await?;

    Ok(serde_json::json!({
        "success": true,
        "directories_found": base_dirs.len(),
        "projects_found": project_dirs.len(),
        "directories_added": dirs_to_add.len(),
        "directories": dirs_to_add,
        "scan_result": scan_result
    }))
}

/// Individual projects when there is a manageable number, else the parents.
fn choose_dirs_to_add(base_dirs: &[String], project_dirs: &[String]) -> Vec<String> {
    if project_dirs.is_empty() || project_dirs.len() > 50 {
        base_dirs.to_vec()
    } else {
        project_dirs.to_vec()
    }
}

/// Keep confirmed folders that exist and are not system folders, drive roots
/// or network shares; expand `~`; drop duplicates.
fn sanitize_confirmed_dirs(list: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for raw in list {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = expand_home(trimmed);
        if !path.is_dir() {
            continue;
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if is_system_directory(&strip_extended_prefix(&canonical)) {
            continue;
        }
        let shown = path.display().to_string();
        if !kept.iter().any(|k| same_path(k, &shown)) {
            kept.push(shown);
        }
    }
    kept
}

fn expand_home(raw: &str) -> PathBuf {
    match (raw.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if raw == "~" => dirs::home_dir().unwrap_or_else(|| PathBuf::from(raw)),
        _ => PathBuf::from(raw),
    }
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Likely project roots directly under each of `roots`: the common folder
/// names in [`ROOT_NAMES`], plus top-level folders that are git repositories.
pub(crate) fn candidates_under(roots: &[PathBuf]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |p: &Path| {
        let shown = p.display().to_string();
        if !found.iter().any(|f| same_path(f, &shown)) {
            found.push(shown);
        }
    };
    for root in roots {
        for name in ROOT_NAMES {
            let candidate = name
                .split('/')
                .fold(root.clone(), |acc, part| acc.join(part));
            if candidate.is_dir() {
                push(&candidate);
            }
        }
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten().take(MAX_ROOT_ENTRIES) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let skipped = name.starts_with('.')
                || name.starts_with('$')
                || SKIP_ROOT_ENTRIES.contains(&name.as_str());
            if !skipped && path.is_dir() && path.join(".git").exists() {
                push(&path);
            }
        }
    }
    found
}

/// Where to look: fixed drives on Windows, the home folder elsewhere.
fn search_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        fixed_drive_roots()
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().into_iter().collect()
    }
}

/// Root paths (`C:\`, `D:\`, ...) of local fixed drives. Removable, network,
/// optical and RAM drives are left out: probing a dead network share can
/// block for the full SMB timeout.
#[cfg(windows)]
#[allow(unsafe_code)]
fn fixed_drive_roots() -> Vec<PathBuf> {
    extern "system" {
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    }
    const DRIVE_FIXED: u32 = 3;

    // SAFETY: takes no arguments and returns a bitmask of drive letters.
    let mask = unsafe { GetLogicalDrives() };
    (0u8..26)
        .filter(|i| mask & (1u32 << i) != 0)
        .filter_map(|i| {
            let root = format!("{}:\\", char::from(b'A' + i));
            let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
            let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
            (kind == DRIVE_FIXED).then(|| PathBuf::from(root))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn concurrent_callers_share_one_scan() {
        let slot = Mutex::new(None);
        let counter = AtomicUsize::new(0);
        let runs = &counter;
        let scan = move || async move {
            runs.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(serde_json::json!({ "success": true, "directories": ["a"] }))
        };
        let (first, second) = tokio::join!(
            single_flight(&slot, None, scan),
            single_flight(&slot, None, scan)
        );
        assert_eq!(runs.load(Ordering::SeqCst), 1, "one scan for two callers");
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first, second);
        assert_eq!(first["success"], true);
    }

    #[tokio::test]
    async fn a_different_folder_list_runs_a_new_scan() {
        let slot = Mutex::new(None);
        let counter = AtomicUsize::new(0);
        let runs = &counter;
        let scan = move || async move {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({ "success": true }))
        };
        single_flight(&slot, Some(vec!["a".into()]), scan)
            .await
            .unwrap();
        single_flight(&slot, Some(vec!["b".into()]), scan)
            .await
            .unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failed_scan_is_not_cached() {
        let slot = Mutex::new(None);
        let failed = single_flight(&slot, None, || async { Err("boom".into()) }).await;
        assert!(failed.is_err());
        let ok = single_flight(&slot, None, || async { Ok(serde_json::json!({ "ok": 1 })) })
            .await
            .unwrap();
        assert_eq!(ok["ok"], 1);
    }

    #[test]
    fn candidate_roots_finds_named_roots_and_top_level_repos_only() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        std::fs::create_dir_all(r.join("code")).unwrap();
        std::fs::create_dir_all(r.join("source").join("repos")).unwrap();
        std::fs::create_dir_all(r.join("myapp").join(".git")).unwrap();
        std::fs::create_dir_all(r.join("photos")).unwrap();
        std::fs::create_dir_all(r.join(".hidden").join(".git")).unwrap();
        std::fs::create_dir_all(r.join("deep").join("nested").join(".git")).unwrap();

        let found = candidates_under(&[r.to_path_buf()]);
        let has = |p: PathBuf| found.contains(&p.display().to_string());
        assert!(has(r.join("code")));
        assert!(has(r.join("source").join("repos")));
        assert!(has(r.join("myapp")));
        assert!(!has(r.join("photos")), "plain folders are not offered");
        assert!(!has(r.join(".hidden")), "hidden folders are skipped");
        assert!(!has(r.join("deep")), "only depth 1 is inspected");
        assert_eq!(found.len(), 3, "{found:?}");
    }

    #[test]
    fn confirmed_dirs_drop_missing_duplicates_and_drive_roots() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("proj");
        std::fs::create_dir_all(&real).unwrap();
        let real_s = real.display().to_string();
        let kept = sanitize_confirmed_dirs(vec![
            real_s.clone(),
            format!("  {real_s}  "),
            root.path().join("missing").display().to_string(),
            String::new(),
            if cfg!(windows) {
                "C:\\".into()
            } else {
                "/".into()
            },
        ]);
        assert_eq!(kept, vec![real_s]);
    }

    #[test]
    fn many_projects_fall_back_to_their_parents() {
        let base = vec!["base".to_string()];
        let few: Vec<String> = (0..3).map(|i| format!("p{i}")).collect();
        let many: Vec<String> = (0..51).map(|i| format!("p{i}")).collect();
        assert_eq!(choose_dirs_to_add(&base, &few), few);
        assert_eq!(choose_dirs_to_add(&base, &many), base);
        assert_eq!(choose_dirs_to_add(&base, &[]), base);
    }
}
