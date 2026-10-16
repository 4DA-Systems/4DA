// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! End-to-end plan tests for the parent hint, the runtime-over-dev tiebreak
//! and severity truth — real advisory ids, ranges and versions from the live
//! 2026-10-02 Preemption audit (project paths neutralised).

use crate::ace::scanner::{DependencyEdge, EdgeScope};
use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::{validate_item, EvidenceItem, Urgency};
use crate::test_utils::test_db;

const BRIDGE: &str = "d:/work/bridge/src-tauri";
const APP: &str = "d:/work/app/src-tauri";
const WEB: &str = "d:/work/web";
const MCP: &str = "d:/work/app/mcp-server";

fn plan(db: &Database) -> Vec<EvidenceItem> {
    crate::evidence::upgrade_plan::build_upgrade_plan_with_drops(db).0
}

fn advisory(db: &Database, id: &str, package: &str, eco: &str, fixed: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("{package} advisory {id}"),
        None,
        package,
        eco,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(cvss),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn instance(name: &str, version: &str, direct: bool, dev: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct: direct,
        is_dev: dev,
        scope: if dev { "dev" } else { "runtime" }.to_string(),
    }
}

fn cargo_edge(parent: &str, version: &str, child: &str) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: Some(version.to_string()),
        child: child.to_string(),
        child_version: None,
        scope: EdgeScope::Runtime,
    }
}

/// The bridge holds rmcp 1.7.0 only through victauri-plugin 0.8.4; the app
/// runs victauri-plugin 0.9.0 resolved against rmcp 3.4.1.
fn seed_rmcp(db: &Database) {
    db.store_transitive_dependency(BRIDGE, "rmcp", Some("1.7.0"), "rust", false)
        .unwrap();
    db.store_dependency_instances(
        BRIDGE,
        "rust",
        &[
            instance("rmcp", "1.7.0", false, false),
            instance("victauri-plugin", "0.8.4", true, false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        BRIDGE,
        "rust",
        &[
            cargo_edge("bridge", "0.1.0", "victauri-plugin"),
            cargo_edge("victauri-plugin", "0.8.4", "rmcp"),
        ],
    )
    .unwrap();
    db.store_dependency(APP, "victauri-plugin", Some("0.9.0"), "rust", false, None)
        .unwrap();
    db.store_dependency_instances(
        APP,
        "rust",
        &[
            instance("rmcp", "3.4.1", false, false),
            instance("victauri-plugin", "0.9.0", true, false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        APP,
        "rust",
        &[
            // A stale edge from an older install the rescan never deleted.
            cargo_edge("victauri-plugin", "0.8.5", "rmcp"),
            cargo_edge("victauri-plugin", "0.9.0", "rmcp"),
        ],
    )
    .unwrap();
    advisory(db, "GHSA-33f5-2c5q-wgwj", "rmcp", "crates.io", "2.0.0", 8.2);
    advisory(db, "GHSA-9pj6-vhgr-3mwh", "rmcp", "crates.io", "2.0.0", 7.5);
    advisory(db, "GHSA-9g45-5xwm-f3wc", "rmcp", "crates.io", "2.1.0", 6.8);
}

fn row<'a>(plan: &'a [EvidenceItem], package: &str) -> &'a EvidenceItem {
    plan.iter()
        .find(|i| i.affected_deps == vec![package.to_string()])
        .unwrap_or_else(|| {
            panic!(
                "{package} missing: {:?}",
                plan.iter().map(|i| &i.title).collect::<Vec<_>>()
            )
        })
}

#[test]
fn a_transitive_step_names_its_parent_and_the_newer_parent_seen_here() {
    let db = test_db();
    seed_rmcp(&db);
    let plan = plan(&db);
    let rmcp = row(&plan, "rmcp");
    validate_item(rmcp).expect("schema-valid");
    assert_eq!(
        rmcp.title,
        "Upgrade rmcp to >= 2.1.0 (major) via victauri-plugin — clears 3 advisories across 1 project"
    );
    assert!(
        rmcp.explanation
            .contains("rmcp 1.7.0 comes in through victauri-plugin 0.8.4 (a direct dependency)"),
        "{}",
        rmcp.explanation
    );
    assert!(
        rmcp.explanation
            .contains("victauri-plugin 0.9.0 resolves rmcp 3.4.1 (d:/work/app/src-tauri)"),
        "{}",
        rmcp.explanation
    );
    // The stale 0.8.5 edge is not an install: never cited.
    assert!(!rmcp.explanation.contains("0.8.5"), "{}", rmcp.explanation);
    let paths: Vec<&str> = rmcp
        .evidence
        .iter()
        .filter(|c| c.source == "dependency-path")
        .map(|c| c.title.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "rmcp 1.7.0 <- victauri-plugin 0.8.4",
            "victauri-plugin 0.9.0 -> rmcp 3.4.1"
        ]
    );
}

/// No recorded parent: the route rests on semver alone, and says so. rmcp
/// 1.7.0 -> 2.1.0 crosses a semver boundary, so a refresh cannot reach it.
#[test]
fn without_a_recorded_parent_the_route_is_the_semver_inference() {
    let db = test_db();
    db.store_transitive_dependency(BRIDGE, "rmcp", Some("1.7.0"), "rust", false)
        .unwrap();
    advisory(
        &db,
        "GHSA-9g45-5xwm-f3wc",
        "rmcp",
        "crates.io",
        "2.1.0",
        6.8,
    );
    let plan = plan(&db);
    let rmcp = row(&plan, "rmcp");
    assert!(
        rmcp.explanation.contains(
            "Fixed only upstream: rmcp 1.7.0 is transitive (the lockfile names no parent 4DA can \
             read); 2.1.0 is a semver-incompatible jump, so the dependency that pulls it in must \
             move to a release that admits it, or be replaced"
        ),
        "{}",
        rmcp.explanation
    );
    assert!(!rmcp.title.contains(" via "), "{}", rmcp.title);
}

/// Live 2026-10-02: both HIGH rows tied on urgency and the dev-only vitest
/// (CVSS 9.8, discounted to High for being dev-only) led the runtime rmcp
/// session leak (8.2, High) because it is fixable now. Runtime leads.
#[test]
fn a_runtime_exposure_outranks_a_dev_only_one_at_equal_urgency() {
    let db = test_db();
    seed_rmcp(&db);
    db.store_dependency(WEB, "vitest", Some("3.2.4"), "npm", true, None)
        .unwrap();
    db.store_dependency_instances(WEB, "npm", &[instance("vitest", "3.2.4", true, true)])
        .unwrap();
    advisory(&db, "GHSA-5xrq-8626-4rwp", "vitest", "npm", "3.2.6", 9.8);
    advisory(&db, "GHSA-82fw-gwwq-j7x9", "vitest", "npm", "4.1.11", 5.9);

    let plan = plan(&db);
    let pos = |p: &str| {
        plan.iter()
            .position(|i| i.affected_deps == vec![p.to_string()])
    };
    assert_eq!(
        row(&plan, "vitest").urgency,
        Urgency::High,
        "Critical, dev-only"
    );
    assert_eq!(row(&plan, "rmcp").urgency, Urgency::High);
    assert!(
        row(&plan, "vitest").explanation.contains("dev-only"),
        "the dev-only discount is labelled"
    );
    assert!(
        pos("rmcp") < pos("vitest"),
        "{:?}",
        plan.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
}

/// GHSA-hxh3-vqpv-xpqv (hono/jsx XSS, CVSS 4.7, fixed 4.13.7): the app's MCP
/// server runs hono 4.13.9 and must not be named; a dormant clone on 4.11.1
/// is, at the advisory's own MEDIUM — never Critical.
#[test]
fn a_patched_hono_is_not_listed_and_the_affected_copy_keeps_advisory_severity() {
    let db = test_db();
    db.store_dependency(MCP, "hono", Some("4.13.9"), "npm", false, None)
        .unwrap();
    db.store_dependency_instances(MCP, "npm", &[instance("hono", "4.13.9", true, false)])
        .unwrap();
    db.store_transitive_dependency(WEB, "hono", Some("4.11.1"), "npm", false)
        .unwrap();
    db.store_dependency_instances(WEB, "npm", &[instance("hono", "4.11.1", false, false)])
        .unwrap();
    advisory(&db, "GHSA-hxh3-vqpv-xpqv", "hono", "npm", "4.13.7", 4.7);

    // The matcher every Preemption surface (plan, alerts, free floor, brief)
    // reads: the patched copy is not an instance of the advisory at all.
    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap();
    let hxh3 = matches
        .iter()
        .find(|m| m.advisory_id == "GHSA-hxh3-vqpv-xpqv")
        .expect("the 4.11.1 copy matches");
    assert_eq!(hxh3.project_paths, vec![WEB.to_string()]);
    assert!(hxh3
        .dependency_instances
        .iter()
        .all(|d| d.installed_version.as_deref() != Some("4.13.9")));

    let plan = plan(&db);
    let hono = row(&plan, "hono");
    assert_eq!(hono.affected_projects, vec![WEB.to_string()]);
    assert_eq!(hono.urgency, Urgency::Medium);
    assert!(
        plan.iter()
            .all(|i| !i.affected_projects.contains(&MCP.to_string())),
        "the patched MCP server is named nowhere"
    );
}

/// Live 2026-10-02: a brace-expansion row whose parent range already admits
/// the fix also read "Seen in your projects: minimatch 10.2.6 resolves
/// brace-expansion 5.0.12" — a major parent jump nobody needs. When a refresh
/// is enough, no newer parent is offered.
#[test]
fn a_refresh_is_enough_so_no_newer_parent_is_offered() {
    use crate::osv::fix_path::{Basis, Refresh};
    use crate::osv::parent_hint::{ParentLink, Resolution, TransitiveRoutes};
    let link = ParentLink {
        project: WEB.to_string(),
        child_version: "1.1.15".to_string(),
        target: Some("1.1.21".to_string()),
        parent: "minimatch".to_string(),
        parent_version: "3.1.5".to_string(),
        parent_is_direct: false,
        requirement: Some("^1.1.7".to_string()),
        refresh: Refresh::Enough(Basis::Requirement),
        command: Some("npm update brace-expansion".to_string()),
        resolutions: vec![Resolution {
            parent_version: "10.2.6".to_string(),
            child_version: "5.0.12".to_string(),
            project: APP.to_string(),
        }],
        through: None,
        standing: Default::default(),
        override_hint: None,
    };
    let links = std::slice::from_ref(&link);
    let routes = TransitiveRoutes {
        links: vec![link.clone()],
        unlinked: Vec::new(),
    };
    assert_eq!(
        super::upstream_note("brace-expansion", &routes, false),
        "No manifest change needed: brace-expansion 1.1.15 comes in through minimatch 3.1.5, \
         whose requirement ^1.1.7 already admits 1.1.21 — a lockfile refresh is enough. \
         Run: `npm update brace-expansion`"
    );
    let cites = super::path_citations("brace-expansion", links);
    assert_eq!(cites.len(), 1, "{cites:?}");
    assert_eq!(super::single_parent(links), None);
}

const LEDGER: &str = "d:/work/ledger";

fn npm_edge(parent: &str, version: &str, child: &str, spec: &str) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: Some(version.to_string()),
        child: child.to_string(),
        child_version: Some(spec.to_string()),
        scope: EdgeScope::Runtime,
    }
}

fn registry_row(db: &Database, title: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
         VALUES ('npm_registry', ?1, ?1, '', ?1, X'00')",
        [title],
    )
    .unwrap();
}

/// uuid 3.4.0 reaches 4da-ledger through request 2.88.2 (`^3.3.2`), which
/// arrives through javascript-opentimestamps 0.4.5, the declared dependency
/// (live 2026-10-12, project path neutralised).
fn seed_uuid(db: &Database) {
    db.store_transitive_dependency(LEDGER, "uuid", Some("3.4.0"), "npm", false)
        .unwrap();
    db.store_dependency_instances(
        LEDGER,
        "npm",
        &[
            instance("javascript-opentimestamps", "0.4.5", true, false),
            instance("request", "2.88.2", false, false),
            instance("uuid", "3.4.0", false, false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        LEDGER,
        "npm",
        &[
            npm_edge("javascript-opentimestamps", "0.4.5", "request", "^2.88.0"),
            npm_edge("request", "2.88.2", "uuid", "^3.3.2"),
        ],
    )
    .unwrap();
    advisory(db, "GHSA-w5hq-g745-h8pq", "uuid", "npm", "11.1.1", 6.3);
}

/// Live 2026-10-12: "request itself must be updated" — but request 2.88.2
/// is its last release and deprecated, and the declared
/// javascript-opentimestamps 0.4.5 is its last release too. No update
/// exists, so the step says replace or override, never "must be updated".
#[test]
fn a_parent_at_its_newest_release_is_not_told_to_update() {
    let db = test_db();
    seed_uuid(&db);
    registry_row(&db, "npm: request v2.88.2");
    registry_row(&db, "[DEPRECATED] request");
    registry_row(&db, "npm: javascript-opentimestamps v0.4.5");
    let plan = plan(&db);
    let uuid = row(&plan, "uuid");
    validate_item(uuid).expect("schema-valid");
    assert!(
        uuid.explanation.contains(
            "uuid 3.4.0 comes in through request 2.88.2, whose requirement ^3.3.2 excludes 11.1.1 \
             — request 2.88.2 is already the newest request release 4DA has seen and is \
             deprecated, and it arrives through javascript-opentimestamps 0.4.5, a direct \
             dependency; javascript-opentimestamps 0.4.5 is already the newest \
             javascript-opentimestamps release 4DA has seen, so no javascript-opentimestamps \
             upgrade is known to admit it: replace javascript-opentimestamps, or force uuid >= \
             11.1.1 with an override in package.json (npm `overrides`, pnpm `pnpm.overrides` or \
             yarn `resolutions`)"
        ),
        "{}",
        uuid.explanation
    );
    assert!(
        !uuid.explanation.contains("must be updated"),
        "{}",
        uuid.explanation
    );
}

/// The declared parent HAS a newer release on record: upgrading it is the
/// move, with the check its requirement needs (not on disk).
#[test]
fn a_declared_parent_with_a_newer_release_is_the_upgrade() {
    let db = test_db();
    seed_uuid(&db);
    registry_row(&db, "npm: javascript-opentimestamps v0.4.5");
    registry_row(&db, "npm: javascript-opentimestamps v0.5.0");
    let plan = plan(&db);
    let uuid = row(&plan, "uuid");
    assert!(
        uuid.explanation.contains(
            "request arrives through javascript-opentimestamps 0.4.5, a direct dependency; \
             upgrade javascript-opentimestamps (javascript-opentimestamps 0.5.0 is the newest \
             release 4DA has seen; check that it admits uuid >= 11.1.1)"
        ),
        "{}",
        uuid.explanation
    );
}

/// Nothing on record about the parent: it must move, and no update is
/// claimed to exist.
#[test]
fn a_parent_with_no_registry_record_must_move_without_a_claimed_update() {
    let db = test_db();
    seed_uuid(&db);
    let plan = plan(&db);
    let uuid = row(&plan, "uuid");
    assert!(
        uuid.explanation.contains(
            "javascript-opentimestamps must move (4DA holds no record of a \
             javascript-opentimestamps release that admits uuid >= 11.1.1): upgrade \
             javascript-opentimestamps if one does, or replace it"
        ),
        "{}",
        uuid.explanation
    );
    assert!(
        !uuid.explanation.contains("must be updated"),
        "{}",
        uuid.explanation
    );
}

const GOAT: &str = "d:/work/nodegoat";

/// NodeGoat (audit E8): debug 2.2.0 ships bundled inside nyc 14.1.1, the
/// declared dev tool. `npm update debug` cannot move it and npm `overrides`
/// do not reach bundled dependencies: the step says the copy is bundled,
/// offers no override, and never prints the edge's internal marker.
#[test]
fn a_bundled_copy_names_its_owner_and_no_override() {
    let db = test_db();
    db.store_transitive_dependency(GOAT, "debug", Some("2.2.0"), "npm", false)
        .unwrap();
    db.store_dependency_instances(
        GOAT,
        "npm",
        &[
            instance("nyc", "14.1.1", true, true),
            instance("debug", "2.2.0", false, true),
        ],
    )
    .unwrap();
    let edges = crate::ace::scanner::ProjectScanner::parse_package_lock_edges(
        r#"{"lockfileVersion": 2, "packages": {
            "": { "devDependencies": { "nyc": "^14.0.0" } },
            "node_modules/nyc": { "version": "14.1.1", "dev": true },
            "node_modules/nyc/node_modules/debug": { "version": "2.2.0", "dev": true, "inBundle": true }
        }}"#,
    );
    db.store_dependency_edges(GOAT, "npm", &edges).unwrap();
    advisory(&db, "GHSA-gxpj-cx7g-858c", "debug", "npm", "2.6.9", 5.3);
    let plan = plan(&db);
    let debug = row(&plan, "debug");
    validate_item(debug).expect("schema-valid");
    let text = &debug.explanation;
    assert!(
        text.contains(
            "debug 2.2.0 comes in through nyc 14.1.1 (a direct dependency), which ships it \
             bundled inside its own package, so neither a lockfile refresh nor an override can \
             move it to 2.6.9"
        ),
        "{text}"
    );
    assert!(!text.contains("(bundled)"), "{text}");
    assert!(!text.contains("overrides"), "{text}");
    assert!(!text.contains("npm update"), "{text}");
}

/// End to end on a real project directory (audit E8): the plan reads the
/// project's package-lock.json from disk (Manager::Npm detection, the cached
/// tree, npm's lookup). needle 2.2.4 admits debug 2.2.0 by range but resolves
/// the hoisted 2.6.9, so it is no parent of 2.2.0; connect holds 2.2.0 by
/// `~2.2.0`; nyc DECLARES and bundles its 2.2.0. No "npm update debug".
#[test]
fn the_plan_reads_the_lockfile_tree_on_disk() {
    let dir = tempfile::Builder::new()
        .prefix("fx-e2e-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("tempdir");
    let project = dir.path().to_string_lossy().replace('\\', "/");
    if crate::project_inclusion::is_hard_excluded(&project) {
        eprintln!("SKIPPED the_plan_reads_the_lockfile_tree_on_disk: {project} is excluded from DB writes here");
        return;
    }
    let lock = r#"{"name": "app", "lockfileVersion": 2, "packages": {
        "": { "name": "app", "dependencies": { "needle": "2.2.4", "connect": "3.4.1", "nyc": "7.1.0" } },
        "node_modules/debug": { "version": "2.6.9" },
        "node_modules/needle": { "version": "2.2.4", "dependencies": { "debug": "^2.1.2" } },
        "node_modules/connect": { "version": "3.4.1", "dependencies": { "debug": "~2.2.0" } },
        "node_modules/connect/node_modules/debug": { "version": "2.2.0" },
        "node_modules/nyc": { "version": "7.1.0", "bundleDependencies": ["debug"], "dependencies": { "debug": "^2.2.0" } },
        "node_modules/nyc/node_modules/debug": { "version": "2.2.0", "inBundle": true }
    }}"#;
    std::fs::write(dir.path().join("package-lock.json"), lock).unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"needle": "2.2.4", "connect": "3.4.1", "nyc": "7.1.0"}}"#,
    )
    .unwrap();

    let db = test_db();
    db.store_transitive_dependency(&project, "debug", Some("2.2.0"), "npm", false)
        .unwrap();
    db.store_dependency_instances(
        &project,
        "npm",
        &[
            instance("needle", "2.2.4", true, false),
            instance("connect", "3.4.1", true, false),
            instance("nyc", "7.1.0", true, false),
            instance("debug", "2.6.9", false, false),
            instance("debug", "2.2.0", false, false),
        ],
    )
    .unwrap();
    let edges = crate::ace::scanner::ProjectScanner::parse_package_lock_edges(lock);
    db.store_dependency_edges(&project, "npm", &edges).unwrap();
    advisory(&db, "GHSA-gxpj-cx7g-858c", "debug", "npm", "2.6.9", 5.3);

    let plan = plan(&db);
    let debug = row(&plan, "debug");
    validate_item(debug).expect("schema-valid");
    let text = &debug.explanation;
    assert!(
        text.contains("comes in through connect 3.4.1 (a direct dependency), whose requirement ~2.2.0 excludes 2.6.9"),
        "{text}"
    );
    assert!(
        text.contains("comes in through nyc 7.1.0 (a direct dependency), which ships it bundled"),
        "{text}"
    );
    assert!(
        !text.contains("needle"),
        "needle resolves the hoisted 2.6.9: {text}"
    );
    assert!(!text.contains("npm update debug"), "{text}");
    assert!(!text.contains("(bundled)"), "{text}");
}

/// Audit E8, direct half: a direct row names the command that moves the
/// dependency where it is declared, read from the project's own manifest. The
/// tilde survives (plain `npm install x@v` wrote `^v` over `~1.2.5`, npm 10.9.2).
#[test]
fn a_direct_row_names_the_command_for_its_declaration() {
    let dir = tempfile::Builder::new()
        .prefix("c2-e2e-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("tempdir");
    let project = dir.path().to_string_lossy().replace('\\', "/");
    if crate::project_inclusion::is_hard_excluded(&project) {
        eprintln!("SKIPPED a_direct_row_names_the_command_for_its_declaration: {project} is excluded from DB writes here");
        return;
    }
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3, "packages": {"": {"dependencies": {"minimist": "~1.2.5"}}, "node_modules/minimist": {"version": "1.2.5"}}}"#,
    )
    .unwrap();
    let db = test_db();
    db.store_dependency(&project, "minimist", Some("1.2.5"), "npm", false, None)
        .unwrap();
    db.store_dependency_instances(
        &project,
        "npm",
        &[instance("minimist", "1.2.5", true, false)],
    )
    .unwrap();
    advisory(&db, "GHSA-xvch-5gv4-984h", "minimist", "npm", "1.2.6", 9.8);
    let plan = plan(&db);
    let row = row(&plan, "minimist");
    validate_item(row).expect("schema-valid");
    assert!(
        row.explanation.contains(
            "Fixable now via a direct dependency bump: `npm install \"minimist@~1.2.6\"`"
        ),
        "{}",
        row.explanation
    );
}
