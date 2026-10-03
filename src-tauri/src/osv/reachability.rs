// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Is the code an advisory names compiled into this build? (AD-051)
//!
//! A version match says the vulnerable FILE ships in the crate; it does not
//! say the project compiles it. Rust crates gate whole modules behind
//! features, and a project that leaves the feature off never builds the bug.
//! Live 2026-10-03 (Screenshot_3845): the brief's Act-now item for atlas's
//! bridge quoted GHSA-33f5, rmcp's OAuth-client metadata bug, but the bridge
//! builds rmcp 1.7.0 with only its server features. Three of the four rmcp
//! advisories it carried sat in `transport/auth.rs` and the reqwest client
//! transport, neither of which exists in that build.
//!
//! The verdict is read, never guessed:
//! - the advisory's own text names the file (`crates/rmcp/src/transport/auth.rs`);
//! - cargo's resolved feature set for that copy comes from the ACE scan
//!   (`ace::cargo_resolve::cached_crate_features`);
//! - the crate's real source (cargo's registry checkout) is parsed with `syn`
//!   from `lib.rs` down the `mod` chain to that file, and every `#[cfg(..)]`
//!   on the way is evaluated against the feature set.
//!
//! Conservative by construction: an advisory stops matching only when EVERY
//! file it names is DEFINITELY gated off. A file it cannot find, a `cfg` it
//! cannot evaluate (`unix`, `test`, `target_os`), a `#[path]` redirect, a
//! `mod` declared inside a macro, a parse failure, a missing feature set or a
//! missing source checkout all answer "maybe compiled", and maybe keeps the
//! advisory. A wrong "not compiled" would bury a real vulnerability; a wrong
//! "maybe" only costs the noise this module exists to remove.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use syn::punctuated::Punctuated;

use crate::db::Database;

use super::types::{MatchedDependency, NotCompiledMatch, StoredAdvisory};

/// At most this many files are read from one advisory's text.
const MAX_NAMED_FILES: usize = 8;

/// `(name, version)` -> resolved features, for one project.
type FeatureMap = HashMap<(String, String), HashSet<String>>;

/// Where a crate release's source lives; swapped in tests.
type SourceDirFn = Box<dyn Fn(&str, &str) -> Option<PathBuf>>;

/// One matcher run's reachability filter. Feature sets are read from the ACE
/// cache once per project per run; verdicts are memoised process-wide.
pub(crate) struct ReachFilter {
    features: HashMap<String, Option<FeatureMap>>,
    source_dir: SourceDirFn,
    /// The copies dropped, for surfaces that explain the omission.
    pub(crate) excluded: Vec<NotCompiledMatch>,
}

impl Default for ReachFilter {
    fn default() -> Self {
        Self {
            features: HashMap::new(),
            source_dir: Box::new(crate_source_dir),
            excluded: Vec::new(),
        }
    }
}

impl ReachFilter {
    /// Drop the crates.io copies whose build gates off every file `advisory`
    /// names. Every other copy, and every other ecosystem, passes untouched.
    pub(crate) fn retain_compiled(
        &mut self,
        db: &Database,
        advisory: &StoredAdvisory,
        instances: &mut Vec<MatchedDependency>,
    ) {
        if super::exposure::canonical(&advisory.ecosystem) != Some("crates.io") {
            return;
        }
        let files = advisory
            .details
            .as_deref()
            .map(|d| named_source_files(d, &advisory.package_name))
            .unwrap_or_default();
        if files.is_empty() {
            return;
        }
        let package = advisory.package_name.to_lowercase();
        let mut kept = Vec::with_capacity(instances.len());
        for inst in instances.drain(..) {
            let features = self
                .features
                .entry(inst.project_path.clone())
                .or_insert_with(|| {
                    crate::ace::cargo_resolve::cached_crate_features(db, &inst.project_path)
                });
            if instance_reach(features.as_ref(), &self.source_dir, &package, &inst, &files)
                == Reach::NotCompiled
            {
                tracing::debug!(
                    target: "4da::osv",
                    advisory = %advisory.advisory_id,
                    package = %package,
                    project = %inst.project_path,
                    "advisory names only code this build does not compile"
                );
                self.excluded.push(NotCompiledMatch {
                    advisory_id: advisory.advisory_id.clone(),
                    summary: advisory.summary.clone(),
                    package_name: advisory.package_name.clone(),
                    ecosystem: advisory.ecosystem.clone(),
                    project_path: inst.project_path.clone(),
                    installed_version: inst.installed_version.clone(),
                    files: files.clone(),
                });
            } else {
                kept.push(inst);
            }
        }
        *instances = kept;
    }
}

/// The verdict for one installed copy: only a version-confirmed copy with a
/// cached feature set for exactly that version can be ruled out.
fn instance_reach(
    features: Option<&FeatureMap>,
    source_dir: &SourceDirFn,
    package: &str,
    inst: &MatchedDependency,
    files: &[String],
) -> Reach {
    if !inst.is_version_confirmed {
        return Reach::Unknown;
    }
    let Some(version) = inst.installed_version.as_deref() else {
        return Reach::Unknown;
    };
    let Some(set) = features.and_then(|f| f.get(&(package.to_string(), version.to_string())))
    else {
        return Reach::Unknown;
    };
    let Some(dir) = source_dir(package, version) else {
        return Reach::Unknown;
    };
    reach_in_checkout(&dir, files, set)
}

/// The verdict for one advisory against one installed copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// At least one named file is compiled.
    Compiled,
    /// Every named file is gated off by the copy's resolved features.
    NotCompiled,
    /// Anything else, including "the advisory names no file".
    Unknown,
}

/// Three-valued `cfg` truth: `Maybe` is a predicate this cannot evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tri {
    Yes,
    No,
    Maybe,
}

impl Tri {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Yes, Self::Yes) => Self::Yes,
            _ => Self::Maybe,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Yes, _) | (_, Self::Yes) => Self::Yes,
            (Self::No, Self::No) => Self::No,
            _ => Self::Maybe,
        }
    }

    fn not(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
            Self::Maybe => Self::Maybe,
        }
    }
}

/// Crate-relative source files (`src/transport/auth.rs`) an advisory's text
/// names for `package`. A path under another crate (`crates/rmcp-macros/src/..`,
/// `tokio/src/..`) is not this package's and is skipped; a bare `src/..` is
/// taken as this package's (its existence in the checkout is checked later).
pub(crate) fn named_source_files(details: &str, package: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = details.as_bytes();
    let mut from = 0;
    while let Some(pos) = details[from..].find("src/") {
        let start = from + pos;
        from = start + 4;
        match path_owner(details, start) {
            Owner::NotAPath => continue,
            Owner::Dir(dir) if !same_crate(dir, package) => continue,
            Owner::Bare | Owner::Dir(_) => {}
        }
        let tail_len = details[start..]
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'/'))
            .count();
        let end = start + tail_len;
        if !details[end..].starts_with(".rs") {
            continue;
        }
        let after = bytes.get(end + 3).copied();
        if after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_') {
            continue;
        }
        let path = format!("{}.rs", &details[start..end]);
        if !path.contains("//") && !out.contains(&path) {
            out.push(path);
        }
        if out.len() >= MAX_NAMED_FILES {
            break;
        }
    }
    out
}

/// Who a `src/` in an advisory's text belongs to.
#[derive(Debug, PartialEq, Eq)]
enum Owner<'a> {
    /// `src/` is mid-word (`mysrc/`): not a path start at all.
    NotAPath,
    /// A bare `src/...`, or a single-crate repository's `blob/<ref>/src/...`.
    Bare,
    /// `<dir>/src/...`: the crate directory `dir`.
    Dir(&'a str),
}

fn path_owner(details: &str, start: usize) -> Owner<'_> {
    let before = &details[..start];
    let Some(prev) = before.chars().last() else {
        return Owner::Bare;
    };
    if prev != '/' {
        let boundary = prev.is_whitespace() || matches!(prev, '`' | '\'' | '"' | '(' | '[' | ':');
        return if boundary {
            Owner::Bare
        } else {
            Owner::NotAPath
        };
    }
    let dir_end = before.len() - 1; // the '/' is one byte
                                    // Advisory text is prose: the character before the segment can be
                                    // multi-byte ("—"), so step past it by its own width, not by one byte.
    let seg_start = before[..dir_end]
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let seg = &before[seg_start..dir_end];
    // `.../blob/<ref>/src/...`: a single-crate repository's own src.
    let ref_parent = before[..seg_start].trim_end_matches('/');
    if ref_parent.ends_with("/blob") || ref_parent.ends_with("/tree") || seg.is_empty() {
        return Owner::Bare;
    }
    Owner::Dir(seg)
}

fn same_crate(a: &str, b: &str) -> bool {
    a.replace('-', "_")
        .eq_ignore_ascii_case(&b.replace('-', "_"))
}

/// The verdict for the crate checked out at `crate_dir`, built with
/// `features`, given the files the advisory names.
pub(crate) fn reach_in_checkout(
    crate_dir: &Path,
    files: &[String],
    features: &HashSet<String>,
) -> Reach {
    // "Every named file is off" is vacuously true of no files.
    if files.is_empty() {
        return Reach::Unknown;
    }
    let mut all_off = true;
    for file in files {
        match file_compiled_memo(crate_dir, file, features) {
            Tri::Yes => return Reach::Compiled,
            Tri::Maybe => all_off = false,
            Tri::No => {}
        }
    }
    if all_off {
        Reach::NotCompiled
    } else {
        Reach::Unknown
    }
}

fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))
}

/// cargo's extracted copy of a crates.io release:
/// `$CARGO_HOME/registry/src/<index>/<name>-<version>`.
fn crate_source_dir(package: &str, version: &str) -> Option<PathBuf> {
    let src = cargo_home()?.join("registry").join("src");
    let names = [
        format!("{package}-{version}"),
        format!("{}-{version}", package.to_lowercase()),
    ];
    std::fs::read_dir(src)
        .ok()?
        .flatten()
        .flat_map(|index| names.iter().map(move |n| index.path().join(n)))
        .find(|dir| dir.join("Cargo.toml").is_file())
}

/// Parse results are deterministic for a (checkout, file, feature set), and
/// the matcher asks the same question on every Preemption refresh.
fn file_compiled_memo(crate_dir: &Path, file: &str, features: &HashSet<String>) -> Tri {
    static MEMO: OnceLock<Mutex<HashMap<String, Tri>>> = OnceLock::new();
    let mut feats: Vec<&str> = features.iter().map(String::as_str).collect();
    feats.sort_unstable();
    let key = format!("{}|{file}|{}", crate_dir.display(), feats.join(","));
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = memo.lock().ok().and_then(|m| m.get(&key).copied()) {
        return hit;
    }
    let verdict = file_compiled(crate_dir, file, features);
    if let Ok(mut m) = memo.lock() {
        m.insert(key, verdict);
    }
    verdict
}

/// Is `rel` (`src/a/b.rs`) compiled in the library target of the crate at
/// `crate_dir` when built with `features`?
pub(crate) fn file_compiled(crate_dir: &Path, rel: &str, features: &HashSet<String>) -> Tri {
    let src_dir = crate_dir.join("src");
    let Some(lib) = lib_root(crate_dir) else {
        return Tri::Maybe;
    };
    // Named paths are `src/`-relative, so only a library rooted in `src/`
    // maps onto them.
    if lib.parent() != Some(src_dir.as_path()) || !crate_dir.join(rel).is_file() {
        return Tri::Maybe;
    }
    let Some(inner) = rel.strip_prefix("src/").and_then(|r| r.strip_suffix(".rs")) else {
        return Tri::Maybe;
    };
    let mut comps: Vec<&str> = inner.split('/').collect();
    if comps.last() == Some(&"mod") {
        comps.pop();
    }
    let lib_stem = lib.file_stem().and_then(|s| s.to_str()).unwrap_or("lib");
    match comps.as_slice() {
        [] => return Tri::Maybe,
        [only] if *only == lib_stem => return Tri::Yes,
        // A binary target is not what a dependent compiles; not this module's call.
        ["main"] | ["bin", ..] => return Tri::Maybe,
        _ => {}
    }
    let Some(root) = parse_file(&lib) else {
        return Tri::Maybe;
    };
    inner_cfg(&root.attrs, features).and(walk_items(&root.items, &src_dir, &comps, features))
}

/// The library target's root file: `[lib] path` when the manifest sets one,
/// else `src/lib.rs`.
fn lib_root(crate_dir: &Path) -> Option<PathBuf> {
    let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).ok()?;
    let mut in_lib = false;
    let mut custom: Option<String> = None;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_lib = line == "[lib]";
            continue;
        }
        if in_lib {
            if let Some(value) = line.strip_prefix("path") {
                let value = value.trim_start();
                if let Some(value) = value.strip_prefix('=') {
                    custom = Some(value.trim().trim_matches('"').replace('\\', "/"));
                }
            }
        }
    }
    let root = crate_dir.join(custom.as_deref().unwrap_or("src/lib.rs"));
    root.is_file().then_some(root)
}

fn parse_file(path: &Path) -> Option<syn::File> {
    let text = std::fs::read_to_string(path).ok()?;
    syn::parse_file(&text).ok()
}

/// Walk `comps` down from `items` (the body of the module whose child files
/// live in `mod_dir`). Several declarations of one name (`#[cfg(a)] mod x;`
/// beside `#[cfg(b)] mod x;`) compile the file if ANY of them does.
fn walk_items(
    items: &[syn::Item],
    mod_dir: &Path,
    comps: &[&str],
    features: &HashSet<String>,
) -> Tri {
    let Some((first, rest)) = comps.split_first() else {
        return Tri::Yes;
    };
    let mut verdict = Tri::No;
    let mut found = false;
    for item in items {
        if let syn::Item::Mod(m) = item {
            if m.ident == first {
                found = true;
                verdict = verdict.or(decl_verdict(m, mod_dir, rest, features));
            }
        }
    }
    // Not declared at this level: it may come from a macro
    // (`cfg_net! { pub mod net; }`) that this does not expand.
    if found {
        verdict
    } else {
        Tri::Maybe
    }
}

fn decl_verdict(
    m: &syn::ItemMod,
    mod_dir: &Path,
    rest: &[&str],
    features: &HashSet<String>,
) -> Tri {
    let mut gate = Tri::Yes;
    for attr in &m.attrs {
        let path = attr.path();
        if path.is_ident("path") {
            return Tri::Maybe;
        }
        if path.is_ident("cfg_attr") {
            // `#[cfg_attr(windows, path = "win.rs")]` may redirect the file.
            if let syn::Meta::List(list) = &attr.meta {
                if list.tokens.to_string().contains("path") {
                    return Tri::Maybe;
                }
            }
            continue;
        }
        if path.is_ident("cfg") {
            gate = gate.and(
                attr.parse_args::<syn::Meta>()
                    .map_or(Tri::Maybe, |meta| eval_cfg(&meta, features)),
            );
        }
    }
    if gate == Tri::No {
        return Tri::No;
    }
    let child_dir = mod_dir.join(m.ident.to_string());
    let below = match &m.content {
        Some((_, items)) => walk_items(items, &child_dir, rest, features),
        None => {
            let candidates = [
                mod_dir.join(format!("{}.rs", m.ident)),
                child_dir.join("mod.rs"),
            ];
            let Some(file) = candidates.iter().find(|p| p.is_file()) else {
                return Tri::Maybe;
            };
            let Some(parsed) = parse_file(file) else {
                return Tri::Maybe;
            };
            inner_cfg(&parsed.attrs, features).and(walk_items(
                &parsed.items,
                &child_dir,
                rest,
                features,
            ))
        }
    };
    gate.and(below)
}

/// A file's own `#![cfg(..)]` gates the whole module.
fn inner_cfg(attrs: &[syn::Attribute], features: &HashSet<String>) -> Tri {
    attrs
        .iter()
        .filter(|a| matches!(a.style, syn::AttrStyle::Inner(_)) && a.path().is_ident("cfg"))
        .map(|a| {
            a.parse_args::<syn::Meta>()
                .map_or(Tri::Maybe, |meta| eval_cfg(&meta, features))
        })
        .fold(Tri::Yes, Tri::and)
}

/// Evaluate one `cfg` predicate. Only `feature = ".."` and the `all` / `any`
/// / `not` combinators are decidable here; every other predicate is `Maybe`.
pub(crate) fn eval_cfg(meta: &syn::Meta, features: &HashSet<String>) -> Tri {
    match meta {
        syn::Meta::NameValue(nv) if nv.path.is_ident("feature") => match &nv.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => {
                if features.contains(&s.value()) {
                    Tri::Yes
                } else {
                    Tri::No
                }
            }
            _ => Tri::Maybe,
        },
        syn::Meta::List(list) => {
            let Ok(nested) =
                list.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
            else {
                return Tri::Maybe;
            };
            if list.path.is_ident("all") {
                nested
                    .iter()
                    .map(|m| eval_cfg(m, features))
                    .fold(Tri::Yes, Tri::and)
            } else if list.path.is_ident("any") {
                nested
                    .iter()
                    .map(|m| eval_cfg(m, features))
                    .fold(Tri::No, Tri::or)
            } else if list.path.is_ident("not") && nested.len() == 1 {
                nested
                    .first()
                    .map_or(Tri::Maybe, |m| eval_cfg(m, features).not())
            } else {
                Tri::Maybe
            }
        }
        _ => Tri::Maybe,
    }
}

#[cfg(test)]
#[path = "reachability_tests.rs"]
mod tests;
