// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `brief_facts` (extracted via #[path]). Every rule is pinned to
//! the live case that motivated it (2026-10-01 audit).

use super::*;

fn link(direct: &str, version: &str, req: Option<&str>) -> ParentLink {
    ParentLink {
        direct: direct.to_string(),
        direct_version: version.to_string(),
        immediate: direct.to_string(),
        immediate_version: version.to_string(),
        requirements: req.map(|r| vec![Requirement::npm(r)]).unwrap_or_default(),
    }
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
            proven: None,
            route: Default::default(),
        }
    );
    let clause = fix_clause("rmcp", &path);
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
            to: "0.23.45".into(),
            inferred: true,
            command: None,
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
            proven: None,
            route: Default::default(),
        }
    );
    // An UNREADABLE requirement falls back to semver compatibility: a patch
    // fix is a refresh, not "the requirement excludes it".
    let odd = link("x", "1.0.0", Some("1 - 3"));
    assert_eq!(
        fix_path(Some("2.0.0"), Some("2.0.1"), Some(false), Some(&odd)),
        FixPath::Refresh {
            to: "2.0.1".into(),
            inferred: true,
            command: None,
        }
    );
    // A caret requirement that admits the fix is a refresh, even across what
    // the installed copy alone would call incompatible.
    let wide = link("minimatch", "3.1.2", Some(">=1.1.7"));
    assert_eq!(
        fix_path(Some("1.1.11"), Some("5.0.12"), Some(false), Some(&wide)),
        FixPath::Refresh {
            to: "5.0.12".into(),
            inferred: false,
            command: None,
        }
    );
}

#[test]
fn direct_unknown_and_missing_fixes() {
    assert_eq!(
        fix_path(Some("3.2.4"), Some("4.1.11"), Some(true), None),
        FixPath::Bump {
            to: "4.1.11".into(),
            fix: None
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
    let no_fix = fix_clause("rsa", &FixPath::NoFix);
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

fn parent_rmcp_facts() -> BriefFacts {
    let mut f = security(KEY, AlertUrgency::High, "1.7.0", "2.1.0");
    f.sites[0].fix_path = FixPath::Parent {
        parent: "victauri-plugin".into(),
        parent_version: "0.8.4".into(),
        to: "2.1.0".into(),
        by_requirement: false,
        proven: None,
        route: Default::default(),
    };
    BriefFacts {
        security: vec![f],
        ..BriefFacts::default()
    }
}

/// Live 2026-10-03: the narration fell to the floor twice over a sentence
/// that put rmcp's fix after victauri-plugin's name. The phrasing the prompt
/// now prescribes must pass the check; the misattributed one must not.
#[test]
fn the_prescribed_parent_phrasing_passes_the_version_check() {
    let pf = package_facts(&parent_rmcp_facts());
    let good = "- **rmcp** (atlas/bridge/src-tauri): upgrade victauri-plugin (its 0.8.4 line \
                cannot reach rmcp >= 2.1.0); a lockfile refresh will not fix it.";
    assert!(
        crate::briefing_groundedness::check_factual_claims(good, &pf).is_empty(),
        "{:?}",
        crate::briefing_groundedness::check_factual_claims(good, &pf)
    );
    let bad = "Upgrade victauri-plugin to reach >= 2.1.0.";
    assert_eq!(
        crate::briefing_groundedness::check_factual_claims(bad, &pf).len(),
        1,
        "a version after the wrong package is still caught"
    );
}

fn proven_090() -> ProvenParent {
    ProvenParent {
        parent_version: "0.9.0".into(),
        child_version: "3.4.1".into(),
        label: "4da/src-tauri".into(),
    }
}

/// Screenshot_3845: "upgrade victauri-plugin to pull in rmcp >= 2.1.0" left
/// the user to find the release while 4da/src-tauri already ran it. With the
/// lockfile proof the clause names the release and the evidence, and the
/// brief may quote both versions without a fault.
#[test]
fn a_proven_parent_release_is_named_with_its_evidence() {
    let path = FixPath::Parent {
        parent: "victauri-plugin".into(),
        parent_version: "0.8.4".into(),
        to: "2.1.0".into(),
        by_requirement: false,
        proven: Some(proven_090()),
        route: Default::default(),
    };
    let clause = fix_clause("rmcp", &path);
    assert!(
        clause.ends_with(
            "upgrade victauri-plugin to 0.9.0 (4da/src-tauri already runs it, resolved to rmcp 3.4.1)"
        ),
        "{clause}"
    );
    let mut facts = parent_rmcp_facts();
    facts.security[0].sites[0].fix_path = path;
    let pf = package_facts(&facts);
    let plugin = pf.iter().find(|p| p.name == "victauri-plugin").unwrap();
    assert!(plugin.versions.contains(&"0.9.0".to_string()));
    let rmcp = pf.iter().find(|p| p.name == "rmcp").unwrap();
    assert!(rmcp.versions.contains(&"3.4.1".to_string()));
    // The narration the prompt prescribes for it passes the version check.
    let prose = "- **rmcp** (atlas/bridge/src-tauri): upgrade victauri-plugin to 0.9.0 \
                 (4da/src-tauri already runs it, resolved to rmcp 3.4.1).";
    assert!(crate::briefing_groundedness::check_factual_claims(prose, &pf).is_empty());
    // Without the proof, 0.9.0 is an invented victauri-plugin upgrade target
    // and the check faults it (the resolved rmcp version is not a target).
    let unproven = package_facts(&parent_rmcp_facts());
    let v = crate::briefing_groundedness::check_factual_claims(prose, &unproven);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].contains("victauri-plugin cited version 0.9.0"),
        "{v:?}"
    );
}

/// Live 2026-10-03: the model quoted the facts' own fix clause verbatim and
/// the version check faulted it, because the clause put rmcp's fix after
/// victauri-plugin's name. Every clause, quoted as-is in an upgrade
/// sentence, must pass the checker.
#[test]
fn every_fix_clause_quoted_verbatim_passes_the_version_check() {
    let bump = |fix: &str| FixPath::Bump {
        to: "2.1.0".into(),
        fix: Some(fix.to_string()),
    };
    let paths = [
        FixPath::Bump {
            to: "2.1.0".into(),
            fix: None,
        },
        // The commands osv::direct_upgrade names (audit E8), as the Brief
        // renders them: npm, Cargo, Go, and the PyPI manifest edit.
        bump("run `npm install \"rmcp@^2.1.0\"`"),
        bump("run `npm install rmcp@2.1.0 --save-exact`"),
        bump("run `cargo add \"rmcp@~2.1.0\" --dev`"),
        bump("run `go get rmcp@v2.1.0`"),
        bump("change `rmcp==1.7.0` to `rmcp==2.1.0` in requirements.txt"),
        FixPath::Refresh {
            to: "2.1.0".into(),
            inferred: false,
            command: None,
        },
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: false,
            proven: None,
            route: Default::default(),
        },
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: true,
            proven: None,
            route: Default::default(),
        },
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: false,
            proven: Some(proven_090()),
            route: Default::default(),
        },
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: true,
            proven: None,
            route: ParentRoute {
                through: Some(Through {
                    name: "atlas-core".into(),
                    version: "0.3.0".into(),
                    standing: ParentStanding::Newer {
                        latest: "0.4.1".into(),
                    },
                    ..Through::default()
                }),
                standing: ParentStanding::Newest { deprecated: true },
                override_hint: None,
            },
        },
        FixPath::Parent {
            parent: "victauri-plugin".into(),
            parent_version: "0.8.4".into(),
            to: "2.1.0".into(),
            by_requirement: true,
            proven: None,
            route: ParentRoute {
                through: None,
                standing: ParentStanding::Newer {
                    latest: "0.9.5".into(),
                },
                override_hint: Some("an `overrides` entry in package.json (npm)".into()),
            },
        },
        FixPath::ParentUnknown { to: "2.1.0".into() },
        FixPath::Reinstall { to: "2.1.0".into() },
        FixPath::PerLine {
            lines: vec![
                LineFix {
                    installed: "1.7.0".into(),
                    to: "1.7.9".into(),
                },
                LineFix {
                    installed: "2.0.0".into(),
                    to: "2.1.0".into(),
                },
            ],
        },
        FixPath::Update { to: "2.1.0".into() },
    ];
    for path in paths {
        let mut facts = parent_rmcp_facts();
        facts.security[0].sites[0].fix_path = path.clone();
        let pf = package_facts(&facts);
        let sentence = format!(
            "- **rmcp 1.7.0** (atlas/bridge/src-tauri): {}.",
            fix_clause("rmcp", &path)
        );
        let v = crate::briefing_groundedness::check_factual_claims(&sentence, &pf);
        assert!(v.is_empty(), "{path:?}: {sentence} -> {v:?}");
    }
}

#[test]
fn the_correction_note_names_the_owner_of_a_misplaced_version() {
    let pf = package_facts(&parent_rmcp_facts());
    let violations = crate::briefing_groundedness::check_factual_claims(
        "Upgrade victauri-plugin to reach >= 2.1.0.",
        &pf,
    );
    let note = correction_note(&violations, &pf);
    assert!(
        note.contains("2.1.0 belongs to rmcp, not victauri-plugin"),
        "{note}"
    );
    assert!(note.contains("\"rmcp >= 2.1.0\""), "{note}");
    let invented = correction_note(
        &["hono cited version 4.7.5 as an upgrade target (on record: 4.13.5)".to_string()],
        &pf,
    );
    assert!(
        invented.contains("4.7.5 is not a version of hono"),
        "{invented}"
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
        FixPath::Bump { to, .. }
        | FixPath::Refresh { to, .. }
        | FixPath::Parent { to, .. }
        | FixPath::ParentUnknown { to }
        | FixPath::Reinstall { to }
        | FixPath::Update { to } => Some(to),
        FixPath::PerLine { .. }
        | FixPath::NoFix
        | FixPath::Introduces { .. }
        | FixPath::Malicious { .. } => None,
    }
}

fn security(key: &str, urgency: AlertUrgency, installed: &str, to: &str) -> SecurityFact {
    SecurityFact {
        key: key.to_string(),
        package: "rmcp".into(),
        ecosystem: "crates.io".into(),
        urgency,
        worst_tier: Some("high".into()),
        worst_tier_from: None,
        advisory_count: 3,
        advisory_ids: vec![],
        title: "t".into(),
        sites: vec![SecuritySite {
            label: "atlas/bridge/src-tauri".into(),
            installed: Some(installed.into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Refresh {
                to: to.into(),
                inferred: false,
                command: None,
            },
            held: None,
        }],
        not_compiled: vec![],
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
    // A new advisory on an already-open package changes it (2026-10-03:
    // GHSA-c9xm joined rmcp's three and the brief kept saying "3").
    let mut grown = a.clone();
    grown.advisory_ids = vec!["GHSA-c9xm-49cp-xcr9".into()];
    assert_ne!(
        fingerprint(&[a.clone()], &[], &[]),
        fingerprint(&[grown], &[], &[]),
        "a new advisory on an open package is news"
    );
    // A fixed lower-severity item changes it too (it was left out before).
    let medium = security("m", AlertUrgency::Medium, "1.0.0", "1.0.1");
    assert_ne!(
        fingerprint(&[a.clone()], &[medium], &[]),
        fingerprint(&[a], &[], &[]),
    );
}

/// Live 2026-10-04: every upgrade already reported, and the model still wrote
/// an "Upgrades to plan" section of filler. Sections without news are cut;
/// the rest of the brief is untouched.
#[test]
fn sections_without_news_are_cut_deterministically() {
    let narrated = "## Act now\n- rmcp: upgrade victauri-plugin.\n\n\
                    ## Upgrades to plan\nNo new upgrade facts today — all 8 tracked upgrades are unchanged.\n\n\
                    ## Worth knowing\nNothing else worth your time today.\n\n\
                    ## Still open\nvitest (navcal, since 2026-10-03)";
    let mut facts = parent_rmcp_facts(); // one NEW security fact, no upgrades
    let out = drop_sections_without_news(narrated, &facts);
    assert!(out.starts_with("## Act now\n- rmcp"), "{out}");
    assert!(!out.contains("Upgrades to plan"), "{out}");
    assert!(!out.contains("No new upgrade facts"), "{out}");
    assert!(
        out.contains("## Worth knowing") && out.contains("## Still open"),
        "{out}"
    );

    // An unchanged security fact empties Act now too.
    facts.security[0].status = FactStatus::Unchanged {
        since: "2026-10-03".into(),
    };
    let out = drop_sections_without_news(narrated, &facts);
    assert!(out.starts_with("## Worth knowing"), "{out}");

    // A NEW upgrade keeps its section.
    let mut with_upgrade = parent_rmcp_facts();
    with_upgrade.upgrades = vec![UpgradeFact {
        key: "crates.io:fastembed".into(),
        package: "fastembed".into(),
        ecosystem: "crates.io".into(),
        announced: "7.1.0".into(),
        published: None,
        yanked: false,
        dev_only: false,
        majors_behind: 2,
        pre_one: false,
        sites: vec![],
        item_id: 1,
        url: None,
        status: FactStatus::New,
    }];
    assert!(drop_sections_without_news(narrated, &with_upgrade).contains("## Upgrades to plan"));

    // Never cut a brief down to nothing.
    let only = "## Upgrades to plan\nfiller";
    assert_eq!(
        drop_sections_without_news(only, &BriefFacts::default()),
        only
    );
}

/// Live, on a REAL corpus snapshot (no model call): resolve the Cargo
/// projects named in `FOURDA_RESOLVE_DIRS` (`;`-separated) so their feature
/// sets are cached, then print what the matcher dropped as not compiled and
/// the security facts the brief would be written from. Run with
/// `FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<dir> FOURDA_RESOLVE_DIRS=<dirs>
///  cargo test --lib live_snapshot_reachability -- --ignored --nocapture`.
#[test]
#[ignore = "runs cargo tree and reads a real database snapshot"]
fn live_snapshot_reachability() {
    let (Ok(_), Ok(dirs)) = (
        std::env::var("FOURDA_DB_PATH"),
        std::env::var("FOURDA_RESOLVE_DIRS"),
    ) else {
        return;
    };
    let db = crate::get_database().expect("snapshot db");
    for dir in dirs.split(';').filter(|d| !d.trim().is_empty()) {
        let resolved =
            crate::ace::cargo_resolve::host_resolved_crates(&db, std::path::Path::new(dir));
        println!("resolved {dir}: {:?} crates", resolved.map(|r| r.len()));
    }
    let (matched, excluded) =
        crate::osv::matching::get_matched_advisories_with_not_compiled(&db).expect("matcher");
    println!("matched advisories: {}", matched.len());
    for n in &excluded {
        println!(
            "NOT COMPILED {} {} {:?} in {} via {:?}",
            n.advisory_id, n.package_name, n.installed_version, n.project_path, n.files
        );
    }
    let facts = build_brief_facts(&db);
    for f in facts.security.iter().chain(facts.also_open.iter()) {
        println!(
            "FACT {} [{:?}] {} advisories {:?}: {}",
            f.package, f.urgency, f.advisory_count, f.advisory_ids, f.title
        );
        for s in &f.sites {
            println!("  {}: {}", s.label, fix_clause(&f.package, &s.fix_path));
        }
        for n in &f.not_compiled {
            println!("  not counted: {} ({})", n.advisory_id, n.summary);
        }
    }
}

// ---- Cadence (audit 2026-10-07: four auto briefs on local 2026-10-06) ----

fn upgrade_at(announced: &str) -> UpgradeFact {
    let mut u = upgrade(false, 1);
    u.key = "npm:@ai-sdk/openai".into();
    u.package = "@ai-sdk/openai".into();
    u.announced = announced.into();
    u
}

/// @ai-sdk/openai 4.0.84 -> 4.0.85 regenerated the brief and re-dated the
/// fact as NEW. A patch inside the reported line is not news.
#[test]
fn a_patch_inside_the_reported_line_keeps_fingerprint_and_status() {
    let before = upgrade_at("4.0.84");
    let after = upgrade_at("4.0.85");
    assert_eq!(
        fingerprint(&[], &[], &[before.clone()]),
        fingerprint(&[], &[], &[after.clone()])
    );
    let mut n = Novelty::default();
    n.record_facts(
        [(before.key.as_str(), upgrade_signature(&before).as_str())],
        "2026-10-05",
    );
    assert_eq!(
        n.status(&after.key, &upgrade_signature(&after), "2026-10-06"),
        FactStatus::Unchanged {
            since: "2026-10-05".into()
        }
    );
    // A record written before this change (the exact version) is the same
    // state too, and adopts the new shape without losing its first date.
    let mut legacy = Novelty::default();
    legacy.record_facts([(after.key.as_str(), "4.0.84")], "2026-10-04");
    assert!(!legacy
        .status(&after.key, &upgrade_signature(&after), "2026-10-06")
        .is_new());
    legacy.record_facts(
        [(after.key.as_str(), upgrade_signature(&after).as_str())],
        "2026-10-06",
    );
    assert_eq!(legacy.facts[&after.key].since, "2026-10-04");
    assert_eq!(legacy.facts[&after.key].sig, "4");
}

#[test]
fn a_new_breaking_line_or_a_yank_moves_the_fingerprint() {
    let fp = |u: UpgradeFact| fingerprint(&[], &[], &[u]);
    assert_ne!(
        fp(upgrade_at("4.9.0")),
        fp(upgrade_at("5.0.0")),
        "4.x -> 5.0"
    );
    assert_ne!(
        fp(upgrade_at("0.8.0")),
        fp(upgrade_at("0.9.0")),
        "0.8 -> 0.9"
    );
    assert_eq!(
        fp(upgrade_at("0.8.1")),
        fp(upgrade_at("0.8.2")),
        "0.8.1 -> 0.8.2"
    );
    let mut yanked = upgrade_at("4.0.85");
    yanked.yanked = true;
    assert_ne!(fp(upgrade_at("4.0.85")), fp(yanked));
    // And novelty agrees: a new line is NEW again.
    let mut n = Novelty::default();
    let four = upgrade_at("4.0.85");
    n.record_facts(
        [(four.key.as_str(), upgrade_signature(&four).as_str())],
        "2026-10-05",
    );
    let five = upgrade_at("5.0.0");
    assert!(n
        .status(&five.key, &upgrade_signature(&five), "2026-10-06")
        .is_new());
}

/// A Medium advisory's fix path moves whenever upstream publishes; on
/// 2026-10-06 that alone regenerated the brief.
#[test]
fn a_lower_severity_fix_path_change_keeps_the_fingerprint() {
    let mut a = security("npm:dotenv:navcal", AlertUrgency::Medium, "1.0.0", "1.1.20");
    a.advisory_ids = vec!["GHSA-b".into(), "GHSA-a".into()];
    let mut b = security("npm:dotenv:navcal", AlertUrgency::Medium, "1.0.0", "1.1.21");
    b.advisory_ids = vec!["GHSA-a".into(), "GHSA-b".into()];
    assert_eq!(
        fingerprint(&[], &[a.clone()], &[]),
        fingerprint(&[], &[b.clone()], &[])
    );
    assert_eq!(security_signature(&b), "Medium|GHSA-a,GHSA-b");
    // A new advisory on it still counts.
    let mut grown = b.clone();
    grown.advisory_ids.push("GHSA-c".into());
    assert_ne!(fingerprint(&[], &[b], &[]), fingerprint(&[], &[grown], &[]));
    // The act-now tier keeps the full state: a new fix is news there.
    let high_a = security("k", AlertUrgency::High, "1.0.0", "1.1.20");
    let high_b = security("k", AlertUrgency::High, "1.0.0", "1.1.21");
    assert_ne!(
        fingerprint(&[high_a], &[], &[]),
        fingerprint(&[high_b], &[], &[])
    );
}

/// The command a bump names (audit E8) is wording from the manifest, not
/// state: adding it must not make an open act-now fact read as changed, and
/// the signature stays the one stored before `fix` existed.
#[test]
fn a_bumps_command_does_not_change_its_signature() {
    let bump = |fix: Option<&str>| {
        let mut f = security("k", AlertUrgency::High, "1.0.0", "1.1.20");
        f.sites[0].fix_path = FixPath::Bump {
            to: "1.1.20".into(),
            fix: fix.map(str::to_string),
        };
        f
    };
    let plain = bump(None);
    let with_command = bump(Some("run `cargo add rmcp@1.1.20`"));
    assert_eq!(
        security_signature(&plain),
        security_signature(&with_command)
    );
    assert_eq!(
        security_signature(&plain),
        "High||atlas/bridge/src-tauri=1.0.0->Bump { to: \"1.1.20\" }"
    );
    let mut moved = plain.clone();
    moved.sites[0].fix_path = FixPath::Bump {
        to: "1.1.21".into(),
        fix: None,
    };
    assert_ne!(security_signature(&plain), security_signature(&moved));
}

#[test]
fn act_now_ids_name_each_advisory_of_each_act_now_fact() {
    let mut a = security("crates.io:rmcp:atlas", AlertUrgency::High, "1.7.0", "2.1.0");
    a.advisory_ids = vec!["GHSA-a".into()];
    let medium = security("npm:x:navcal", AlertUrgency::Medium, "1.0.0", "1.0.1");
    let facts = BriefFacts {
        security: vec![a],
        also_open: vec![medium],
        ..BriefFacts::default()
    };
    assert_eq!(
        crate::brief_cadence::act_now_ids(&facts),
        vec!["crates.io:rmcp:atlas#GHSA-a".to_string()]
    );
}

// ============================================================================
// Per-line fix targets and honest parent moves (2026-10-12)
// ============================================================================

use crate::osv::fix_target::{InstallSite, LineTarget};
use crate::osv::parent_hint::{ParentStanding, Through};

const NAVCAL: &str = "d:/runyourempire/navcal";

fn line_in(project: &str, installed: &str, target: Option<&str>) -> LineTarget {
    LineTarget {
        installed_version: installed.into(),
        target_version: target.map(str::to_string),
        clears_all_known: target.is_some(),
        introduces: Vec::new(),
        malicious: Vec::new(),
        held: None,
        upgrade_type: None,
        sites: vec![InstallSite {
            project_path: project.into(),
            is_direct: false,
            is_dev: true,
        }],
    }
}

/// Live 2026-10-12: navcal holds undici 5.28.4 and 7.28.0. Each line has its
/// own fix, so the alert carries no single `fixed_version` — and the brief
/// read that as "no fix published" for 26 advisories that were all fixed.
/// Each copy now takes its own line's target.
#[test]
fn copies_on_several_lines_each_take_their_own_target_not_no_fix() {
    let lines = vec![
        line_in(NAVCAL, "5.28.4", Some("6.28.1")),
        line_in(NAVCAL, "7.28.0", Some("7.29.1")),
    ];
    assert_eq!(
        copy_fix(&lines, NAVCAL, Some("7.28.0"), None),
        CopyFix::Target(Some("7.29.1".into()))
    );
    assert_eq!(
        copy_fix(&lines, NAVCAL, Some("5.28.4"), None),
        CopyFix::Target(Some("6.28.1".into()))
    );
    // The copy's own target becomes a real path, never NoFix.
    let CopyFix::Target(fix) = copy_fix(&lines, NAVCAL, Some("7.28.0"), None) else {
        panic!("a known copy has a target");
    };
    assert_eq!(
        fix_path(Some("7.28.0"), fix.as_deref(), Some(false), None),
        FixPath::Refresh {
            to: "7.29.1".into(),
            inferred: true,
            command: None,
        }
    );

    // A copy whose version is unknown cannot be placed on a line: every
    // line's fix is stated, and the clause never says "no fix".
    let CopyFix::PerLine(per_line) = copy_fix(&lines, NAVCAL, None, None) else {
        panic!("an unplaceable copy lists each line");
    };
    let clause = fix_clause("undici", &FixPath::PerLine { lines: per_line });
    assert_eq!(
        clause,
        "fixed on each line: undici 5.28.4 needs >= 6.28.1; undici 7.28.0 needs >= 7.29.1"
    );
    assert!(!clause.contains("no fix"), "{clause}");
}

/// The other side of the same rule: "no fix published" stays for a copy
/// whose line has no way out, and only for it.
#[test]
fn no_fix_is_produced_only_when_the_copys_line_has_none() {
    let lines = vec![
        line_in(NAVCAL, "3.0.3", None),
        line_in("d:/4da/site", "3.0.3", None),
    ];
    let fix = copy_fix(&lines, NAVCAL, Some("3.0.3"), None);
    assert_eq!(fix, CopyFix::Target(None));
    assert_eq!(
        fix_path(Some("3.0.3"), None, Some(false), None),
        FixPath::NoFix
    );
    // Unplaceable, and no line has a fix: still genuinely none.
    assert_eq!(copy_fix(&lines, NAVCAL, None, None), CopyFix::Target(None));
    // One line fixed, one not: each copy says what is true of IT.
    let mixed = vec![
        line_in(NAVCAL, "1.1.12", Some("1.1.21")),
        line_in(NAVCAL, "0.9.0", None),
    ];
    assert_eq!(
        copy_fix(&mixed, NAVCAL, Some("0.9.0"), None),
        CopyFix::Target(None)
    );
    assert_eq!(
        copy_fix(&mixed, NAVCAL, Some("1.1.12"), None),
        CopyFix::Target(Some("1.1.21".into()))
    );
    // A copy matched by name only (no line) keeps the alert's single fix.
    assert_eq!(
        copy_fix(&[], NAVCAL, Some("2.0.0"), Some("2.0.1")),
        CopyFix::Target(Some("2.0.1".into()))
    );
}

fn uuid_link() -> ParentLink {
    ParentLink {
        direct: "javascript-opentimestamps".into(),
        direct_version: "0.4.5".into(),
        immediate: "request".into(),
        immediate_version: "2.88.2".into(),
        requirements: vec![Requirement::npm("^3.3.2")],
    }
}

/// Live 2026-10-12: Preemption named request (whose ^3.3.2 excludes the
/// fix) and the brief named javascript-opentimestamps as if IT required a
/// uuid line. The brief now names the parent that holds the copy back, and
/// the direct dependency it arrives through.
#[test]
fn the_brief_names_the_blocking_parent_and_the_direct_dependency_it_arrives_through() {
    let path = fix_path(
        Some("3.4.0"),
        Some("11.1.1"),
        Some(false),
        Some(&uuid_link()),
    );
    let FixPath::Parent {
        parent,
        parent_version,
        by_requirement,
        route,
        ..
    } = &path
    else {
        panic!("{path:?}");
    };
    assert_eq!(parent, "request");
    assert_eq!(parent_version, "2.88.2");
    assert!(by_requirement);
    let through = route
        .through
        .as_ref()
        .expect("arrives through a direct dependency");
    assert_eq!(through.name, "javascript-opentimestamps");
    assert_eq!(through.version, "0.4.5");

    // A direct parent has nothing to arrive through.
    let direct = fix_path(
        Some("1.7.0"),
        Some("2.1.0"),
        Some(false),
        Some(&link("victauri-plugin", "0.8.4", None)),
    );
    assert!(matches!(&direct, FixPath::Parent { route, .. } if route.through.is_none()));
}

fn uuid_path(standing: ParentStanding, through_standing: ParentStanding) -> FixPath {
    FixPath::Parent {
        parent: "request".into(),
        parent_version: "2.88.2".into(),
        to: "11.1.1".into(),
        by_requirement: true,
        proven: None,
        route: ParentRoute {
            through: Some(Through {
                name: "javascript-opentimestamps".into(),
                version: "0.4.5".into(),
                standing: through_standing,
                ..Through::default()
            }),
            standing,
            override_hint: Some("an `overrides` entry in package.json (npm)".into()),
        },
    }
}

/// request 2.88.2 is its last release and is deprecated, and
/// javascript-opentimestamps 0.4.5 is its last release too: no upgrade
/// exists, so the clause must not tell the reader to upgrade one.
#[test]
fn a_parent_at_its_newest_release_gets_replace_or_override_not_upgrade() {
    let path = uuid_path(
        ParentStanding::Newest { deprecated: true },
        ParentStanding::Newest { deprecated: false },
    );
    let clause = fix_clause("uuid", &path);
    assert_eq!(
        clause,
        "transitive; request 2.88.2 requires a uuid line that excludes the fix (uuid >= 11.1.1), \
         so a lockfile refresh will NOT fix it: request 2.88.2 is already the newest request \
         release 4DA has seen and is deprecated, and it arrives through javascript-opentimestamps \
         0.4.5, a direct dependency; javascript-opentimestamps 0.4.5 is already the newest \
         javascript-opentimestamps release 4DA has seen, so no javascript-opentimestamps upgrade \
         is known to admit it: replace javascript-opentimestamps, or force uuid >= 11.1.1 with an \
         `overrides` entry in package.json (npm)"
    );
    assert!(!clause.contains("upgrade request"), "{clause}");
    assert!(!clause.contains("must be updated"), "{clause}");

    // Quoted verbatim, every version sits after its own package.
    let mut facts = parent_rmcp_facts();
    facts.security[0].package = "uuid".into();
    facts.security[0].sites[0].installed = Some("3.4.0".into());
    facts.security[0].sites[0].fix_path = path;
    let pf = package_facts(&facts);
    let sentence = format!("- **uuid 3.4.0** (4da-ledger): {clause}.");
    let v = crate::briefing_groundedness::check_factual_claims(&sentence, &pf);
    assert!(v.is_empty(), "{v:?}");
}

/// A newer release on record: upgrading the lever is the move, and the
/// clause says the requirement of that release is not on disk.
#[test]
fn a_lever_with_a_newer_release_on_record_is_upgraded_with_a_check() {
    let clause = fix_clause(
        "uuid",
        &uuid_path(
            ParentStanding::Unknown,
            ParentStanding::Newer {
                latest: "0.5.0".into(),
            },
        ),
    );
    assert!(
        clause.ends_with(
            "request arrives through javascript-opentimestamps 0.4.5, a direct dependency; upgrade \
             javascript-opentimestamps (javascript-opentimestamps 0.5.0 is the newest release 4DA \
             has seen; check that it admits uuid >= 11.1.1), or force uuid >= 11.1.1 with an \
             `overrides` entry in package.json (npm)"
        ),
        "{clause}"
    );
    // Nothing on record: the parent must move, and no update is claimed.
    let unknown = fix_clause(
        "uuid",
        &uuid_path(ParentStanding::Unknown, ParentStanding::Unknown),
    );
    assert!(
        unknown.contains(
            "javascript-opentimestamps must move (4DA holds no record of a \
             javascript-opentimestamps release that admits uuid >= 11.1.1)"
        ),
        "{unknown}"
    );
}

/// Review of #1008, S4: the malware clause names no version (the MCP's
/// words), so the version check never faults the brief that quotes it.
#[test]
fn the_malicious_clause_passes_the_version_check() {
    let mut facts = parent_rmcp_facts();
    facts.security[0].sites[0].fix_path = FixPath::Malicious {
        advisories: vec!["MAL-2026-0001".into()],
        introduces: vec!["GHSA-synt-b".into()],
    };
    let clause = fix_clause("rmcp", &facts.security[0].sites[0].fix_path);
    assert!(
        clause.contains("rmcp is flagged malicious (MAL-2026-0001): remove or replace rmcp"),
        "{clause}"
    );
    let pf = package_facts(&facts);
    let prose = format!("- **rmcp** (atlas/bridge/src-tauri): {clause}.");
    assert!(
        crate::briefing_groundedness::check_factual_claims(&prose, &pf).is_empty(),
        "{:?}",
        crate::briefing_groundedness::check_factual_claims(&prose, &pf)
    );
}

/// Review of #1008 (nit): the Brief's Debug-based signatures of existing
/// `Introduces` facts are unchanged by the malware variant.
#[test]
fn an_introduces_fact_keeps_its_signature_shape() {
    let path = FixPath::Introduces {
        advisories: vec!["GHSA-x".into()],
    };
    assert_eq!(
        format!("{path:?}"),
        r#"Introduces { advisories: ["GHSA-x"] }"#
    );
}

/// Audit 6.2.5 D-4: a target the held-dependency rule could not call clean
/// carries its caveat into every fix clause; a moved target needs none.
#[test]
fn a_held_caveat_follows_the_fix_clause() {
    use fourda_depengine::held::HeldOutcome;
    let text = |o: &HeldOutcome| clause::held_caveat("express", o).map(|c| c.text);
    assert_eq!(
        text(&blocked_express()).as_deref(),
        Some("express 4.20.0 is NOT a clean fix: it installs an affected qs 6.11.0")
    );
    let unchecked = HeldOutcome::Unchecked {
        target: "4.20.0".into(),
        reason: "offline".into(),
    };
    assert_eq!(
        text(&unchecked).as_deref(),
        Some("whether what express 4.20.0 pins or narrowly ranges is affected was NOT CHECKED")
    );
    let moved = HeldOutcome::Moved {
        from: "4.20.0".into(),
        to: "4.22.3".into(),
        past: vec![],
    };
    assert_eq!(clause::held_caveat("express", &moved), None);
    let site = express_site(clause::held_caveat("express", &unchecked));
    assert_eq!(
        site_fix_clause("express", &site),
        "bump express to >= 4.20.0 (whether what express 4.20.0 pins or narrowly ranges is \
         affected was NOT CHECKED)"
    );
}

/// Review of #1039, S3: the caveat's versions (qs 6.11.0) are facts, so a
/// brief that quotes the fix clause is not faulted by the version check.
#[test]
fn a_held_caveats_versions_pass_the_version_check() {
    let site = express_site(clause::held_caveat("express", &blocked_express()));
    let mut fact = security(
        "npm:express:nodegoat",
        AlertUrgency::High,
        "4.16.4",
        "4.20.0",
    );
    fact.package = "express".into();
    fact.sites = vec![site.clone()];
    let facts = BriefFacts {
        security: vec![fact],
        ..BriefFacts::default()
    };
    let pf = package_facts(&facts);
    let quoted = format!(
        "- **express** (nodegoat): {}",
        site_fix_clause("express", &site)
    );
    assert!(quoted.contains("qs 6.11.0"), "{quoted}");
    assert_eq!(
        crate::briefing_groundedness::check_factual_claims(&quoted, &pf),
        Vec::<String>::new()
    );
    let versions = crate::morning_audit::fact_versions(&facts);
    assert!(versions
        .iter()
        .any(|(p, v, what)| p == "qs" && v == "6.11.0" && *what == "held dependency"));
}

fn blocked_express() -> fourda_depengine::held::HeldOutcome {
    fourda_depengine::held::HeldOutcome::Blocked {
        target: "4.20.0".into(),
        affected: vec![fourda_depengine::held::HeldDependency {
            name: "qs".into(),
            spec: "6.11.0".into(),
            exact: true,
            version: "6.11.0".into(),
            ids: vec!["GHSA-4mjr-xmp4-gh2g".into()],
        }],
        searched: vec!["4.20.0".into()],
        unchecked: vec![],
    }
}

fn express_site(held: Option<HeldCaveat>) -> SecuritySite {
    SecuritySite {
        label: "nodegoat".into(),
        installed: Some("4.16.4".into()),
        dev_only: false,
        scratch: false,
        dormant_days: None,
        fix_path: FixPath::Bump {
            to: "4.20.0".into(),
            fix: None,
        },
        held,
    }
}
