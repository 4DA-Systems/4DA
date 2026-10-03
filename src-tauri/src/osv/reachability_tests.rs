// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::collections::HashSet;
use std::path::PathBuf;

use super::*;

/// A throwaway crate checkout: `files` are (crate-relative path, contents).
fn checkout(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("4da-reach-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (rel, body) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    if !files.iter().any(|(rel, _)| *rel == "Cargo.toml") {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
    }
    dir
}

fn feats(list: &[&str]) -> HashSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The gate shapes of rmcp 1.7.0, copied from the registry checkout on
/// 2026-10-03 (lib.rs:29, transport.rs:96/110/123, common.rs:6,
/// common/reqwest.rs:1, streamable_http_server.rs:2).
fn rmcp_like(tag: &str) -> PathBuf {
    checkout(
        tag,
        &[
            (
                "src/lib.rs",
                "#[cfg(feature = \"server\")]\npub mod task_manager;\n\
                 #[cfg(any(feature = \"client\", feature = \"server\"))]\npub mod transport;\n",
            ),
            ("src/task_manager.rs", ""),
            (
                "src/transport.rs",
                "#[cfg(feature = \"auth\")]\npub mod auth;\n\
                 #[cfg(feature = \"transport-streamable-http-server-session\")]\npub mod streamable_http_server;\n\
                 /// Common use codes\npub mod common;\n",
            ),
            ("src/transport/auth.rs", "pub struct ResourceServerMetadata;"),
            (
                "src/transport/common.rs",
                "pub mod http_header;\n#[cfg(feature = \"__reqwest\")]\nmod reqwest;\n",
            ),
            ("src/transport/common/http_header.rs", ""),
            (
                "src/transport/common/reqwest.rs",
                "#[cfg(feature = \"transport-streamable-http-client-reqwest\")]\nmod streamable_http_client;\n",
            ),
            ("src/transport/common/reqwest/streamable_http_client.rs", "fn default_http_client() {}"),
            (
                "src/transport/streamable_http_server.rs",
                "pub mod session;\n\
                 #[cfg(all(feature = \"transport-streamable-http-server\", not(feature = \"local\")))]\npub mod tower;\n",
            ),
            ("src/transport/streamable_http_server/session.rs", ""),
            ("src/transport/streamable_http_server/tower.rs", "fn handle_post() {}"),
        ],
    )
}

/// What atlas's bridge resolves for rmcp 1.7.0 (`cargo tree --format {f}`).
fn bridge_features() -> HashSet<String> {
    feats(&[
        "base64",
        "default",
        "macros",
        "server",
        "server-side-http",
        "tower",
        "transport-async-rw",
        "transport-streamable-http-server",
        "transport-streamable-http-server-session",
        "transport-worker",
        "uuid",
    ])
}

#[test]
fn the_bridge_build_compiles_the_session_bug_and_not_the_client_bugs() {
    let dir = rmcp_like("bridge");
    let f = bridge_features();
    // GHSA-33f5 / GHSA-c9xm: OAuth client.
    assert_eq!(file_compiled(&dir, "src/transport/auth.rs", &f), Tri::No);
    // GHSA-9g45: reqwest client transport (two gates deep).
    assert_eq!(
        file_compiled(
            &dir,
            "src/transport/common/reqwest/streamable_http_client.rs",
            &f
        ),
        Tri::No
    );
    // GHSA-9pj6: the server's session table — compiled, `local` off.
    assert_eq!(
        file_compiled(&dir, "src/transport/streamable_http_server/tower.rs", &f),
        Tri::Yes
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_client_build_compiles_the_client_bugs() {
    let dir = rmcp_like("client");
    let f = feats(&[
        "client",
        "auth",
        "__reqwest",
        "transport-streamable-http-client-reqwest",
    ]);
    assert_eq!(file_compiled(&dir, "src/transport/auth.rs", &f), Tri::Yes);
    assert_eq!(
        file_compiled(
            &dir,
            "src/transport/common/reqwest/streamable_http_client.rs",
            &f
        ),
        Tri::Yes
    );
    assert_eq!(
        reach_in_checkout(&dir, &["src/transport/auth.rs".to_string()], &f),
        Reach::Compiled
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_local_is_decided_by_the_features_actually_on() {
    let dir = rmcp_like("local");
    let mut f = bridge_features();
    f.insert("local".to_string());
    assert_eq!(
        file_compiled(&dir, "src/transport/streamable_http_server/tower.rs", &f),
        Tri::No
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_advisory_is_not_compiled_only_when_every_named_file_is_off() {
    let dir = rmcp_like("all-off");
    let f = bridge_features();
    let client_only = vec![
        "src/transport/auth.rs".to_string(),
        "src/transport/common/reqwest/streamable_http_client.rs".to_string(),
    ];
    assert_eq!(
        reach_in_checkout(&dir, &client_only, &f),
        Reach::NotCompiled
    );
    // One compiled file among them keeps the advisory.
    let mixed = vec![
        "src/transport/auth.rs".to_string(),
        "src/transport/streamable_http_server/tower.rs".to_string(),
    ];
    assert_eq!(reach_in_checkout(&dir, &mixed, &f), Reach::Compiled);
    // A file the checkout does not have is unknown, and unknown keeps it.
    let ghost = vec![
        "src/transport/auth.rs".to_string(),
        "src/transport/gone.rs".to_string(),
    ];
    assert_eq!(reach_in_checkout(&dir, &ghost, &f), Reach::Unknown);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn naming_no_file_is_unknown_not_vacuously_off() {
    let dir = rmcp_like("nofile");
    assert_eq!(
        reach_in_checkout(&dir, &[], &bridge_features()),
        Reach::Unknown
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn instance(project: &str, version: &str, confirmed: bool) -> MatchedDependency {
    MatchedDependency {
        project_path: project.to_string(),
        installed_version: Some(version.to_string()),
        is_direct: false,
        is_dev: false,
        is_version_confirmed: confirmed,
        fixed_version: Some("2.0.0".to_string()),
        clean_version: None,
    }
}

fn advisory(eco: &str, details: &str) -> StoredAdvisory {
    StoredAdvisory {
        id: 1,
        advisory_id: "GHSA-33f5-2c5q-wgwj".to_string(),
        summary: "RMCP: Missing Resource Field Validation".to_string(),
        details: Some(details.to_string()),
        package_name: "rmcp".to_string(),
        ecosystem: eco.to_string(),
        affected_ranges: None,
        fixed_versions: None,
        severity_type: None,
        cvss_score: Some(8.2),
        source_url: None,
        published_at: None,
        modified_at: None,
        withdrawn_at: None,
        synced_at: String::new(),
        aliases: Vec::new(),
        severity_label: Some("high".to_string()),
    }
}

/// A filter whose feature cache says `bridge` builds rmcp 1.7.0 with the
/// bridge's features, `client` with the client's, and `nocache` is unknown.
fn filter_over(dir: PathBuf) -> ReachFilter {
    let mut f = ReachFilter {
        source_dir: Box::new(move |_, _| Some(dir.clone())),
        ..ReachFilter::default()
    };
    let key = ("rmcp".to_string(), "1.7.0".to_string());
    f.features.insert(
        "bridge".into(),
        Some(HashMap::from([(key.clone(), bridge_features())])),
    );
    f.features.insert(
        "client".into(),
        Some(HashMap::from([(key, feats(&["client", "auth"]))])),
    );
    f.features.insert("nocache".into(), None);
    f
}

#[test]
fn the_filter_drops_only_the_copy_that_cannot_compile_the_bug() {
    let dir = rmcp_like("filter");
    let db = crate::test_utils::test_db();
    let mut filter = filter_over(dir.clone());
    let mut instances = vec![
        instance("bridge", "1.7.0", true),
        instance("client", "1.7.0", true),
        instance("nocache", "1.7.0", true),
        instance("bridge", "1.7.0", false),
    ];
    let adv = advisory("crates.io", "in `crates/rmcp/src/transport/auth.rs`");
    filter.retain_compiled(&db, &adv, &mut instances);

    let kept: Vec<(&str, bool)> = instances
        .iter()
        .map(|i| (i.project_path.as_str(), i.is_version_confirmed))
        .collect();
    assert_eq!(
        kept,
        vec![("client", true), ("nocache", true), ("bridge", false)],
        "client compiles auth; no feature cache is unknown; an unconfirmed version is unknown"
    );
    assert_eq!(filter.excluded.len(), 1);
    assert_eq!(filter.excluded[0].project_path, "bridge");
    assert_eq!(filter.excluded[0].files, vec!["src/transport/auth.rs"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_filter_leaves_other_ecosystems_and_fileless_advisories_alone() {
    let dir = rmcp_like("filter-skip");
    let db = crate::test_utils::test_db();
    let mut filter = filter_over(dir.clone());
    let mut instances = vec![instance("bridge", "1.7.0", true)];
    // Same text, npm ecosystem: never touched.
    filter.retain_compiled(
        &db,
        &advisory("npm", "`crates/rmcp/src/transport/auth.rs`"),
        &mut instances,
    );
    // crates.io, but the text names no file.
    filter.retain_compiled(
        &db,
        &advisory("crates.io", "an OAuth client bug"),
        &mut instances,
    );
    assert_eq!(instances.len(), 1);
    assert!(filter.excluded.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- conservative fallbacks: each of these must never answer "No" -------

#[test]
fn an_undecidable_cfg_is_maybe_not_no() {
    let dir = checkout(
        "unix",
        &[
            ("src/lib.rs", "#[cfg(unix)]\nmod a;\n#[cfg(all(feature = \"x\", target_os = \"linux\"))]\nmod b;\n"),
            ("src/a.rs", ""),
            ("src/b.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/a.rs", &feats(&[])), Tri::Maybe);
    assert_eq!(file_compiled(&dir, "src/b.rs", &feats(&["x"])), Tri::Maybe);
    // ...but a definitely-off feature inside all() still decides it.
    assert_eq!(file_compiled(&dir, "src/b.rs", &feats(&[])), Tri::No);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_redirect_is_maybe() {
    let dir = checkout(
        "path",
        &[
            ("src/lib.rs", "#[cfg(feature = \"x\")]\n#[path = \"other.rs\"]\nmod a;\n#[cfg_attr(windows, path = \"w.rs\")]\nmod b;\n"),
            ("src/a.rs", ""),
            ("src/b.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/a.rs", &feats(&[])), Tri::Maybe);
    assert_eq!(file_compiled(&dir, "src/b.rs", &feats(&[])), Tri::Maybe);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_module_declared_inside_a_macro_is_maybe() {
    // tokio's `cfg_net! { pub mod net; }`: the gate is in the macro.
    let dir = checkout(
        "macro",
        &[
            ("src/lib.rs", "cfg_net! {\n    pub mod net;\n}\n"),
            ("src/net.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/net.rs", &feats(&[])), Tri::Maybe);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unparseable_file_is_maybe() {
    let dir = checkout(
        "parse",
        &[
            (
                "src/lib.rs",
                "#[cfg(feature = \"x\")]\nmod a;\nfn broken( {",
            ),
            ("src/a.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/a.rs", &feats(&[])), Tri::Maybe);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_custom_lib_root_outside_src_is_maybe() {
    let dir = checkout(
        "libpath",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"x\"\nversion = \"1.0.0\"\n\n[lib]\npath = \"lib.rs\"\n",
            ),
            ("lib.rs", "#[cfg(feature = \"x\")]\nmod a;\n"),
            ("src/a.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/a.rs", &feats(&[])), Tri::Maybe);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- layouts the walk must follow ------------------------------------------

#[test]
fn duplicate_declarations_compile_the_file_if_any_does() {
    let dir = checkout(
        "dup",
        &[
            (
                "src/lib.rs",
                "#[cfg(feature = \"a\")]\nmod x;\n#[cfg(feature = \"b\")]\nmod x;\n",
            ),
            ("src/x.rs", ""),
        ],
    );
    assert_eq!(file_compiled(&dir, "src/x.rs", &feats(&["b"])), Tri::Yes);
    assert_eq!(file_compiled(&dir, "src/x.rs", &feats(&[])), Tri::No);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mod_rs_layout_inline_modules_and_inner_cfg_are_followed() {
    let dir = checkout(
        "layout",
        &[
            (
                "src/lib.rs",
                "pub mod outer;\nmod wrap {\n    #[cfg(feature = \"w\")]\n    pub mod deep;\n}\n",
            ),
            (
                "src/outer/mod.rs",
                "#![cfg(feature = \"o\")]\npub mod leaf;\n",
            ),
            ("src/outer/leaf.rs", ""),
            ("src/wrap/deep.rs", ""),
        ],
    );
    assert_eq!(
        file_compiled(&dir, "src/outer/leaf.rs", &feats(&[])),
        Tri::No,
        "inner #![cfg]"
    );
    assert_eq!(
        file_compiled(&dir, "src/outer/leaf.rs", &feats(&["o"])),
        Tri::Yes
    );
    assert_eq!(
        file_compiled(&dir, "src/outer/mod.rs", &feats(&["o"])),
        Tri::Yes,
        "mod.rs names the module"
    );
    assert_eq!(
        file_compiled(&dir, "src/wrap/deep.rs", &feats(&[])),
        Tri::No,
        "inline module"
    );
    assert_eq!(
        file_compiled(&dir, "src/wrap/deep.rs", &feats(&["w"])),
        Tri::Yes
    );
    assert_eq!(
        file_compiled(&dir, "src/lib.rs", &feats(&[])),
        Tri::Yes,
        "the root itself"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn not_and_any_combinators() {
    let parse = |s: &str| syn::parse_str::<syn::Meta>(s).unwrap();
    let f = feats(&["a"]);
    assert_eq!(eval_cfg(&parse("feature = \"a\""), &f), Tri::Yes);
    assert_eq!(eval_cfg(&parse("not(feature = \"a\")"), &f), Tri::No);
    assert_eq!(
        eval_cfg(&parse("any(feature = \"b\", feature = \"a\")"), &f),
        Tri::Yes
    );
    assert_eq!(
        eval_cfg(&parse("any(feature = \"b\", unix)"), &f),
        Tri::Maybe
    );
    assert_eq!(eval_cfg(&parse("all(feature = \"b\", unix)"), &f), Tri::No);
    assert_eq!(eval_cfg(&parse("not(unix)"), &f), Tri::Maybe);
}

// ---- reading file names out of advisory text -------------------------------

/// Live: the real rmcp 1.7.0 checkout in cargo's registry against the
/// features atlas's bridge resolves. `cargo test --lib live_rmcp -- --ignored`.
#[test]
#[ignore = "reads the machine's cargo registry"]
fn live_rmcp_1_7_0_checkout_against_the_bridge_build() {
    let dir = crate_source_dir("rmcp", "1.7.0").expect("rmcp 1.7.0 in the cargo registry");
    let f = bridge_features();
    let verdict = |rel: &str| reach_in_checkout(&dir, &[rel.to_string()], &f);
    assert_eq!(
        verdict("src/transport/auth.rs"),
        Reach::NotCompiled,
        "GHSA-33f5, GHSA-c9xm"
    );
    assert_eq!(
        verdict("src/transport/common/reqwest/streamable_http_client.rs"),
        Reach::NotCompiled,
        "GHSA-9g45"
    );
    assert_eq!(
        verdict("src/transport/streamable_http_server/tower.rs"),
        Reach::Compiled,
        "GHSA-9pj6"
    );
}

#[test]
fn files_are_read_from_the_four_real_rmcp_advisories() {
    // Excerpts of the stored details, 2026-10-03.
    let ghsa_33f5 =
        "In the current implementation (`crates/rmcp/src/transport/auth.rs`), the struct";
    let ghsa_9g45 =
        "**File:** `crates/rmcp/src/transport/common/reqwest/streamable_http_client.rs`";
    let ghsa_9pj6 =
        "The bug lives in `crates/rmcp/src/transport/streamable_http_server/tower.rs` inside";
    let ghsa_c9xm = "- Affected file: `crates/rmcp/src/transport/auth.rs`\n- File blob: `3aa3e9`";
    assert_eq!(
        named_source_files(ghsa_33f5, "rmcp"),
        vec!["src/transport/auth.rs"]
    );
    assert_eq!(
        named_source_files(ghsa_9g45, "rmcp"),
        vec!["src/transport/common/reqwest/streamable_http_client.rs"]
    );
    assert_eq!(
        named_source_files(ghsa_9pj6, "rmcp"),
        vec!["src/transport/streamable_http_server/tower.rs"]
    );
    assert_eq!(
        named_source_files(ghsa_c9xm, "rmcp"),
        vec!["src/transport/auth.rs"]
    );
}

#[test]
fn other_crates_paths_and_non_paths_are_skipped() {
    let text = "see crates/rmcp-macros/src/lib.rs, tokio/src/net/tcp.rs, mysrc/x.rs, \
                src/transport/auth.rsx and https://github.com/o/rmcp/blob/main/src/lib.rs#L4 \
                then src/handler.rs, again src/handler.rs.";
    assert_eq!(
        named_source_files(text, "rmcp"),
        vec!["src/lib.rs", "src/handler.rs"],
        "the blob URL is this repo's own src; duplicates collapse"
    );
    assert!(named_source_files("no paths here", "rmcp").is_empty());
}

/// Advisory text is prose with em-dashes and arrows. A multi-byte character
/// right before a path segment must not land a byte slice mid-character
/// (a panic inside the matcher would cost the user every surface).
#[test]
fn multi_byte_text_around_paths_never_panics() {
    let text = "fix—crates/rmcp/src/a.rs → see «src/b.rs» and ünïcode/src/c.rs; —/src/d.rs";
    let files = named_source_files(text, "rmcp");
    assert!(files.contains(&"src/a.rs".to_string()), "{files:?}");
    assert!(
        !files.contains(&"src/c.rs".to_string()),
        "another dir's path: {files:?}"
    );
    // Every non-ASCII boundary combination, exhaustively over a sample.
    for prefix in ["—", "é", "→", "日本", "🦀"] {
        for joiner in ["", "/", "x/"] {
            let t = format!("{prefix}{joiner}src/z.rs {prefix}");
            let _ = named_source_files(&t, "rmcp");
        }
    }
}

#[test]
fn hyphen_and_underscore_spellings_name_the_same_crate() {
    assert_eq!(
        named_source_files("in crates/foo_bar/src/a.rs", "foo-bar"),
        vec!["src/a.rs"]
    );
}
