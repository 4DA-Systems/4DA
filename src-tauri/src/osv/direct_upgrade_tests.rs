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
        "cargo add winapi@0.3.9 --target 'cfg(windows)'"
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

#[test]
fn go_get_only_for_a_direct_requirement() {
    let go_mod = "module example.com/m\n\ngo 1.22\n\nrequire golang.org/x/text v0.3.7\n\nrequire (\n\tgolang.org/x/net v0.20.0\n\tgolang.org/x/sys v0.16.0 // indirect\n)\n";
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/text", "0.3.8").as_deref(),
        Some("go get golang.org/x/text@v0.3.8")
    );
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/net", "v0.23.0").as_deref(),
        Some("go get golang.org/x/net@v0.23.0")
    );
    assert_eq!(
        go_direct_command(go_mod, "golang.org/x/sys", "0.20.0"),
        None
    );
}

#[test]
fn a_pinned_requirement_gets_an_exact_edit() {
    let req = "Django==4.2 # web\nrequests>=2.0\n";
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "django", "5.2.17").as_deref(),
        Some("change `Django==4.2` to `Django==5.2.17` in requirements.txt")
    );
    // Not an == pin: no edit is invented.
    assert_eq!(
        pypi_direct_instruction(req, "requirements.txt", "requests", "2.32.0"),
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
        direct_fix(dir.path(), "npm", "minimist", "1.2.8"),
        Some(DirectFix::Command(
            "npm install \"minimist@~1.2.8\"".to_string()
        ))
    );
    assert_eq!(direct_fix(dir.path(), "PyPI", "django", "5.2.17"), None);
}
