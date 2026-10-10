// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the metadata-only Context Files listing (`get_context_files`).

use super::*;

#[test]
fn count_lines_matches_str_lines() {
    for text in ["", "a", "a\n", "a\nb", "a\nb\n", "\n\n", "a\r\nb\r\n"] {
        assert_eq!(
            count_lines(text.as_bytes()),
            text.lines().count(),
            "{text:?}"
        );
    }
}

#[test]
fn page_window_defaults_clamps_and_never_overruns() {
    assert_eq!(page_window(1913, None, None), (0, 500, 500));
    assert_eq!(page_window(1913, Some(1900), None), (1900, 1913, 500));
    assert_eq!(page_window(1913, Some(5000), Some(10)), (1913, 1913, 10));
    assert_eq!(page_window(1913, None, Some(0)), (0, 1, 1));
    assert_eq!(page_window(10_000, None, Some(1_000_000)), (0, 2000, 2000));
    assert_eq!(page_window(0, None, None), (0, 0, 500));
}

/// The panel's payload is metadata: a 1 MB file contributes a path, a size
/// and a line count — never its bytes.
#[test]
fn context_meta_carries_no_contents_and_admission_rules_hold() {
    let dir = tempfile::tempdir().expect("tempdir");
    let big = "x".repeat(1024) + "\n";
    std::fs::write(dir.path().join("big.rs"), big.repeat(1024)).unwrap();
    std::fs::write(dir.path().join("notes.md"), "one\ntwo").unwrap();
    std::fs::write(dir.path().join("image.png"), [0u8; 16]).unwrap();
    std::fs::create_dir(dir.path().join("node_modules")).unwrap();
    std::fs::write(dir.path().join("node_modules").join("dep.js"), "x").unwrap();

    let mut paths = Vec::new();
    collect_context_paths(dir.path(), &mut paths, 0);
    paths.sort();
    let names: Vec<_> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["big.rs", "notes.md"]);

    let meta = context_file_meta(&paths[0]);
    assert_eq!(meta.lines, 1024);
    assert_eq!(meta.size_bytes, 1025 * 1024);
    assert_eq!(meta.kind, "rs");
    assert!(meta.modified_at.is_some());
    let json = serde_json::to_string(&meta).unwrap();
    assert!(
        json.len() < 512,
        "metadata row must not carry contents: {} bytes",
        json.len()
    );
    assert_eq!(context_file_meta(&paths[1]).lines, 2);
}
