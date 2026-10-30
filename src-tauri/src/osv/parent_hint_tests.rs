// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `osv::parent_hint`'s database half: the registry-row standing.
//! The link rules' tests moved with them to `fourda_depengine::parent_hint`
//! (single-engine C0.4).

use super::*;

fn rows(titles: &[&str]) -> Vec<(i64, String)> {
    titles
        .iter()
        .enumerate()
        .map(|(i, t)| (i as i64 + 1, (*t).to_string()))
        .collect()
}

/// Live 2026-10-12: request 2.88.2 is the last request release and is
/// deprecated. The registry rows 4DA holds say so; nothing is fetched.
#[test]
fn a_parent_at_its_newest_deprecated_release_reads_as_such() {
    let held = rows(&["npm: request v2.88.2", "[DEPRECATED] request"]);
    assert_eq!(
        standing_from(&held, "request", "2.88.2"),
        ParentStanding::Newest { deprecated: true }
    );
    // Deprecation without a recorded version still says deprecated.
    assert_eq!(
        standing_from(&rows(&["[DEPRECATED] request"]), "request", "2.88.2"),
        ParentStanding::Deprecated
    );
    // Not deprecated, already the newest.
    assert_eq!(
        standing_from(
            &rows(&["npm: javascript-opentimestamps v0.4.5"]),
            "javascript-opentimestamps",
            "0.4.5"
        ),
        ParentStanding::Newest { deprecated: false }
    );
}

#[test]
fn a_newer_parent_release_on_record_is_named_and_lookalikes_are_not() {
    let held = rows(&[
        "npm: minimatch v9.0.5",
        "npm: minimatch v10.2.6",
        "npm: minimatch-browser v12.0.0",
        "npm: minimatch v11.0.0-rc.1",
    ]);
    assert_eq!(
        standing_from(&held, "minimatch", "3.1.5"),
        ParentStanding::Newer {
            latest: "10.2.6".into()
        },
        "the highest stable release of THIS package"
    );
    // No row for the package: nothing is claimed either way.
    assert_eq!(
        standing_from(&held, "request", "2.88.2"),
        ParentStanding::Unknown
    );
    // A later undeprecation: the newest row decides.
    let undeprecated = rows(&["[DEPRECATED] left-pad", "npm: left-pad v1.3.0"]);
    assert_eq!(
        standing_from(&undeprecated, "left-pad", "1.3.0"),
        ParentStanding::Newest { deprecated: false }
    );
    // crates.io titles read the same way.
    assert_eq!(
        standing_from(
            &rows(&["crates.io: victauri-plugin v0.9.0"]),
            "victauri-plugin",
            "0.8.4"
        ),
        ParentStanding::Newer {
            latest: "0.9.0".into()
        }
    );
}
