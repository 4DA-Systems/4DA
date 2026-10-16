// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `brief_facts_build`'s direct-fix step (audit E8).

use super::*;

/// Audit E8 in the Brief: a declared copy's "bump" names the command that
/// keeps how it is declared (the tilde survives), read from the project's
/// own manifest; a path that is not a bump is left alone.
#[test]
fn a_bump_names_the_command_for_its_declaration() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3}"#,
    )
    .unwrap();
    let project = dir.path().to_string_lossy().to_string();
    let path = with_direct_fix(
        FixPath::Bump {
            to: "1.2.6".into(),
            fix: None,
        },
        &project,
        "npm",
        "minimist",
        Some("1.2.5"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("minimist", &path),
        "bump minimist to >= 1.2.6: run `npm install \"minimist@~1.2.6\"`"
    );
    // Not declared here: plain "bump", nothing invented.
    let plain = with_direct_fix(
        FixPath::Bump {
            to: "2.0.0".into(),
            fix: None,
        },
        &project,
        "npm",
        "absent",
        Some("1.0.0"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("absent", &plain),
        "bump absent to >= 2.0.0"
    );
    // A copy the declaration does not admit is another copy: nothing named.
    let other = with_direct_fix(
        FixPath::Bump {
            to: "1.2.6".into(),
            fix: None,
        },
        &project,
        "npm",
        "minimist",
        Some("0.0.8"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("minimist", &other),
        "bump minimist to >= 1.2.6"
    );
    let refresh = FixPath::Refresh {
        to: "1.2.6".into(),
        inferred: false,
        command: None,
    };
    assert_eq!(
        with_direct_fix(refresh.clone(), &project, "npm", "minimist", Some("1.2.5")),
        refresh
    );
}
