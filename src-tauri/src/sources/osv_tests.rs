use super::*;

#[test]
fn test_matched_advisory_title_is_grounding_compatible() {
    // The ledger's grounding gate grounds a vulnerability on the LEADING title token
    // after stripping a `[id]` prefix. This test pins that contract: title must be
    // `[<advisory_id>] <package_name>: <summary>` and the item must carry source_type
    // "osv" with source_id = advisory_id.
    let m = crate::osv::types::MatchedAdvisory {
        advisory_id: "GHSA-xxxx-yyyy-zzzz".to_string(),
        summary: "SSRF via crafted URL".to_string(),
        details: Some("Long details".to_string()),
        package_name: "axios".to_string(),
        ecosystem: "npm".to_string(),
        installed_version: Some("1.6.0".to_string()),
        fixed_version: Some("1.6.8".to_string()),
        severity_type: Some("CVSS_V3".to_string()),
        cvss_score: Some(7.5),
        source_url: Some("https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string()),
        is_version_confirmed: true,
        project_paths: vec!["/stack".to_string()],
        published_at: Some("2026-01-01T00:00:00Z".to_string()),
        dependency_instances: vec![],
    };

    let item = matched_advisory_to_source_item(&m);
    assert_eq!(item.source_type, "osv");
    assert_eq!(item.source_id, "GHSA-xxxx-yyyy-zzzz");
    assert_eq!(
        item.title,
        "[GHSA-xxxx-yyyy-zzzz] axios: SSRF via crafted URL"
    );

    // Replicate the ledger's grounding extraction (grounding.mjs isGrounded, vuln branch):
    // strip the leading `[...]` id prefix, take the first token before whitespace/colon.
    let body = item.title.trim_start_matches('[');
    let after_id = body.splitn(2, ']').nth(1).unwrap().trim();
    let leading = after_id.split([' ', ':']).next().unwrap();
    assert_eq!(
        leading, "axios",
        "leading title token must be the pinned package"
    );

    // Content names the package+ecosystem and the fix, so the receipt is self-describing.
    assert!(item.content.contains("axios (npm)"));
    assert!(item.content.contains("Fixed in: 1.6.8"));
}

#[test]
fn test_osv_source_creation() {
    let source = OsvSource::new();
    assert_eq!(source.source_type(), "osv");
    assert_eq!(source.name(), "OSV.dev");
    assert!(source.config().enabled);
    assert_eq!(source.config().max_items, 50);
    assert_eq!(source.config().fetch_interval_secs, 3600);
}

#[test]
fn test_osv_source_default() {
    let source = OsvSource::default();
    assert_eq!(source.source_type(), "osv");
}

#[test]
fn test_default_packages_cover_all_nine_osv_ecosystems() {
    // Both fetch paths share ONE fallback list now. Every OSV ecosystem the
    // adapter monitors must have defaults — the batch path previously only
    // knew npm/crates.io/PyPI and silently skipped the other six.
    for eco in [
        "npm",
        "crates.io",
        "PyPI",
        "Go",
        "Maven",
        "NuGet",
        "RubyGems",
        "Packagist",
        "Pub",
    ] {
        assert!(
            !default_packages_for(eco).is_empty(),
            "no default packages for {eco}"
        );
    }
    // Defaults are versionless; unknown ecosystems get nothing.
    assert!(default_packages_for("npm").iter().all(|(_, v)| v.is_none()));
    assert!(default_packages_for("swift").is_empty());
    assert!(default_packages_for("made-up").is_empty());
}

fn pair(pkg: &str) -> (OsvQueryRequest, String) {
    (
        OsvQueryRequest {
            package: Some(OsvPackage {
                name: pkg.to_string(),
                ecosystem: "Go".to_string(),
            }),
            version: None,
        },
        pkg.to_string(),
    )
}

#[test]
fn test_chunk_query_pairs_preserves_positional_pairing() {
    // 2 chunks of 3 + a remainder of 1: within every chunk, query[i] must
    // still describe packages[i] — the positional pairing is the only link
    // from a batch response row back to the dep it was queried for.
    let pairs: Vec<_> = ["a", "b", "c", "d", "e", "f", "g"]
        .iter()
        .map(|p| pair(p))
        .collect();
    let chunks = chunk_query_pairs(pairs, 3);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].1, vec!["a", "b", "c"]);
    assert_eq!(chunks[1].1, vec!["d", "e", "f"]);
    assert_eq!(chunks[2].1, vec!["g"]);
    for (queries, packages) in &chunks {
        assert_eq!(queries.len(), packages.len());
        for (q, p) in queries.iter().zip(packages) {
            assert_eq!(&q.package.as_ref().unwrap().name, p);
        }
    }
}

#[test]
fn test_chunk_query_pairs_edges() {
    // Empty input -> no chunks (no empty batch request goes on the wire).
    assert!(chunk_query_pairs(Vec::new(), 1000).is_empty());
    // Under-capacity input -> a single chunk.
    let chunks = chunk_query_pairs(vec![pair("serde"), pair("tokio")], 1000);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].1, vec!["serde", "tokio"]);
    // A zero chunk_size caller must not panic or loop — clamped to 1.
    let chunks = chunk_query_pairs(vec![pair("serde"), pair("tokio")], 0);
    assert_eq!(chunks.len(), 2);
}

#[test]
fn test_chunk_query_pairs_never_drops_synthetic_go_deps() {
    // go.mod directives can surface deps literally named "stdlib" and
    // "toolchain" (synthetic Go entries). Nothing in the discovery path may
    // name-filter them out.
    let pairs = vec![pair("stdlib"), pair("toolchain"), pair("golang.org/x/net")];
    let chunks = chunk_query_pairs(pairs, 1000);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].1, vec!["stdlib", "toolchain", "golang.org/x/net"]);
}

#[test]
fn test_vuln_to_source_item_full() {
    let vuln = OsvVulnerability {
        id: "GHSA-xxxx-yyyy-zzzz".to_string(),
        summary: Some("XSS in React Router".to_string()),
        details: Some("A cross-site scripting vulnerability exists in...".to_string()),
        severity: Some(vec![OsvSeverity {
            severity_type: "CVSS_V3".to_string(),
            score: "7.5".to_string(),
        }]),
        affected: Some(vec![OsvAffected {
            package: Some(OsvPackage {
                name: "react-router".to_string(),
                ecosystem: "npm".to_string(),
            }),
            ranges: Some(vec![OsvRange {
                range_type: "SEMVER".to_string(),
                events: Some(vec![
                    serde_json::json!({"introduced": "0"}),
                    serde_json::json!({"fixed": "6.4.5"}),
                ]),
            }]),
            versions: None,
        }]),
        references: Some(vec![
            OsvReference {
                ref_type: "ADVISORY".to_string(),
                url: "https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string(),
            },
            OsvReference {
                ref_type: "WEB".to_string(),
                url: "https://example.com/blog".to_string(),
            },
        ]),
        published: Some("2026-03-15T10:00:00Z".to_string()),
        modified: Some("2026-03-20T12:00:00Z".to_string()),
    };

    let item = vuln_to_source_item(&vuln, None);

    assert_eq!(item.source_type, "osv");
    assert_eq!(item.source_id, "GHSA-xxxx-yyyy-zzzz");
    assert_eq!(item.title, "[GHSA-xxxx-yyyy-zzzz] XSS in React Router");
    assert_eq!(
        item.url,
        Some("https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string())
    );
    assert!(item.content.contains("XSS in React Router"));
    assert!(item.content.contains("CVSS_V3: 7.5"));
    assert!(item.content.contains("react-router (npm)"));
    assert!(item.content.contains("Fixed in: 6.4.5"));

    let metadata = item.metadata.unwrap();
    assert_eq!(metadata["severity"], "CVSS_V3: 7.5");
    assert_eq!(metadata["cvss_score"], 7.5);
    assert_eq!(metadata["published"], "2026-03-15T10:00:00Z");
    assert_eq!(metadata["modified"], "2026-03-20T12:00:00Z");
    assert_eq!(metadata["fixed_versions"], serde_json::json!(["6.4.5"]));
}

#[test]
fn test_vuln_to_source_item_minimal() {
    let vuln = OsvVulnerability {
        id: "OSV-2026-1234".to_string(),
        summary: None,
        details: None,
        severity: None,
        affected: None,
        references: None,
        published: None,
        modified: None,
    };

    let item = vuln_to_source_item(&vuln, None);

    assert_eq!(item.source_id, "OSV-2026-1234");
    assert_eq!(item.title, "[OSV-2026-1234] Security advisory");
    assert_eq!(
        item.url,
        Some("https://osv.dev/vulnerability/OSV-2026-1234".to_string())
    );
    assert!(item.content.contains("Severity: Unknown"));
    assert!(item.content.contains("Affected: Unknown"));
}

#[test]
fn test_batch_husk_is_never_informative() {
    // `/v1/querybatch` returns `{id, modified}` only. A record in that shape
    // must never be stored: it produced the "[ID] Security advisory /
    // Severity: Unknown / Affected: Unknown" husks that filled the OSV lane
    // with unscoreable noise (374/374 items, live audit 2026-08-20). This
    // pins the guard both fetch paths run before conversion.
    let husk = OsvVulnerability {
        id: "PYSEC-2026-3717".to_string(),
        summary: None,
        details: None,
        severity: None,
        affected: None,
        references: None,
        published: None,
        modified: Some("2026-08-19T00:00:00Z".to_string()),
    };
    assert!(!vuln_is_informative(&husk), "an id-only record is a husk");

    // Any single descriptive field rescues the record.
    let with_summary = OsvVulnerability {
        summary: Some("RCE in parser".to_string()),
        ..husk
    };
    assert!(vuln_is_informative(&with_summary));
}

#[test]
fn test_hydrated_title_leads_with_the_queried_package() {
    // The queried package is one of the USER'S manifest dependencies. The
    // title leads with it — the same `[id] pkg: summary` contract as the
    // strict-manifest path, which the ledger grounding gate and the scoring
    // dep-matcher's standalone-title match both key on.
    let vuln = OsvVulnerability {
        id: "GHSA-aaaa-bbbb-cccc".to_string(),
        summary: Some("Prototype pollution".to_string()),
        details: None,
        severity: None,
        affected: None,
        references: None,
        published: None,
        modified: None,
    };
    let item = vuln_to_source_item(&vuln, Some("lodash"));
    assert_eq!(
        item.title,
        "[GHSA-aaaa-bbbb-cccc] lodash: Prototype pollution"
    );
    let metadata = item.metadata.unwrap();
    assert_eq!(metadata["package"], "lodash");
}

#[test]
fn test_vuln_to_source_item_prefers_advisory_url() {
    let vuln = OsvVulnerability {
        id: "TEST-001".to_string(),
        summary: Some("Test".to_string()),
        details: None,
        severity: None,
        affected: None,
        references: Some(vec![
            OsvReference {
                ref_type: "WEB".to_string(),
                url: "https://web.example.com".to_string(),
            },
            OsvReference {
                ref_type: "ADVISORY".to_string(),
                url: "https://advisory.example.com".to_string(),
            },
        ]),
        published: None,
        modified: None,
    };

    let item = vuln_to_source_item(&vuln, None);
    assert_eq!(item.url, Some("https://advisory.example.com".to_string()));
}

#[test]
fn test_vuln_to_source_item_prefers_cvss_v3() {
    let vuln = OsvVulnerability {
        id: "TEST-002".to_string(),
        summary: Some("Test".to_string()),
        details: None,
        severity: Some(vec![
            OsvSeverity {
                severity_type: "CVSS_V2".to_string(),
                score: "5.0".to_string(),
            },
            OsvSeverity {
                severity_type: "CVSS_V3".to_string(),
                score: "8.1".to_string(),
            },
        ]),
        affected: None,
        references: None,
        published: None,
        modified: None,
    };

    let item = vuln_to_source_item(&vuln, None);
    assert!(item.content.contains("CVSS_V3: 8.1"));
}

#[test]
fn test_osv_json_parsing() {
    let json = r#"{
        "vulns": [
            {
                "id": "GHSA-test-0001",
                "summary": "SQL injection in ORM",
                "details": "A SQL injection vulnerability...",
                "severity": [
                    { "type": "CVSS_V3", "score": "9.8" }
                ],
                "affected": [
                    {
                        "package": { "name": "some-orm", "ecosystem": "npm" },
                        "ranges": [
                            {
                                "type": "SEMVER",
                                "events": [
                                    { "introduced": "0" },
                                    { "fixed": "3.2.1" }
                                ]
                            }
                        ]
                    }
                ],
                "references": [
                    { "type": "ADVISORY", "url": "https://github.com/advisories/GHSA-test-0001" }
                ],
                "published": "2026-03-10T00:00:00Z",
                "modified": "2026-03-12T00:00:00Z"
            }
        ]
    }"#;

    let response: OsvQueryResponse = serde_json::from_str(json).unwrap();
    let vulns = response.vulns.unwrap();
    assert_eq!(vulns.len(), 1);
    assert_eq!(vulns[0].id, "GHSA-test-0001");
    assert_eq!(vulns[0].summary.as_deref(), Some("SQL injection in ORM"));

    let severity = vulns[0].severity.as_ref().unwrap();
    assert_eq!(severity[0].severity_type, "CVSS_V3");
    assert_eq!(severity[0].score, "9.8");

    let affected = vulns[0].affected.as_ref().unwrap();
    assert_eq!(affected[0].package.as_ref().unwrap().ecosystem, "npm");
}

#[test]
fn test_osv_batch_response_parsing() {
    let json = r#"{
        "results": [
            {
                "vulns": [
                    { "id": "VULN-A", "summary": "Vuln A" }
                ]
            },
            {
                "vulns": [
                    { "id": "VULN-B", "summary": "Vuln B" },
                    { "id": "VULN-C", "summary": "Vuln C" }
                ]
            },
            {
                "vulns": null
            }
        ]
    }"#;

    let response: OsvBatchResponse = serde_json::from_str(json).unwrap();
    let results = response.results.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].vulns.as_ref().unwrap().len(), 1);
    assert_eq!(results[1].vulns.as_ref().unwrap().len(), 2);
    assert!(results[2].vulns.is_none());
}

#[test]
fn test_ecosystem_map_coverage() {
    // Verify all expected manifest files are mapped
    let manifests: Vec<&str> = ECOSYSTEM_MAP.iter().map(|(m, _)| *m).collect();
    assert!(manifests.contains(&"Cargo.toml"));
    assert!(manifests.contains(&"package.json"));
    assert!(manifests.contains(&"pyproject.toml"));
    assert!(manifests.contains(&"requirements.txt"));
    assert!(manifests.contains(&"go.mod"));
    assert!(manifests.contains(&"pom.xml"));
    assert!(manifests.contains(&"build.gradle"));
    assert!(manifests.contains(&"Gemfile"));
    assert!(manifests.contains(&".csproj"));
    assert!(manifests.contains(&"composer.json"));
    assert!(manifests.contains(&"pubspec.yaml"));
}

#[test]
fn test_multiple_affected_packages() {
    let vuln = OsvVulnerability {
        id: "MULTI-001".to_string(),
        summary: Some("Cross-ecosystem vuln".to_string()),
        details: None,
        severity: None,
        affected: Some(vec![
            OsvAffected {
                package: Some(OsvPackage {
                    name: "pkg-a".to_string(),
                    ecosystem: "npm".to_string(),
                }),
                ranges: None,
                versions: None,
            },
            OsvAffected {
                package: Some(OsvPackage {
                    name: "pkg-b".to_string(),
                    ecosystem: "PyPI".to_string(),
                }),
                ranges: None,
                versions: None,
            },
        ]),
        references: None,
        published: None,
        modified: None,
    };

    let item = vuln_to_source_item(&vuln, None);
    assert!(item.content.contains("pkg-a (npm)"));
    assert!(item.content.contains("pkg-b (PyPI)"));

    let metadata = item.metadata.unwrap();
    let pkgs = metadata["affected_packages"].as_array().unwrap();
    assert_eq!(pkgs.len(), 2);
}
