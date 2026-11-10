// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A package.json override replaces every parent's requirement on the
//! package it names (npm `overrides`, pnpm `pnpm.overrides`, yarn
//! `resolutions`).
//!
//! Live 2026-11-10 (PR #1115 verification): the plan said request 2.88.2
//! holds qs at `~6.5.2`, while the installed qs was 6.16.0. Both facts were
//! true and came from the same project (4da-ledger). The lockfile edge
//! records request's own requirement (`~6.5.2`), but the project's
//! package.json has `"overrides": { "qs": "^6.16.0" }`, so npm installs
//! 6.16.0 and `npm update qs` moves it to the fixed 6.16.1. The route
//! claimed the parent must move, and the step went blocked upstream, while a
//! lockfile refresh fixes it. The override is now the requirement the
//! refresh verdict reads, and the text names it.

use std::collections::HashMap;
use std::path::Path;

use super::fix_path::{refresh_verdict, Manager, Refresh, Requirement};
use super::parent_hint::ParentLink;

/// The spec `package.json` (as text) forces for `package`, if any. Only a
/// plain top-level entry (`"qs": "^6.16.0"`, or the yarn glob `"**/qs"`)
/// counts; a nested, parent-scoped override applies to one parent only and
/// is left to the lockfile.
pub(crate) fn override_spec(manifest: &serde_json::Value, package: &str) -> Option<String> {
    let tables = [
        manifest.get("overrides"),
        manifest.get("pnpm").and_then(|p| p.get("overrides")),
        manifest.get("resolutions"),
    ];
    let keys = [package.to_string(), format!("**/{package}")];
    tables.into_iter().flatten().find_map(|t| {
        keys.iter()
            .find_map(|k| t.get(k).and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.starts_with('$'))
    })
}

/// [`override_spec`] read from `dir`'s package.json (the Brief's walk).
pub(crate) fn project_override(dir: &str, package: &str) -> Option<String> {
    let text = std::fs::read_to_string(Path::new(dir).join("package.json")).ok()?;
    override_spec(&serde_json::from_str(&text).ok()?, package)
}

/// Apply each npm project's override to its links: the override is the
/// requirement, the refresh verdict and command are recomputed from it, and
/// the requirement text says it replaces the parent's own.
pub(crate) fn apply(
    package: &str,
    links: &mut [ParentLink],
    dir: &dyn Fn(&str) -> String,
    manager: &dyn Fn(&str) -> Option<Manager>,
) {
    let mut manifests: HashMap<String, Option<serde_json::Value>> = HashMap::new();
    for link in links.iter_mut() {
        if link.is_bundled() {
            continue;
        }
        let Some(target) = link.target.clone() else {
            continue;
        };
        let manifest = manifests.entry(link.project.clone()).or_insert_with(|| {
            let path = Path::new(&dir(&link.project)).join("package.json");
            std::fs::read_to_string(path)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
        });
        let Some(spec) = manifest.as_ref().and_then(|m| override_spec(m, package)) else {
            continue;
        };
        let refresh = refresh_verdict(&link.child_version, &target, &[Requirement::npm(&spec)]);
        if matches!(refresh, Refresh::Unknown) {
            continue;
        }
        let own = link
            .requirement
            .as_deref()
            .map_or_else(String::new, |r| format!(" {r}"));
        link.requirement = Some(format!(
            "{spec} (a package.json override, replacing {}'s own{own})",
            link.parent
        ));
        link.refresh = refresh;
        link.command = refresh
            .is_enough()
            .then(|| manager(&link.project))
            .flatten()
            .and_then(|m| m.refresh_command(package, &link.child_version, &target));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_three_override_tables() {
        let npm = serde_json::json!({"overrides": {"qs": "^6.16.0", "lodash": "^4.18.1"}});
        assert_eq!(override_spec(&npm, "qs").as_deref(), Some("^6.16.0"));
        let pnpm = serde_json::json!({"pnpm": {"overrides": {"qs": "6.16.1"}}});
        assert_eq!(override_spec(&pnpm, "qs").as_deref(), Some("6.16.1"));
        let yarn = serde_json::json!({"resolutions": {"**/qs": "^6.16.1"}});
        assert_eq!(override_spec(&yarn, "qs").as_deref(), Some("^6.16.1"));
        // A parent-scoped (nested) override and a `$ref` are not a plain spec.
        let nested =
            serde_json::json!({"overrides": {"request": {"qs": "^6.16.0"}, "uuid": "$uuid"}});
        assert_eq!(override_spec(&nested, "qs"), None);
        assert_eq!(override_spec(&nested, "uuid"), None);
    }
}
