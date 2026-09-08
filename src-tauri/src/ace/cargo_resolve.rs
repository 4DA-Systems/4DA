// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Which crates of a Cargo project are actually COMPILED on this host.
//!
//! `Cargo.lock` is a *superset*. It is target-agnostic (a Windows lockfile
//! still lists the whole Linux GTK3 stack Tauri pulls in) AND
//! feature-agnostic (it lists every optional dependency, including the ones
//! no enabled feature turns on). A finding against a crate in that superset
//! that never reaches `rustc` on this machine is unreachable noise.
//!
//! Live 2026-09-07, founder instance: Preemption's #1 item was a HIGH,
//! "version-confirmed" advisory for `quinn-proto` in `D:\4DA` — an optional
//! dependency of `reqwest`'s `http3` feature, which this tree does not
//! enable. It has never been compiled here. Measured on this repo the
//! lockfile holds 788 distinct crate names and only 538 are built for the
//! host: **250 of them are unreachable**, `quinn`/`quinn-proto`/`openssl`,
//! the entire GTK3 and `objc2` clusters, and every wrong-arch `windows_*`
//! shim among them.
//!
//! # Why `cargo tree`, not `cargo metadata --filter-platform`
//!
//! Measured on this repo, 2026-09-08, host `x86_64-pc-windows-msvc`:
//!
//! | mechanism                                    | distinct crates | `quinn-proto` |
//! |----------------------------------------------|-----------------|---------------|
//! | `Cargo.lock`                                 | 788             | present       |
//! | `cargo metadata --filter-platform <triple>`  | 560             | **present**   |
//! | `cargo tree` (host, default features)        | 538             | absent        |
//!
//! `cargo metadata`'s resolve graph is platform-filtered but NOT
//! feature-resolved: it keeps the `reqwest -> quinn` edge even though
//! `reqwest`'s enabled feature set contains no `http3`. Only `cargo tree`
//! applies both, and it is what the build itself does. It answers in ~0.8s.
//!
//! # Conservative by construction
//!
//! `None` means *unknown*, and every caller then leaves every crate ACTIVE.
//! An empty set would mark the whole lockfile unreachable and silently bury
//! real advisories, so an empty parse is reported as `None`, never as "found
//! nothing". Cargo missing, a stale lockfile, a cold `--offline` registry, a
//! held package-cache lock: all of them mean unknown.
//!
//! Mirrors `mcp-4da-server/src/live/cargo-platform.ts`, which resolves the
//! same predicate for the MCP server's `vulnerability_scan`. The shared
//! definition of "platform-inactive" is documented in
//! [`crate::platform_filter`].

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::db::Database;

/// Written to `user_dependencies.target_cfg` for a crate the lockfile
/// resolves but this host never compiles. Distinct from a `cfg(...)` spec —
/// which names a build target the user *does* have — so the surfaces can say
/// which of the two happened.
pub(crate) const LOCKFILE_ONLY_MARKER: &str = "lockfile-only";

/// Cargo holds the package-cache lock, so a concurrent `cargo build` in the
/// same tree blocks this. Generous, and never fatal: a timeout is "unknown".
const CARGO_TREE_TIMEOUT: Duration = Duration::from_secs(45);

/// A pathological tree cannot exhaust memory. 4DA's own is ~60 KB.
const MAX_TREE_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

/// Bumped when the cache VALUE shape or the resolution mechanism changes, so
/// an upgrade re-resolves instead of trusting a verdict a different
/// definition produced.
const CACHE_VERSION: u32 = 1;

/// One cached answer, keyed in `kv_store` by project path.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedResolve {
    version: u32,
    /// SHA-256 of the `Cargo.lock` the answer was computed from.
    lock_hash: String,
    /// Lowercased crate names the host build resolves.
    resolved: Vec<String>,
}

/// Lowercased names of the crates `dir`'s Cargo project compiles on THIS
/// host, or `None` when cargo could not answer.
///
/// Cached in `kv_store` against the `Cargo.lock` content hash: an unchanged
/// lockfile never re-runs cargo, and any edit invalidates the answer.
pub(crate) fn host_resolved_crates(db: &Database, dir: &Path) -> Option<HashSet<String>> {
    if !dir.join("Cargo.toml").exists() {
        return None;
    }
    let lock_content = std::fs::read(dir.join("Cargo.lock")).ok()?;
    let lock_hash = hex_digest(&lock_content);
    let cache_key = cache_key_for(dir);

    if let Some(hit) = read_cache(db, &cache_key, &lock_hash) {
        return Some(hit);
    }

    let resolved = parse_tree_names(&run_cargo_tree(dir)?);
    // An empty answer for a project that HAS a lockfile is not credible —
    // treat it as unknown rather than declaring the whole tree unreachable.
    if resolved.is_empty() {
        return None;
    }
    write_cache(db, &cache_key, &lock_hash, &resolved);
    Some(resolved)
}

/// The crates `packages` lists that the host build never compiles.
/// Empty when the answer is unknown — nothing is ever marked unreachable on
/// a guess.
pub(crate) fn lockfile_only_crates(
    db: &Database,
    dir: &Path,
    packages: &[(String, String)],
) -> Vec<String> {
    let Some(resolved) = host_resolved_crates(db, dir) else {
        // Logged at debug, not warn: a user with no Rust toolchain is a
        // normal state, and this runs once per project per scan.
        tracing::debug!(
            target: "4da::ace",
            dir = %dir.display(),
            "cargo could not resolve the host build — every crate stays active"
        );
        return Vec::new();
    };
    let mut seen = HashSet::new();
    packages
        .iter()
        .map(|(name, _)| name.to_lowercase())
        .filter(|name| !resolved.contains(name) && seen.insert(name.clone()))
        .collect()
}

/// Ask cargo which of this lockfile's crates the host actually compiles, and
/// persist the verdict for the ones it does not.
///
/// The lockfile walk calls this once per Cargo project, AFTER the rows exist
/// (the writer UPDATEs them) and after the stale-row prune (so it never marks
/// a row that is about to be deleted). Silent and harmless when cargo cannot
/// answer: [`lockfile_only_crates`] returns empty and the writer then only
/// clears markers a previous scan left behind.
pub(crate) fn record_unreachable_crates(
    db: &Database,
    dir: &Path,
    project_path: &str,
    packages: &[(String, String)],
) {
    let unreachable = lockfile_only_crates(db, dir, packages);
    match db.mark_lockfile_only_crates(project_path, "rust", &unreachable) {
        Ok(0) => {}
        Ok(marked) => tracing::info!(
            target: "4da::ace",
            project = %project_path,
            marked,
            lockfile_crates = packages.len(),
            "Crates in the lockfile that this host never compiles — de-prioritised, not hidden"
        ),
        Err(e) => tracing::warn!(
            target: "4da::ace",
            error = %e,
            project = %project_path,
            "Failed to record host-unreachable crates"
        ),
    }
}

fn cache_key_for(dir: &Path) -> String {
    format!(
        "cargo_resolve:{}",
        hex_digest(dir.to_string_lossy().as_bytes())
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn read_cache(db: &Database, key: &str, lock_hash: &str) -> Option<HashSet<String>> {
    let json = db.get_kv(key).ok().flatten()?;
    let cached: CachedResolve = serde_json::from_str(&json).ok()?;
    if cached.version != CACHE_VERSION || cached.lock_hash != lock_hash {
        return None;
    }
    // A cached empty set would be the "declare everything unreachable" bug
    // arriving through the cache instead of the parser. Refuse it there too.
    if cached.resolved.is_empty() {
        return None;
    }
    Some(cached.resolved.into_iter().collect())
}

fn write_cache(db: &Database, key: &str, lock_hash: &str, resolved: &HashSet<String>) {
    let mut names: Vec<String> = resolved.iter().cloned().collect();
    names.sort();
    let payload = CachedResolve {
        version: CACHE_VERSION,
        lock_hash: lock_hash.to_string(),
        resolved: names,
    };
    match serde_json::to_string(&payload) {
        Ok(json) => {
            if let Err(e) = db.set_kv(key, &json) {
                tracing::debug!(target: "4da::ace", error = %e, "failed to cache host crate resolution");
            }
        }
        Err(e) => {
            tracing::debug!(target: "4da::ace", error = %e, "failed to encode host crate resolution")
        }
    }
}

/// Ask cargo to resolve the host build. `--locked` and `--offline` make this
/// strictly read-only: it can neither rewrite the user's `Cargo.lock` nor
/// touch the network from a background scan.
fn run_cargo_tree(dir: &Path) -> Option<String> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args([
        "tree",
        "--offline",
        "--locked",
        "--prefix",
        "none",
        // Dev-dependencies are compiled by `cargo test`, so an advisory
        // against one is reachable; proc-macro deps ride along with normal.
        "--edges",
        "normal,build,dev",
    ])
    .current_dir(dir)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().ok()?;
    let output = read_bounded(&mut child);
    match output {
        Some(text) if child_succeeded(&mut child) => Some(text),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Drain stdout on a worker thread while polling for exit, so a tree larger
/// than the pipe buffer cannot deadlock the poll loop.
fn read_bounded(child: &mut std::process::Child) -> Option<String> {
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || -> Option<Vec<u8>> {
        let mut handle = stdout?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match handle.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > MAX_TREE_OUTPUT_BYTES {
                        return None;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
        }
        Some(buf)
    });

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > CARGO_TREE_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    tracing::debug!(target: "4da::ace", "cargo tree timed out — host resolution unknown");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                let _ = reader.join();
                return None;
            }
        }
    }
    let bytes = reader.join().ok().flatten()?;
    String::from_utf8(bytes).ok()
}

/// The child has already exited by the time this runs; re-reaping is the
/// cheapest way to read the status the poll loop discarded.
fn child_succeeded(child: &mut std::process::Child) -> bool {
    matches!(child.wait(), Ok(status) if status.success())
}

/// `cargo tree --prefix none` emits one package per line as
/// `name vX.Y.Z [(source)] [(proc-macro)] [(*)]`. Anything that does not
/// start with a crate-name token is not a package line.
fn parse_tree_names(stdout: &str) -> HashSet<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let token = line.split_whitespace().next()?;
            let is_crate_name = !token.is_empty()
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'));
            is_crate_name.then(|| token.to_lowercase())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_lines_and_ignores_decorations() {
        let out = "fourda v1.0.2 (D:\\4DA\\src-tauri)\n\
                   ammonia v4.1.4\n\
                   cssparser-macros v0.7.0 (proc-macro)\n\
                   quote v1.0.45 (*)\n";
        let names = parse_tree_names(out);
        assert_eq!(names.len(), 4);
        assert!(names.contains("fourda"));
        assert!(
            names.contains("cssparser-macros"),
            "proc-macro suffix ignored"
        );
        assert!(names.contains("quote"), "dedupe marker ignored");
    }

    #[test]
    fn skips_blank_and_non_package_lines() {
        // `--prefix none` emits no section headers, but a cargo that did
        // would not be allowed to inject `[dev-dependencies]` as a crate.
        let names = parse_tree_names("\n[dev-dependencies]\n   \nserde v1.0.0\n");
        assert_eq!(names, HashSet::from(["serde".to_string()]));
    }

    #[test]
    fn names_are_lowercased_for_case_insensitive_matching() {
        // The DB compares on LOWER(package_name); the set must agree.
        let names = parse_tree_names("Inflector v0.11.4\n");
        assert!(names.contains("inflector"));
    }

    // ---- negative tests for the gate (a wrong answer must never suppress) --

    #[test]
    fn empty_output_yields_no_names_so_the_caller_reports_unknown() {
        // `host_resolved_crates` turns this into `None`, and
        // `lockfile_only_crates` then marks NOTHING inactive. An empty set
        // treated as an answer would bury every advisory in the tree.
        assert!(parse_tree_names("").is_empty());
        assert!(parse_tree_names("warning: nothing to print.\n").contains("warning:") == false);
    }

    #[test]
    fn lockfile_only_is_empty_when_resolution_is_unknown() {
        // No Cargo.toml in a temp dir -> host_resolved_crates returns None ->
        // every crate stays active.
        let dir = std::env::temp_dir().join("4da-cargo-resolve-absent");
        std::fs::create_dir_all(&dir).ok();
        let db = crate::test_utils::test_db();
        let packages = vec![("gtk".to_string(), "0.18.2".to_string())];
        assert!(
            lockfile_only_crates(&db, &dir, &packages).is_empty(),
            "unknown resolution must never mark a crate unreachable"
        );
    }
}
