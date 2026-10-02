// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A package the user's own workspace builds is not a pin of that package.
//!
//! The release grade (`release_grade`) compares an announced registry
//! version with every project's pinned copy. A project that IS the package
//! — the crate's own manifest, a member of the workspace that publishes it,
//! or a project that takes it by `path =` — carries the package's version in
//! `user_dependencies` too, because the lockfile walk records every
//! `[[package]]` and a member's `x = { workspace = true }` inherits the
//! workspace's `{ version, path }` entry. Those rows are not installs of the
//! published release: they are its source.
//!
//! Live 2026-10-02: `crates.io: victauri-core v0.9.0` (and -macros, -plugin,
//! -test) read "Breaking upgrade of your dependency" at 0.90, graded against
//! the victauri workspace's own members on 0.8.8 — the operator publishes
//! victauri. The one real pin, `<sibling>/apps/bridge/src-tauri` on 0.8.4, is
//! still graded and named; the workspace that builds the crate is not.
//!
//! The evidence is the lockfile, not a name heuristic: Cargo writes no
//! `source =` line for a workspace member or a path dependency, and a
//! registry or git package always has one. A Rust pin is the project's own
//! when its manifest's `[package] name` is the package, or when the nearest
//! `Cargo.lock` (the project's, else its workspace's, at most
//! [`MAX_ANCESTORS`] directories up and never past a repository root)
//! records the package without a source. An npm pin is the project's own when
//! its `package.json` `name` is the package (a workspace link is already
//! dropped: `workspace:*` / `link:` never parse as a version). A path that is
//! not a directory on this machine is never the user's own — synthetic test
//! paths and pins from a vanished checkout keep their old meaning.
//!
//! Reads are cached by file path and modification time, so a drain grading
//! hundreds of releases stats each lockfile once per pin and parses it once.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use parking_lot::Mutex;

/// How far above a project to look for its workspace's `Cargo.lock`.
const MAX_ANCESTORS: usize = 6;

/// Names a file declares, cached against its modification time.
type CachedNames = (Option<SystemTime>, HashSet<String>);

fn cache() -> &'static Mutex<HashMap<PathBuf, CachedNames>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedNames>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// crates.io treats `-` and `_` as one namespace; npm names compare
/// case-insensitively. One normal form serves both.
fn norm(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

/// The names `parse` reads from `path`, or `None` when the file is absent or
/// unreadable. Re-parsed only when the file's modification time moves.
fn cached_names(path: &Path, parse: fn(&str) -> HashSet<String>) -> Option<HashSet<String>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mtime = meta.modified().ok();
    if let Some((cached_at, names)) = cache().lock().get(path) {
        if *cached_at == mtime && mtime.is_some() {
            return Some(names.clone());
        }
    }
    let content = std::fs::read_to_string(path).ok()?;
    let names = parse(&content);
    cache()
        .lock()
        .insert(path.to_path_buf(), (mtime, names.clone()));
    Some(names)
}

/// The `[[package]]` entries of a `Cargo.lock` that carry no `source` — the
/// workspace's own members and its path dependencies.
pub(crate) fn cargo_lock_local_packages(content: &str) -> HashSet<String> {
    let mut locals = HashSet::new();
    // (name, has_source) of the `[[package]]` block being read.
    let mut block: Option<(Option<String>, bool)> = None;
    let mut flush = |block: &mut Option<(Option<String>, bool)>| {
        if let Some((Some(name), false)) = block.take() {
            locals.insert(norm(&name));
        }
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            flush(&mut block);
            block = Some((None, false));
        } else if line.starts_with('[') {
            // Any other table ([metadata], [[patch.unused]]) ends the block.
            flush(&mut block);
        } else if let Some((name, has_source)) = block.as_mut() {
            if let Some(rest) = trimmed.strip_prefix("name = ") {
                *name = Some(rest.trim_matches('"').to_string());
            } else if trimmed.starts_with("source = ") {
                *has_source = true;
            }
        }
    }
    flush(&mut block);
    locals
}

/// The `[package] name` of a `Cargo.toml` (empty for a virtual workspace).
pub(crate) fn cargo_manifest_package_name(content: &str) -> HashSet<String> {
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            if key.trim() == "name" {
                let value = value.trim().trim_matches('"');
                if !value.is_empty() && !value.contains('{') {
                    return HashSet::from([norm(value)]);
                }
            }
        }
    }
    HashSet::new()
}

/// The `name` of a `package.json`.
pub(crate) fn package_json_name(content: &str) -> HashSet<String> {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(norm))
        .into_iter()
        .collect()
}

/// Does the project at `project_path` build `package` itself, rather than
/// install a published release of it? `lang` is the registry's manifest
/// language (`rust`, `javascript`); other languages are never the user's own.
pub(crate) fn is_own_package(project_path: &str, package: &str, lang: &str) -> bool {
    let dir = Path::new(project_path);
    if !dir.is_dir() {
        return false;
    }
    let pkg = norm(package);
    match lang {
        "rust" => {
            if cached_names(&dir.join("Cargo.toml"), cargo_manifest_package_name)
                .is_some_and(|names| names.contains(&pkg))
            {
                return true;
            }
            for ancestor in dir.ancestors().take(MAX_ANCESTORS) {
                if let Some(locals) =
                    cached_names(&ancestor.join("Cargo.lock"), cargo_lock_local_packages)
                {
                    return locals.contains(&pkg);
                }
                if ancestor.join(".git").exists() {
                    break;
                }
            }
            false
        }
        "javascript" => cached_names(&dir.join("package.json"), package_json_name)
            .is_some_and(|names| names.contains(&pkg)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of the live victauri lockfile: members carry no `source`.
    const VICTAURI_LOCK: &str = r#"
version = 4

[[package]]
name = "tokio"
version = "1.52.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "bytes",
]

[[package]]
name = "victauri-core"
version = "0.8.8"
dependencies = [
 "axum",
]

[[package]]
name = "victauri_macros"
version = "0.8.8"

[[package]]
name = "vendored"
version = "0.1.0"
source = "git+https://github.com/example/vendored#abc"
"#;

    #[test]
    fn lockfile_members_and_path_deps_are_local_registry_and_git_are_not() {
        let locals = cargo_lock_local_packages(VICTAURI_LOCK);
        assert!(locals.contains("victauri-core"));
        assert!(locals.contains("victauri-macros"), "`_` folds to `-`");
        assert!(!locals.contains("tokio"), "a registry package has a source");
        assert!(!locals.contains("vendored"), "a git package has a source");
    }

    #[test]
    fn manifest_package_name_reads_only_the_package_table() {
        let toml = "[package]\nname = \"victauri-core\"\nversion.workspace = true\n\n[dependencies]\nname = \"not-this\"\n";
        assert!(cargo_manifest_package_name(toml).contains("victauri-core"));
        let virtual_ws = "[workspace]\nmembers = [\"crates/*\"]\n";
        assert!(cargo_manifest_package_name(virtual_ws).is_empty());
        assert!(
            package_json_name(r#"{"name":"@4da/mcp-server","version":"4.0.0"}"#)
                .contains("@4da/mcp-server")
        );
    }

    /// A workspace that publishes `victauri-core`: the root, a member crate
    /// that takes it `{ workspace = true }`, and the crate itself are all the
    /// package's source. A sibling repository with its own lockfile pinning
    /// the registry release is not.
    #[test]
    fn a_workspace_member_and_the_crate_itself_are_own_a_registry_pin_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("victauri");
        let core = ws.join("crates").join("victauri-core");
        let cli = ws.join("crates").join("victauri-cli");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        std::fs::create_dir_all(&core).unwrap();
        std::fs::create_dir_all(&cli).unwrap();
        std::fs::write(ws.join("Cargo.lock"), VICTAURI_LOCK).unwrap();
        std::fs::write(
            core.join("Cargo.toml"),
            "[package]\nname = \"victauri-core\"\n",
        )
        .unwrap();
        std::fs::write(
            cli.join("Cargo.toml"),
            "[package]\nname = \"victauri-cli\"\n[dependencies]\nvictauri-core = { workspace = true }\n",
        )
        .unwrap();

        let sibling = tmp.path().join("sibling-bridge");
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(
            sibling.join("Cargo.lock"),
            "[[package]]\nname = \"victauri-core\"\nversion = \"0.8.4\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        )
        .unwrap();

        let p = |d: &Path| d.to_string_lossy().replace('\\', "/");
        assert!(is_own_package(&p(&ws), "victauri-core", "rust"));
        assert!(is_own_package(&p(&cli), "victauri-core", "rust"));
        assert!(is_own_package(&p(&core), "victauri_core", "rust"));
        assert!(
            !is_own_package(&p(&ws), "tokio", "rust"),
            "the workspace's registry dependencies stay pins"
        );
        assert!(
            !is_own_package(&p(&sibling), "victauri-core", "rust"),
            "a project that installs the published crate is a pin"
        );
        assert!(
            !is_own_package(&p(&ws), "victauri-core", "javascript"),
            "ecosystems do not cross"
        );
    }

    #[test]
    fn a_path_that_is_not_on_this_machine_is_never_own() {
        assert!(!is_own_package(
            "/definitely/not/a/real/project",
            "victauri-core",
            "rust"
        ));
    }

    #[test]
    fn an_npm_project_is_its_own_package_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("package.json"),
            r#"{"name":"@4da/mcp-server","dependencies":{"hono":"^4"}}"#,
        )
        .unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        assert!(is_own_package(&path, "@4da/mcp-server", "javascript"));
        assert!(!is_own_package(&path, "hono", "javascript"));
    }
}
