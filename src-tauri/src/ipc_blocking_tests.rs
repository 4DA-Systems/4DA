// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Source-scan gate: no registered Tauri command may do blocking work on the
//! UI thread (audit 2026-10-07, wave 2c).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;

/// Registered commands that may stay SYNC, with the reason each never
/// blocks. Keyed by `(file relative to src/, fn name)`. Anything else that
/// is registered must be `async`. Adding an entry is a reviewed claim that
/// the body only reads in-memory state.
const SYNC_ALLOWLIST: &[(&str, &str, &str)] = &[
    (
        "capabilities.rs",
        "get_capability_states",
        "clones the in-memory capability RwLock registry",
    ),
    (
        "capabilities.rs",
        "get_capability_summary",
        "counts the in-memory capability RwLock registry",
    ),
    (
        "ai_costs.rs",
        "get_ai_cost_estimate",
        "pure arithmetic over its arguments",
    ),
    (
        "stack_commands.rs",
        "get_stack_profiles",
        "maps the compiled-in static stack profile table",
    ),
    (
        "content_translation_commands.rs",
        "get_translation_config",
        "clones in-memory settings; no keychain hydration, no file IO",
    ),
    (
        "reembed.rs",
        "get_embedding_model_info",
        "in-memory settings read plus an atomic flag",
    ),
    (
        "quit_command.rs",
        "quit_app",
        "only calls app.exit(0); nothing to wait on",
    ),
    (
        "achievement_commands_stub.rs",
        "get_achievement_state",
        "constant JSON stub for builds without the achievements feature",
    ),
    (
        "achievement_commands_stub.rs",
        "get_achievements",
        "constant JSON stub for builds without the achievements feature",
    ),
    (
        "achievement_commands_stub.rs",
        "check_daily_streak",
        "constant JSON stub for builds without the achievements feature",
    ),
];

/// APIs that block on SQLite, the filesystem, the OS keychain, a child
/// process, the network, or a corpus-scale computation.
const BLOCKING_MARKERS: &[&str] = &[
    "open_db_connection",
    "get_database",
    ".conn.lock(",
    "get_conn()",
    "read_conn(",
    "std::fs::",
    "fs::read",
    "fs::write",
    "read_dir",
    "Command::new",
    "reqwest",
    "keystore::",
    "ensure_keys_hydrated",
    "get_ace_engine",
    "get_ace_context",
    "generate_blind_spot_report",
    "load_english_strings",
    "load_overrides",
    "initialize_startup_health_cache",
    "engine_scheduler::",
];

struct CommandFn {
    file: String,
    name: String,
    is_async: bool,
    body: String,
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn registered_commands() -> HashSet<String> {
    let lib = std::fs::read_to_string(src_dir().join("lib.rs")).expect("read lib.rs");
    let start = lib
        .find("generate_handler![")
        .expect("generate_handler! block");
    let block = &lib[start..];
    let end = block.find(']').expect("generate_handler! close");
    block[..end]
        .lines()
        .filter_map(|line| {
            let code = line.split("//").next().unwrap_or("").trim();
            code.contains("::").then(|| {
                let path = code.trim_end_matches(',').trim();
                path.rsplit("::").next().unwrap_or(path).trim().to_string()
            })
        })
        .collect()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read src dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Index just past the `}` closing the block that opens at `open` (a `{`).
/// Skips string, raw-string and char literals so a brace inside
/// `format!("{x}")` or `'{'` never unbalances the count.
fn block_end(src: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < src.len() {
        match src[i] {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            b'"' => i = skip_string(src, i),
            b'r' if src.get(i + 1).is_some_and(|c| *c == b'#' || *c == b'"') => {
                i = skip_raw_string(src, i);
            }
            b'\'' if src.get(i + 2) == Some(&b'\'') => i += 2,
            b'\'' if src.get(i + 1) == Some(&b'\\') && src.get(i + 3) == Some(&b'\'') => i += 3,
            _ => {}
        }
        i += 1;
    }
    src.len()
}

fn skip_string(src: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < src.len() {
        match src[i] {
            b'\\' => i += 1,
            b'"' => return i,
            _ => {}
        }
        i += 1;
    }
    i
}

fn skip_raw_string(src: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    let mut hashes = 0;
    while src.get(i) == Some(&b'#') {
        hashes += 1;
        i += 1;
    }
    if src.get(i) != Some(&b'"') {
        return start; // an identifier starting with `r`, not a raw string
    }
    i += 1;
    while i < src.len() {
        if src[i] == b'"' && (1..=hashes).all(|k| src.get(i + k) == Some(&b'#')) {
            return i + hashes;
        }
        i += 1;
    }
    i
}

/// The command attribute, split so the repo's own command scanners
/// (`scripts/validate-commands.cjs`) never read this file as a definition.
const COMMAND_ATTR: &str = concat!("#[tauri", "::command");

/// Every command fn in one file: name, asyncness, body text.
fn commands_in(rel: &str, src: &str) -> Vec<CommandFn> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find(COMMAND_ATTR) {
        let attr = from + pos;
        from = attr + 1;
        let Some(fn_pos) = src[attr..].find("fn ").map(|p| attr + p) else {
            break;
        };
        let header = &src[attr..fn_pos];
        let is_async = header.split_whitespace().any(|w| w == "async");
        let name: String = src[fn_pos + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let Some(open) = src[fn_pos..].find('{').map(|p| fn_pos + p) else {
            break;
        };
        let end = block_end(bytes, open);
        out.push(CommandFn {
            file: rel.to_string(),
            name,
            is_async,
            body: src[open..end].to_string(),
        });
    }
    out
}

fn all_commands() -> Vec<CommandFn> {
    let root = src_dir();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut out = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path).expect("read source file");
        out.extend(commands_in(&rel, &src));
    }
    out
}

fn allow_reason(file: &str, name: &str) -> Option<&'static str> {
    SYNC_ALLOWLIST
        .iter()
        .find(|(f, n, _)| *f == file && *n == name)
        .map(|(_, _, why)| *why)
}

#[test]
fn no_registered_sync_command_blocks_the_ui_thread() {
    let registered = registered_commands();
    let commands = all_commands();
    assert!(
        commands.len() >= 300,
        "scanner found only {} commands — the parser is broken",
        commands.len()
    );
    let mut violations = Vec::new();
    for c in commands
        .iter()
        .filter(|c| !c.is_async && registered.contains(&c.name))
    {
        let blocking: Vec<&str> = BLOCKING_MARKERS
            .iter()
            .copied()
            .filter(|m| c.body.contains(m))
            .collect();
        match allow_reason(&c.file, &c.name) {
            None => violations.push(format!(
                "{}::{} is a sync command — make it `async` and run the body via \
                 crate::ipc_blocking::off_ui_thread, or allowlist it with a reason",
                c.file, c.name
            )),
            Some(_) if !blocking.is_empty() => violations.push(format!(
                "{}::{} is allowlisted as sync but calls {blocking:?}",
                c.file, c.name
            )),
            Some(_) => {}
        }
    }
    assert!(
        violations.is_empty(),
        "UI-thread blocking commands:\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_allowlist_entry_is_a_live_sync_command() {
    let registered = registered_commands();
    let commands = all_commands();
    for (file, name, _) in SYNC_ALLOWLIST {
        let live = commands
            .iter()
            .any(|c| c.file == *file && c.name == *name && !c.is_async);
        assert!(
            live && registered.contains(*name),
            "stale allowlist entry {file}::{name} — remove it"
        );
    }
}

#[test]
fn the_scanner_reads_asyncness_and_bodies() {
    let src = "@]\npub fn a() -> String { format!(\"{x}\") }\n\
               /// doc\n@]\n#[inline]\npub async fn b() { let c = '{'; get_database(); }\n"
        .replace('@', COMMAND_ATTR);
    let cmds = commands_in("x.rs", &src);
    assert_eq!(cmds.len(), 2);
    assert!(!cmds[0].is_async && cmds[0].name == "a");
    assert!(cmds[0].body.ends_with('}') && !cmds[0].body.contains("pub async"));
    assert!(cmds[1].is_async && cmds[1].name == "b");
    assert!(cmds[1].body.contains("get_database"));
}

#[tokio::test]
async fn off_ui_thread_returns_the_body_result() {
    let v: Result<u32, String> = off_ui_thread("t", || Ok(7)).await;
    assert_eq!(v, Ok(7));
    let e: Result<u32, String> = off_ui_thread("t", || Err("boom".to_string())).await;
    assert_eq!(e, Err("boom".to_string()));
}

#[tokio::test]
#[allow(clippy::panic)]
async fn a_panicking_body_becomes_an_error_not_a_hang() {
    let r: Result<u32, String> =
        off_ui_thread("panicky", || -> Result<u32, String> { panic!("x") }).await;
    let msg = r.expect_err("panic surfaces as Err");
    assert!(msg.starts_with("panicky:"), "{msg}");
    let ok = off_ui_thread_infallible("t", || 3u8).await;
    assert_eq!(ok, Ok(3));
}

/// How long a task spawned from a worker waits while that worker's task then
/// computes synchronously for `block` — with or without `cpu_bound`.
fn spawned_task_delay(block: std::time::Duration, use_cpu_bound: bool) -> std::time::Duration {
    use std::time::Instant;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async move {
        tokio::spawn(async move {
            let spawned_at = Instant::now();
            // Spawned from a worker: lands in this worker's LIFO slot, which
            // no other worker may steal.
            let waiter = tokio::spawn(async move { spawned_at.elapsed() });
            let work = || std::thread::sleep(block);
            if use_cpu_bound {
                cpu_bound(work);
            } else {
                work();
            }
            waiter.await.expect("waiter")
        })
        .await
        .expect("outer")
    })
}

/// The analysis' scoring pass is tens of seconds of synchronous work inside
/// one poll. A task scheduled on its worker just before that stretch waits for
/// all of it; under `cpu_bound` it runs at once (fresh-profile E2E 2026-10-09:
/// "Enter 4DA" frozen ~30 s while a scoring pass ran).
#[test]
fn cpu_bound_does_not_strand_tasks_queued_on_its_worker() {
    let block = std::time::Duration::from_millis(1500);
    let stranded = spawned_task_delay(block, false);
    assert!(
        stranded >= std::time::Duration::from_millis(1000),
        "control: the LIFO-slot task waited only {stranded:?}"
    );
    let freed = spawned_task_delay(block, true);
    assert!(
        freed < std::time::Duration::from_millis(500),
        "under cpu_bound the task still waited {freed:?}"
    );
}

#[test]
fn cpu_bound_runs_inline_off_a_multi_thread_runtime() {
    // Plain thread: no runtime at all.
    assert_eq!(cpu_bound(|| 1 + 1), 2);
    // Current-thread runtime: block_in_place would panic here.
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    assert_eq!(rt.block_on(async { cpu_bound(|| 21 * 2) }), 42);
}
