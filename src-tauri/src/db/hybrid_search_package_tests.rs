// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the package lane: the `Affected:` parser, the foreign-ecosystem test,
//! the merge, and the audit L8 query ("sharp vulnerability") on a real index.

use super::*;
use crate::db::Database;
use crate::test_utils::{seed_embedding, test_db};

fn affected(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(p, e)| ((*p).to_string(), (*e).to_string()))
        .collect()
}

#[test]
fn parses_the_affected_line_the_adapters_write() {
    let osv = "Prototype Pollution in lodash\n\nSeverity: High\n\
               Affected: lodash (npm), lodash-rails (RubyGems)\nFixed in: 4.17.5";
    assert_eq!(
        parse_affected(osv),
        affected(&[("lodash", "npm"), ("lodash-rails", "rubygems")])
    );
    // A version range after the ecosystem is ignored.
    assert_eq!(
        parse_affected("Affected: hono (npm) < 4.12.0\nFixed in: 4.12.0"),
        affected(&[("hono", "npm")])
    );
    // Scoped names lose the leading `@`; underscores fold into hyphens.
    assert_eq!(
        parse_affected("Affected: @orpc/zod (npm), serde_json (crates.io)"),
        affected(&[("orpc/zod", "npm"), ("serde-json", "crates.io")])
    );
    assert!(parse_affected("Affected: Unknown").is_empty());
    assert!(parse_affected("Affected: ").is_empty());
    assert!(parse_affected("no metadata at all").is_empty());
}

#[test]
fn named_package_comes_from_the_affected_list_only() {
    let next = affected(&[("next", "npm")]);
    let sharp = affected(&[("sharp", "npm")]);
    let named = vec!["sharp".to_string()];
    assert_eq!(
        affected_named_package(&sharp, &named).as_deref(),
        Some("sharp")
    );
    // A Next.js advisory whose prose mentions sharp is not a sharp advisory.
    assert_eq!(affected_named_package(&next, &named), None);
    assert_eq!(affected_named_package(&sharp, &[]), None);
}

#[test]
fn foreign_ecosystem_means_none_of_the_affected_ecosystems_is_installed() {
    let installed = [Ecosystem::Npm, Ecosystem::Cargo];
    let nuget = affected(&[("Microsoft.NETCore.App.Runtime.win-x64", "nuget")]);
    assert!(is_foreign_ecosystem(&nuget, &installed));
    assert!(is_foreign_ecosystem(
        &affected(&[("getkirby/cms", "composer")]),
        &installed
    ));
    // An ecosystem 4DA does not read can never hold an installed dependency.
    assert!(is_foreign_ecosystem(
        &affected(&[("actions/checkout", "github actions")]),
        &installed
    ));
    // One installed ecosystem in the list is enough to keep it.
    assert!(!is_foreign_ecosystem(
        &affected(&[("lodash", "npm"), ("lodash-rails", "rubygems")]),
        &installed
    ));
    assert!(!is_foreign_ecosystem(
        &affected(&[("zip", "crates.io")]),
        &installed
    ));
    assert!(!is_foreign_ecosystem(&[], &installed));
}

fn cand(id: i64) -> Candidate {
    Candidate {
        id,
        title: format!("item {id}"),
        content: String::new(),
        source_type: "cve".to_string(),
        url: None,
        created_at: None,
        distance: None,
    }
}

#[test]
fn merge_keeps_limit_and_never_evicts_exact_or_earlier_pins() {
    let mut base: Vec<HybridSearchResult> = (1..=5).map(|i| cand(i).to_result()).collect();
    base[0].exact_title = true;
    let lane: Vec<Candidate> = vec![cand(3), cand(100), cand(101), cand(102), cand(103)];
    let merged = merge_package_lane(base, &lane, 5);
    let ids: Vec<i64> = merged.iter().map(|r| r.item_id).collect();
    assert_eq!(merged.len(), 5);
    assert_eq!(ids[0], 1, "exact pin stays");
    assert!(
        ids.contains(&3),
        "lane item already present keeps its place"
    );
    // Three slots (2, 4, 5) were free to replace; the fourth new pin finds none.
    for id in [100, 101, 102] {
        assert!(ids.contains(&id), "pin {id} merged");
    }
    assert!(!ids.contains(&103));
    // Below the limit, pins are appended.
    let merged = merge_package_lane(Vec::new(), &[cand(7)], 30);
    assert_eq!(merged.len(), 1);
}

fn install(db: &Database, ecosystem: &str, name: &str) {
    db.conn
        .lock()
        .execute(
            "INSERT INTO dependency_instances (project_path, ecosystem, package_name, version)
             VALUES ('/p', ?1, ?2, '1.0.0')",
            rusqlite::params![ecosystem, name],
        )
        .expect("install dep");
}

fn advisory(
    db: &Database,
    key: &str,
    title: &str,
    prose: &str,
    affected: &str,
    emb: &[f32],
) -> i64 {
    let content = format!("{prose}\n\nSeverity: HIGH\nAffected: {affected}\n");
    db.upsert_source_item("cve", key, None, title, &content, emb)
        .expect("upsert advisory")
}

/// Audit L8 on a real FTS5 + sqlite-vec index. The query embedding sits on the
/// Next.js and .NET advisories (as the live corpus's did), and "vulnerability"
/// is absent from one sharp advisory, so neither leg ranks sharp first.
#[test]
fn sharp_vulnerability_marks_sharp_advisories_and_foreign_ones() {
    let db = test_db();
    install(&db, "npm", "sharp");
    install(&db, "npm", "next");
    install(&db, "crates.io", "tokio");
    let probe = seed_embedding("image optimization vulnerability");

    let next_a = advisory(
        &db,
        "next-a",
        "Next.js: Remote Code Execution in Image Optimization API",
        "A vulnerability in libheif used by `sharp` which Next.js uses for images.",
        "next (npm)",
        &probe,
    );
    let dotnet = advisory(
        &db,
        "dotnet",
        ".NET Elevation of Privilege Vulnerability",
        "Microsoft is releasing this security advisory for a vulnerability.",
        "Microsoft.NETCore.App.Runtime.win-x64 (nuget)",
        &probe,
    );
    let sharp_a = advisory(
        &db,
        "sharp-a",
        "sharp : Vulnerability in librsvg dependency",
        "Please upgrade sharp to the latest version.",
        "sharp (npm)",
        &seed_embedding("librsvg"),
    );
    let sharp_b = advisory(
        &db,
        "sharp-b",
        "sharp: Issues in libheif",
        "Critical issues fixed in the upstream libheif dependency of sharp.",
        "sharp (npm)",
        &seed_embedding("libheif"),
    );
    for i in 0..60 {
        db.upsert_source_item(
            "rss",
            &format!("crowd-{i}"),
            None,
            &format!("image tooling {i}"),
            "a vulnerability write-up",
            &probe,
        )
        .expect("upsert crowd");
    }

    let results = db.hybrid_search("sharp vulnerability", &probe, 30, 0.4, 0.6);
    assert!(results.len() <= 30);
    let get = |id: i64| results.iter().find(|r| r.item_id == id);
    for id in [sharp_a, sharp_b] {
        let r = get(id).expect("sharp advisory must be in the results");
        assert_eq!(r.query_package.as_deref(), Some("sharp"));
        assert!(!r.foreign_ecosystem);
    }
    let next = get(next_a).expect("Next.js advisory still found");
    assert!(
        next.query_package.is_none(),
        "mentions sharp, does not affect it"
    );
    assert!(!next.foreign_ecosystem);
    let net = get(dotnet).expect(".NET advisory still found");
    assert!(net.foreign_ecosystem, "no NuGet dependencies installed");

    // A query that names no installed package is left exactly as it was.
    let topic = db.hybrid_search("image optimization", &probe, 30, 0.4, 0.6);
    assert!(topic
        .iter()
        .all(|r| r.query_package.is_none() && !r.foreign_ecosystem));
}

#[test]
fn package_context_reads_both_inventories_and_skips_unknown_terms() {
    let db = test_db();
    install(&db, "npm", "@tauri-apps/api");
    db.conn
        .lock()
        .execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem)
             VALUES ('/p', 'serde_json', 'rust')",
            [],
        )
        .expect("manifest dep");
    let conn = db.conn.lock();
    let ctx = package_context(&conn, "@tauri-apps/api serde-json vulnerability");
    assert_eq!(ctx.named, vec!["@tauri-apps/api", "serde-json"]);
    assert!(ctx.installed_ecosystems.contains(&Ecosystem::Npm));
    assert!(ctx.installed_ecosystems.contains(&Ecosystem::Cargo));
    assert!(!package_context(&conn, "rust async runtime").is_active());
    // Naming an installed package without asking about security is a topic query.
    assert!(!package_context(&conn, "serde-json streaming parser").is_active());
    assert!(package_context(&conn, "serde-json CVE").is_active());
    assert!(!package_context(&conn, "").is_active());
}
