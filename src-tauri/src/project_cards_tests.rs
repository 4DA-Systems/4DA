use super::*;

fn row(
    path: &str,
    name: &str,
    deps: &[&str],
    days_ago: i64,
    now: chrono::DateTime<chrono::Utc>,
) -> ProjectRow {
    ProjectRow {
        path: path.to_string(),
        name: name.to_string(),
        languages: vec!["rust".into()],
        frameworks: vec![],
        dependencies: deps.iter().map(|d| d.to_string()).collect(),
        last_activity: Some(now - chrono::Duration::days(days_ago)),
    }
}

fn sep() -> &'static str {
    std::path::MAIN_SEPARATOR_STR
}

/// Nested packages fold into their root; a project idle for most of a year
/// gets no card.
#[test]
fn active_roots_absorb_their_packages_and_stale_projects_are_dropped() {
    let now = chrono::Utc::now();
    let s = sep();
    let rows = vec![
        row(
            &format!("{s}work{s}tools{s}crates{s}core"),
            "tools-core",
            &["tokio", "serde.workspace"],
            3,
            now,
        ),
        row(
            &format!("{s}work{s}tools"),
            "tools",
            &["clap", "tokio"],
            1,
            now,
        ),
        row(&format!("{s}old{s}mvp"), "old-mvp", &["express"], 300, now),
        row(&format!("{s}work{s}web"), "web", &["react"], 10, now),
    ];
    let cards = build_cards(&rows, now);
    let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["tools", "web"]);
    assert_eq!(cards[0].dependencies, ["clap", "tokio", "serde"]);
}

#[test]
fn readme_purpose_skips_badges_headings_and_code() {
    let readme = "# Tool\n\n[![ci](x)](y) ![badge](z)\n\n```sh\ncargo install tool --with a long line that is code not prose at all ok\n```\n\n\
        Tool coordinates many AI coding agents on one repository and re-runs their **claimed** proofs into signed verdicts.\n\nMore text.";
    assert_eq!(
        readme_purpose(readme).as_deref(),
        Some("Tool coordinates many AI coding agents on one repository and re-runs their claimed proofs into signed verdicts.")
    );
    assert_eq!(readme_purpose("# Title\n\nShort.\n"), None);
}

/// Decision B: README prose and hardware reach an on-machine judge only.
#[test]
fn cloud_judges_get_nouns_only() {
    let now = chrono::Utc::now();
    let cards = build_cards(
        &[row(&format!("{}p", sep()), "proj", &["axum"], 1, now)],
        now,
    );
    let readme = |_: &Path| {
        Some("Proj is a verification daemon that re-runs agent proofs and signs verdicts for teams.\n".to_string())
    };

    let cloud = render(&cards, false, readme);
    assert!(cloud.contains("Project proj\n"), "{cloud}");
    assert!(cloud.contains("Direct dependencies: axum"));
    assert!(
        !cloud.contains("verification daemon"),
        "no README prose off the machine"
    );
    assert!(!cloud.contains("Machine:"), "no hardware off the machine");

    let local = render(&cards, true, readme);
    assert!(
        local.contains("Project proj: Proj is a verification daemon"),
        "{local}"
    );
    assert!(local.contains("Machine:"));
}
