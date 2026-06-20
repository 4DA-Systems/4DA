// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Live per-version OSV surfacing for strict-manifest mode.
//!
//! Split from `osv.rs` to stay under the file-size limit. In strict mode the engine sources
//! surfacing DIRECTLY from OSV, version-exact: for every pinned (ecosystem, package, version)
//! it asks OSV which advisories affect THAT version (`/v1/querybatch` with the version set —
//! OSV range-matches server-side), hydrates each match's full record (`/v1/vulns/{id}`), upserts
//! the advisories into `osv_advisories` (so the ledger can build its re-checkable vuln_match
//! proof, and the engine matcher can join), and emits grounding-compatible `SourceItem`s. The
//! surfaced set equals `POST /v1/query {package, version}` — authoritative + complete by
//! construction. Live-first; on offline/429/parse failure it falls back to the mirror-matched
//! path. Desktop never reaches here (non-strict mode keeps the popular-package + mirror flow).

use tracing::{info, warn};

use super::osv_types::*;
use super::{SourceError, SourceItem, SourceResult};

/// Entry point for strict mode (called from `OsvSource::fetch_items`/`fetch_items_deep`).
pub(super) async fn live_matched_advisories_as_items(client: &reqwest::Client) -> Vec<SourceItem> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::sources", error = %e, "OSV live strict: database unavailable");
            return Vec::new();
        }
    };
    let deps = gather_versioned_auditable_deps(&db);
    if deps.is_empty() {
        // No dependency carries a concrete version in the DB (e.g. a requirements.txt stack whose
        // scan didn't capture pins). The version-exact live query can't run, but the mirror path
        // matches conservatively against osv_advisories and still surfaces the advisories (the
        // ledger then version-grounds them from the manifest) — so fall back rather than surface
        // NOTHING, which would silently blind an entire stack.
        info!(target: "4da::sources", "OSV live strict: no versioned dependencies — falling back to mirror-matched advisories");
        return super::osv::matched_advisories_as_items().await;
    }
    match query_live_version_matched(client, &db, &deps).await {
        Ok(items) => {
            info!(
                target: "4da::sources",
                count = items.len(),
                deps = deps.len(),
                "OSV live strict: surfaced version-matched advisories directly from OSV"
            );
            items
        }
        Err(e) => {
            warn!(
                target: "4da::sources",
                error = %e,
                "OSV live strict: live query failed — falling back to mirror-matched advisories"
            );
            super::osv::matched_advisories_as_items().await
        }
    }
}

/// Batch-query OSV with the version set per dep (results align with queries by index), hydrate
/// the unique matched advisories, persist them, and build grounding-compatible items.
async fn query_live_version_matched(
    client: &reqwest::Client,
    db: &crate::db::Database,
    deps: &[(String, String, String)],
) -> SourceResult<Vec<SourceItem>> {
    const MAX_BATCH: usize = 1000;

    let mut per_dep_ids: Vec<Vec<String>> = Vec::with_capacity(deps.len());
    for chunk in deps.chunks(MAX_BATCH) {
        let queries = chunk
            .iter()
            .map(|(eco, name, ver)| OsvQueryRequest {
                package: Some(OsvPackage {
                    name: name.clone(),
                    ecosystem: eco.clone(),
                }),
                version: Some(ver.clone()),
            })
            .collect();
        let response = client
            .post("https://api.osv.dev/v1/querybatch")
            .json(&OsvBatchRequest { queries })
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "OSV querybatch rate limited (429)".to_string(),
            ));
        }
        super::check_http_status(status, "OSV querybatch")?;
        let parsed: OsvBatchResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;
        // Results are returned in query order; pad any short tail with empties so the per-dep
        // alignment below never panics.
        let mut results = parsed.results.unwrap_or_default().into_iter();
        for _ in 0..chunk.len() {
            let ids = results
                .next()
                .and_then(|r| r.vulns)
                .map(|vulns| vulns.into_iter().map(|v| v.id).collect())
                .unwrap_or_default();
            per_dep_ids.push(ids);
        }
        if deps.len() > MAX_BATCH {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // Hydrate each unique advisory id to its full record (the batch path returns id-stubs).
    let mut unique: Vec<String> = per_dep_ids.iter().flatten().cloned().collect();
    unique.sort();
    unique.dedup();
    let mut hydrated: std::collections::HashMap<String, OsvVulnerability> =
        std::collections::HashMap::new();
    for id in &unique {
        match hydrate_vuln(client, id).await {
            Ok(Some(vuln)) => {
                hydrated.insert(id.clone(), vuln);
            }
            Ok(None) => {}
            Err(e @ SourceError::RateLimited(_)) => return Err(e),
            Err(e) => {
                warn!(target: "4da::sources", id = %id, error = %e, "OSV live: advisory hydrate failed — skipping");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    }

    // Persist the hydrated advisories into `osv_advisories` so the LEDGER can build its
    // re-checkable vuln_match proof — run-cycle.mjs reads affected_ranges/fixed_versions from
    // this table to ground a published exposure. The live path is now the authoritative,
    // version-matched source for BOTH the engine matcher and the ledger.
    for vuln in hydrated.values() {
        upsert_hydrated_advisory(db, vuln);
    }

    // Build grounding-compatible items per matched dep; dedup (id, package, ecosystem).
    let mut seen: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    let mut items = Vec::new();
    for ((eco, name, ver), ids) in deps.iter().zip(per_dep_ids.iter()) {
        for id in ids {
            let Some(vuln) = hydrated.get(id) else {
                continue;
            };
            if !seen.insert((id.clone(), name.to_lowercase(), eco.clone())) {
                continue;
            }
            items.push(live_vuln_to_grounded_item(vuln, name, eco, ver));
        }
    }
    Ok(items)
}

/// Fetch one advisory's full record from OSV (`/v1/vulns/{id}`). `Ok(None)` = not found.
async fn hydrate_vuln(
    client: &reqwest::Client,
    id: &str,
) -> SourceResult<Option<OsvVulnerability>> {
    let response = client
        .get(format!("https://api.osv.dev/v1/vulns/{id}"))
        .header("User-Agent", "4DA-Developer-OS/1.0")
        .send()
        .await
        .map_err(|e| SourceError::Network(e.to_string()))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SourceError::RateLimited(
            "OSV vuln hydrate rate limited (429)".to_string(),
        ));
    }
    super::check_http_status(status, "OSV vuln hydrate")?;
    let vuln: OsvVulnerability = response
        .json()
        .await
        .map_err(|e| SourceError::Parse(e.to_string()))?;
    Ok(Some(vuln))
}

/// Gather the stack's auditable pinned dependencies that carry a concrete version, mapped to OSV
/// ecosystem ids and deduped by (ecosystem, package, version). Only versioned deps can be
/// version-queried against OSV; unversioned deps are left to the mirror/conservative path.
fn gather_versioned_auditable_deps(db: &crate::db::Database) -> Vec<(String, String, String)> {
    let mut deps = db.get_auditable_user_dependencies().unwrap_or_default();
    if let Ok(scanned) = db.get_auditable_scanned_dependencies() {
        deps.extend(scanned);
    }
    let mut seen: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    let mut out = Vec::new();
    for dep in deps {
        let Some(version) = dep
            .version
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let Some(eco) = dep_ecosystem_to_osv(&dep.ecosystem) else {
            continue;
        };
        let key = (
            eco.to_string(),
            dep.package_name.to_lowercase(),
            version.to_string(),
        );
        if seen.insert(key) {
            out.push((
                eco.to_string(),
                dep.package_name.clone(),
                version.to_string(),
            ));
        }
    }
    out
}

/// Map an ACE/DB ecosystem name to its OSV ecosystem id (`None` = unsupported by OSV).
fn dep_ecosystem_to_osv(ecosystem: &str) -> Option<&'static str> {
    match ecosystem.to_lowercase().as_str() {
        "npm" | "javascript" | "typescript" => Some("npm"),
        "rust" | "crates.io" => Some("crates.io"),
        "python" | "pypi" | "pip" => Some("PyPI"),
        "go" | "golang" => Some("Go"),
        "java" | "maven" => Some("Maven"),
        "ruby" | "rubygems" => Some("RubyGems"),
        "nuget" => Some("NuGet"),
        "packagist" => Some("Packagist"),
        "pub" => Some("Pub"),
        _ => None,
    }
}

/// Build a grounding-compatible `SourceItem` from a live-hydrated OSV advisory matched to a
/// pinned dep. Title LEADS with the package (`[id] pkg: summary`) so the ledger's grounding gate
/// verifies it names a pinned dependency; content + metadata carry installed/fixed versions and
/// severity.
fn live_vuln_to_grounded_item(
    vuln: &OsvVulnerability,
    package: &str,
    ecosystem: &str,
    installed: &str,
) -> SourceItem {
    let summary = vuln.summary.as_deref().unwrap_or("Security advisory");
    let title = format!("[{}] {package}: {summary}", vuln.id);
    let url = vuln
        .references
        .as_ref()
        .and_then(|refs| {
            refs.iter()
                .find(|r| r.ref_type == "ADVISORY")
                .or_else(|| refs.iter().find(|r| r.ref_type == "WEB"))
                .or_else(|| refs.first())
        })
        .map(|r| r.url.clone())
        .unwrap_or_else(|| format!("https://osv.dev/vulnerability/{}", vuln.id));
    let fixed = fixed_version_for(vuln, package, ecosystem);
    let (severity_type, cvss_score) = best_severity(vuln);

    let mut content_parts = vec![format!("{package} ({ecosystem})"), summary.to_string()];
    if let Some(details) = &vuln.details {
        content_parts.push(details.clone());
    }
    content_parts.push(format!("Installed: {installed}"));
    if let Some(fixed) = &fixed {
        content_parts.push(format!("Fixed in: {fixed}"));
    }
    let content = content_parts.join("\n");

    let metadata = serde_json::json!({
        "ecosystem": ecosystem,
        "package": package,
        "advisory_id": vuln.id,
        "installed_version": installed,
        "fixed_version": fixed,
        "cvss_score": cvss_score,
        "severity": severity_type,
        "is_version_confirmed": true,
        "manifest_grounded": true,
        "source_name": "osv",
        "published": vuln.published,
    });

    SourceItem::new("osv", &vuln.id, &title)
        .with_url(Some(url))
        .with_content(content)
        .with_metadata(metadata)
}

/// First concrete fixed version for `package` in this advisory (ECOSYSTEM/SEMVER ranges only;
/// GIT commit hashes and "-NA" unknown bounds are skipped).
fn fixed_version_for(vuln: &OsvVulnerability, package: &str, ecosystem: &str) -> Option<String> {
    for affected in vuln.affected.as_ref()? {
        let Some(pkg) = &affected.package else {
            continue;
        };
        if !pkg.name.eq_ignore_ascii_case(package) || !pkg.ecosystem.eq_ignore_ascii_case(ecosystem)
        {
            continue;
        }
        for range in affected.ranges.as_deref().unwrap_or_default() {
            if range.range_type != "SEMVER" && range.range_type != "ECOSYSTEM" {
                continue;
            }
            for event in range.events.as_deref().unwrap_or_default() {
                if let Some(fixed) = event
                    .as_object()
                    .and_then(|o| o.get("fixed"))
                    .and_then(serde_json::Value::as_str)
                {
                    let trimmed = fixed.trim();
                    if !trimmed.is_empty() && !trimmed.ends_with("-NA") && !trimmed.ends_with("-na")
                    {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Prefer CVSS_V3 severity; return (severity_type, numeric_score).
fn best_severity(vuln: &OsvVulnerability) -> (Option<String>, Option<f64>) {
    let Some(severities) = &vuln.severity else {
        return (None, None);
    };
    let chosen = severities
        .iter()
        .find(|s| s.severity_type == "CVSS_V3")
        .or_else(|| severities.first());
    match chosen {
        Some(s) => (Some(s.severity_type.clone()), s.score.parse::<f64>().ok()),
        None => (None, None),
    }
}

/// Merge an advisory's affected ranges by (package, ecosystem). An advisory can list the SAME
/// package in MULTIPLE affected entries (different version branches, e.g. next [13.4.6,15.5.16)
/// AND [16.0.0,16.2.5)). `osv_advisories` holds one row per (advisory, package), so we UNION every
/// entry's ranges — storing only one branch drops the one that matches the pinned version (the
/// completeness gap the live cycle exposed). Returns (package, ecosystem, ranges_json, fixed_json).
fn merged_ranges_by_package(
    vuln: &OsvVulnerability,
) -> Vec<(String, String, Option<String>, Option<String>)> {
    let mut by_pkg: std::collections::HashMap<(String, String), Vec<&OsvRange>> =
        std::collections::HashMap::new();
    let mut order: Vec<(String, String)> = Vec::new();
    for affected in vuln.affected.as_deref().unwrap_or_default() {
        let Some(pkg) = &affected.package else {
            continue;
        };
        let key = (pkg.name.clone(), pkg.ecosystem.clone());
        if !by_pkg.contains_key(&key) {
            order.push(key.clone());
        }
        let entry = by_pkg.entry(key).or_default();
        if let Some(ranges) = &affected.ranges {
            entry.extend(ranges.iter());
        }
    }
    order
        .into_iter()
        .map(|key| {
            let ranges = &by_pkg[&key];
            let ranges_json = if ranges.is_empty() {
                None
            } else {
                serde_json::to_string(ranges).ok()
            };
            let fixed: Vec<String> = ranges
                .iter()
                .filter_map(|r| r.events.as_deref())
                .flatten()
                .filter_map(|e| {
                    e.as_object()
                        .and_then(|o| o.get("fixed"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect();
            let fixed_json = if fixed.is_empty() {
                None
            } else {
                serde_json::to_string(&fixed).ok()
            };
            (key.0, key.1, ranges_json, fixed_json)
        })
        .collect()
}

/// Upsert a hydrated advisory's affected packages into `osv_advisories` (the table the ledger
/// reads affected_ranges/fixed_versions from to build its vuln_match proof, and the engine's own
/// matcher joins against). Best-effort: a failed upsert is logged, never fatal to surfacing.
fn upsert_hydrated_advisory(db: &crate::db::Database, vuln: &OsvVulnerability) {
    let (severity_type, cvss_score) = best_severity(vuln);
    let summary = vuln.summary.as_deref().unwrap_or("Security advisory");
    let source_url = vuln
        .references
        .as_ref()
        .and_then(|refs| {
            refs.iter()
                .find(|r| r.ref_type == "ADVISORY")
                .or_else(|| refs.iter().find(|r| r.ref_type == "WEB"))
                .or_else(|| refs.first())
        })
        .map(|r| r.url.clone());

    for (name, eco, ranges_json, fixed_json) in merged_ranges_by_package(vuln) {
        if let Err(e) = db.upsert_osv_advisory(
            &vuln.id,
            summary,
            vuln.details.as_deref(),
            &name,
            &eco,
            ranges_json.as_deref(),
            fixed_json.as_deref(),
            severity_type.as_deref(),
            cvss_score,
            source_url.as_deref(),
            vuln.published.as_deref(),
            vuln.modified.as_deref(),
            None, // OSV /v1/query excludes withdrawn advisories
        ) {
            warn!(target: "4da::sources", id = %vuln.id, error = %e, "OSV live: osv_advisories upsert failed");
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn vuln_with(affected: Vec<OsvAffected>) -> OsvVulnerability {
        OsvVulnerability {
            id: "GHSA-test".to_string(),
            summary: Some("s".to_string()),
            details: None,
            severity: Some(vec![OsvSeverity {
                severity_type: "CVSS_V3".to_string(),
                score: "5.3".to_string(),
            }]),
            affected: Some(affected),
            references: Some(vec![OsvReference {
                ref_type: "ADVISORY".to_string(),
                url: "https://example.com/a".to_string(),
            }]),
            published: Some("2026-06-15T00:00:00Z".to_string()),
            modified: None,
        }
    }
    fn pkg(name: &str, eco: &str) -> OsvPackage {
        OsvPackage {
            name: name.to_string(),
            ecosystem: eco.to_string(),
        }
    }
    fn semver(intro: &str, fixed: &str) -> OsvRange {
        OsvRange {
            range_type: "SEMVER".to_string(),
            events: Some(vec![
                serde_json::json!({ "introduced": intro }),
                serde_json::json!({ "fixed": fixed }),
            ]),
        }
    }

    #[test]
    fn test_merged_ranges_union_multi_branch() {
        // Real shape of GHSA-vfv6 (next): affected in TWO branches as separate entries. The merged
        // row must carry BOTH ranges so the ledger can match a pin in EITHER (the live-cycle gap).
        let v = vuln_with(vec![
            OsvAffected {
                package: Some(pkg("next", "npm")),
                ranges: Some(vec![semver("13.4.6", "15.5.16")]),
                versions: None,
            },
            OsvAffected {
                package: Some(pkg("next", "npm")),
                ranges: Some(vec![semver("16.0.0", "16.2.5")]),
                versions: None,
            },
        ]);
        let merged = merged_ranges_by_package(&v);
        assert_eq!(merged.len(), 1, "one merged row for (next, npm)");
        let (name, eco, ranges_json, fixed_json) = &merged[0];
        assert_eq!(name, "next");
        assert_eq!(eco, "npm");
        let rj = ranges_json.as_ref().unwrap();
        assert!(
            rj.contains("13.4.6") && rj.contains("16.0.0"),
            "BOTH branches present: {rj}"
        );
        let fj = fixed_json.as_ref().unwrap();
        assert!(
            fj.contains("15.5.16") && fj.contains("16.2.5"),
            "both fixes present: {fj}"
        );
    }

    #[test]
    fn test_live_vuln_to_grounded_item_title_and_fields() {
        let v = vuln_with(vec![OsvAffected {
            package: Some(pkg("js-yaml", "npm")),
            ranges: Some(vec![semver("0", "4.2.0")]),
            versions: None,
        }]);
        let item = live_vuln_to_grounded_item(&v, "js-yaml", "npm", "4.1.1");
        assert_eq!(item.source_type, "osv");
        assert_eq!(item.source_id, "GHSA-test");
        assert_eq!(item.title, "[GHSA-test] js-yaml: s");
        let after_id = item
            .title
            .trim_start_matches('[')
            .splitn(2, ']')
            .nth(1)
            .unwrap()
            .trim();
        assert_eq!(after_id.split([' ', ':']).next().unwrap(), "js-yaml");
        assert!(item.content.contains("js-yaml (npm)"));
        assert!(item.content.contains("Installed: 4.1.1"));
        assert!(item.content.contains("Fixed in: 4.2.0"));
        let md = item.metadata.unwrap();
        assert_eq!(md["fixed_version"], "4.2.0");
        assert_eq!(md["cvss_score"], 5.3);
        assert_eq!(md["manifest_grounded"], true);
    }

    #[test]
    fn test_fixed_version_skips_git_and_na() {
        let v = vuln_with(vec![OsvAffected {
            package: Some(pkg("torch", "PyPI")),
            ranges: Some(vec![
                OsvRange {
                    range_type: "GIT".to_string(),
                    events: Some(vec![serde_json::json!({"fixed": "7c35874deadbeef"})]),
                },
                semver("0", "2.2.0"),
            ]),
            versions: None,
        }]);
        assert_eq!(
            fixed_version_for(&v, "torch", "PyPI").as_deref(),
            Some("2.2.0")
        );
    }

    #[test]
    fn test_dep_ecosystem_to_osv() {
        assert_eq!(dep_ecosystem_to_osv("rust"), Some("crates.io"));
        assert_eq!(dep_ecosystem_to_osv("npm"), Some("npm"));
        assert_eq!(dep_ecosystem_to_osv("typescript"), Some("npm"));
        assert_eq!(dep_ecosystem_to_osv("pypi"), Some("PyPI"));
        assert_eq!(dep_ecosystem_to_osv("go"), Some("Go"));
        assert_eq!(dep_ecosystem_to_osv("cocoapods"), None);
    }
}
