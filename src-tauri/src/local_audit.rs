// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Local audit tool integration — runs `npm audit` and `cargo audit` when available.
//!
//! Supplements the GitHub Advisory Database CVE scan with findings from
//! locally-installed audit tools that have access to the full dependency tree.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use tokio::process::Command;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
pub(crate) struct LocalAuditFinding {
    pub package_name: String,
    pub ecosystem: String,
    pub severity: String,
    pub title: String,
    pub description: Option<String>,
    pub affected_versions: Option<String>,
    pub source_url: Option<String>,
    pub fix_version: Option<String>,
}

impl LocalAuditFinding {
    /// Identity of the advisory this finding reports, in the same shape the
    /// `dependency_alerts` dedup key uses. Reconciliation compares live audit
    /// output against stored alerts on exactly this triple.
    pub(crate) fn alert_key(&self) -> (String, String, String) {
        (
            self.package_name.clone(),
            crate::sources::cve_matching::normalize_ecosystem(&self.ecosystem).to_string(),
            self.title.clone(),
        )
    }
}

/// Outcome of one audit tool invocation.
///
/// `ran` distinguishes "the tool completed and reported exactly this set" from
/// "the tool was absent, timed out, or failed". Both previously returned an
/// empty `Vec`, making a broken toolchain indistinguishable from a clean bill
/// of health — so only a *completed* run is allowed to retire an alert.
pub(crate) struct AuditRun {
    pub ran: bool,
    pub findings: Vec<LocalAuditFinding>,
}

impl AuditRun {
    /// The tool did not produce a trustworthy verdict — never retire on this.
    fn skipped() -> Self {
        Self {
            ran: false,
            findings: Vec::new(),
        }
    }

    /// The tool ran to completion; `findings` is the authoritative full set.
    fn completed(findings: Vec<LocalAuditFinding>) -> Self {
        Self {
            ran: true,
            findings,
        }
    }
}

/// Aggregate result of every audit tool across every discovered project.
pub(crate) struct LocalAuditOutcome {
    pub findings: Vec<LocalAuditFinding>,
    /// Normalized ecosystems for which at least one audit completed. Only
    /// these may have their stale alerts auto-resolved; an ecosystem whose
    /// tooling never ran keeps every existing alert untouched.
    pub audited_ecosystems: HashSet<String>,
}

fn suppress_console_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.stdin(std::process::Stdio::null());
}

// ============================================================================
// Advisory range semantics
// ============================================================================

/// Render a parsed comparator back to a bare version string (`1.11.1`,
/// `0.8.0-rc.1`), dropping the operator.
fn comparator_version(c: &semver::Comparator) -> String {
    let mut s = c.major.to_string();
    if let Some(minor) = c.minor {
        s.push('.');
        s.push_str(&minor.to_string());
        if let Some(patch) = c.patch {
            s.push('.');
            s.push_str(&patch.to_string());
        }
    }
    if !c.pre.is_empty() {
        s.push('-');
        s.push_str(c.pre.as_str());
    }
    s
}

/// The single version bound in `reqs`, but only when the list holds exactly one
/// requirement built from exactly one comparator using `op`. Any richer shape
/// (compound `">= 0.6.1, < 0.7.0"`, caret `"^ 2.2.1"`, multiple branches)
/// returns `None` — it cannot be negated into a single range.
fn sole_bound(reqs: &[String], op: semver::Op) -> Option<String> {
    let [only] = reqs else { return None };
    let req = semver::VersionReq::parse(only).ok()?;
    let [cmp] = req.comparators.as_slice() else {
        return None;
    };
    (cmp.op == op).then(|| comparator_version(cmp))
}

/// Derive the AFFECTED version range for a RustSec advisory.
///
/// RustSec never states which versions are vulnerable. It states which are
/// SAFE, in two independent lists:
/// - `patched` — the bug is fixed from here on (e.g. `">= 1.11.1"`)
/// - `unaffected` — the bug did not exist yet (e.g. `"< 1.2.1"`)
///
/// Vulnerable is everything else: the NEGATION of their union. That negation
/// collapses to a single `VersionReq` only when each list holds one simple
/// bound — `>= P` negates to `< P`, `< U` negates to `>= U` — giving
/// `">=U, <P"`.
///
/// Every other shape returns `None`, meaning *unknown*: a multi-branch backport
/// (`[">= 3.1.0", ">= 2.1.3, < 3.0.0"]`, 10.5% of the 1,245-advisory RustSec
/// corpus), a caret requirement, or no published fix at all (39.8%). Consumers
/// treat an unknown range as still-affected, so an inexpressible advisory can
/// never silently clear a live vulnerability.
///
/// This replaces a read of the `unaffected` field *as if it were* the affected
/// range, which inverted the test outright: `bytes 1.10.1` sits squarely inside
/// the real affected window `[1.2.1, 1.11.1)` yet tested "safe" against
/// `< 1.2.1`, and the auto-resolver closed a live memory-corruption advisory on
/// that basis.
fn derive_affected_range(patched: &[String], unaffected: &[String]) -> Option<String> {
    // No single published fix boundary -> decline to narrow, rather than
    // inventing a bound that would let the resolver clear the alert.
    let upper = sole_bound(patched, semver::Op::GreaterEq)?;

    if unaffected.is_empty() {
        return Some(format!("<{upper}"));
    }
    let lower = sole_bound(unaffected, semver::Op::Less)?;
    Some(format!(">={lower}, <{upper}"))
}

/// Collect a `versions.<key>` string array from a cargo-audit vulnerability.
fn version_reqs(vuln: &serde_json::Value, key: &str) -> Vec<String> {
    vuln.get("versions")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

// ============================================================================
// npm audit
// ============================================================================

/// Run `npm audit --json` in the given project directory.
///
/// Returns a *skipped* run when npm is missing, `package-lock.json` is absent,
/// or the command fails — never an empty "all clear". Note this deliberately
/// requires `package-lock.json`: a pnpm/yarn project yields a skipped run, so
/// its npm alerts are never auto-retired on the strength of an audit that
/// never happened.
pub(crate) async fn run_npm_audit(project_path: &Path) -> AuditRun {
    // Check for package-lock.json
    if !project_path.join("package-lock.json").exists() {
        return AuditRun::skipped();
    }

    // Check if npm is available
    let check = {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("where");
            c.arg("npm");
            c
        } else {
            let mut c = Command::new("which");
            c.arg("npm");
            c
        };
        suppress_console_window(&mut cmd);
        cmd.output().await
    };

    if check.is_err() || !check.as_ref().is_ok_and(|o| o.status.success()) {
        debug!(target: "4da::audit", "npm not found — skipping npm audit");
        return AuditRun::skipped();
    }

    // Run npm audit
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let mut cmd = Command::new("npm");
        cmd.args(["audit", "--json"]).current_dir(project_path);
        suppress_console_window(&mut cmd);
        cmd.output().await
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            debug!(target: "4da::audit", "npm audit failed to execute: {e}");
            return AuditRun::skipped();
        }
        Err(_) => {
            warn!(target: "4da::audit", "npm audit timed out after 30s");
            return AuditRun::skipped();
        }
    };

    // npm audit returns exit code 1 when vulnerabilities are found, which is normal
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() && !output.status.success() && output.stdout.is_empty() {
        debug!(target: "4da::audit", "npm audit stderr: {stderr}");
        return AuditRun::skipped();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // A body that does not parse is a failed run, not an empty one.
    match parse_npm_audit_payload(&stdout) {
        Some(findings) => AuditRun::completed(findings),
        None => AuditRun::skipped(),
    }
}

/// Parse npm audit v2 JSON output into `LocalAuditFinding` entries.
///
/// Test-facing wrapper: collapses "unparseable" and "clean" into an empty vec.
/// Production callers use [`parse_npm_audit_payload`], which keeps them apart.
#[cfg(test)]
fn parse_npm_audit_json(json_str: &str) -> Vec<LocalAuditFinding> {
    parse_npm_audit_payload(json_str).unwrap_or_default()
}

/// Parse npm audit v2 JSON output.
///
/// `Some(vec![])` means the audit ran and found nothing; `None` means the
/// payload could not be understood, which must never be read as a clean
/// result.
fn parse_npm_audit_payload(json_str: &str) -> Option<Vec<LocalAuditFinding>> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::audit", "Failed to parse npm audit JSON: {e}");
            return None;
        }
    };

    let vulnerabilities = parsed.get("vulnerabilities").and_then(|v| v.as_object())?;

    let mut findings = Vec::new();

    for (_pkg_name, vuln) in vulnerabilities {
        let name = match vuln.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => continue,
        };

        let severity = vuln
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_lowercase();

        // Extract title from "via" array — first object with a "title" field
        let title = vuln
            .get("via")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find_map(|item| item.get("title").and_then(|t| t.as_str()))
            })
            .unwrap_or("Unknown vulnerability")
            .to_string();

        // Extract URL from "via" array
        let source_url = vuln
            .get("via")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find_map(|item| item.get("url").and_then(|u| u.as_str()))
            })
            .map(String::from);

        let affected_versions = vuln.get("range").and_then(|v| v.as_str()).map(String::from);

        let fix_version = vuln
            .get("fixAvailable")
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from);

        findings.push(LocalAuditFinding {
            package_name: name.to_string(),
            ecosystem: "npm".to_string(),
            severity,
            title,
            description: None,
            affected_versions,
            source_url,
            fix_version,
        });
    }

    Some(findings)
}

// ============================================================================
// cargo audit
// ============================================================================

/// Run `cargo audit --json` in the given project directory.
///
/// Returns a *skipped* run when cargo-audit is missing, `Cargo.lock` is absent,
/// or the command fails — never an empty "all clear".
pub(crate) async fn run_cargo_audit(project_path: &Path) -> AuditRun {
    // Check for Cargo.lock
    if !project_path.join("Cargo.lock").exists() {
        return AuditRun::skipped();
    }

    // Check if cargo-audit is available
    let check = {
        let mut cmd = Command::new("cargo");
        cmd.args(["audit", "--version"]);
        suppress_console_window(&mut cmd);
        cmd.output().await
    };

    if check.is_err() || !check.as_ref().is_ok_and(|o| o.status.success()) {
        debug!(target: "4da::audit", "cargo-audit not installed — skipping cargo audit");
        return AuditRun::skipped();
    }

    // Run cargo audit
    let result = tokio::time::timeout(Duration::from_mins(1), async {
        let mut cmd = Command::new("cargo");
        cmd.args(["audit", "--json"]).current_dir(project_path);
        suppress_console_window(&mut cmd);
        cmd.output().await
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            debug!(target: "4da::audit", "cargo audit failed to execute: {e}");
            return AuditRun::skipped();
        }
        Err(_) => {
            warn!(target: "4da::audit", "cargo audit timed out after 60s");
            return AuditRun::skipped();
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        debug!(target: "4da::audit", "cargo audit stderr: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // A body that does not parse is a failed run, not an empty one.
    match parse_cargo_audit_payload(&stdout) {
        Some(findings) => AuditRun::completed(findings),
        None => AuditRun::skipped(),
    }
}

/// Parse cargo-audit JSON output into `LocalAuditFinding` entries.
///
/// Test-facing wrapper: collapses "unparseable" and "clean" into an empty vec.
/// Production callers use [`parse_cargo_audit_payload`], which keeps them apart.
#[cfg(test)]
fn parse_cargo_audit_json(json_str: &str) -> Vec<LocalAuditFinding> {
    parse_cargo_audit_payload(json_str).unwrap_or_default()
}

/// Parse cargo-audit JSON output.
///
/// `Some(vec![])` means the audit ran and found nothing; `None` means the
/// payload could not be understood, which must never be read as a clean
/// result.
fn parse_cargo_audit_payload(json_str: &str) -> Option<Vec<LocalAuditFinding>> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::audit", "Failed to parse cargo audit JSON: {e}");
            return None;
        }
    };

    let vuln_list = parsed
        .get("vulnerabilities")
        .and_then(|v| v.get("list"))
        .and_then(|v| v.as_array())?;

    let mut findings = Vec::new();

    for vuln in vuln_list {
        let advisory = match vuln.get("advisory") {
            Some(a) => a,
            None => continue,
        };

        let id = advisory
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");

        let package = advisory
            .get("package")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let title = advisory
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown vulnerability");

        let description = advisory
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let url = advisory
            .get("url")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Map RUSTSEC informational to severity; cargo-audit doesn't always have severity
        // so we infer from the advisory kind or default to "medium"
        let severity = advisory
            .get("cvss")
            .and_then(serde_json::Value::as_f64)
            .map_or("medium", |score| {
                if score >= 9.0 {
                    "critical"
                } else if score >= 7.0 {
                    "high"
                } else if score >= 4.0 {
                    "medium"
                } else {
                    "low"
                }
            })
            .to_string();

        // RustSec publishes the SAFE sets, never the vulnerable one. Read both
        // and negate their union; see `derive_affected_range` for why some
        // advisories legitimately yield `None` (unknown => still affected).
        let patched = version_reqs(vuln, "patched");
        let unaffected = version_reqs(vuln, "unaffected");

        let fix_version = patched.first().cloned();
        let affected_versions = derive_affected_range(&patched, &unaffected);

        findings.push(LocalAuditFinding {
            package_name: package.to_string(),
            ecosystem: "crates.io".to_string(),
            severity,
            title: format!("{id}: {title}"),
            description,
            affected_versions,
            source_url: url,
            fix_version,
        });
    }

    Some(findings)
}

// ============================================================================
// Combined runner
// ============================================================================

/// Run all local audit tools across discovered project directories.
///
/// Collects unique project paths from `user_dependencies`, checks which lock
/// files exist, and runs the appropriate audit tool. Results are deduplicated.
///
/// The returned `audited_ecosystems` records which ecosystems produced a
/// trustworthy verdict somewhere. An ecosystem missing from that set had no
/// completed run anywhere, so its stored alerts must be left alone — a missing
/// `cargo-audit` binary or a pnpm-only tree must never read as "nothing is
/// vulnerable any more".
pub(crate) async fn run_local_audits() -> LocalAuditOutcome {
    let empty = || LocalAuditOutcome {
        findings: Vec::new(),
        audited_ecosystems: HashSet::new(),
    };

    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            debug!(target: "4da::audit", "Cannot run local audits — database unavailable: {e}");
            return empty();
        }
    };

    let deps = match db.get_auditable_user_dependencies() {
        Ok(d) => d,
        Err(e) => {
            debug!(target: "4da::audit", "Cannot load dependencies for local audit: {e}");
            return empty();
        }
    };

    // Collect unique project paths
    let project_paths: HashSet<String> = deps.into_iter().map(|d| d.project_path).collect();

    let mut all_findings = Vec::new();
    let mut audited_ecosystems: HashSet<String> = HashSet::new();

    for project_path in &project_paths {
        let path = Path::new(project_path);
        if !path.exists() {
            continue;
        }

        for (ecosystem, run) in [
            ("npm", run_npm_audit(path).await),
            ("crates.io", run_cargo_audit(path).await),
        ] {
            if run.ran {
                audited_ecosystems.insert(
                    crate::sources::cve_matching::normalize_ecosystem(ecosystem).to_string(),
                );
            }
            all_findings.extend(run.findings);
        }
    }

    // Deduplicate by (package_name, ecosystem, title)
    let mut seen = HashSet::new();
    all_findings
        .retain(|f| seen.insert((f.package_name.clone(), f.ecosystem.clone(), f.title.clone())));

    debug!(
        target: "4da::audit",
        projects = project_paths.len(),
        findings = all_findings.len(),
        audited = audited_ecosystems.len(),
        "Local audit scan complete"
    );

    LocalAuditOutcome {
        findings: all_findings,
        audited_ecosystems,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npm_audit_with_vulnerabilities() {
        let json = r#"{
            "vulnerabilities": {
                "lodash": {
                    "name": "lodash",
                    "severity": "high",
                    "via": [
                        {
                            "title": "Prototype Pollution",
                            "url": "https://github.com/advisories/GHSA-test"
                        }
                    ],
                    "range": "<4.17.21",
                    "fixAvailable": {
                        "name": "lodash",
                        "version": "4.17.21"
                    }
                },
                "minimist": {
                    "name": "minimist",
                    "severity": "critical",
                    "via": [
                        {
                            "title": "Prototype Pollution in minimist",
                            "url": "https://github.com/advisories/GHSA-min"
                        }
                    ],
                    "range": "<1.2.6",
                    "fixAvailable": {
                        "name": "minimist",
                        "version": "1.2.6"
                    }
                }
            }
        }"#;

        let findings = parse_npm_audit_json(json);
        assert_eq!(findings.len(), 2);

        let lodash = findings
            .iter()
            .find(|f| f.package_name == "lodash")
            .unwrap();
        assert_eq!(lodash.severity, "high");
        assert_eq!(lodash.title, "Prototype Pollution");
        assert_eq!(lodash.ecosystem, "npm");
        assert_eq!(lodash.affected_versions.as_deref(), Some("<4.17.21"));
        assert_eq!(lodash.fix_version.as_deref(), Some("4.17.21"));
        assert_eq!(
            lodash.source_url.as_deref(),
            Some("https://github.com/advisories/GHSA-test")
        );

        let minimist = findings
            .iter()
            .find(|f| f.package_name == "minimist")
            .unwrap();
        assert_eq!(minimist.severity, "critical");
    }

    #[test]
    fn test_parse_npm_audit_no_vulnerabilities() {
        let json = r#"{
            "vulnerabilities": {}
        }"#;
        let findings = parse_npm_audit_json(json);
        assert!(findings.is_empty());

        // Also test completely empty response
        let json_empty = r#"{}"#;
        let findings_empty = parse_npm_audit_json(json_empty);
        assert!(findings_empty.is_empty());
    }

    #[test]
    fn test_parse_cargo_audit_with_findings() {
        let json = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {
                            "id": "RUSTSEC-2024-0001",
                            "package": "some-crate",
                            "title": "Memory safety issue in some-crate",
                            "description": "A memory safety issue was found in some-crate versions prior to 1.2.3.",
                            "url": "https://rustsec.org/advisories/RUSTSEC-2024-0001",
                            "cvss": 7.5
                        },
                        "versions": {
                            "patched": [">=1.2.3"],
                            "unaffected": ["<1.0.0"]
                        }
                    },
                    {
                        "advisory": {
                            "id": "RUSTSEC-2024-0002",
                            "package": "another-crate",
                            "title": "Denial of service",
                            "description": "A denial of service vulnerability.",
                            "url": "https://rustsec.org/advisories/RUSTSEC-2024-0002",
                            "cvss": 9.1
                        },
                        "versions": {
                            "patched": [">=2.0.0"],
                            "unaffected": []
                        }
                    }
                ]
            }
        }"#;

        let findings = parse_cargo_audit_json(json);
        assert_eq!(findings.len(), 2);

        let some_crate = findings
            .iter()
            .find(|f| f.package_name == "some-crate")
            .unwrap();
        assert_eq!(some_crate.ecosystem, "crates.io");
        assert_eq!(some_crate.severity, "high"); // cvss 7.5 -> high
        assert_eq!(
            some_crate.title,
            "RUSTSEC-2024-0001: Memory safety issue in some-crate"
        );
        assert!(some_crate.description.is_some());
        assert_eq!(some_crate.fix_version.as_deref(), Some(">=1.2.3"));
        assert_eq!(
            some_crate.source_url.as_deref(),
            Some("https://rustsec.org/advisories/RUSTSEC-2024-0001")
        );

        let another = findings
            .iter()
            .find(|f| f.package_name == "another-crate")
            .unwrap();
        assert_eq!(another.severity, "critical"); // cvss 9.1 -> critical
    }

    #[test]
    fn test_parse_malformed_json_gracefully() {
        // Completely invalid JSON
        let findings = parse_npm_audit_json("not json at all");
        assert!(findings.is_empty());

        let findings = parse_cargo_audit_json("{invalid}");
        assert!(findings.is_empty());

        // Valid JSON but wrong structure
        let findings = parse_npm_audit_json(r#"{"error": "something went wrong"}"#);
        assert!(findings.is_empty());

        let findings = parse_cargo_audit_json(r#"{"vulnerabilities": "not-an-object"}"#);
        assert!(findings.is_empty());

        // Empty string
        let findings = parse_npm_audit_json("");
        assert!(findings.is_empty());

        let findings = parse_cargo_audit_json("");
        assert!(findings.is_empty());
    }
}

// ============================================================================
// Advisory range regression tests
// ============================================================================
//
// Every fixture below is copied verbatim from the real RustSec advisory-db
// entry named in the test. The bug these guard against was invisible to
// synthetic data: reading `unaffected` as the affected range produces a
// perfectly well-formed VersionReq that simply answers the opposite question.

#[cfg(test)]
mod advisory_range_tests {
    use super::*;
    use crate::sources::cve_matching::version_is_affected;

    /// RUSTSEC-2026-0007 (bytes): `patched = [">= 1.11.1"]`,
    /// `unaffected = ["< 1.2.1"]`. Real vulnerable window is `[1.2.1, 1.11.1)`.
    #[test]
    fn bytes_advisory_derives_the_true_vulnerable_window() {
        let range = derive_affected_range(&[">= 1.11.1".into()], &["< 1.2.1".into()])
            .expect("single-bound advisory must derive");
        assert_eq!(range, ">=1.2.1, <1.11.1");

        // The version that shipped the bug fix is safe...
        assert!(!version_is_affected(Some("1.11.1"), &range));
        // ...the version that predates the bug is safe...
        assert!(!version_is_affected(Some("1.0.0"), &range));
        // ...and the version in the middle is NOT.
        assert!(version_is_affected(Some("1.10.1"), &range));
    }

    /// The exact inversion this replaces. `bytes 1.10.1` is genuinely
    /// vulnerable, yet testing it against the raw `unaffected` string reports
    /// "safe" — which is how the auto-resolver closed a live advisory.
    #[test]
    fn raw_unaffected_string_would_clear_a_live_vulnerability() {
        let inverted = "<1.2.1"; // what the old code stored
        assert!(
            !version_is_affected(Some("1.10.1"), inverted),
            "guard premise: the old range really did read as safe"
        );

        let correct = derive_affected_range(&[">= 1.11.1".into()], &["< 1.2.1".into()]).unwrap();
        assert!(
            version_is_affected(Some("1.10.1"), &correct),
            "the derived range must keep a genuinely vulnerable install flagged"
        );
    }

    /// RUSTSEC-2026-0233 (rkyv): prerelease bound on the `unaffected` side.
    /// `patched = [">= 0.8.17"]`, `unaffected = ["< 0.8.0-rc.1"]`.
    #[test]
    fn rkyv_advisory_handles_prerelease_lower_bound() {
        let range = derive_affected_range(&[">= 0.8.17".into()], &["< 0.8.0-rc.1".into()])
            .expect("prerelease bound must still derive");
        assert_eq!(range, ">=0.8.0-rc.1, <0.8.17");
        // The installed version on this machine — genuinely affected.
        assert!(version_is_affected(Some("0.8.10"), &range));
        assert!(!version_is_affected(Some("0.8.17"), &range));
    }

    /// No `unaffected` list at all — the common shape. Everything below the
    /// fix is vulnerable.
    #[test]
    fn patched_only_advisory_derives_open_lower_bound() {
        let range = derive_affected_range(&[">= 0.10.79".into()], &[]).unwrap();
        assert_eq!(range, "<0.10.79");
        assert!(version_is_affected(Some("0.10.78"), &range));
        assert!(!version_is_affected(Some("0.10.79"), &range));
    }

    /// RUSTSEC-2021-0074 (ammonia) shape: a backport branch plus a mainline
    /// fix. The negation is a union, not a range — 10.5% of the corpus. Must
    /// decline rather than guess.
    #[test]
    fn multi_branch_patched_declines_to_guess() {
        assert_eq!(
            derive_affected_range(&[">= 3.1.0".into(), ">= 2.1.3, < 3.0.0".into()], &[]),
            None
        );
    }

    /// 39.8% of the corpus has no published fix (e.g. RUSTSEC-2023-0071, the
    /// Marvin attack on `rsa`). Unknown must stay unknown.
    #[test]
    fn unfixed_advisory_yields_no_range() {
        assert_eq!(derive_affected_range(&[], &[]), None);
        assert_eq!(derive_affected_range(&[], &["< 1.0.0".into()]), None);
    }

    /// RUSTSEC-2021-0081 (actix-http) uses a caret requirement, which has no
    /// single-comparator negation.
    #[test]
    fn compound_and_caret_requirements_decline() {
        assert_eq!(derive_affected_range(&["^ 2.2.1".into()], &[]), None);
        assert_eq!(
            derive_affected_range(&[">= 0.6.1, < 0.7.0".into()], &[]),
            None
        );
    }

    /// An unknown range must never let a consumer conclude "safe".
    #[test]
    fn declined_range_keeps_every_install_flagged() {
        assert!(derive_affected_range(&["^ 2.2.1".into()], &[]).is_none());
        // How the resolver treats the resulting empty/absent range:
        assert!(version_is_affected(Some("2.2.1"), ""));
        assert!(version_is_affected(None, ">=1.0.0, <2.0.0"));
    }

    /// End-to-end through the cargo-audit parser, with the advisory nested
    /// exactly as cargo-audit emits it.
    #[test]
    fn cargo_audit_payload_stores_derived_range_not_unaffected() {
        let json = r#"{
            "vulnerabilities": {
                "list": [{
                    "advisory": {
                        "id": "RUSTSEC-2026-0007",
                        "package": "bytes",
                        "title": "Integer overflow in `BytesMut::reserve`",
                        "description": "unchecked addition",
                        "url": "https://rustsec.org/advisories/RUSTSEC-2026-0007"
                    },
                    "versions": {
                        "patched": [">= 1.11.1"],
                        "unaffected": ["< 1.2.1"]
                    }
                }]
            }
        }"#;

        let findings = parse_cargo_audit_payload(json).expect("well-formed payload parses");
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert_eq!(f.package_name, "bytes");
        assert_eq!(f.ecosystem, "crates.io");
        assert_eq!(f.affected_versions.as_deref(), Some(">=1.2.1, <1.11.1"));
        // The patched bound is no longer parsed and discarded.
        assert_eq!(f.fix_version.as_deref(), Some(">= 1.11.1"));
        assert_ne!(
            f.affected_versions.as_deref(),
            Some("< 1.2.1"),
            "the unaffected range must never be stored as the affected range"
        );
    }

    /// A clean audit and a broken audit must not look the same.
    #[test]
    fn clean_run_and_unreadable_run_are_distinguishable() {
        let clean = parse_cargo_audit_payload(r#"{"vulnerabilities":{"list":[]}}"#);
        assert_eq!(clean.map(|f| f.len()), Some(0), "clean audit -> ran, empty");

        assert!(
            parse_cargo_audit_payload("not json").is_none(),
            "unparseable audit -> no verdict"
        );
        assert!(
            parse_cargo_audit_payload("{}").is_none(),
            "missing vulnerability list -> no verdict"
        );

        assert_eq!(
            parse_npm_audit_payload(r#"{"vulnerabilities":{}}"#).map(|f| f.len()),
            Some(0)
        );
        assert!(parse_npm_audit_payload("").is_none());
    }

    /// The reconcile key must match the shape `dependency_alerts` stores, or
    /// every alert would look absent and be retired on the first pass.
    #[test]
    fn alert_key_uses_the_normalized_ecosystem() {
        let f = LocalAuditFinding {
            package_name: "bytes".into(),
            ecosystem: "crates.io".into(),
            severity: "medium".into(),
            title: "RUSTSEC-2026-0007: overflow".into(),
            description: None,
            affected_versions: None,
            source_url: None,
            fix_version: None,
        };
        let (pkg, eco, title) = f.alert_key();
        assert_eq!(pkg, "bytes");
        assert_eq!(title, "RUSTSEC-2026-0007: overflow");
        assert_eq!(
            eco,
            crate::sources::cve_matching::normalize_ecosystem("rust"),
            "a finding tagged crates.io and a dep tagged rust must key alike"
        );
    }
}
