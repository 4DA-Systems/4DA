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

// ============================================================================
// npm audit
// ============================================================================

/// Run `npm audit --json` in the given project directory.
/// Returns an empty vec if npm is not installed or `package-lock.json` is absent.
pub(crate) async fn run_npm_audit(project_path: &Path) -> Vec<LocalAuditFinding> {
    // Check for package-lock.json
    if !project_path.join("package-lock.json").exists() {
        return Vec::new();
    }

    // Check if npm is available
    let check = if cfg!(windows) {
        Command::new("where").arg("npm").output().await
    } else {
        Command::new("which").arg("npm").output().await
    };

    if check.is_err() || !check.as_ref().map_or(false, |o| o.status.success()) {
        debug!(target: "4da::audit", "npm not found — skipping npm audit");
        return Vec::new();
    }

    // Run npm audit
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        Command::new("npm")
            .args(["audit", "--json"])
            .current_dir(project_path)
            .output()
            .await
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            debug!(target: "4da::audit", "npm audit failed to execute: {e}");
            return Vec::new();
        }
        Err(_) => {
            warn!(target: "4da::audit", "npm audit timed out after 30s");
            return Vec::new();
        }
    };

    // npm audit returns exit code 1 when vulnerabilities are found, which is normal
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() && !output.status.success() && output.stdout.is_empty() {
        debug!(target: "4da::audit", "npm audit stderr: {stderr}");
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_npm_audit_json(&stdout)
}

/// Parse npm audit v2 JSON output into `LocalAuditFinding` entries.
fn parse_npm_audit_json(json_str: &str) -> Vec<LocalAuditFinding> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::audit", "Failed to parse npm audit JSON: {e}");
            return Vec::new();
        }
    };

    let vulnerabilities = match parsed.get("vulnerabilities").and_then(|v| v.as_object()) {
        Some(v) => v,
        None => return Vec::new(),
    };

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

    findings
}

// ============================================================================
// cargo audit
// ============================================================================

/// Run `cargo audit --json` in the given project directory.
/// Returns an empty vec if cargo-audit is not installed or `Cargo.lock` is absent.
pub(crate) async fn run_cargo_audit(project_path: &Path) -> Vec<LocalAuditFinding> {
    // Check for Cargo.lock
    if !project_path.join("Cargo.lock").exists() {
        return Vec::new();
    }

    // Check if cargo-audit is available
    let check = Command::new("cargo")
        .args(["audit", "--version"])
        .output()
        .await;

    if check.is_err() || !check.as_ref().map_or(false, |o| o.status.success()) {
        debug!(target: "4da::audit", "cargo-audit not installed — skipping cargo audit");
        return Vec::new();
    }

    // Run cargo audit
    let result = tokio::time::timeout(Duration::from_secs(60), async {
        Command::new("cargo")
            .args(["audit", "--json"])
            .current_dir(project_path)
            .output()
            .await
    })
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            debug!(target: "4da::audit", "cargo audit failed to execute: {e}");
            return Vec::new();
        }
        Err(_) => {
            warn!(target: "4da::audit", "cargo audit timed out after 60s");
            return Vec::new();
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        debug!(target: "4da::audit", "cargo audit stderr: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_cargo_audit_json(&stdout)
}

/// Parse cargo-audit JSON output into `LocalAuditFinding` entries.
fn parse_cargo_audit_json(json_str: &str) -> Vec<LocalAuditFinding> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::audit", "Failed to parse cargo audit JSON: {e}");
            return Vec::new();
        }
    };

    let vuln_list = match parsed
        .get("vulnerabilities")
        .and_then(|v| v.get("list"))
        .and_then(|v| v.as_array())
    {
        Some(list) => list,
        None => return Vec::new(),
    };

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
            .and_then(|v| v.as_f64())
            .map(|score| {
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
            .unwrap_or("medium")
            .to_string();

        // Extract patched versions
        let fix_version = vuln
            .get("versions")
            .and_then(|v| v.get("patched"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(String::from);

        // Extract affected version range
        let affected_versions = vuln
            .get("versions")
            .and_then(|v| v.get("unaffected"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|s| !s.is_empty());

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

    findings
}

// ============================================================================
// Combined runner
// ============================================================================

/// Run all local audit tools across discovered project directories.
///
/// Collects unique project paths from user_dependencies, checks which lock files
/// exist, and runs the appropriate audit tool. Results are deduplicated.
pub(crate) async fn run_local_audits() -> Vec<LocalAuditFinding> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            debug!(target: "4da::audit", "Cannot run local audits — database unavailable: {e}");
            return Vec::new();
        }
    };

    let deps = match db.get_all_user_dependencies() {
        Ok(d) => d,
        Err(e) => {
            debug!(target: "4da::audit", "Cannot load dependencies for local audit: {e}");
            return Vec::new();
        }
    };

    // Collect unique project paths
    let project_paths: HashSet<String> = deps.into_iter().map(|d| d.project_path).collect();

    let mut all_findings = Vec::new();

    for project_path in &project_paths {
        let path = Path::new(project_path);
        if !path.exists() {
            continue;
        }

        let npm_findings = run_npm_audit(path).await;
        let cargo_findings = run_cargo_audit(path).await;

        all_findings.extend(npm_findings);
        all_findings.extend(cargo_findings);
    }

    // Deduplicate by (package_name, ecosystem, title)
    let mut seen = HashSet::new();
    all_findings
        .retain(|f| seen.insert((f.package_name.clone(), f.ecosystem.clone(), f.title.clone())));

    debug!(
        target: "4da::audit",
        projects = project_paths.len(),
        findings = all_findings.len(),
        "Local audit scan complete"
    );

    all_findings
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
