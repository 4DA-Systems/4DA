// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV.dev source adapter — aggregated vulnerability intelligence
//!
//! Queries the Open Source Vulnerabilities database for security advisories
//! affecting user's installed dependencies. Covers all major ecosystems:
//! npm, crates.io, PyPI, Go, Maven, NuGet, RubyGems, Packagist, Pub.
//!
//! API docs: <https://osv.dev/docs/>
//!
//! Types, constants, and conversion helpers live in `osv_types`.

use super::osv_types::*;

use async_trait::async_trait;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

/// Max queries per `/v1/querybatch` request — the OSV API's documented batch
/// ceiling, and the same capacity the Preemption mirror lane batches at
/// (`osv::sync::MAX_BATCH_SIZE`). The content lane covers the FULL monitored
/// dep list per ecosystem up to this bound; the previous `.take(5)`/`.take(15)`
/// caps starved it down to an alphabetically-first prefix of deps forever
/// (scoring audit 2026-08-23).
const OSV_QUERYBATCH_MAX: usize = 1000;

// ============================================================================
// OSV Source
// ============================================================================

/// OSV.dev source — fetches aggregated open-source vulnerability data
pub struct OsvSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl OsvSource {
    /// Create a new OSV source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 50,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
        }
    }

    /// Fetch vulnerabilities for popular packages in an ecosystem, keeping the
    /// package each advisory was queried FOR (the user's own dependency — the
    /// grounding the scoring dep-matcher keys on).
    ///
    /// The OSV API requires a package name — cannot query by ecosystem alone.
    /// Uses well-known packages per ecosystem, augmented by ACE deps when available.
    async fn fetch_ecosystem_vulns(
        &self,
        ecosystem: &str,
    ) -> SourceResult<Vec<(OsvVulnerability, String)>> {
        // Get packages to check: ACE deps with versions when available. Cover the
        // FULL monitored dep list, bounded only by the batch endpoint's per-request
        // capacity — the old `.take(15)` pinned this lane to an alphabetical prefix.
        let ace_packages = crate::source_fetching::load_ace_packages_with_versions(ecosystem);
        let packages: Vec<(String, Option<String>)> = if !ace_packages.is_empty() {
            ace_packages.into_iter().take(OSV_QUERYBATCH_MAX).collect()
        } else {
            default_packages_for(ecosystem)
        };

        let mut all_vulns = Vec::new();
        for (pkg_name, pkg_version) in &packages {
            let body = OsvQueryRequest {
                package: Some(OsvPackage {
                    name: pkg_name.clone(),
                    ecosystem: ecosystem.to_string(),
                }),
                version: pkg_version.clone(),
            };

            let response = match self
                .client
                .post("https://api.osv.dev/v1/query")
                .json(&body)
                .header("User-Agent", "4DA-Developer-OS/1.0")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(target: "4da::sources", package = %pkg_name, error = %e, "OSV: network error");
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                break; // Stop querying this ecosystem
            }
            if !status.is_success() {
                continue; // Skip this package
            }

            if let Ok(result) = response.json::<OsvQueryResponse>().await {
                all_vulns.extend(
                    result
                        .vulns
                        .unwrap_or_default()
                        .into_iter()
                        .map(|v| (v, pkg_name.clone())),
                );
            }

            // Rate limit between per-package queries
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        Ok(all_vulns)
    }

    /// Discover advisory ids across multiple ecosystems using the batch endpoint.
    ///
    /// `/v1/querybatch` returns ONLY `{id, modified}` per vulnerability — it is a
    /// DISCOVERY endpoint, not a data endpoint (live-verified 2026-08-20). The
    /// previous implementation converted these id-only records straight into
    /// SourceItems, which is how every stored OSV item became a content-empty
    /// husk. Returns `(advisory_id, modified, queried_package)` triples for the
    /// caller to hydrate via [`Self::hydrate_vuln`].
    async fn fetch_batch_vuln_refs(
        &self,
        ecosystems: &[&str],
    ) -> SourceResult<Vec<(String, Option<String>, String)>> {
        // Build queries with actual package names + versions per ecosystem.
        // The FULL monitored dep list goes in — the old `.take(5)` locked
        // discovery to the same five alphabetically-first deps forever.
        let mut query_pairs: Vec<(OsvQueryRequest, String)> = Vec::new();
        for eco in ecosystems {
            let ace_packages = crate::source_fetching::load_ace_packages_with_versions(eco);
            let pkgs: Vec<(String, Option<String>)> = if ace_packages.is_empty() {
                default_packages_for(eco)
            } else {
                ace_packages
            };
            for (pkg, version) in pkgs {
                let query = OsvQueryRequest {
                    package: Some(OsvPackage {
                        name: pkg.clone(),
                        ecosystem: eco.to_string(),
                    }),
                    version,
                };
                query_pairs.push((query, pkg));
            }
        }

        // The response's `results` array is positional with `queries` — that
        // ordering is the ONLY thing tying an advisory id back to the package
        // it was found for, so the package list stays parallel to the request
        // WITHIN each chunk. Chunk at the batch endpoint's capacity, mirroring
        // the Preemption mirror lane's batching.
        let mut refs = Vec::new();
        let chunks = chunk_query_pairs(query_pairs, OSV_QUERYBATCH_MAX);
        let chunk_count = chunks.len();
        for (chunk_idx, (queries, query_packages)) in chunks.into_iter().enumerate() {
            let body = OsvBatchRequest { queries };

            let chunk_result: SourceResult<OsvBatchResponse> = async {
                let response = self
                    .client
                    .post("https://api.osv.dev/v1/querybatch")
                    .json(&body)
                    .header("User-Agent", "4DA-Developer-OS/1.0")
                    .send()
                    .await
                    .map_err(|e| SourceError::Network(e.to_string()))?;

                super::classify_http_status(response.status(), "OSV batch API")?;

                response
                    .json()
                    .await
                    .map_err(|e| SourceError::Parse(e.to_string()))
            }
            .await;

            let result = match chunk_result {
                Ok(r) => r,
                // First chunk failed — nothing learned yet, so surface the error
                // and let the caller run its sequential fallback.
                Err(e) if chunk_idx == 0 => return Err(e),
                Err(e) => {
                    // Later chunk failed after earlier ones landed: keep the
                    // partial discovery rather than discarding it — the caller's
                    // sequential fallback would re-query everything from zero.
                    warn!(
                        target: "4da::sources",
                        chunk = chunk_idx,
                        error = ?e,
                        "OSV batch: chunk failed, keeping refs discovered so far"
                    );
                    break;
                }
            };

            if let Some(results) = result.results {
                for (i, resp) in results.into_iter().enumerate() {
                    let pkg = query_packages.get(i).cloned().unwrap_or_default();
                    if let Some(vulns) = resp.vulns {
                        for v in vulns {
                            refs.push((v.id, v.modified, pkg.clone()));
                        }
                    }
                }
            }

            // Brief pause between batches to be respectful (same cadence as the
            // mirror lane's sync).
            if chunk_idx + 1 < chunk_count {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        Ok(refs)
    }

    /// Fetch the full advisory record for one id via `GET /v1/vulns/{id}`.
    ///
    /// This is the hydration step the batch endpoint requires. `None` on any
    /// network/HTTP/parse failure — the caller skips the advisory rather than
    /// storing an empty husk for it.
    async fn hydrate_vuln(&self, id: &str) -> Option<OsvVulnerability> {
        let url = format!("https://api.osv.dev/v1/vulns/{id}");
        let response = match self
            .client
            .get(&url)
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(target: "4da::sources", advisory = %id, error = %e, "OSV: hydration network error");
                return None;
            }
        };
        if !response.status().is_success() {
            warn!(target: "4da::sources", advisory = %id, status = %response.status(), "OSV: hydration HTTP error");
            return None;
        }
        match response.json::<OsvVulnerability>().await {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(target: "4da::sources", advisory = %id, error = %e, "OSV: hydration parse error");
                None
            }
        }
    }
}

impl Default for OsvSource {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Fallback packages + batch chunking
// ============================================================================

/// Well-known fallback packages per OSV ecosystem, used ONLY when ACE has no
/// dependency data for it yet (first run, no projects scanned). One list feeds
/// both the per-package query path and the batch discovery path — previously
/// the batch path had its own 3-ecosystem subset and silently skipped the
/// other six ecosystems entirely.
fn default_packages_for(ecosystem: &str) -> Vec<(String, Option<String>)> {
    let names: &[&str] = match ecosystem {
        "npm" => &["express", "react", "next", "lodash", "axios", "webpack"],
        "crates.io" => &["serde", "tokio", "reqwest", "axum", "clap", "anyhow"],
        "PyPI" => &[
            "django", "flask", "requests", "numpy", "fastapi", "pydantic",
        ],
        "Go" => &[
            "golang.org/x/net",
            "golang.org/x/crypto",
            "github.com/gin-gonic/gin",
        ],
        "Maven" => &[
            "org.apache.logging.log4j:log4j-core",
            "com.google.guava:guava",
        ],
        "NuGet" => &[
            "Newtonsoft.Json",
            "System.Text.Json",
            "Microsoft.Data.SqlClient",
        ],
        "RubyGems" => &["rails", "nokogiri", "rack"],
        "Packagist" => &[
            "laravel/framework",
            "symfony/http-kernel",
            "guzzlehttp/guzzle",
        ],
        "Pub" => &["http", "dio", "shared_preferences"],
        _ => &[],
    };
    names.iter().map(|s| (s.to_string(), None)).collect()
}

/// Split (query, package) pairs into batch-endpoint-sized chunks, keeping the
/// positional pairing intact per chunk. The pairing is what ties each response
/// row back to the dep it was queried for — a chunk boundary must never shift
/// it. Pure so the invariant is testable without a network.
fn chunk_query_pairs(
    pairs: Vec<(OsvQueryRequest, String)>,
    chunk_size: usize,
) -> Vec<(Vec<OsvQueryRequest>, Vec<String>)> {
    // `chunk_size` comes from OSV_QUERYBATCH_MAX (non-zero const); max(1) keeps
    // this total for any future caller rather than panicking in chunks().
    let chunk_size = chunk_size.max(1);
    let mut chunks = Vec::new();
    let mut remaining = pairs;
    while !remaining.is_empty() {
        let tail = remaining.split_off(remaining.len().min(chunk_size));
        chunks.push(remaining.into_iter().unzip());
        remaining = tail;
    }
    chunks
}

// ============================================================================
// ACE-based ecosystem filtering
// ============================================================================

/// Get the OSV ecosystem names for which the user has actual runtime
/// dependencies tracked by ACE. Returns an empty vec when no ACE data
/// is available (first run, no projects scanned).
fn get_active_osv_ecosystems() -> Vec<String> {
    // Maps (ACE lookup key, OSV ecosystem name).
    // ACE uses "npm", "rust", "pypi", etc.; OSV uses "npm", "crates.io", "PyPI", etc.
    let ecosystem_map: &[(&str, &str)] = &[
        ("npm", "npm"),
        ("rust", "crates.io"),
        ("pypi", "PyPI"),
        ("go", "Go"),
        ("maven", "Maven"),
        ("nuget", "NuGet"),
        ("rubygems", "RubyGems"),
        ("packagist", "Packagist"),
        ("pub", "Pub"),
    ];

    ecosystem_map
        .iter()
        .filter(|(ace_key, _)| {
            !crate::source_fetching::load_ace_packages_for_ecosystem(ace_key).is_empty()
        })
        .map(|(_, osv_eco)| osv_eco.to_string())
        .collect()
}

// ============================================================================
// Strict manifest mode — dependency-matched advisories
// ============================================================================

/// Strict manifest mode: surface only vulnerabilities that are **version-matched to the
/// stack's pinned dependencies**, via `osv::matching::get_matched_advisories`, instead of
/// the global popular-package query flow above. The advisory mirror is synced first when
/// stale so a single `--once` cycle can surface grounded vulns (the headless step-3 sync's
/// freshness gate then skips the just-synced mirror — no double download).
pub(super) async fn matched_advisories_as_items() -> Vec<SourceItem> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::sources", error = %e, "OSV strict mode: database unavailable");
            return Vec::new();
        }
    };

    let needs_sync = crate::osv::sync::needs_sync(&db, crate::osv::sync::osv_sync_max_age_hours())
        .unwrap_or(true);
    if needs_sync {
        if let Err(e) = crate::osv::sync::sync(&db).await {
            warn!(target: "4da::sources", error = %e, "OSV strict mode: advisory sync failed — matching against existing mirror");
        }
    }

    let matched = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let items: Vec<SourceItem> = matched
        .iter()
        .map(matched_advisory_to_source_item)
        .collect();
    info!(
        target: "4da::sources",
        count = items.len(),
        "OSV strict mode: surfaced manifest-matched advisories"
    );
    items
}

/// Build a `SourceItem` from a dependency-matched advisory. The title LEADS with the
/// affected package name (after the `[advisory-id]` prefix) so the ledger's grounding gate
/// — which grounds a vulnerability on the leading title token — verifies it names a pinned
/// dependency. e.g. `[GHSA-xxxx-yyyy-zzzz] axios: SSRF via crafted URL`.
fn matched_advisory_to_source_item(m: &crate::osv::types::MatchedAdvisory) -> SourceItem {
    let title = format!("[{}] {}: {}", m.advisory_id, m.package_name, m.summary);
    let url = m
        .source_url
        .clone()
        .or_else(|| Some(format!("https://osv.dev/vulnerability/{}", m.advisory_id)));

    let mut content_parts = vec![
        format!("{} ({})", m.package_name, m.ecosystem),
        m.summary.clone(),
    ];
    if let Some(details) = &m.details {
        content_parts.push(details.clone());
    }
    if let Some(installed) = &m.installed_version {
        content_parts.push(format!("Installed: {installed}"));
    }
    if let Some(fixed) = &m.fixed_version {
        content_parts.push(format!("Fixed in: {fixed}"));
    }
    let content = content_parts.join("\n");

    let metadata = serde_json::json!({
        "ecosystem": m.ecosystem,
        "package": m.package_name,
        "advisory_id": m.advisory_id,
        "installed_version": m.installed_version,
        "fixed_version": m.fixed_version,
        "cvss_score": m.cvss_score,
        "severity": m.severity_type,
        "is_version_confirmed": m.is_version_confirmed,
        "manifest_grounded": true,
        "source_name": "osv",
    });

    SourceItem::new("osv", &m.advisory_id, &title)
        .with_url(url)
        .with_content(content)
        .with_metadata(metadata)
}

// ============================================================================
// Source Trait Implementation
// ============================================================================

#[async_trait]
impl Source for OsvSource {
    fn source_type(&self) -> &'static str {
        "osv"
    }

    fn name(&self) -> &'static str {
        "OSV.dev"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Security,
            default_content_type: "security_advisory",
            default_multiplier: 1.30,
            label: "OSV",
            color_hint: "red",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode: route through deterministic dependency matching and
        // suppress the global popular-package query flow entirely.
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(super::osv_live::live_matched_advisories_as_items(&self.client).await);
        }

        // Determine which ecosystems the user actually has dependencies in
        let active_ecosystems = get_active_osv_ecosystems();
        let ecosystems: Vec<&str> = if active_ecosystems.is_empty() {
            // Fallback: no ACE data yet, query the two most common ecosystems
            vec!["npm", "crates.io"]
        } else {
            active_ecosystems.iter().map(|s| s.as_str()).collect()
        };

        info!(ecosystems = ?ecosystems, "Fetching OSV.dev vulnerabilities");

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for eco in &ecosystems {
            match self.fetch_ecosystem_vulns(eco).await {
                Ok(vulns) => {
                    info!(ecosystem = eco, count = vulns.len(), "Fetched OSV vulns");
                    for (vuln, pkg) in &vulns {
                        if vuln_is_informative(vuln) && seen_ids.insert(vuln.id.clone()) {
                            all_items.push(vuln_to_source_item(vuln, Some(pkg)));
                        }
                    }
                }
                Err(e) => {
                    warn!(ecosystem = eco, error = ?e, "Failed to fetch OSV vulns");
                }
            }
        }

        // Respect max_items limit
        all_items.truncate(self.config.max_items);

        info!(total = all_items.len(), "Total OSV items after dedup");
        Ok(all_items)
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode: deterministic dependency matching (same as the shallow
        // path); the global batch query is suppressed.
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(super::osv_live::live_matched_advisories_as_items(&self.client).await);
        }

        // Only query ecosystems the user has dependencies in
        let active_ecosystems = get_active_osv_ecosystems();
        let ecosystems: Vec<&str> = if active_ecosystems.is_empty() {
            // Fallback: no ACE data yet, use all defaults
            DEFAULT_ECOSYSTEMS.to_vec()
        } else {
            active_ecosystems.iter().map(|s| s.as_str()).collect()
        };

        info!(ecosystems = ?ecosystems, "Deep fetching OSV.dev vulnerabilities");

        // Discovery via the batch endpoint (ids only), then hydrate each id via
        // `GET /v1/vulns/{id}` before conversion. Skipping hydration is what
        // filled the corpus with content-empty husks — see fetch_batch_vuln_refs.
        let items: Vec<SourceItem> = match self.fetch_batch_vuln_refs(&ecosystems).await {
            Ok(mut refs) => {
                // Newest first, dedup by id (one advisory can hit several deps —
                // the first/newest queried package wins the title slot; the full
                // affected list still lands in content), then bound hydration:
                // one GET per advisory, capped by the caller's fetch budget.
                refs.sort_by(|a, b| b.1.cmp(&a.1));
                let mut seen_ids = std::collections::HashSet::new();
                refs.retain(|(id, _, _)| seen_ids.insert(id.clone()));
                let cap = items_per_category.clamp(1, self.config.max_items);
                refs.truncate(cap);

                let mut hydrated = Vec::new();
                for (id, _, pkg) in &refs {
                    if let Some(vuln) = self.hydrate_vuln(id).await {
                        if vuln_is_informative(&vuln) {
                            hydrated.push(vuln_to_source_item(&vuln, Some(pkg)));
                        }
                    }
                    // Rate limit between hydration requests
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                info!(
                    discovered = refs.len(),
                    hydrated = hydrated.len(),
                    "OSV deep fetch: hydrated batch-discovered advisories"
                );
                hydrated
            }
            Err(e) => {
                warn!(error = ?e, "Batch fetch failed, falling back to sequential");
                // Fallback: the per-package query endpoint returns FULL records,
                // so no hydration pass is needed here.
                let mut seen_ids = std::collections::HashSet::new();
                let mut fallback_items = Vec::new();
                for eco in &ecosystems {
                    match self.fetch_ecosystem_vulns(eco).await {
                        Ok(vulns) => {
                            for (vuln, pkg) in &vulns {
                                if vuln_is_informative(vuln) && seen_ids.insert(vuln.id.clone()) {
                                    fallback_items.push(vuln_to_source_item(vuln, Some(pkg)));
                                }
                            }
                        }
                        Err(e) => {
                            warn!(ecosystem = eco, error = ?e, "Failed to fetch OSV ecosystem");
                        }
                    }
                }
                fallback_items
            }
        };

        info!(total = items.len(), "Total deep OSV items after dedup");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // OSV items already have full content from the API response
        if !item.content.is_empty() {
            return Ok(item.content.clone());
        }

        // If content is somehow empty, try fetching the individual vuln
        let vuln_url = format!("https://api.osv.dev/v1/vulns/{}", item.source_id);

        let response = self
            .client
            .get(&vuln_url)
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(item.content.clone());
        }

        let vuln: OsvVulnerability = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let enriched = vuln_to_source_item(&vuln, None);
        Ok(enriched.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "osv_tests.rs"]
mod tests;
