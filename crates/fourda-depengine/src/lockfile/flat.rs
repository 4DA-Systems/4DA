// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Flat `(name, version)` lists from Cargo.lock, pnpm-lock.yaml and
//! composer.lock, and pnpm's package-key spellings. Moved from
//! `ProjectScanner` in the app (`ace/scanner.rs`), which keeps its
//! `parse_cargo_lock` / `parse_pnpm_lock_yaml` / `parse_composer_lock`
//! names as calls into these.

/// Every `(name, version)` a Cargo.lock lists, the project's own crates
/// included (the readers drop those with `cargo_facts::local_packages`).
pub fn cargo_lock(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            // Flush previous package
            if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                packages.push((name, version));
            }
        } else if let Some(rest) = trimmed.strip_prefix("name = ") {
            current_name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("version = ") {
            current_version = Some(rest.trim_matches('"').to_string());
        }
    }
    // Don't forget the last package
    if let (Some(name), Some(version)) = (current_name, current_version) {
        packages.push((name, version));
    }

    packages
}

/// Parse a pnpm-lock.yaml (v5/v6/v9) and return (package_name, version) pairs.
/// Uses focused line-by-line parsing (no YAML crate needed) since the `packages:`
/// section has a predictable structure: top-level keys at 2-space indent.
///
/// v5/v6 keys: `  /@scope/pkg/1.2.3:` or `  /pkg/1.2.3:`
/// v9 keys:    `  @scope/pkg@1.2.3:` or `  pkg@1.2.3:`
/// Some entries have a nested `version:` field instead of version-in-key.
///
/// An entry that records its own `name:` (pnpm writes one when the key is
/// not `name@version`: a tarball URL, a git or directory source) is
/// identified by its `name:` / `version:` fields, never by its key — trpc's
/// `'@registry.npmjs.com/@prisma/client/-/client-4.7.1.tgz?id=…'` key was
/// read as a package name (conformance corpus, 2026-10-10). Such an entry
/// is a registry release only when it resolves to a registry tarball
/// (`…/<name>/-/<name>-<version>.tgz`); git, codeload and directory
/// sources are local code and are not returned.
pub fn pnpm_lock_yaml(content: &str) -> Vec<(String, String)> {
    let mut packages = Vec::new();
    let mut in_packages = false;
    let mut entry: Option<PnpmEntry> = None;

    for line in content.lines() {
        if line.starts_with("packages:") {
            in_packages = true;
            continue;
        }
        if !in_packages {
            continue;
        }
        // A new top-level key (no indent) ends the packages section
        if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        let trimmed = line.trim();

        // Top-level package key: exactly 2 spaces (or 1 tab) of indentation + ends with ':'
        let is_package_key = (line.starts_with("  ") && !line.starts_with("    "))
            || (line.starts_with('\t') && !line.starts_with("\t\t"));

        if is_package_key && trimmed.ends_with(':') {
            packages.extend(entry.take().and_then(PnpmEntry::identity));
            let key = trimmed.trim_end_matches(':');
            // Strip optional YAML quoting
            let key = key.trim_matches('\'').trim_matches('"');
            entry = Some(PnpmEntry::new(key));
        } else if let Some(current) = entry.as_mut() {
            current.field_line(line, trimmed);
        }
    }
    packages.extend(entry.take().and_then(PnpmEntry::identity));

    packages
}

/// `packages` + `packages-dev` of a composer.lock, a leading `v` dropped.
/// Unparseable JSON yields nothing.
pub fn composer_lock(content: &str) -> Vec<(String, String)> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };

    let mut packages = Vec::new();

    for key in &["packages", "packages-dev"] {
        if let Some(arr) = parsed.get(key).and_then(|v| v.as_array()) {
            for entry in arr {
                let name = entry.get("name").and_then(|v| v.as_str());
                let version = entry.get("version").and_then(|v| v.as_str());
                if let (Some(n), Some(v)) = (name, version) {
                    let v = v.strip_prefix('v').unwrap_or(v);
                    if !n.is_empty() && !v.is_empty() {
                        packages.push((n.to_string(), v.to_string()));
                    }
                }
            }
        }
    }

    packages
}

/// One `packages:` entry of a pnpm lockfile, as far as its identity goes.
struct PnpmEntry {
    key: String,
    /// The entry's own `name:` field (written when the key is not `name@version`).
    name: Option<String>,
    version: Option<String>,
    tarball: Option<String>,
    /// A git or directory resolution (`commit:` / `repo:` / `directory:`).
    local: bool,
    in_resolution: bool,
}

impl PnpmEntry {
    fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            name: None,
            version: None,
            tarball: None,
            local: false,
            in_resolution: false,
        }
    }

    /// A line below the entry key: an entry field at 4 spaces (or 2 tabs),
    /// or a continuation of a multi-line `resolution:` mapping.
    fn field_line(&mut self, line: &str, trimmed: &str) {
        let field_level = (line.starts_with("    ") && !line.starts_with("     "))
            || (line.starts_with("\t\t") && !line.starts_with("\t\t\t"));
        if !field_level {
            if self.in_resolution {
                self.resolution_text(trimmed);
            }
            return;
        }
        self.in_resolution = false;
        if let Some(name) = yaml_scalar_field(trimmed, "name") {
            self.name = Some(name);
        } else if let Some(version) = yaml_scalar_field(trimmed, "version") {
            self.version = Some(strip_pnpm_peer_suffix(&version).to_string());
        } else if let Some(rest) = trimmed.strip_prefix("resolution:") {
            self.in_resolution = true;
            self.resolution_text(rest);
        }
    }

    fn resolution_text(&mut self, text: &str) {
        for part in text.split([',', '{', '}']) {
            let part = part.trim();
            if let Some(url) = part.strip_prefix("tarball:") {
                self.tarball = Some(url.trim().trim_matches(['\'', '"']).to_string());
            } else if ["commit:", "repo:", "directory:"]
                .iter()
                .any(|k| part.starts_with(k))
            {
                self.local = true;
            }
        }
    }

    /// The `(name, version)` this entry installs from a registry, if any.
    fn identity(self) -> Option<(String, String)> {
        if self.local {
            return None;
        }
        if let Some(name) = self.name {
            if self
                .tarball
                .as_deref()
                .is_some_and(|t| !is_registry_tarball(t))
            {
                return None;
            }
            let version = self
                .version
                .or_else(|| parse_pnpm_package_key(&self.key).map(|(_, v)| v))?;
            return (!name.is_empty() && !version.is_empty()).then_some((name, version));
        }
        if let Some(id) = parse_pnpm_package_key(&self.key) {
            return Some(id);
        }
        let name = self.key.trim_start_matches('/');
        let version = self.version?;
        (!name.is_empty() && !version.is_empty()).then(|| (name.to_string(), version))
    }
}

/// `key: value` (value unquoted) when the line is exactly that field.
fn yaml_scalar_field(trimmed: &str, key: &str) -> Option<String> {
    let value = trimmed.strip_prefix(key)?.strip_prefix(':')?.trim();
    let value = value.trim_matches(['\'', '"']).trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// A registry tarball (`https://<registry>/<name>/-/<name>-<version>.tgz`,
/// query allowed), as opposed to a codeload / git / arbitrary URL archive.
fn is_registry_tarball(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.contains("/-/")
        && std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
}

/// Parse pnpm package key formats:
/// - v9: `@scope/pkg@1.2.3` or `pkg@1.2.3` (`snapshots:` keys add `(peer@x)`)
/// - v6: `/@scope/pkg@1.2.3` or `/pkg@1.2.3`, peers as `(peer@x)`
/// - v5: `/@scope/pkg/1.2.3` or `/pkg/1.2.3`, peers as `_peer@x`
///
/// The peer suffix is never part of the identity: `@hono/node-server@2.1.0(hono@4.13.5)`
/// is `@hono/node-server` 2.1.0. Read with the old `rfind('@')` it was the package
/// `@hono/node-server@2.1.0(hono`, version `4.13.5)` — and a v6 scoped key
/// `/@babel/core@7.24.0` was the package `@babel`, version `core@7.24.0`.
pub fn parse_pnpm_package_key(key: &str) -> Option<(String, String)> {
    let key = strip_pnpm_peer_suffix(key);
    match key.strip_prefix('/') {
        Some(rest) => parse_pnpm_slash_key(rest),
        None => split_pnpm_name_at_version(key),
    }
}

/// Drop pnpm's peer-resolution suffix (`(react@18.2.0)`, v6/v9): everything
/// from the first `(` on. v5's `_peer` form is handled where versions are read.
pub fn strip_pnpm_peer_suffix(s: &str) -> &str {
    s.split('(').next().unwrap_or(s).trim()
}

/// `name@version`, scope-aware (`@scope/name@1.0.0`): the LAST `@` past the
/// scope marker separates the two.
pub fn split_pnpm_name_at_version(s: &str) -> Option<(String, String)> {
    let at = s.rfind('@').filter(|&p| p > 0)?;
    let (name, version) = (&s[..at], &s[at + 1..]);
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name.to_string(), version.to_string()))
}

/// A v5/v6 key after its leading `/`: v5 puts the version after a `/` (with
/// peers after `_`), v6 after an `@`. The scope segment belongs to the name in
/// both.
fn parse_pnpm_slash_key(rest: &str) -> Option<(String, String)> {
    let (scope, body) = match rest.strip_prefix('@') {
        Some(scoped) => {
            let (scope, body) = scoped.split_once('/')?;
            (Some(scope), body)
        }
        None => (None, rest),
    };
    let (name, version) = if let Some((name, version)) = body.split_once('/') {
        (name, version.split('_').next().unwrap_or(version))
    } else {
        let at = body.rfind('@').filter(|&p| p > 0)?;
        (&body[..at], &body[at + 1..])
    };
    if name.is_empty() || version.is_empty() {
        return None;
    }
    let name = match scope {
        Some(scope) => format!("@{scope}/{name}"),
        None => name.to_string(),
    };
    Some((name, version.to_string()))
}
