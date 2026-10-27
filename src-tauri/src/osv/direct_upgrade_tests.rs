// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `osv::direct_upgrade`. Every command shape was run against the
//! real tool on 2026-10-15 and kept the dependency as declared.

use super::*;

#[test]
fn npm_family_commands_keep_the_declared_operator() {
    let cases: &[(Manager, &str, &str)] = &[
        // npm: an exact pin needs --save-exact, else npm writes ^target.
        (
            Manager::Npm,
            "1.2.5",
            "npm install minimist@1.2.8 --save-exact",
        ),
        (Manager::Npm, "^1.2.5", "npm install \"minimist@^1.2.8\""),
        // npm turned `~2.1.1` into `^2.1.3` without the operator.
        (Manager::Npm, "~1.2.5", "npm install \"minimist@~1.2.8\""),
        (Manager::Pnpm, "~1.2.5", "pnpm up \"minimist@~1.2.8\""),
        (Manager::PnpmWorkspace, "1.2.5", "pnpm up minimist@1.2.8"),
        // yarn 1 and berry wrote an exact pin over ^ and ~ without the operator.
        (
            Manager::YarnClassic,
            "^1.2.5",
            "yarn upgrade \"minimist@^1.2.8\"",
        ),
        (Manager::YarnBerry, "~1.2.5", "yarn up \"minimist@~1.2.8\""),
        (Manager::YarnBerry, "1.2.5", "yarn up minimist@1.2.8"),
    ];
    for (manager, declared, want) in cases {
        assert_eq!(
            npm_direct_command(*manager, declared, "minimist", "1.2.8").as_deref(),
            Some(*want),
            "{manager:?} {declared}"
        );
    }
    assert_eq!(
        npm_direct_command(Manager::Npm, "^7.20.5", "@babel/traverse", "7.23.2").as_deref(),
        Some("npm install \"@babel/traverse@^7.23.2\"")
    );
}

#[test]
fn npm_specs_it_cannot_carry_name_no_command() {
    for declared in [
        ">=1.2.0",
        "1.x",
        "*",
        "latest",
        "^1.2 || ^2",
        "workspace:*",
        "github:a/b",
        "npm:other@^1",
        "file:../x",
    ] {
        assert_eq!(
            npm_direct_command(Manager::Npm, declared, "x", "1.2.8"),
            None,
            "{declared}"
        );
    }
}

#[test]
fn npm_declared_reads_one_section() {
    let pkg = r#"{"dependencies": {"ms": "~2.1.1"}, "devDependencies": {"jest": "^29.0.0", "ms": "2.1.1"}}"#;
    assert_eq!(npm_declared(pkg, "jest").as_deref(), Some("^29.0.0"));
    // Declared twice: ambiguous, so nothing is named.
    assert_eq!(npm_declared(pkg, "ms"), None);
    assert_eq!(npm_declared(pkg, "absent"), None);
}

const CARGO: &str = r#"
[package]
name = "t"
version = "0.1.0"

[dependencies]
serde = { version = "1.0.100", features = ["derive"] }
myitoa = { package = "itoa", version = "1.0.1" }
exact = "=1.0.1"
tokio.workspace = true

[dev-dependencies]
ryu = "~1.0.5"

[build-dependencies]
cc = "1.0.50"

[target.'cfg(windows)'.dependencies]
winapi = "0.3.8"

[dependencies.regex]
version = "1.5.0"
"#;

fn cargo_cmd(package: &str, target: &str) -> Option<DirectFix> {
    match cargo_declarations(CARGO, package).as_slice() {
        [only] => cargo_direct(only, package, target),
        other => panic!("{package}: {other:?}"),
    }
}

#[test]
fn cargo_commands_follow_the_declaring_table() {
    let cmd = |p: &str, t: &str| match cargo_cmd(p, t) {
        Some(DirectFix::Command(c)) => c,
        other => panic!("{p}: {other:?}"),
    };
    assert_eq!(cmd("serde", "1.0.200"), "cargo add serde@1.0.200");
    assert_eq!(
        cmd("itoa", "1.0.11"),
        "cargo add itoa@1.0.11 --rename myitoa"
    );
    assert_eq!(cmd("exact", "1.0.11"), "cargo add \"exact@=1.0.11\"");
    assert_eq!(cmd("ryu", "1.0.15"), "cargo add \"ryu@~1.0.15\" --dev");
    assert_eq!(cmd("cc", "1.0.83"), "cargo add cc@1.0.83 --build");
    assert_eq!(
        cmd("winapi", "0.3.9"),
        "cargo add winapi@0.3.9 --target \"cfg(windows)\""
    );
    assert_eq!(cmd("regex", "1.5.6"), "cargo add regex@1.5.6");
}

#[test]
fn a_workspace_inherited_crate_moves_in_the_root() {
    // `cargo add` here replaced `tokio.workspace = true` with a local pin (cargo 1.95).
    match cargo_cmd("tokio", "1.38.2") {
        Some(DirectFix::Instruction(i)) => {
            assert!(i.contains("[workspace.dependencies]"), "{i}");
            assert!(!i.starts_with("cargo add"), "{i}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn cargo_declared_twice_or_not_at_all_names_nothing() {
    let twice = "[dependencies]\nitoa = \"1\"\n\n[dev-dependencies]\nitoa = \"1\"\n";
    assert_eq!(cargo_declarations(twice, "itoa").len(), 2);
    assert!(cargo_declarations(CARGO, "absent").is_empty());
    let range = "[dependencies]\nitoa = \">=1.0, <2\"\n";
    let decls = cargo_declarations(range, "itoa");
    assert_eq!(cargo_direct(&decls[0], "itoa", "1.0.11"), None);
}

/// `cargo add <crate>@<v>` on a git, path or alternate-registry dependency
/// re-points it at crates.io: those name nothing.
#[test]
fn a_crate_from_another_source_names_nothing() {
    let manifest = r#"
[dependencies]
a = { git = "https://example.com/a", version = "1.0.0" }
b = { path = "../b", version = "1.0.0" }
c = { version = "1.0.0", registry = "corp" }

[dependencies.d]
version = "1.0.0"
git = "https://example.com/d"
"#;
    for package in ["a", "b", "c", "d"] {
        let decls = cargo_declarations(manifest, package);
        assert_eq!(decls.len(), 1, "{package}");
        assert!(decls[0].other_source, "{package}");
        assert_eq!(cargo_direct(&decls[0], package, "1.0.1"), None, "{package}");
    }
}

#[test]
fn go_get_only_for_a_direct_requirement() {
    let go_mod = "module example.com/m\n\ngo 1.22\n\nrequire golang.org/x/text v0.3.7\n\nrequire (\n\tgolang.org/x/net v0.20.0\n\tgolang.org/x/sys v0.16.0 // indirect\n)\n";
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/text", "0.3.7", "0.3.8").as_deref(),
        Some("go get golang.org/x/text@v0.3.8")
    );
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/net", "v0.20.0", "v0.23.0").as_deref(),
        Some("go get golang.org/x/net@v0.23.0")
    );
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/sys", "0.16.0", "0.20.0"),
        None
    );
    // Required at another version than the one installed: nothing.
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/text", "0.3.6", "0.3.8"),
        None
    );
}

/// A `replace` points the module elsewhere: `go get` would not move what is built.
#[test]
fn a_replaced_go_module_names_nothing() {
    let single =
        "module m\n\nrequire golang.org/x/text v0.3.7\n\nreplace golang.org/x/text => ../text\n";
    let block = "module m\n\nrequire golang.org/x/text v0.3.7\n\nreplace (\n\tgolang.org/x/text v0.3.7 => example.com/fork v0.3.9\n)\n";
    for go_mod in [single, block] {
        assert_eq!(
            go_direct_command(go_mod, "golang.org/x/text", "0.3.7", "0.3.8"),
            None,
            "{go_mod}"
        );
    }
}

#[test]
fn a_pinned_requirement_gets_an_exact_edit() {
    let req = "Django==4.2 # web\nrequests>=2.0\n";
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "django", "4.2", "5.2.17").as_deref(),
        Some("change `Django==4.2 # web` to `Django==5.2.17 # web` in requirements.txt")
    );
    // Not an == pin: no edit is invented.
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "requests", "2.31.0", "2.32.0"),
        None
    );
    // Pinned at another version than the one installed: nothing.
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "django", "4.1", "5.2.17"),
        None
    );
}

/// Only the version token changes: markers and extras stay.
#[test]
fn a_pin_keeps_its_markers_and_extras() {
    let req = "requests[socks]==2.31.0 ; python_version >= \"3.8\"\nurllib3==1.26.5;python_version<\"3.12\"\n";
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "requests", "2.31.0", "2.32.4").as_deref(),
        Some(
            "change `requests[socks]==2.31.0 ; python_version >= \"3.8\"` to \
             `requests[socks]==2.32.4 ; python_version >= \"3.8\"` in requirements.txt"
        )
    );
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "urllib3", "1.26.5", "1.26.19").as_deref(),
        Some(
            "change `urllib3==1.26.5;python_version<\"3.12\"` to \
             `urllib3==1.26.19;python_version<\"3.12\"` in requirements.txt"
        )
    );
}

/// A hashed pin fails `--require-hashes` once the version alone changes, and
/// pip-compile output is regenerated from its `.in` source: both name nothing.
#[test]
fn hashed_or_compiled_requirements_name_nothing() {
    let continued = "django==4.2 \\\n    --hash=sha256:abc\n";
    let inline = "django==4.2 --hash=sha256:abc\n";
    let next_line = "django==4.2\n    --hash=sha256:abc\n";
    for req in [continued, inline, next_line] {
        assert_eq!(
            pypi_direct_instruction(req, "requirements.txt", "django", "4.2", "4.2.17"),
            None,
            "{req}"
        );
    }
    let compiled =
        "#\n# This file is autogenerated by pip-compile with Python 3.12\n#\ndjango==4.2\n";
    assert_eq!(
        pypi_direct_instruction(compiled, "requirements.txt", "django", "4.2", "4.2.17"),
        None
    );
}

#[test]
fn the_reader_uses_the_projects_own_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"devDependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3}"#,
    )
    .unwrap();
    assert_eq!(
        direct_fix(dir.path(), "npm", "minimist", "1.2.6", "1.2.8"),
        Some(DirectFix::Refresh("npm update minimist".to_string()))
    );
    assert_eq!(
        direct_fix(dir.path(), "npm", "minimist", "1.2.6", "1.3.0"),
        Some(DirectFix::Command(
            "npm install \"minimist@~1.3.0\"".to_string()
        ))
    );
    assert_eq!(
        direct_fix(dir.path(), "PyPI", "django", "4.2", "5.2.17"),
        None
    );
}

/// The declaration must admit the copy that is installed: otherwise the row
/// is about another copy than the one this manifest declares.
#[test]
fn a_declaration_that_does_not_admit_the_installed_copy_names_nothing() {
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
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[dependencies]\nitoa = \"~1.0.5\"\n",
    )
    .unwrap();
    assert_eq!(
        direct_fix(dir.path(), "npm", "minimist", "0.0.8", "1.2.8"),
        None
    );
    assert_eq!(
        direct_fix(dir.path(), "crates.io", "itoa", "0.4.8", "1.0.11"),
        None
    );
    assert_eq!(
        direct_fix(dir.path(), "crates.io", "itoa", "1.0.6", "1.0.11"),
        Some(DirectFix::Refresh(
            "cargo update -p itoa@1.0.6 --precise 1.0.11".to_string()
        ))
    );
    assert_eq!(
        direct_fix(dir.path(), "crates.io", "itoa", "1.0.6", "1.1.0"),
        Some(DirectFix::Command("cargo add \"itoa@~1.1.0\"".to_string()))
    );
}

/// `yarn up` / `yarn upgrade` act on the whole workspace, so a member (whose
/// yarn.lock sits in the root) names nothing; the root itself does.
#[test]
fn a_yarn_workspace_member_names_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let member = root.path().join("packages").join("app");
    std::fs::create_dir_all(&member).unwrap();
    std::fs::write(root.path().join("yarn.lock"), "# yarn lockfile v1\n").unwrap();
    let pkg = r#"{"dependencies": {"minimist": "^1.2.5"}}"#;
    std::fs::write(root.path().join("package.json"), pkg).unwrap();
    std::fs::write(member.join("package.json"), pkg).unwrap();
    assert_eq!(
        direct_fix(&member, "npm", "minimist", "1.2.6", "1.2.8"),
        None
    );
    assert_eq!(
        direct_fix(root.path(), "npm", "minimist", "1.2.6", "1.2.8"),
        Some(DirectFix::Refresh("yarn upgrade minimist".to_string()))
    );
    assert_eq!(
        direct_fix(root.path(), "npm", "minimist", "1.2.6", "2.0.0"),
        Some(DirectFix::Command(
            "yarn upgrade \"minimist@^2.0.0\"".to_string()
        ))
    );
}

/// Findings carry project paths lowercased (`normalize_project_path`). On a
/// case-sensitive filesystem that path does not exist, so the manifest read
/// failed and the row named no command (#972's Linux queue run): the
/// directory on disk is resolved first, component by component.
#[test]
fn a_lowercased_project_path_reads_the_manifest_on_disk() {
    let root = tempfile::tempdir().expect("tempdir");
    let project = root.path().join("Work").join("MyApp");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("package.json"),
        r#"{"dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        project.join("package-lock.json"),
        r#"{"lockfileVersion": 3}"#,
    )
    .unwrap();
    let lowered = format!(
        "{}/work/myapp",
        root.path().to_string_lossy().replace('\\', "/")
    );
    let resolved = on_disk(std::path::Path::new(&lowered)).expect("resolves");
    assert!(resolved.join("package.json").is_file(), "{resolved:?}");
    assert_eq!(
        direct_fix(
            std::path::Path::new(&lowered),
            "npm",
            "minimist",
            "1.2.5",
            "1.2.6"
        ),
        Some(DirectFix::Refresh("npm update minimist".to_string()))
    );
    assert_eq!(
        on_disk(std::path::Path::new(&format!("{lowered}/absent"))),
        None
    );
}

/// Live 2026-10-18 (navcal): sharp, next and vite were told to
/// `npm install "x@^v"` though their `^` range already admitted the fix. When
/// the declaration admits the target, the advice is the tool's in-range
/// lockfile refresh, which leaves the manifest as it is. Each command below
/// was run on 2026-10-18 and kept package.json unchanged (pnpm needs
/// `--no-save`: plain `pnpm update` rewrote `^2.1.1` to `^2.1.3`).
#[test]
fn a_declaration_that_admits_the_target_gets_a_refresh_not_a_bump() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "package-lock.json",
            "{\"lockfileVersion\": 3}",
            "npm update ms",
        ),
        (
            "pnpm-lock.yaml",
            "lockfileVersion: '9.0'\n",
            "pnpm update ms --no-save",
        ),
        ("yarn.lock", "# yarn lockfile v1\n\n", "yarn upgrade ms"),
        (
            "yarn.lock",
            "# This file is generated by running \"yarn install\"\n\n__metadata:\n  version: 8\n",
            "yarn up -R ms",
        ),
    ];
    for (lockfile, body, want) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"ms": "^2.1.1"}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join(lockfile), body).unwrap();
        assert_eq!(
            direct_fix(dir.path(), "npm", "ms", "2.1.1", "2.1.3"),
            Some(DirectFix::Refresh((*want).to_string())),
            "{lockfile}"
        );
    }
    // A `>=` range admits the target but has no upper bound: a refresh would
    // install the NEWEST release in range (a major jump, or a withdrawn or held
    // release), and there is no operator to carry, so it names nothing (#993 S3).
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"ms": ">=2.0.0"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        "{\"lockfileVersion\": 3}",
    )
    .unwrap();
    assert_eq!(direct_fix(dir.path(), "npm", "ms", "2.1.1", "2.1.3"), None);
}

/// The row wording follows: all refreshes read as a refresh; in a mixed row
/// the refresh says what it is; a direct site whose fix could not be named
/// keeps the bump wording (#993 review S2).
#[test]
fn the_note_words_a_refresh_as_one() {
    use crate::osv::fix_target::{InstallSite, LineTarget};
    let line = |projects: &[&str]| LineTarget {
        installed_version: "1.2.5".into(),
        target_version: Some("1.2.8".into()),
        clears_all_known: true,
        introduces: Vec::new(),
        malicious: Vec::new(),
        held: None,
        upgrade_type: None,
        sites: projects
            .iter()
            .map(|p| InstallSite {
                project_path: (*p).to_string(),
                is_direct: true,
                is_dev: false,
            })
            .collect(),
    };
    let refresh = (
        "/a".to_string(),
        "1.2.8".to_string(),
        DirectFix::Refresh("npm update minimist".to_string()),
    );
    let bump = (
        "/b".to_string(),
        "1.2.8".to_string(),
        DirectFix::Command("npm install minimist@1.2.8 --save-exact".to_string()),
    );
    assert!(all_refresh(
        std::slice::from_ref(&refresh),
        &[line(&["/a"])]
    ));
    // /b is declared too, but its fix could not be named: still a bump row.
    assert!(!all_refresh(
        std::slice::from_ref(&refresh),
        &[line(&["/a", "/b"])]
    ));
    assert_eq!(
        direct_fix_note(std::slice::from_ref(&refresh), false),
        ": `npm update minimist`"
    );
    let mixed = direct_fix_note(&[refresh, bump], true);
    assert!(
        mixed.contains(
            "`npm update minimist` (a lockfile refresh: the declared range already admits it) in /a"
        ),
        "{mixed}"
    );
    assert!(
        mixed.contains("`npm install minimist@1.2.8 --save-exact` in /b"),
        "{mixed}"
    );
}

/// A pre-release base keeps `^` / `~` bounded (decided for both engines,
/// 2026-10-24; NodeGoat's async `^2.0.0-rc.4`). The admission check agrees in
/// both dialects: npm reads `>=2.0.0-rc.4 <3.0.0-0`, Cargo the same caret.
#[test]
fn a_prerelease_base_is_bounded_and_admits_like_its_release() {
    assert_eq!(npm_operator("^2.0.0-rc.4"), Some("^"));
    assert_eq!(npm_operator("~2.0.0-rc.4"), Some("~"));
    assert_eq!(requirement_admits("^2.0.0-rc.4", "2.6.4"), Some(true));
    assert_eq!(requirement_admits("^2.0.0-rc.4", "3.0.0"), Some(false));
    assert_eq!(requirement_admits("^1.0.0-rc.1", "2.0.1"), Some(false));
    assert_eq!(
        Requirement::cargo("^2.0.0-rc.4").admits("2.6.4"),
        Some(true)
    );
    assert_eq!(
        Requirement::cargo("^2.0.0-rc.4").admits("3.0.0"),
        Some(false)
    );
    assert_eq!(
        npm_direct_command(Manager::Npm, "^1.0.0-rc.1", "pkg", "2.0.1").as_deref(),
        Some("npm install \"pkg@^2.0.1\"")
    );
}

/// Review of #1008, S5: the MCP's `PRERELEASE_BASE` exactly: no `v`, no
/// `+build` after a caret or tilde.
#[test]
fn a_build_or_v_prefixed_base_is_not_bounded() {
    assert_eq!(npm_operator("^1.2.3+build"), None);
    assert_eq!(npm_operator("^v1.2.3"), None);
    assert_eq!(npm_operator("~1.2.3-rc.1+b"), None);
    assert_eq!(npm_operator("^1.2.3-rc-1.x"), Some("^"));
    assert_eq!(npm_operator("^1.2"), Some("^"));
}
