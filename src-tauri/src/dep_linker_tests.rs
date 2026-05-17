use super::*;

#[test]
fn test_matches_dep_in_title_word_boundary() {
    // "react" appears as a whole word — should return 0.50
    let result = matches_dep_in_title("react 19 released", "react");
    assert_eq!(result, Some(0.50));

    // At end of string
    let result = matches_dep_in_title("major update for react", "react");
    assert_eq!(result, Some(0.50));

    // Surrounded by punctuation (still word-boundary)
    let result = matches_dep_in_title("[react] v19 is out", "react");
    assert_eq!(result, Some(0.50));

    // Hyphen/underscore normalization
    let result = matches_dep_in_title("async-trait 0.2 released", "async_trait");
    assert_eq!(result, Some(0.50));
}

#[test]
fn test_matches_dep_in_title_substring() {
    let result = matches_dep_in_title("reactivity patterns in Vue", "react");
    assert_eq!(
        result, None,
        "substring-only title matches are too noisy for trust links"
    );

    let result = matches_dep_in_title("rediscovering the handcart", "redis");
    assert_eq!(
        result, None,
        "package names embedded in unrelated words must not match"
    );
}

#[test]
fn test_ambiguous_names_skipped() {
    // "image" is in the ambiguous list — classify_item_dep_match should return None
    // for title-heuristic tier (tier 3).
    let item = UnlinkedItem {
        id: 1,
        title: "image processing article".to_string(),
        content: String::new(),
        source_type: "hn".to_string(),
        content_type: None,
        source_id: "12345".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "image");
    assert!(result.is_none(), "Ambiguous dep 'image' should be skipped");
}

#[test]
fn test_short_names_skipped() {
    // Dep names < 4 chars are too generic for title heuristic.
    let item = UnlinkedItem {
        id: 2,
        title: "all about the arc reactor".to_string(),
        content: String::new(),
        source_type: "hn".to_string(),
        content_type: None,
        source_id: "99".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "arc");
    assert!(result.is_none(), "3-char dep 'arc' should be skipped");
}

#[test]
fn test_exact_registry_confidence() {
    // npm source_id matches dep name exactly — 0.95
    let item = UnlinkedItem {
        id: 3,
        title: "axios 1.7.0".to_string(),
        content: String::new(),
        source_type: "npm".to_string(),
        content_type: None,
        source_id: "axios".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "axios");
    assert_eq!(result, Some(("exact_registry", 0.95)));
}

#[test]
fn test_exact_registry_hyphen_normalization() {
    // crates_io source with underscore vs hyphen in dep name
    let item = UnlinkedItem {
        id: 4,
        title: "async-trait update".to_string(),
        content: String::new(),
        source_type: "crates_io".to_string(),
        content_type: None,
        source_id: "async-trait".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "async_trait");
    assert_eq!(result, Some(("exact_registry", 0.95)));
}

#[test]
fn test_advisory_match() {
    // OSV advisory mentioning tokio
    let item = UnlinkedItem {
        id: 5,
        title: "RUSTSEC-2023-0001: tokio race condition".to_string(),
        content: "Severity: HIGH\nAffected: tokio (crates.io)\nFixed in: 1.0.0".to_string(),
        source_type: "osv".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "RUSTSEC-2023-0001".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "tokio");
    assert_eq!(result, Some(("advisory", 0.90)));
}

#[test]
fn test_advisory_via_content_type() {
    // Generic source but content_type marks it as advisory
    let item = UnlinkedItem {
        id: 6,
        title: "Critical vulnerability in serde_json".to_string(),
        content: "Severity: HIGH\nAffected: serde-json (crates.io)".to_string(),
        source_type: "hn".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "40001".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "serde_json");
    assert_eq!(result, Some(("advisory", 0.90)));
}

#[test]
fn test_advisory_title_fallback_only_for_specific_names() {
    let item = UnlinkedItem {
        id: 61,
        title: "React / Next.js denial-of-service vulnerability".to_string(),
        content: "Security post without structured affected packages".to_string(),
        source_type: "rss".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "rss-1".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "react"),
        Some(("advisory", 0.75))
    );
    assert_eq!(classify_item_dep_match(&item, "path"), None);
}

#[test]
fn test_advisory_rejects_generic_substring_false_positives() {
    let item = UnlinkedItem {
        id: 62,
        title: "[CVE-2026-42461] Arcane discloses custom compose templates".to_string(),
        content: "Severity: HIGH\nAffected: arcane (go)".to_string(),
        source_type: "cve".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "CVE-2026-42461".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "os"),
        None,
        "short/generic dep names must not match substrings in advisory titles"
    );
    assert_eq!(
        classify_item_dep_match(&item, "arcane"),
        Some(("advisory", 0.90))
    );
}

#[test]
fn test_ambiguous_advisory_requires_structured_affected_package() {
    let imagemagick = UnlinkedItem {
        id: 63,
        title: "ImageMagick heap overflow in image handling".to_string(),
        content: "Severity: HIGH\nAffected: imagemagick (packagist)".to_string(),
        source_type: "cve".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "GHSA-image".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&imagemagick, "image"),
        None,
        "ambiguous package 'image' must not match ImageMagick advisories"
    );

    let rust_image = UnlinkedItem {
        id: 64,
        title: "image crate vulnerability".to_string(),
        content: "Severity: HIGH\nAffected: image (crates.io)".to_string(),
        source_type: "osv".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "RUSTSEC-image".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&rust_image, "image"),
        Some(("advisory", 0.90))
    );
}

#[test]
fn test_title_heuristic_general_source() {
    // HN article mentioning a dep by name (word boundary)
    let item = UnlinkedItem {
        id: 7,
        title: "Why we migrated from axios to fetch".to_string(),
        content: String::new(),
        source_type: "hn".to_string(),
        content_type: None,
        source_id: "40002".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "axios");
    assert_eq!(result, Some(("title_heuristic", 0.50)));
}

#[test]
fn test_no_match_returns_none() {
    let item = UnlinkedItem {
        id: 8,
        title: "Introduction to quantum computing".to_string(),
        content: String::new(),
        source_type: "hn".to_string(),
        content_type: None,
        source_id: "40003".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "tokio");
    assert!(result.is_none());
}

#[test]
fn test_registry_non_match() {
    // npm source but source_id doesn't match the dep we're checking
    let item = UnlinkedItem {
        id: 9,
        title: "lodash 5.0 released".to_string(),
        content: String::new(),
        source_type: "npm".to_string(),
        content_type: None,
        source_id: "lodash".to_string(),
        url: None,
    };
    // Title heuristic for "axios" won't match, and registry source_id is "lodash"
    let result = classify_item_dep_match(&item, "axios");
    assert!(result.is_none());
}

/// Regression test: proves the INSERT into source_item_dependencies works
/// against the real migrated schema. A previous bug used `dependency_name`
/// instead of `package_name`, causing a column mismatch at runtime.
#[test]
fn test_link_items_inserts_into_real_schema() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();

    // Insert a minimal source_items row
    conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, title, content, content_hash, embedding)
             VALUES (1, 'crates_io', 'serde', 'serde 1.0.200 released', '', 'hash1', zeroblob(1536))",
            [],
        )
        .expect("insert source_items");

    // Insert a minimal project_dependencies row so load_dependency_names can find it
    conn.execute(
            "INSERT INTO project_dependencies (package_name, project_path, manifest_type, language, is_dev, is_direct, project_relevance)
             VALUES ('serde', '/home/user/project', 'Cargo.toml', 'rust', 0, 1, 1.0)",
            [],
        )
        .expect("insert project_dependencies");

    // Build an UnlinkedItem that will match via exact_registry tier
    let items = vec![UnlinkedItem {
        id: 1,
        title: "serde 1.0.200 released".into(),
        content: String::new(),
        source_type: "crates_io".into(),
        content_type: None,
        source_id: "serde".into(),
        url: None,
    }];
    let dep_names = vec!["serde".to_string()];

    // This is the call that would fail with "table source_item_dependencies
    // has no column named dependency_name" if the INSERT used the wrong column.
    let linked = link_items_to_deps(&conn, &items, &dep_names)
        .expect("link_items_to_deps should succeed against real schema");
    assert_eq!(linked, 1, "Expected exactly 1 link row");

    // Verify the row exists with the correct columns
    let (pkg, eco, mt, conf): (String, Option<String>, String, f64) = conn
        .query_row(
            "SELECT package_name, ecosystem, match_type, confidence
                 FROM source_item_dependencies
                 WHERE source_item_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("should find the inserted row");

    assert_eq!(pkg, "serde");
    assert_eq!(eco.as_deref(), Some("crates.io"));
    assert_eq!(mt, "exact_registry");
    assert!((conf - 0.95).abs() < f64::EPSILON);

    // Verify total row count is exactly 1
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_item_dependencies", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_link_items_upgrades_existing_weak_row_with_exact_evidence() {
    use crate::test_utils::test_db;

    let db = test_db();
    let conn = db.conn.lock();
    conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, url, title, content, content_hash, embedding)
             VALUES (1, 'npm_registry', 'axios', 'https://www.npmjs.com/package/axios', 'axios 2.0 released', '', 'hash1', zeroblob(1536))",
            [],
        )
        .expect("insert source_item");
    conn.execute(
        "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (1, 'axios', 'npm', 'title_heuristic', 0.50)",
        [],
    )
    .expect("insert weak link");

    let items = vec![UnlinkedItem {
        id: 1,
        title: "axios 2.0 released".into(),
        content: String::new(),
        source_type: "npm_registry".into(),
        content_type: None,
        source_id: "axios".into(),
        url: Some("https://www.npmjs.com/package/axios".into()),
    }];
    let dep_names = vec!["axios".to_string()];

    let changed = link_items_to_deps(&conn, &items, &dep_names).expect("upsert exact link");
    assert_eq!(changed, 1);

    let (mt, conf, evidence, source_url): (String, f64, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT match_type, confidence, evidence_text, source_url
                 FROM source_item_dependencies
                 WHERE source_item_id = 1 AND package_name = 'axios'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(mt, "exact_registry");
    assert!((conf - 0.95).abs() < f64::EPSILON);
    assert!(evidence.unwrap().contains("Registry source"));
    assert_eq!(
        source_url.as_deref(),
        Some("https://www.npmjs.com/package/axios")
    );
}

#[test]
fn test_link_recent_items_revisits_existing_rows_for_upgrade() {
    use crate::test_utils::test_db;

    let db = test_db();
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO source_items
                (id, source_type, source_id, url, title, content, content_hash, embedding)
             VALUES
                (1, 'npm_registry', 'axios', 'https://www.npmjs.com/package/axios',
                 'axios 2.0 released', '', 'hash1', zeroblob(1536))",
            [],
        )
        .expect("insert recent source item");
        conn.execute(
            "INSERT INTO project_dependencies
                (package_name, project_path, manifest_type, language, is_dev, is_direct, project_relevance)
             VALUES
                ('axios', '/home/user/project', 'package.json', 'typescript', 0, 1, 1.0)",
            [],
        )
        .expect("insert project dependency");
        conn.execute(
            "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (1, 'axios', 'npm', 'title_heuristic', 0.50)",
            [],
        )
        .expect("insert weak existing link");
    }

    let changed = link_recent_items(&db).expect("reconcile recent links");
    assert_eq!(changed, 1);

    let conn = db.conn.lock();
    let (match_type, confidence, evidence_text, source_url): (
        String,
        f64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT match_type, confidence, evidence_text, source_url
             FROM source_item_dependencies
             WHERE source_item_id = 1 AND package_name = 'axios'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();

    assert_eq!(match_type, "exact_registry");
    assert!((confidence - 0.95).abs() < f64::EPSILON);
    assert!(evidence_text.unwrap().contains("Registry source"));
    assert_eq!(
        source_url.as_deref(),
        Some("https://www.npmjs.com/package/axios")
    );
}

#[test]
fn test_prune_invalid_links_removes_stale_false_positive() {
    use crate::test_utils::test_db;

    let db = test_db();
    {
        let conn = db.conn.lock();
        conn.execute(
                "INSERT INTO source_items (id, source_type, source_id, url, title, content, content_hash, embedding, content_type)
                 VALUES (1, 'cve', 'CVE-2026-42461', 'https://example.test/advisory',
                         '[CVE-2026-42461] Arcane discloses custom compose templates',
                         'Severity: HIGH\nAffected: arcane (go)', 'hash1', zeroblob(1536), 'security_advisory')",
                [],
            )
            .expect("insert source item");
        conn.execute(
            "INSERT INTO source_item_dependencies
                    (source_item_id, package_name, ecosystem, match_type, confidence)
                 VALUES (1, 'os', 'advisory', 'advisory', 0.90)",
            [],
        )
        .expect("insert stale false-positive link");
    }

    let pruned = prune_invalid_links(&db).expect("prune invalid links");
    assert_eq!(pruned, 1);

    let conn = db.conn.lock();
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_item_dependencies", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn test_is_registry_source() {
    assert!(is_registry_source("npm"));
    assert!(is_registry_source("npm_registry"));
    assert!(is_registry_source("crates_io"));
    assert!(is_registry_source("pypi"));
    assert!(is_registry_source("go_modules"));
    assert!(is_registry_source("go"));
    assert!(!is_registry_source("hn"));
    assert!(!is_registry_source("reddit"));
}

#[test]
fn test_canonical_npm_registry_exact_match() {
    let item = UnlinkedItem {
        id: 10,
        title: "axios 2.0 released".to_string(),
        content: String::new(),
        source_type: "npm_registry".to_string(),
        content_type: None,
        source_id: "axios".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "axios");
    assert_eq!(
        result,
        Some(("exact_registry", 0.95)),
        "npm_registry source should get exact_registry confidence"
    );
}

#[test]
fn test_canonical_go_modules_exact_match() {
    let item = UnlinkedItem {
        id: 11,
        title: "golang.org/x/net security update".to_string(),
        content: String::new(),
        source_type: "go_modules".to_string(),
        content_type: None,
        source_id: "golang.org/x/net".to_string(),
        url: None,
    };
    let result = classify_item_dep_match(&item, "golang.org/x/net");
    assert_eq!(
        result,
        Some(("exact_registry", 0.95)),
        "go_modules source should get exact_registry confidence"
    );
}

#[test]
fn test_is_advisory_source() {
    assert!(is_advisory_source("osv", None));
    assert!(is_advisory_source("cve", None));
    assert!(is_advisory_source("hn", Some("security_advisory")));
    assert!(is_advisory_source("reddit", Some("vulnerability_report")));
    assert!(!is_advisory_source("hn", None));
    assert!(!is_advisory_source("hn", Some("discussion")));
}

#[test]
fn test_advisory_blocks_title_fallback_when_affected_names_different_package() {
    // OSV advisory about "next" — the Affected: field names "next", not "react".
    // Even though "react" appears in the title, it must NOT match as advisory.
    let item = UnlinkedItem {
        id: 100,
        title: "React Server Components vulnerability in Next.js".to_string(),
        content: "Severity: HIGH\nAffected: next (npm)\nFixed in: 14.2.1".to_string(),
        source_type: "osv".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "GHSA-next-123".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "next"),
        Some(("advisory", 0.90)),
        "next IS in the Affected field — should match"
    );
    assert_eq!(
        classify_item_dep_match(&item, "react"),
        None,
        "react is NOT in the Affected field — title fallback must be blocked"
    );
}

#[test]
fn test_advisory_allows_title_fallback_when_no_affected_metadata() {
    // RSS security post with no structured Affected: field
    let item = UnlinkedItem {
        id: 101,
        title: "Critical XSS in axios request interceptors".to_string(),
        content: "A security vulnerability was discovered...".to_string(),
        source_type: "rss".to_string(),
        content_type: Some("security_advisory".to_string()),
        source_id: "rss-sec-1".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "axios"),
        Some(("advisory", 0.75)),
        "no Affected: metadata — title fallback should be allowed for specific names"
    );
}

#[test]
fn test_npm_registry_source_id_version_stripping() {
    let item = UnlinkedItem {
        id: 102,
        title: "react 19.2.5 published".to_string(),
        content: String::new(),
        source_type: "npm_registry".to_string(),
        content_type: None,
        source_id: "react@19.2.5".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "react"),
        Some(("exact_registry", 0.95)),
        "npm source_id 'react@19.2.5' must strip version and match 'react'"
    );
}

#[test]
fn test_npm_scoped_package_version_stripping() {
    let item = UnlinkedItem {
        id: 103,
        title: "@tanstack/react-query 5.0.0".to_string(),
        content: String::new(),
        source_type: "npm_registry".to_string(),
        content_type: None,
        source_id: "@tanstack/react-query@5.0.0".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "@tanstack/react-query"),
        Some(("exact_registry", 0.95)),
        "scoped npm source_id must strip version correctly"
    );
}

#[test]
fn test_crates_io_source_id_prefix_stripping() {
    let item = UnlinkedItem {
        id: 104,
        title: "serde 1.0.200 released".to_string(),
        content: String::new(),
        source_type: "crates_io".to_string(),
        content_type: None,
        source_id: "crate-serde".to_string(),
        url: None,
    };
    assert_eq!(
        classify_item_dep_match(&item, "serde"),
        Some(("exact_registry", 0.95)),
        "crates_io source_id 'crate-serde' must strip prefix and match 'serde'"
    );
}

#[test]
fn test_prune_reclassifies_stale_advisory_to_title_heuristic() {
    use crate::test_utils::test_db;

    let db = test_db();
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO source_items (id, source_type, source_id, url, title, content, content_hash, embedding, content_type)
             VALUES (1, 'osv', 'GHSA-next-123', NULL,
                     'React Server Components vulnerability in Next.js',
                     'Severity: HIGH\nAffected: next (npm)', 'hash1', zeroblob(1536), 'security_advisory')",
            [],
        ).expect("insert source item");
        // Stale row: "react" was marked as advisory 0.90 by old matcher,
        // but current classifier should reject it (Affected: names "next", not "react")
        conn.execute(
            "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (1, 'react', 'npm', 'advisory', 0.90)",
            [],
        )
        .expect("insert stale advisory link");
    }

    let reconciled = prune_invalid_links(&db).expect("prune/reconcile");
    assert!(reconciled >= 1, "stale advisory link should be pruned");

    let conn = db.conn.lock();
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_item_dependencies", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        remaining, 0,
        "react→next-advisory link should be deleted (no valid match)"
    );
}

#[test]
fn test_extract_registry_package_formats() {
    // npm with version
    assert_eq!(
        extract_registry_package("npm_registry", "react@19.2.5"),
        Some("react".to_string())
    );
    // npm without version
    assert_eq!(
        extract_registry_package("npm", "axios"),
        Some("axios".to_string())
    );
    // scoped npm with version
    assert_eq!(
        extract_registry_package("npm_registry", "@tanstack/react-query@5.0.0"),
        Some("@tanstack/react-query".to_string())
    );
    // crates_io with prefix
    assert_eq!(
        extract_registry_package("crates_io", "crate-serde"),
        Some("serde".to_string())
    );
    // crates_io without prefix (legacy)
    assert_eq!(
        extract_registry_package("crates", "tokio"),
        Some("tokio".to_string())
    );
    // non-registry
    assert_eq!(extract_registry_package("hn", "12345"), None);
}

// ============================================================================
// Word boundary: compound package names must NOT match partial segments
// ============================================================================

#[test]
fn test_title_rejects_compound_package_partial() {
    // "react" must NOT match in "react-native" — hyphen is package-internal
    assert_eq!(
        matches_dep_in_title("react-native 0.74 released", "react"),
        None,
        "react must not match inside react-native"
    );
    assert_eq!(
        matches_dep_in_title("upgrade react-query to v5", "react"),
        None,
        "react must not match inside react-query"
    );
    // underscore variant
    assert_eq!(
        matches_dep_in_title("async_std performance improvements", "async"),
        None,
        "async must not match inside async_std"
    );
    // dot-separated (e.g. Java packages, Go modules)
    assert_eq!(
        matches_dep_in_title("com.google.gson vulnerability", "google"),
        None,
        "google must not match inside com.google.gson"
    );
}

#[test]
fn test_title_matches_compound_package_exact() {
    // Full compound name SHOULD match
    assert_eq!(
        matches_dep_in_title("react-native 0.74 released", "react-native"),
        Some(0.50)
    );
    assert_eq!(
        matches_dep_in_title("upgrade to async_trait 0.2", "async-trait"),
        Some(0.50),
        "normalized hyphen variant should match underscore in title"
    );
}

#[test]
fn test_title_matches_when_first_occurrence_is_substring() {
    // "reaction about react" — first find("react") hits "reaction" (invalid),
    // but the fix iterates to find the standalone "react" later.
    assert_eq!(
        matches_dep_in_title("reaction to the new react release", "react"),
        Some(0.50),
        "should find standalone match even if first occurrence is a substring"
    );
}

#[test]
fn test_title_at_boundary_with_various_separators() {
    // Parentheses, brackets, colons, slashes are valid boundaries
    assert_eq!(
        matches_dep_in_title("(react) v19 released", "react"),
        Some(0.50)
    );
    assert_eq!(
        matches_dep_in_title("libraries: react, vue, angular", "react"),
        Some(0.50)
    );
    assert_eq!(
        matches_dep_in_title("lib/react/core update", "react"),
        Some(0.50)
    );
}

// ============================================================================
// Helper function unit tests
// ============================================================================

#[test]
fn test_extract_affected_package_name() {
    assert_eq!(extract_affected_package_name("tokio (crates.io)"), "tokio");
    assert_eq!(extract_affected_package_name("react (npm)"), "react");
    assert_eq!(
        extract_affected_package_name("  @tanstack/react-query (npm) "),
        "@tanstack/react-query"
    );
    // No ecosystem marker
    assert_eq!(extract_affected_package_name("serde"), "serde");
    assert_eq!(extract_affected_package_name("  lodash  "), "lodash");
}

#[test]
fn test_exact_package_name_match() {
    assert!(exact_package_name_match("react", "react"));
    assert!(exact_package_name_match("React", "react"));
    assert!(exact_package_name_match("async_trait", "async-trait"));
    assert!(exact_package_name_match("Async-Trait", "async_trait"));
    // Scoped packages: both @ stripped, then compared
    assert!(exact_package_name_match(
        "@tanstack/react-query",
        "@tanstack/react-query"
    ));
    assert!(exact_package_name_match(
        "tanstack/react-query",
        "@tanstack/react-query"
    ));
    // Non-matches
    assert!(!exact_package_name_match("react", "react-native"));
    assert!(!exact_package_name_match("serde", "serde-json"));
}

#[test]
fn test_normalize_package_for_exact_match() {
    assert_eq!(normalize_package_for_exact_match("React"), "react");
    assert_eq!(
        normalize_package_for_exact_match("async_trait"),
        "async-trait"
    );
    assert_eq!(
        normalize_package_for_exact_match("@tanstack/react-query"),
        "tanstack/react-query"
    );
    assert_eq!(normalize_package_for_exact_match("  Serde  "), "serde");
}

#[test]
fn test_infer_ecosystem() {
    let npm_item = UnlinkedItem {
        id: 1,
        title: String::new(),
        content: String::new(),
        source_type: "npm_registry".into(),
        content_type: None,
        source_id: String::new(),
        url: None,
    };
    assert_eq!(infer_ecosystem(&npm_item, "react"), Some("npm".into()));

    let crates_item = UnlinkedItem {
        id: 2,
        title: String::new(),
        content: String::new(),
        source_type: "crates_io".into(),
        content_type: None,
        source_id: String::new(),
        url: None,
    };
    assert_eq!(
        infer_ecosystem(&crates_item, "serde"),
        Some("crates.io".into())
    );

    let hn_item = UnlinkedItem {
        id: 3,
        title: String::new(),
        content: String::new(),
        source_type: "hn".into(),
        content_type: None,
        source_id: String::new(),
        url: None,
    };
    assert_eq!(infer_ecosystem(&hn_item, "react"), None);
}

#[test]
fn test_advisory_affected_status_edge_cases() {
    // Multiple packages
    let content = "Affected: lodash (npm), underscore (npm)";
    assert!(matches!(
        advisory_affected_status(content, "lodash"),
        AffectedStatus::Matched
    ));
    assert!(matches!(
        advisory_affected_status(content, "underscore"),
        AffectedStatus::Matched
    ));
    assert!(matches!(
        advisory_affected_status(content, "react"),
        AffectedStatus::MetadataExistsNoMatch
    ));

    // Unknown treated as no metadata
    let content = "Affected: unknown";
    assert!(matches!(
        advisory_affected_status(content, "react"),
        AffectedStatus::NoMetadata
    ));

    // Empty affected line treated as no metadata
    let content = "Affected: ";
    assert!(matches!(
        advisory_affected_status(content, "react"),
        AffectedStatus::NoMetadata
    ));

    // No Affected: line at all
    let content = "This is a security advisory about a buffer overflow.";
    assert!(matches!(
        advisory_affected_status(content, "react"),
        AffectedStatus::NoMetadata
    ));
}

#[test]
fn test_is_package_boundary_chars() {
    // These are package-internal (NOT boundaries)
    assert!(!is_package_boundary(b'-'));
    assert!(!is_package_boundary(b'_'));
    assert!(!is_package_boundary(b'.'));
    assert!(!is_package_boundary(b'@'));
    assert!(!is_package_boundary(b'a'));
    assert!(!is_package_boundary(b'Z'));
    assert!(!is_package_boundary(b'9'));

    // These ARE valid boundaries
    assert!(is_package_boundary(b' '));
    assert!(is_package_boundary(b'/'));
    assert!(is_package_boundary(b'('));
    assert!(is_package_boundary(b')'));
    assert!(is_package_boundary(b':'));
    assert!(is_package_boundary(b','));
    assert!(is_package_boundary(b'['));
    assert!(is_package_boundary(b']'));
}
