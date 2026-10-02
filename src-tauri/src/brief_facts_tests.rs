// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `brief_facts` (extracted via #[path]). Every rule is pinned to
//! the live case that motivated it (2026-10-01 audit).

use super::*;

fn link(direct: &str, version: &str, req: Option<&str>) -> ParentLink {
    ParentLink {
        direct: direct.to_string(),
        direct_version: version.to_string(),
        requirement: req.map(str::to_string),
    }
}

#[test]
fn semver_compatibility_follows_the_caret_rule() {
    assert_eq!(
        semver_compatible("1.7.0", "2.1.0"),
        Some(false),
        "rmcp 1 -> 2"
    );
    assert_eq!(semver_compatible("1.7.0", "1.9.3"), Some(true));
    assert_eq!(
        semver_compatible("0.23.39", "0.23.45"),
        Some(true),
        "rustls patch"
    );
    assert_eq!(
        semver_compatible("0.8.4", "0.9.0"),
        Some(false),
        "0.x minor is breaking"
    );
    assert_eq!(semver_compatible("v3.2.4", "3.2.6"), Some(true));
    assert_eq!(semver_compatible("garbage", "1.0.0"), None);
}

#[test]
fn npm_requirements_are_read_with_npm_semantics() {
    // A bare npm version is an EXACT pin (Cargo would read it as ^).
    assert_eq!(requirement_admits("5.28.4", "5.28.5"), Some(false));
    assert_eq!(requirement_admits("5.28.4", "5.28.4"), Some(true));
    assert_eq!(requirement_admits("^1.1.7", "1.1.12"), Some(true));
    assert_eq!(requirement_admits("^1.1.7", "5.0.12"), Some(false));
    assert_eq!(requirement_admits("~7.4.0", "7.4.9"), Some(true));
    assert_eq!(requirement_admits(">=1.0.0 <2.0.0", "1.9.0"), Some(true));
    assert_eq!(requirement_admits(">=1.0.0 <2.0.0", "2.0.0"), Some(false));
    assert_eq!(requirement_admits("^2.0.0 || ^3.0.0", "3.4.1"), Some(true));
    assert_eq!(requirement_admits("1.x", "1.4.0"), Some(true));
    assert_eq!(requirement_admits("*", "9.9.9"), Some(true));
    assert_eq!(
        requirement_admits("workspace:*", "1.0.0"),
        None,
        "unreadable"
    );
    // A space after the operator is npm syntax (6 live rows, 2026-10-02).
    assert_eq!(requirement_admits(">= 4.21.0", "5.0.0"), Some(true));
    assert_eq!(requirement_admits(">= 1.0.0 < 2.0.0", "2.1.0"), Some(false));
    // Hyphen ranges: full bounds translate, partial ones are unreadable.
    assert_eq!(requirement_admits("1.2.3 - 2.3.4", "2.3.4"), Some(true));
    assert_eq!(requirement_admits("1.2.3 - 2.3.4", "2.3.5"), Some(false));
    assert_eq!(requirement_admits("1 - 3", "2.0.0"), None);
    assert_eq!(requirement_admits(">=", "1.0.0"), None, "dangling operator");
}

/// THE rmcp case: 10 of 10 briefs said "a lockfile refresh should resolve
/// it". The parent pins the old line, so only the parent can move.
#[test]
fn a_semver_incompatible_transitive_fix_names_the_parent() {
    let parent = link("victauri-plugin", "0.8.4", None);
    let path = fix_path(Some("1.7.0"), Some("2.1.0"), Some(false), Some(&parent));
    assert_eq!(
        path,
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: false,
        }
    );
    let clause = fix_clause(&path);
    assert!(clause.contains("upgrade victauri-plugin"), "{clause}");
    assert!(clause.contains("will NOT fix it"), "{clause}");
    // Inferred, not read: the clause must not claim a requirement it never saw.
    assert!(!clause.contains("requirement"), "{clause}");
}

#[test]
fn a_compatible_transitive_fix_is_a_lockfile_refresh() {
    let parent = link("tokio-rustls", "0.26.1", None);
    assert_eq!(
        fix_path(Some("0.23.39"), Some("0.23.45"), Some(false), Some(&parent)),
        FixPath::Refresh {
            to: "0.23.45".into()
        }
    );
}

/// npm records the requirement: an exact pin blocks even a patch fix
/// (navcal undici 5.28.4 pinned by @vercel/node).
#[test]
fn an_exact_npm_pin_sends_the_fix_to_the_parent() {
    let parent = link("@vercel/node", "5.5.6", Some("5.28.4"));
    assert_eq!(
        fix_path(Some("5.28.4"), Some("5.28.5"), Some(false), Some(&parent)),
        FixPath::Parent {
            parent: "@vercel/node".into(),
            parent_version: "5.5.6".into(),
            to: "5.28.5".into(),
            by_requirement: true,
        }
    );
    // An UNREADABLE requirement falls back to semver compatibility: a patch
    // fix is a refresh, not "the requirement excludes it".
    let odd = link("x", "1.0.0", Some("1 - 3"));
    assert_eq!(
        fix_path(Some("2.0.0"), Some("2.0.1"), Some(false), Some(&odd)),
        FixPath::Refresh { to: "2.0.1".into() }
    );
    // A caret requirement that admits the fix is a refresh, even across what
    // the installed copy alone would call incompatible.
    let wide = link("minimatch", "3.1.2", Some(">=1.1.7"));
    assert_eq!(
        fix_path(Some("1.1.11"), Some("5.0.12"), Some(false), Some(&wide)),
        FixPath::Refresh {
            to: "5.0.12".into()
        }
    );
}

#[test]
fn direct_unknown_and_missing_fixes() {
    assert_eq!(
        fix_path(Some("3.2.4"), Some("4.1.11"), Some(true), None),
        FixPath::Bump {
            to: "4.1.11".into()
        }
    );
    assert_eq!(
        fix_path(None, Some("1.0.1"), None, None),
        FixPath::Update { to: "1.0.1".into() }
    );
    assert_eq!(
        fix_path(Some("0.9.6"), None, Some(false), None),
        FixPath::NoFix
    );
    assert_eq!(
        fix_path(Some("1.7.0"), Some("2.1.0"), Some(false), None),
        FixPath::ParentUnknown { to: "2.1.0".into() }
    );
    let no_fix = fix_clause(&FixPath::NoFix);
    assert!(no_fix.contains("no fix published") && !no_fix.contains("bump"));
}

#[test]
fn labels_name_the_repository() {
    assert_eq!(
        label_from("atlas", &["apps", "bridge", "src-tauri"]),
        "atlas/bridge/src-tauri",
        "bridge/src-tauri alone never said it was atlas (2026-10-01)"
    );
    assert_eq!(label_from("4DA", &["src-tauri"]), "4da/src-tauri");
    assert_eq!(label_from("navcal", &[]), "navcal");
}

#[test]
fn own_packages_are_recognised_with_cargo_name_rules() {
    let own: HashSet<String> = ["victauri-core", "@4da/mcp-server"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert!(is_own_package("victauri_core", &own));
    assert!(is_own_package("@4DA/mcp-server", &own));
    assert!(!is_own_package("fastembed", &own));
}

#[test]
fn majors_behind_counts_compatibility_units() {
    assert_eq!(majors_behind("5.17.4", "7.1.0"), 2, "fastembed");
    assert_eq!(majors_behind("0.8.6", "0.9.0"), 1, "sqlx 0.x minor");
    assert_eq!(majors_behind("6.7.0", "7.25.0"), 1);
    assert_eq!(majors_behind("x", "1.0.0"), 0);
}

fn upgrade(pre_one: bool, behind: u64) -> UpgradeFact {
    UpgradeFact {
        key: "k".into(),
        package: "p".into(),
        ecosystem: "crates.io".into(),
        announced: "1.0.0".into(),
        published: None,
        yanked: false,
        dev_only: false,
        majors_behind: behind,
        pre_one,
        sites: vec![],
        item_id: 1,
        url: None,
        status: FactStatus::New,
    }
}

#[test]
fn gap_words_do_not_call_a_0x_minor_a_major() {
    assert_eq!(gap_words(true, 3), "3 breaking releases behind (0.x)");
    assert_eq!(gap_words(true, 1), "one breaking release behind (0.x)");
    assert_eq!(gap_words(false, 2), "2 major versions behind");
    assert_eq!(gap_words(false, 1), "one major version behind");
    assert_eq!(gap_words(false, 0), "breaking release");
}

/// Live 2026-10-02: one gap for the whole fact told the brief navcal was
/// three majors behind on stripe (it runs 22.3.0; 23.0.0 is one major).
#[test]
fn upgrade_sites_line_states_each_projects_own_gap() {
    let mut u = upgrade(false, 3);
    u.announced = "23.0.0".into();
    u.sites = vec![
        UpgradeSite {
            label: "4da/site".into(),
            installed: "20.3.1".into(),
        },
        UpgradeSite {
            label: "navcal".into(),
            installed: "22.3.0".into(),
        },
    ];
    assert_eq!(
        upgrade_sites_line(&u),
        "4da/site on 20.3.1 (3 major versions behind); navcal on 22.3.0 (one major version behind)"
    );
}

#[test]
fn also_open_line_caps_the_list_and_counts_the_rest() {
    let many: Vec<SecurityFact> = (0..9)
        .map(|i| security(&format!("k{i}"), AlertUrgency::Medium, "1.0.0", "1.0.1"))
        .collect();
    let line = also_open_line(&many);
    assert_eq!(line.matches("rmcp (").count(), ALSO_OPEN_SHOWN, "{line}");
    assert!(line.ends_with("and 3 more on the Preemption tab"), "{line}");
    assert!(!also_open_line(&many[..2]).contains("more on"));
}

#[test]
fn excerpt_collapses_whitespace_and_cuts_on_a_word() {
    assert_eq!(excerpt("a  b\n\nc", 50), "a b c");
    let long = "word ".repeat(200);
    let cut = excerpt(&long, 30);
    assert!(cut.ends_with(" …"), "{cut}");
    assert!(cut.chars().count() <= 32, "{cut}");
    // Multi-byte text never splits a char.
    let cjk = "漢字 ".repeat(100);
    assert!(excerpt(&cjk, 25).chars().count() <= 27);
}

const KEY: &str = "crates.io:rmcp:atlas";

#[test]
fn novelty_folds_unchanged_facts_and_reopens_changed_ones() {
    let mut n = Novelty::default();
    assert_eq!(n.status(KEY, "HIGH|2.1.0", "2026-09-16"), FactStatus::New);
    n.record_facts([(KEY, "HIGH|2.1.0")], "2026-09-16");
    assert_eq!(
        n.status(KEY, "HIGH|2.1.0", "2026-09-17"),
        FactStatus::Unchanged {
            since: "2026-09-16".into()
        }
    );
    // Re-recording the same state keeps the ORIGINAL date, refreshes seen.
    n.record_facts([(KEY, "HIGH|2.1.0")], "2026-10-02");
    assert_eq!(n.facts[KEY].since, "2026-09-16");
    assert_eq!(n.facts[KEY].seen, "2026-10-02");
    // A changed fix is news again, dated from the change.
    assert_eq!(n.status(KEY, "HIGH|2.2.0", "2026-10-02"), FactStatus::New);
    n.record_facts([(KEY, "HIGH|2.2.0")], "2026-10-02");
    assert_eq!(n.facts[KEY].since, "2026-10-02");
}

/// Live 2026-10-02: the second brief of the same run moved rmcp and vitest
/// out of Act now ("unchanged from before") — a same-day regeneration.
#[test]
fn a_fact_first_reported_today_is_still_new_today() {
    let mut n = Novelty::default();
    n.record_facts([(KEY, "HIGH|2.1.0")], "2026-10-02");
    assert_eq!(n.status(KEY, "HIGH|2.1.0", "2026-10-02"), FactStatus::New);
    assert!(!n.status(KEY, "HIGH|2.1.0", "2026-10-03").is_new());
}

/// Pruning by FIRST date would bring a fact still open after 45 days back as
/// NEW; last-seen keeps it folded while briefs keep carrying it.
#[test]
fn prune_forgets_by_last_seen_not_first_reported() {
    let mut n = Novelty::default();
    n.record_facts([(KEY, "s")], "2026-08-01");
    n.record_facts([(KEY, "s")], "2026-10-01");
    n.prune("2026-09-15");
    assert_eq!(n.facts[KEY].since, "2026-08-01", "still carried: kept");
    n.prune("2026-10-02");
    assert!(
        n.facts.is_empty(),
        "not carried since the cutoff: forgotten"
    );
}

fn cand(id: i64, title: &str, excerpt: &str) -> WorthKnowingCandidate {
    WorthKnowingCandidate {
        id,
        title: title.into(),
        url: None,
        source_type: "hackernews".into(),
        published: "2026-10-01".into(),
        excerpt: excerpt.into(),
    }
}

/// Featured = named in the brief. A candidate cut for space is neither
/// featured nor rejected (2026-10-02 review).
#[test]
fn featured_in_counts_only_titles_the_brief_names() {
    let cands = vec![
        cand(1, "Announcing Tauri 2.12", ""),
        cand(2, "A Stable Rust Tool Facade for Humans and AI", ""),
        cand(3, "Short", ""),
    ];
    let brief = "## Worth knowing\n- **Announcing Tauri 2.12** drops Windows 7 support.";
    assert_eq!(featured_in(brief, &cands), vec![1]);
    assert!(
        featured_in("short", &cands).is_empty(),
        "titles under 8 chars never match"
    );
}

/// The prompt allows quoting a version an article states; the version check
/// must allow it too, or a correct brief falls to the floor.
#[test]
fn package_facts_allow_versions_a_candidate_names_for_a_fact_package() {
    let mut facts = BriefFacts {
        upgrades: vec![upgrade(false, 1)],
        ..BriefFacts::default()
    };
    facts.upgrades[0].package = "vite".into();
    facts.upgrades[0].announced = "8.0.0".into();
    facts.worth_knowing = vec![
        cand(1, "Vite 7.1 released", "Vite 7.1.3 brings faster HMR"),
        cand(2, "Unrelated 9.9", "invite-only 3.3 beta"),
    ];
    let pf = package_facts(&facts);
    let vite = pf.iter().find(|p| p.name == "vite").expect("vite fact");
    for v in ["8.0.0", "7.1", "7.1.3"] {
        assert!(
            vite.versions.iter().any(|x| x == v),
            "{v} in {:?}",
            vite.versions
        );
    }
    assert!(
        !vite.versions.iter().any(|x| x == "9.9" || x == "3.3"),
        "{:?}",
        vite.versions
    );
}

#[test]
fn featured_items_are_withheld_only_on_later_days() {
    let mut n = Novelty::default();
    n.record_featured(&[42], "2026-10-01");
    assert!(
        !n.featured_before(42, "2026-10-01"),
        "same-day regeneration may repeat it"
    );
    assert!(n.featured_before(42, "2026-10-02"));
    assert!(!n.featured_before(7, "2026-10-02"));
    // Re-featuring keeps the first date.
    n.record_featured(&[42], "2026-10-05");
    assert_eq!(n.featured[&42], "2026-10-01");
    n.prune("2026-10-02");
    assert!(n.featured.is_empty());
}

#[test]
fn novelty_round_trips_through_json() {
    let mut n = Novelty::default();
    n.record_facts([("k", "s")], "2026-10-02");
    n.record_featured(&[1, 2], "2026-10-02");
    let json = serde_json::to_string(&n).unwrap();
    let back: Novelty = serde_json::from_str(&json).unwrap();
    assert_eq!(back, n);
    // An older shape (no `featured`) still loads.
    let legacy: Novelty = serde_json::from_str(r#"{"facts":{}}"#).unwrap();
    assert!(legacy.featured.is_empty());
}

/// Live verification on a REAL corpus snapshot (recipe-live-verify-rust-on-
/// db-snapshot): builds the facts and asserts the invariants the 2026-10-01
/// audit found broken. Run with
/// `FOURDA_DB_PATH=<snapshot> cargo test --lib live_snapshot_brief_facts -- --ignored --nocapture`.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_snapshot_brief_facts() {
    if std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    let db = crate::get_database().expect("open snapshot");
    let facts = build_brief_facts(db);
    println!("{}", serde_json::to_string_pretty(&facts).unwrap());
    println!(
        "\n===== PROMPT FACTS =====\n{}",
        crate::digest_commands::render_facts_for_prompt(&facts)
    );
    println!(
        "\n===== FLOOR =====\n{}",
        crate::briefing_deterministic::build_deterministic_brief(
            &facts,
            crate::briefing_deterministic::FloorReason::NoCapableModel
        )
    );

    let own = own_package_names();
    for f in &facts.security {
        assert!(
            matches!(f.urgency, AlertUrgency::Critical | AlertUrgency::High),
            "act-now holds only HIGH/CRITICAL: {}",
            f.key
        );
        for s in &f.sites {
            if let (Some(i), Some(to)) = (s.installed.as_deref(), fix_target(&s.fix_path)) {
                if let (Some(iv), Some(tv)) = (lenient_semver(i, None), lenient_semver(to, None)) {
                    assert!(
                        iv < tv || matches!(s.fix_path, FixPath::Reinstall { .. }),
                        "{}: installed {i} is already at/after the fix {to}",
                        f.key
                    );
                }
            }
        }
    }
    for u in &facts.upgrades {
        assert!(
            !is_own_package(&u.package, &own),
            "own package as an upgrade: {}",
            u.package
        );
        let announced = lenient_semver(&u.announced, None).expect("announced parses");
        for s in &u.sites {
            let installed = lenient_semver(&s.installed, None).expect("installed parses");
            assert!(
                installed < announced,
                "{} already runs {}",
                s.label,
                u.announced
            );
        }
    }
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(WORTH_KNOWING_WINDOW_DAYS + 1))
        .format("%Y-%m-%d")
        .to_string();
    for c in &facts.worth_knowing {
        assert!(
            c.published >= cutoff,
            "stale candidate {} ({})",
            c.title,
            c.published
        );
    }
}

fn fix_target(path: &FixPath) -> Option<&str> {
    match path {
        FixPath::Bump { to }
        | FixPath::Refresh { to }
        | FixPath::Parent { to, .. }
        | FixPath::ParentUnknown { to }
        | FixPath::Reinstall { to }
        | FixPath::Update { to } => Some(to),
        FixPath::NoFix => None,
    }
}

fn security(key: &str, urgency: AlertUrgency, installed: &str, to: &str) -> SecurityFact {
    SecurityFact {
        key: key.to_string(),
        package: "rmcp".into(),
        ecosystem: "crates.io".into(),
        urgency,
        worst_tier: Some("high".into()),
        advisory_count: 3,
        advisory_ids: vec![],
        title: "t".into(),
        sites: vec![SecuritySite {
            label: "atlas/bridge/src-tauri".into(),
            installed: Some(installed.into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Refresh { to: to.into() },
        }],
        first_seen: None,
        status: FactStatus::New,
    }
}

#[test]
fn the_fingerprint_moves_with_the_facts_not_their_order() {
    let a = security("a", AlertUrgency::High, "1.7.0", "2.1.0");
    let b = security("b", AlertUrgency::Medium, "1.0.0", "1.0.1");
    let f1 = fingerprint(&[a.clone(), b.clone()], &[], &[]);
    let f2 = fingerprint(&[b.clone(), a.clone()], &[], &[]);
    assert_eq!(f1, f2, "order-independent");
    let fixed = security("a", AlertUrgency::High, "2.1.0", "2.1.0");
    assert_ne!(
        f1,
        fingerprint(&[fixed, b.clone()], &[], &[]),
        "a new install changes it"
    );
    // A fixed lower-severity item changes it too (it was left out before).
    let medium = security("m", AlertUrgency::Medium, "1.0.0", "1.0.1");
    assert_ne!(
        fingerprint(&[a.clone()], &[medium], &[]),
        fingerprint(&[a], &[], &[]),
    );
}
