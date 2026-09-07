// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The version a release row announces, read from its title (v33).
//!
//! Registry rows name it outright (`crates.io: tauri v2.11.5`, `npm:
//! react-dom v19.2.8`); editorial announcements carry it after the
//! dependency's own name ("Announcing TypeScript 5.9 Beta", "axum 0.8.0
//! released"). Two consumers share this reading: the scorer's
//! already-installed gate (`pipeline_v2`) and the reconcile pass's
//! release-train collapse (`Database::reconcile_release_train`).
//!
//! Live 2026-09-07: twenty of the feed's fifty-one "New release in your
//! stack" rows announced a version the user already ran (sha2 0.11.0,
//! ed25519-dalek 3.0.0, tracing 0.1.44, TypeScript 5.9 against an installed
//! 5.9.3 …), and the whole TypeScript train — 5.9 Beta, 5.9 RC, 5.9, 6.0
//! Beta, 6.0 RC, 6.0, 7.0 Beta, 7.0 RC, 7.0 — sat in the feed at 0.76–0.90.
//! Nothing compared the announced version with the installed one; the only
//! superseded rule was age (24 months).

use semver::{Prerelease, Version};

/// Parse a version literal leniently: an optional `v`, one or two missing
/// components ("5.9" → 5.9.0, "7" → 7.0.0), an inline pre-release
/// ("1.0.0-rc.1") or one carried by the NEXT word ("5.9 Beta" → 5.9.0-beta).
/// semver orders a pre-release below its final, which is exactly the
/// release-train order.
pub(crate) fn lenient_semver(token: &str, next: Option<&str>) -> Option<Version> {
    let raw = token
        .trim()
        .trim_start_matches(['v', 'V'])
        .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
    let (core, inline_pre) = match raw.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (raw, None),
    };
    let mut parts: Vec<u64> = Vec::with_capacity(3);
    for seg in core.split('.') {
        if seg.is_empty() || parts.len() == 3 {
            return None;
        }
        parts.push(seg.parse().ok()?);
    }
    if parts.is_empty() {
        return None;
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    let mut version = Version::new(parts[0], parts[1], parts[2]);
    let pre = inline_pre
        .map(|p| p.to_ascii_lowercase())
        .or_else(|| next.and_then(prerelease_word));
    if let Some(p) = pre {
        version.pre = Prerelease::new(&p).ok()?;
    }
    Some(version)
}

fn prerelease_word(word: &str) -> Option<String> {
    let w = word
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    match w.as_str() {
        "alpha" | "beta" | "rc" | "preview" | "nightly" | "canary" => Some(w),
        _ => None,
    }
}

/// The version `title` announces FOR `dep_name`, or `None` when the title
/// announces nothing, or announces some other package.
pub(crate) fn announced_release_version(
    title: &str,
    source_type: &str,
    dep_name: &str,
) -> Option<Version> {
    if crate::dep_linker::is_registry_source(source_type) {
        let (subject, version) = crate::dep_linker::registry_title_subject(title)?;
        if !crate::dep_linker::registry_names_equal(&subject, dep_name) {
            return None;
        }
        return lenient_semver(&version?, None);
    }
    // Editorial: the literal after THIS dependency's name, word-bounded with
    // `-`/`_` as name characters ("react-query v5" is not a react release).
    let lower = title.to_lowercase();
    let dep = dep_name.to_lowercase();
    if dep.is_empty() {
        return None;
    }
    let is_boundary =
        |c: Option<char>| c.is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_' || ch == '-'));
    for (pos, _) in lower.match_indices(&dep) {
        let before = lower.get(..pos).and_then(|s| s.chars().next_back());
        let after = lower.get(pos + dep.len()..).unwrap_or("");
        if !is_boundary(before) || !is_boundary(after.chars().next()) {
            continue;
        }
        let mut tokens = after.split_whitespace();
        let Some(first) = tokens.next() else {
            continue;
        };
        let first = first.trim_matches(|c: char| c == ':' || c == ',' || c == '(' || c == ')');
        if let Some(v) = lenient_semver(first, tokens.next()) {
            return Some(v);
        }
    }
    None
}

/// Two versions are on the same release line when their majors agree — for
/// 0.x crates, when major and minor agree (0.8.x and 0.9.x are different
/// lines, as Cargo treats them).
pub(crate) fn same_release_line(a: &Version, b: &Version) -> bool {
    a.major == b.major && (a.major != 0 || a.minor == b.minor)
}

/// Does EVERY installed copy already run `announced` or newer? Empty
/// `installed` is "cannot tell" → false, so an unknown install never hides a
/// release.
pub(crate) fn already_installed(announced: &Version, installed: &[Version]) -> bool {
    !installed.is_empty() && installed.iter().all(|i| i >= announced)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn lenient_semver_fills_components_and_reads_prerelease_words() {
        assert_eq!(lenient_semver("5.9", None), Some(v("5.9.0")));
        assert_eq!(lenient_semver("v2.11.5", None), Some(v("2.11.5")));
        assert_eq!(lenient_semver("7", None), Some(v("7.0.0")));
        assert_eq!(lenient_semver("5.9", Some("Beta")), Some(v("5.9.0-beta")));
        assert_eq!(lenient_semver("7.0", Some("RC")), Some(v("7.0.0-rc")));
        assert_eq!(lenient_semver("1.0.0-rc.1", None), Some(v("1.0.0-rc.1")));
        assert_eq!(lenient_semver("5.9", Some("released")), Some(v("5.9.0")));
        assert_eq!(lenient_semver("released", None), None);
        assert_eq!(lenient_semver("1.2.3.4", None), None);
        // The train order: beta < rc < final.
        assert!(v("5.9.0-beta") < v("5.9.0-rc"));
        assert!(v("5.9.0-rc") < v("5.9.0"));
    }

    #[test]
    fn announced_version_reads_registry_subjects_and_editorial_titles() {
        assert_eq!(
            announced_release_version("crates.io: tauri v2.11.5", "crates_io", "tauri"),
            Some(v("2.11.5"))
        );
        assert_eq!(
            announced_release_version("crates.io: axum-stack v0.1.0", "crates_io", "axum"),
            None,
            "a registry row for another crate announces nothing for axum"
        );
        assert_eq!(
            announced_release_version("Announcing TypeScript 5.9", "rss", "typescript"),
            Some(v("5.9.0"))
        );
        assert_eq!(
            announced_release_version("Announcing TypeScript 5.9 Beta", "rss", "typescript"),
            Some(v("5.9.0-beta"))
        );
        assert_eq!(
            announced_release_version("Announcing axum 0.8.0", "rss", "axum"),
            Some(v("0.8.0"))
        );
        assert_eq!(
            announced_release_version("TanStack react-query v5 is out", "rss", "react"),
            None,
            "react-query is not react"
        );
        assert_eq!(
            announced_release_version("Rust has become a spiritual experience", "reddit", "rust"),
            None
        );
    }

    #[test]
    fn release_lines_and_installed_checks() {
        assert!(same_release_line(&v("5.9.0-beta"), &v("5.9.3")));
        assert!(same_release_line(&v("6.0.0"), &v("6.4.1")));
        assert!(!same_release_line(&v("6.0.0"), &v("7.0.0")));
        assert!(same_release_line(&v("0.8.0"), &v("0.8.9")));
        assert!(!same_release_line(&v("0.8.9"), &v("0.9.0")));

        assert!(already_installed(&v("5.9.0"), &[v("5.9.3")]));
        assert!(already_installed(&v("0.11.0"), &[v("0.11.0")]));
        assert!(!already_installed(&v("7.0.0"), &[v("5.9.3"), v("6.0.3")]));
        assert!(
            !already_installed(&v("1.53.1"), &[v("1.53.1"), v("1.52.3")]),
            "one project still below the release keeps it new"
        );
        assert!(
            !already_installed(&v("1.0.0"), &[]),
            "unknown install never hides a release"
        );
    }
}
