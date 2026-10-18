// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the deterministic brief floor (pure rendering — no feed/DB needed).

use super::{build_deterministic_brief, escape_markdown, FloorReason, NO_LOCKFILES_LINE};
use crate::brief_facts::{
    BriefFacts, FactStatus, FixPath, SecurityFact, SecuritySite, UpgradeFact, UpgradeSite,
    WorthKnowingCandidate,
};
use crate::preemption::AlertUrgency;

fn rmcp(status: FactStatus) -> SecurityFact {
    SecurityFact {
        key: "crates.io:rmcp:atlas/bridge/src-tauri".into(),
        package: "rmcp".into(),
        ecosystem: "crates.io".into(),
        urgency: AlertUrgency::High,
        worst_tier: Some("high".into()),
        advisory_count: 3,
        advisory_ids: vec![],
        title: "Unauthenticated session leak".into(),
        sites: vec![SecuritySite {
            label: "atlas/bridge/src-tauri".into(),
            installed: Some("1.7.0".into()),
            dev_only: false,
            scratch: false,
            dormant_days: None,
            fix_path: FixPath::Parent {
                parent: "victauri-plugin".into(),
                parent_version: "0.8.4".into(),
                to: "2.1.0".into(),
                by_requirement: false,
                proven: None,
                route: Default::default(),
            },
        }],
        not_compiled: vec![],
        first_seen: Some("2026-09-16".into()),
        status,
    }
}

fn fastembed(status: FactStatus) -> UpgradeFact {
    UpgradeFact {
        key: "crates.io:fastembed".into(),
        package: "fastembed".into(),
        ecosystem: "crates.io".into(),
        announced: "7.1.0".into(),
        published: Some("2026-09-22".into()),
        yanked: false,
        dev_only: false,
        majors_behind: 2,
        pre_one: false,
        sites: vec![UpgradeSite {
            label: "4da/src-tauri".into(),
            installed: "5.17.4".into(),
        }],
        item_id: 1,
        url: Some("https://crates.io/crates/fastembed".into()),
        status,
    }
}

#[test]
fn new_facts_render_in_the_narrated_sections() {
    let facts = BriefFacts {
        security: vec![rmcp(FactStatus::New)],
        upgrades: vec![fastembed(FactStatus::New)],
        worth_knowing: vec![WorthKnowingCandidate {
            id: 9,
            title: "Deser: Rethinking Rust Serialization".into(),
            url: Some("https://lucumr.pocoo.org/2026/9/29/deser/".into()),
            source_type: "hackernews".into(),
            published: "2026-09-29".into(),
            excerpt: String::new(),
        }],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(out.contains("## Act now"), "{out}");
    assert!(
        out.contains("**rmcp** (crates.io, High — 3 advisories)"),
        "{out}"
    );
    assert!(out.contains("atlas/bridge/src-tauri on 1.7.0"), "{out}");
    assert!(out.contains("upgrade victauri-plugin"), "{out}");
    assert!(out.contains("## Upgrades to plan"), "{out}");
    assert!(out.contains("fastembed 7.1.0"), "{out}");
    assert!(out.contains("4da/src-tauri on 5.17.4"), "{out}");
    assert!(out.contains("2 major versions behind"), "{out}");
    assert!(out.contains(
        "[Deser: Rethinking Rust Serialization](https://lucumr.pocoo.org/2026/9/29/deser/)"
    ));
    assert!(!out.contains("Nothing new touches your code today"));
}

/// The 2026-10-01 nag: rmcp led ten consecutive briefs. Unchanged facts
/// fold into one Still-open line.
#[test]
fn unchanged_facts_fold_into_still_open() {
    let facts = BriefFacts {
        security: vec![rmcp(FactStatus::Unchanged {
            since: "2026-09-16".into(),
        })],
        upgrades: vec![fastembed(FactStatus::Unchanged {
            since: "2026-10-02".into(),
        })],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(!out.contains("## Act now"), "{out}");
    assert!(!out.contains("## Upgrades to plan"), "{out}");
    assert!(
        out.contains("Nothing new touches your code today."),
        "{out}"
    );
    assert!(out.contains("## Still open"), "{out}");
    assert!(
        out.contains("rmcp (atlas/bridge/src-tauri, since 2026-09-16)"),
        "{out}"
    );
    assert!(
        out.contains("fastembed (4da/src-tauri on 5.17.4, since 2026-10-02)"),
        "{out}"
    );
}

/// AD-051: the floor names the advisories it does not count, and why, and
/// carries the proven parent release in the fix clause.
#[test]
fn the_floor_names_uncounted_advisories_and_the_proven_parent() {
    let mut fact = rmcp(FactStatus::New);
    fact.advisory_count = 1;
    fact.not_compiled = vec![
        crate::brief_facts::NotCompiledNote {
            advisory_id: "GHSA-33f5-2c5q-wgwj".into(),
            summary: "OAuth metadata".into(),
        },
        crate::brief_facts::NotCompiledNote {
            advisory_id: "GHSA-9g45-5xwm-f3wc".into(),
            summary: "headers on redirect".into(),
        },
    ];
    if let FixPath::Parent { proven, .. } = &mut fact.sites[0].fix_path {
        *proven = Some(crate::brief_facts::ProvenParent {
            parent_version: "0.9.0".into(),
            child_version: "3.4.1".into(),
            label: "4da/src-tauri".into(),
        });
    }
    let facts = BriefFacts {
        security: vec![fact],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(
        out.contains(
            "Not counted: GHSA-33f5-2c5q-wgwj, GHSA-9g45-5xwm-f3wc — the code they name is \
             feature-gated out of this build."
        ),
        "{out}"
    );
    assert!(
        out.contains("upgrade victauri-plugin to 0.9.0 (4da/src-tauri already runs it, resolved to rmcp 3.4.1)"),
        "{out}"
    );
}

#[test]
fn an_empty_day_is_one_honest_line() {
    let out = build_deterministic_brief(&BriefFacts::default(), FloorReason::NoCapableModel);
    assert!(out.contains("Nothing new touches your code today."));
    assert!(!out.contains("## Still open"));
    assert!(
        !out.contains("C:\\") && !out.contains("D:\\"),
        "no absolute paths"
    );
}

/// Fresh-profile E2E 2026-10-09: with no lockfile read, the floor said
/// "Nothing new touches your code today" and "Computed from your lockfiles".
/// Nothing was checked, so it says so and names the action instead.
#[test]
fn no_lockfiles_is_never_an_all_clear() {
    let facts = BriefFacts {
        no_dependencies_known: true,
        ..BriefFacts::default()
    };
    for reason in [FloorReason::NoCapableModel, FloorReason::NarrationRejected] {
        let out = build_deterministic_brief(&facts, reason);
        assert!(!out.contains("Nothing new touches your code"), "{out}");
        assert!(!out.contains("from your lockfiles"), "{out}");
        assert!(out.contains(NO_LOCKFILES_LINE), "{out}");
        assert!(out.contains("Settings → Projects"), "{out}");
    }

    // Articles still show; the no-lockfiles line still replaces any all-clear.
    let with_articles = BriefFacts {
        no_dependencies_known: true,
        worth_knowing: vec![WorthKnowingCandidate {
            id: 1,
            title: "This Week in Rust 672".into(),
            url: Some("https://this-week-in-rust.org/".into()),
            source_type: "rss".into(),
            published: "2026-10-08".into(),
            excerpt: String::new(),
        }],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&with_articles, FloorReason::NoCapableModel);
    assert!(out.contains("## Worth knowing"), "{out}");
    assert!(out.contains(NO_LOCKFILES_LINE), "{out}");
}

/// The narrated path holds the same line whatever the model wrote, and the
/// prompt no longer hands it "none" lines that read as an all-clear.
#[test]
fn a_narration_without_lockfiles_ends_on_the_scan_instruction() {
    let unknown = BriefFacts {
        no_dependencies_known: true,
        ..BriefFacts::default()
    };
    let draft = "## Worth knowing\n- Tokio 2 is out.\n\nNothing new touches your code today.\n";
    let out = super::honest_without_lockfiles(draft, &unknown);
    assert!(!out.contains("Nothing new touches your code"), "{out}");
    assert!(out.contains("- Tokio 2 is out."), "{out}");
    assert!(out.trim_end().ends_with(NO_LOCKFILES_LINE), "{out}");
    assert_eq!(
        super::honest_without_lockfiles(&out, &unknown)
            .matches(NO_LOCKFILES_LINE)
            .count(),
        1,
        "idempotent"
    );
    assert_eq!(
        super::honest_without_lockfiles(draft, &BriefFacts::default()),
        draft,
        "a user with lockfiles is untouched"
    );

    let prompt = crate::digest_commands::render_facts_for_prompt(&unknown);
    assert!(prompt.contains("NOT checked"), "{prompt}");
    assert!(!prompt.contains("none —"), "{prompt}");
}

/// LWN's subscriber marker "[$]" closed the link label early and the brief
/// showed raw Markdown (fresh-profile E2E 2026-10-09).
#[test]
fn titles_are_escaped_where_they_enter_markdown() {
    let facts = BriefFacts {
        worth_knowing: vec![
            WorthKnowingCandidate {
                id: 2,
                title: "[$] An update on Rust's project goals".into(),
                url: Some("https://fedi.lwn.net/@lwn/117406357348125355".into()),
                source_type: "mastodon".into(),
                published: "2026-10-08".into(),
                excerpt: String::new(),
            },
            WorthKnowingCandidate {
                id: 3,
                title: "Why *every* `__init__` matters".into(),
                url: None,
                source_type: "devto".into(),
                published: "2026-10-08".into(),
                excerpt: String::new(),
            },
        ],
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(
        out.contains(
            "- [\\[$\\] An update on Rust's project goals](https://fedi.lwn.net/@lwn/117406357348125355) (mastodon)"
        ),
        "{out}"
    );
    assert!(
        out.contains("- Why \\*every\\* \\`\\_\\_init\\_\\_\\` matters (devto)"),
        "{out}"
    );
    assert_eq!(escape_markdown("a\\b"), "a\\\\b");
    assert_eq!(escape_markdown("plain title"), "plain title");
}

/// Fresh-profile audit E3 (2026-10-12): with lockfiles read but the advisory
/// database never synced, the brief said "Nothing new touches your code
/// today" against 156 vulnerable packages. Only a ready advisory check allows
/// an all-clear; every other state says plainly where the check stands.
#[test]
fn no_all_clear_until_the_advisory_check_is_ready() {
    use crate::osv::readiness::AdvisorySyncState as S;
    let cases = [
        (S::NotStarted, "has not been downloaded yet"),
        (S::Syncing, "downloading vulnerability advisories"),
        (S::Matching, "downloading vulnerability advisories"),
        (S::Failed, "check failed"),
        (S::Stale, "more than three days old"),
        (S::NoInventory, "can check against vulnerability advisories"),
    ];
    for (state, phrase) in cases {
        let facts = BriefFacts {
            advisories: Some(state),
            ..BriefFacts::default()
        };
        for reason in [
            FloorReason::NoCapableModel,
            FloorReason::NarrationRejected,
            FloorReason::Morning(super::MorningWhy::Skipped),
        ] {
            let out = build_deterministic_brief(&facts, reason);
            assert!(
                !out.contains("Nothing new touches your code"),
                "{state:?}: {out}"
            );
            assert!(out.contains(phrase), "{state:?}: {out}");
            assert!(!out.contains("OSV advisories"), "{state:?}: {out}");
        }
    }
    let ready = BriefFacts {
        advisories: Some(S::Ready),
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&ready, FloorReason::NoCapableModel);
    assert!(
        out.contains("Nothing new touches your code today."),
        "{out}"
    );
}

/// PR #961 review: when every project was skipped as a temp copy, the brief
/// names that reason (as Preemption does), not missing advisory coverage.
#[test]
fn no_inventory_from_temp_skips_names_the_temp_folder() {
    let facts = BriefFacts {
        advisories: Some(crate::osv::readiness::AdvisorySyncState::NoInventory),
        advisory_temp_skipped: 2,
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(out.contains("system temp folder"), "{out}");
    assert!(!out.contains("in an ecosystem it can check"), "{out}");
    assert!(!out.contains("Nothing new touches your code"), "{out}");
}

#[test]
fn a_narration_before_the_advisory_check_drops_its_all_clear() {
    let checking = BriefFacts {
        advisories: Some(crate::osv::readiness::AdvisorySyncState::Syncing),
        ..BriefFacts::default()
    };
    let draft = "## Worth knowing\n- Tokio 2 is out.\n\nNothing new touches your code today.\n";
    let out = super::honest_without_lockfiles(draft, &checking);
    assert!(!out.contains("Nothing new touches your code"), "{out}");
    assert!(out.contains("- Tokio 2 is out."), "{out}");
    assert!(
        out.contains("downloading vulnerability advisories"),
        "{out}"
    );
    assert_eq!(
        super::honest_without_lockfiles(&out, &checking),
        out,
        "idempotent"
    );

    let prompt = crate::digest_commands::render_facts_for_prompt(&checking);
    assert!(prompt.contains("ADVISORY CHECK NOT COMPLETE"), "{prompt}");
    assert!(!prompt.contains("none — no confirmed advisory"), "{prompt}");
}

fn not_covered(
    files: &[(&str, &str)],
    unreadable: &[(&str, &str)],
) -> crate::osv::readiness::NotCovered {
    crate::osv::readiness::NotCovered {
        count: files.len() as u32,
        files: files
            .iter()
            .map(|(path, eco)| crate::osv::readiness::NotCoveredFile {
                path: (*path).to_string(),
                ecosystem: (*eco).to_string(),
            })
            .collect(),
        unreadable_count: unreadable.len() as u32,
        unreadable: unreadable
            .iter()
            .map(|(path, reason)| crate::osv::readiness::UnreadableFile {
                path: (*path).to_string(),
                reason: (*reason).to_string(),
            })
            .collect(),
    }
}

fn ready_with_gaps(nc: crate::osv::readiness::NotCovered) -> BriefFacts {
    BriefFacts {
        advisories: Some(crate::osv::readiness::AdvisorySyncState::Ready),
        not_covered: Some(nc),
        ..BriefFacts::default()
    }
}

/// W5a: with a pom.xml 4DA does not read, "Nothing new touches your code
/// today" claimed code it never checked. The all-clear is scoped to what was
/// checked, and the files it did not check are named.
#[test]
fn the_all_clear_names_what_was_not_checked() {
    let facts = ready_with_gaps(not_covered(
        &[("C:/p/spring-petclinic/pom.xml", "Maven")],
        &[],
    ));
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(!out.contains("Nothing new touches your code"), "{out}");
    assert!(
        out.contains("Nothing new touches the code 4DA checked today."),
        "{out}"
    );
    assert!(
        out.contains(
            "Not checked: 1 dependency file 4DA does not read installed versions from: \
             spring-petclinic/pom.xml (Maven)."
        ),
        "{out}"
    );
}

#[test]
fn the_not_checked_line_counts_the_rest_and_names_unreadable_lockfiles() {
    let facts = ready_with_gaps(not_covered(
        &[
            ("C:/p/a/pom.xml", "Maven"),
            ("C:/p/b/build.gradle", "Gradle"),
            ("C:/p/c/build.gradle.kts", "Gradle"),
            ("C:/p/d/packages.lock.json", "NuGet lockfile"),
        ],
        &[("C:/p/web/package-lock.json", "invalid JSON")],
    ));
    let line = super::not_covered_line(&facts).expect("a line");
    assert!(line.contains("4 dependency files"), "{line}");
    assert!(
        line.contains("c/build.gradle.kts (Gradle), and 1 more."),
        "{line}"
    );
    assert!(
        !line.contains("packages.lock.json"),
        "only three are named: {line}"
    );
    assert!(
        line.contains(
            "1 lockfile could not be read, so what it pinned was not checked: \
             web/package-lock.json (invalid JSON)."
        ),
        "{line}"
    );
}

/// A narrated brief: the model's all-clear is scoped and the not-checked line
/// appended; running it twice changes nothing.
#[test]
fn a_narrated_all_clear_is_scoped_to_what_was_checked() {
    let facts = ready_with_gaps(not_covered(
        &[("C:/p/spring-petclinic/pom.xml", "Maven")],
        &[],
    ));
    let draft = "## Worth knowing\n- Vite 7.1\n\nNothing new touches your code today.\n";
    let out = super::honest_without_lockfiles(draft, &facts);
    assert!(!out.contains("Nothing new touches your code"), "{out}");
    assert!(
        out.contains("Nothing new touches the code 4DA checked today."),
        "{out}"
    );
    assert!(out.contains("spring-petclinic/pom.xml (Maven)"), "{out}");
    assert_eq!(super::honest_without_lockfiles(&out, &facts), out);
}

/// The model is told before it writes: complete only for what was read.
#[test]
fn the_prompt_says_not_everything_was_checked() {
    let facts = ready_with_gaps(not_covered(
        &[("C:/p/spring-petclinic/pom.xml", "Maven")],
        &[],
    ));
    let prompt = crate::digest_commands::render_facts_for_prompt(&facts);
    assert!(
        prompt.starts_with("NOT EVERYTHING WAS CHECKED:"),
        "{prompt}"
    );
    assert!(
        prompt.contains("spring-petclinic/pom.xml (Maven)"),
        "{prompt}"
    );
    assert!(!prompt.contains("authoritative and complete"), "{prompt}");
}

/// Every file read: the brief is as before.
#[test]
fn with_everything_read_the_all_clear_is_unchanged() {
    let facts = BriefFacts {
        advisories: Some(crate::osv::readiness::AdvisorySyncState::Ready),
        ..BriefFacts::default()
    };
    let out = build_deterministic_brief(&facts, FloorReason::NoCapableModel);
    assert!(
        out.contains("Nothing new touches your code today."),
        "{out}"
    );
    assert!(!out.contains("Not checked:"), "{out}");
    let draft = "Nothing new touches your code today.\n";
    assert_eq!(super::honest_without_lockfiles(draft, &facts), draft);
}
