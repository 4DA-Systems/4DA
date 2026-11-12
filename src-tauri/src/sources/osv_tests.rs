use super::*;
use crate::sources::osv_types::*;

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
        aliases: vec![],
        severity_label: None,
        cwe_ids: Vec::new(),
        severity_from: None,
        informational: None,
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
    // The scoring pipeline reads the CVSS score from this line.
    assert!(
        item.content.contains("Severity: CVSS_V3: 7.5"),
        "{}",
        item.content
    );
}

/// P2 (audit 2026-11-11): the desktop OSV source used to send every
/// dependency name and installed version to `api.osv.dev` each hour. It now
/// reads the local mirror, so this module names no host at all; strict
/// manifest mode's live query lives in `osv_live` (ledger only).
#[test]
fn the_desktop_osv_source_contacts_no_host() {
    let source = include_str!("osv.rs");
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("https://api.osv.dev"),
        "osv.rs must not call the OSV API"
    );
    assert!(!code.contains(".post("), "osv.rs must not POST anything");
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

/// Mirror lane, end to end on the real record: GO-2026-6603 names `stdlib` (listed FIRST in
/// affected[]) and golang.org/x/net <0.60.0. A go.mod `go 1.22` directive is inventoried as
/// stdlib 1.22.0, so the matcher returns a row for EACH package; strict mode must still
/// surface one item, named for the pinned module (see `osv_collapse`).
#[test]
fn mirror_lane_surfaces_one_go_item_named_for_the_pinned_module() {
    use crate::test_utils::test_db;

    let db = test_db();
    let project = "/stack/go-service";
    db.store_dependency(project, "stdlib", Some("1.22.0"), "go", false, None)
        .unwrap();
    db.store_dependency(
        project,
        "golang.org/x/net",
        Some("0.21.0"),
        "go",
        false,
        None,
    )
    .unwrap();
    let vuln: crate::osv::types::Vulnerability = serde_json::from_str(include_str!(
        "../../tests/conformance/osv/GO-2026-6603.json"
    ))
    .unwrap();
    crate::osv::sync::store_vulnerability(&db, &vuln, &mut std::collections::HashSet::new())
        .unwrap();

    let matched = crate::osv::matching::get_matched_advisories(&db).unwrap();
    let mut packages: Vec<&str> = matched.iter().map(|m| m.package_name.as_str()).collect();
    packages.sort_unstable();
    assert_eq!(packages, ["golang.org/x/net", "stdlib"], "both rows match");

    let items = crate::sources::osv_collapse::one_item_per_advisory(
        matched
            .iter()
            .map(matched_advisory_to_source_item)
            .collect(),
    );
    let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(items.len(), 1, "one item per advisory id, got {titles:?}");
    assert!(
        items[0]
            .title
            .starts_with("[GO-2026-6603] golang.org/x/net: "),
        "{:?}",
        items[0].title
    );
    assert!(
        items[0].content.contains("Fixed in: 0.60.0"),
        "{}",
        items[0].content
    );
}
