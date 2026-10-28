// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Copies bundled inside a dependency's tarball (`package-lock.json`
//! `inBundle`). Moved from the app's `osv/fix_path.rs`, which re-exports
//! them: the package-lock edge reader ([`super::graph`]) gives such a copy one
//! edge from its owner, and the fix-path reader treats that edge as frozen.

use serde_json::{Map, Value};

/// Marks a `package-lock.json` edge to a BUNDLED copy (`inBundle`): the
/// exact version the bundle owner ships, e.g. `2.2.0 (bundled)`. Every reader
/// sees an exact pin (the text after `(` is dropped), so no refresh is
/// enough; and since npm `overrides` do not reach bundled dependencies, a
/// bundled link names no override either — only a new owner release moves it.
pub const BUNDLED_SUFFIX: &str = " (bundled)";

/// The edge spec names a bundled copy ([`BUNDLED_SUFFIX`]).
pub fn is_bundled(spec: &str) -> bool {
    spec.ends_with(BUNDLED_SUFFIX)
}

/// A `package-lock.json` v2/v3 key that installs a DEPENDENCY (under some
/// `node_modules/`), as opposed to the root (`""`) or a workspace member's
/// folder (`packages/web`).
pub fn is_dependency_key(key: &str) -> bool {
    key.starts_with("node_modules/") || key.contains("/node_modules/")
}

/// The key of the package a bundled entry at `key` ships inside: the nearest
/// enclosing entry that is not itself `inBundle` (a level the lockfile does
/// not list is passed over). `None` when no enclosing entry is listed.
pub fn npm_bundle_owner<'a>(pkgs: &'a Map<String, Value>, key: &'a str) -> Option<&'a str> {
    let mut at = key;
    loop {
        let (enclosing, _) = at.rsplit_once("/node_modules/")?;
        at = enclosing;
        match pkgs.get(enclosing) {
            Some(entry)
                if !entry
                    .get("inBundle")
                    .and_then(Value::as_bool)
                    .unwrap_or(false) =>
            {
                return Some(enclosing)
            }
            _ => {}
        }
    }
}

/// Whether the entry at `key` is a copy bundled inside a DEPENDENCY's tarball:
/// `inBundle`, with a dependency as its bundler. A project's or workspace
/// member's own `bundleDependencies` also set `inBundle` (npm Arborist's
/// `getBundler()` can be the root), but those copies sit in the user's own
/// tree, where `npm update` and overrides reach them like any other.
pub fn npm_lock_entry_bundled(pkgs: &Map<String, Value>, key: &str) -> bool {
    let in_bundle = pkgs
        .get(key)
        .and_then(|e| e.get("inBundle"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    in_bundle && npm_bundle_owner(pkgs, key).is_some_and(is_dependency_key)
}
