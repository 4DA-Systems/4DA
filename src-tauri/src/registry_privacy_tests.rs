// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The private-name rules (audit P1): every case a work machine has.

use std::path::Path;

use super::*;

fn patterns(list: &str) -> Vec<String> {
    list.split(',').map(str::to_string).collect()
}

fn project(root: &Path, deps: &[(&str, &str)]) -> ProjectDeps {
    ProjectDeps {
        root: root.to_path_buf(),
        deps: deps
            .iter()
            .map(|(e, n)| ((*e).to_string(), (*n).to_string()))
            .collect(),
    }
}

fn write(root: &Path, name: &str, text: &str) {
    if let Some(parent) = root.join(name).parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(root.join(name), text).expect("write");
}

// --- Go -------------------------------------------------------------------

#[test]
fn go_patterns_follow_gos_prefix_rules() {
    let p = patterns("github.com/corp,*.internal.example,gitlab.example.com/team/*");
    assert!(go_matches_any(&p, "github.com/corp"));
    assert!(go_matches_any(&p, "github.com/corp/svc"));
    assert!(go_matches_any(&p, "github.com/corp/svc/v2"));
    assert!(
        !go_matches_any(&p, "github.com/corporate/svc"),
        "a prefix is per path element"
    );
    assert!(!go_matches_any(&p, "github.com/gin-gonic/gin"));
    assert!(go_matches_any(&p, "git.internal.example/a/b"));
    assert!(
        !go_matches_any(&p, "internal.example/a"),
        "`*.` needs a subdomain"
    );
    assert!(go_matches_any(&p, "gitlab.example.com/team/x/y"));
    assert!(
        !go_matches_any(&p, "gitlab.example.com/team"),
        "too few elements"
    );
    assert!(go_matches_any(
        &patterns("github.com/c?rp"),
        "github.com/corp/x"
    ));
    assert!(go_matches_any(
        &patterns("github.com/[a-c]orp"),
        "github.com/corp/x"
    ));
    assert!(!go_matches_any(
        &patterns("github.com/[^c]orp"),
        "github.com/corp/x"
    ));
    assert!(!go_matches_any(&patterns(""), "github.com/corp/x"));
    assert!(!go_matches_any(&patterns(" , "), "github.com/corp/x"));
}

#[test]
fn goprivate_comes_from_the_environment_then_the_go_env_file() {
    let mut user = UserConfig {
        go_env_file: Some("GOPRIVATE=github.com/from-file\nGOFLAGS=-mod=mod\n".into()),
        ..UserConfig::default()
    };
    let names = build(&user, &[]);
    assert!(names.is_private(Ecosystem::Go, "github.com/from-file/svc"));

    user.env
        .insert("GOPRIVATE".into(), "github.com/from-env".into());
    user.env
        .insert("GONOSUMDB".into(), "go.corp.example".into());
    let names = build(&user, &[]);
    assert!(names.is_private(Ecosystem::Go, "github.com/from-env/svc"));
    assert!(
        !names.is_private(Ecosystem::Go, "github.com/from-file/svc"),
        "the environment overrides the file's GOPRIVATE"
    );
    assert!(names.is_private(Ecosystem::Go, "go.corp.example/lib"));
    assert!(!names.is_private(Ecosystem::Go, "golang.org/x/net"));
    // GONOPROXY alone is enough too.
    let only_noproxy = UserConfig {
        env: [("GONOPROXY".to_string(), "corp.example".to_string())].into(),
        ..UserConfig::default()
    };
    assert!(build(&only_noproxy, &[]).is_private(Ecosystem::Go, "corp.example/a"));
}

#[test]
fn a_go_mod_names_its_own_module_and_local_replacements() {
    let gomod = "module github.com/acme/api\n\ngo 1.22\n\nrequire (\n\tgithub.com/acme/shared v0.0.0\n\tgolang.org/x/net v0.21.0\n)\n\nreplace github.com/acme/shared => ../shared\nreplace (\n\tgithub.com/acme/tools v1.0.0 => ./tools // local\n\tgolang.org/x/crypto => golang.org/x/crypto v0.20.0\n)\n";
    let mut found = go_mod_private(gomod);
    found.sort();
    assert_eq!(
        found,
        [
            "github.com/acme/api",
            "github.com/acme/shared",
            "github.com/acme/tools"
        ]
    );
}

// --- npm ------------------------------------------------------------------

#[test]
fn npm_scopes_and_registries_from_npmrc_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("web");
    std::fs::create_dir_all(&root).expect("mkdir");
    write(
        dir.path(),
        ".npmrc",
        "@corp:registry=https://npm.corp.example/\n@public:registry=https://registry.npmjs.org/\n",
    );
    let names = build(
        &UserConfig {
            npmrc: Some("@team:registry=https://npm.pkg.github.com\n".into()),
            ..UserConfig::default()
        },
        &[project(&root, &[("npm", "@corp/ui"), ("npm", "react")])],
    );
    assert!(
        names.is_private(Ecosystem::Npm, "@corp/ui"),
        "parent .npmrc"
    );
    assert!(names.is_private(Ecosystem::Npm, "@team/lib"), "~/.npmrc");
    assert!(!names.is_private(Ecosystem::Npm, "@public/x"));
    assert!(!names.is_private(Ecosystem::Npm, "react"));
    assert_eq!(names.withheld, 1);
}

#[test]
fn a_project_default_registry_makes_its_npm_names_private() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        ".npmrc",
        "registry=https://artifactory.corp.example/api/npm/npm/\n",
    );
    let other = tempfile::tempdir().expect("tempdir");
    let names = build(
        &UserConfig::default(),
        &[
            project(
                dir.path(),
                &[("npm", "left-pad-internal"), ("npm", "react")],
            ),
            project(other.path(), &[("npm", "vite")]),
        ],
    );
    assert!(names.is_private(Ecosystem::Npm, "left-pad-internal"));
    assert!(
        names.is_private(Ecosystem::Npm, "react"),
        "conservative: the whole project"
    );
    assert!(
        !names.is_private(Ecosystem::Npm, "vite"),
        "another project is unaffected"
    );

    let user_level = UserConfig {
        npmrc: Some("registry=https://npm.corp.example\n".into()),
        ..UserConfig::default()
    };
    assert!(build(&user_level, &[]).is_private(Ecosystem::Npm, "anything"));
    let public = UserConfig {
        npmrc: Some("registry=https://registry.npmjs.org/\n".into()),
        env: [(
            "npm_config_registry".to_string(),
            "https://registry.npmjs.org".to_string(),
        )]
        .into(),
        ..UserConfig::default()
    };
    assert!(!build(&public, &[]).is_private(Ecosystem::Npm, "anything"));
}

// --- Cargo ----------------------------------------------------------------

#[test]
fn cargo_lock_sources_other_than_crates_io_are_private() {
    let lock = r#"
version = 4

[[package]]
name = "my-app"
version = "0.1.0"
dependencies = ["serde"]

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"

[[package]]
name = "sparse-one"
version = "1.0.0"
source = "sparse+https://index.crates.io/"

[[package]]
name = "corp-auth"
version = "0.3.0"
source = "git+https://git.corp.example/auth?branch=main#deadbeef"

[[package]]
name = "corp_metrics"
version = "2.0.0"
source = "registry+https://cargo.corp.example/index"
"#;
    let mut found = cargo_lock_private(lock);
    found.sort();
    assert_eq!(found, ["corp-auth", "corp_metrics", "my-app"]);
}

#[test]
fn cargo_toml_path_git_and_registry_dependencies_are_private() {
    let manifest = r#"
[package]
name = "acme-cli"
version = "0.1.0"

[dependencies]
serde = { version = "1", features = ["derive"] }
acme-core = { path = "../core" }
renamed = { package = "acme-proto", git = "https://git.corp.example/proto" }
internal = { version = "1", registry = "corp" }
tokio = "1"

[target.'cfg(windows)'.dependencies]
winacme = { path = "win" }

[dependencies.acme-table]
path = "../table"

[dev-dependencies.criterion]
version = "0.5"
"#;
    let mut found = cargo_toml_private(manifest);
    found.sort();
    assert_eq!(
        found,
        [
            "acme-cli",
            "acme-core",
            "acme-proto",
            "acme-table",
            "internal",
            "winacme"
        ]
    );
}

#[test]
fn crate_names_compare_dashes_and_underscores_alike() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "Cargo.lock",
        "[[package]]\nname = \"corp_metrics\"\nversion = \"1.0.0\"\nsource = \"git+https://x\"\n",
    );
    let names = build(&UserConfig::default(), &[project(dir.path(), &[])]);
    assert!(names.is_private(Ecosystem::Cargo, "corp-metrics"));
    assert!(names.is_private(Ecosystem::Cargo, "Corp_Metrics"));
    assert!(!names.is_private(Ecosystem::Cargo, "serde"));
}

// --- PyPI -----------------------------------------------------------------

#[test]
fn requirements_name_other_indexes_and_url_packages() {
    let facts = requirements_private(
        "--index-url https://pypi.corp.example/simple\nrequests==2.31.0\ncorp-sdk @ git+https://git.corp.example/sdk.git\n# a comment\n",
    );
    assert!(facts.other_index);
    assert_eq!(facts.names, ["corp-sdk"]);
    assert!(!requirements_private("-i https://pypi.org/simple\nflask==3.0\n").other_index);
    assert!(requirements_private("--extra-index-url=https://x.corp.example/simple\n").other_index);
    assert!(!requirements_private("django==5.0  # --index-url in a comment\n").other_index);
}

#[test]
fn a_project_with_another_index_withholds_every_pypi_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "requirements/base.txt",
        "-i https://nexus.corp.example/repository/pypi/simple\nflask==3.0.0\n",
    );
    let names = build(
        &UserConfig::default(),
        &[project(
            dir.path(),
            &[("pypi", "flask"), ("python", "corp_tools")],
        )],
    );
    assert!(names.is_private(Ecosystem::PyPI, "flask"));
    assert!(
        names.is_private(Ecosystem::PyPI, "Corp.Tools"),
        "PEP 503 names"
    );
    assert_eq!(names.withheld, 2);
}

#[test]
fn uv_and_poetry_locks_name_packages_from_elsewhere() {
    let uv = r#"
version = 1

[[package]]
name = "requests"
version = "2.31.0"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "corp-auth"
version = "1.0.0"
source = { registry = "https://pypi.corp.example/simple" }

[[package]]
name = "my-service"
version = "0.1.0"
source = { editable = "." }
"#;
    let mut found = python_lock_private(uv);
    found.sort();
    assert_eq!(found, ["corp-auth", "my-service"]);

    let poetry = r#"
[[package]]
name = "flask"
version = "3.0.0"

[[package]]
name = "corp-models"
version = "0.2.0"

[package.source]
type = "legacy"
url = "https://pypi.corp.example/simple"
reference = "corp"
"#;
    assert_eq!(python_lock_private(poetry), ["corp-models"]);
}

#[test]
fn pyproject_names_its_own_package_and_other_indexes() {
    let facts = pyproject_private(
        "[project]\nname = \"acme-service\"\n\n[[tool.uv.index]]\nname = \"corp\"\nurl = \"https://pypi.corp.example/simple\"\n",
    );
    assert!(facts.other_index);
    assert_eq!(facts.names, ["acme-service"]);
    let plain = pyproject_private("[project]\nname = \"x\"\ndependencies = [\"requests>=2\"]\n");
    assert!(!plain.other_index);
}

#[test]
fn user_level_python_configuration_withholds_every_pypi_name() {
    let pip = UserConfig {
        python_configs: vec!["[global]\nindex-url = https://pypi.corp.example/simple\n".into()],
        ..UserConfig::default()
    };
    assert!(build(&pip, &[]).is_private(Ecosystem::PyPI, "requests"));
    let env = UserConfig {
        env: [(
            "PIP_EXTRA_INDEX_URL".to_string(),
            "https://pypi.corp.example/simple".to_string(),
        )]
        .into(),
        ..UserConfig::default()
    };
    assert!(build(&env, &[]).is_private(Ecosystem::PyPI, "requests"));
    let public = UserConfig {
        python_configs: vec!["[global]\nindex-url = https://pypi.org/simple\n".into()],
        env: [(
            "UV_DEFAULT_INDEX".to_string(),
            "https://pypi.org/simple".to_string(),
        )]
        .into(),
        ..UserConfig::default()
    };
    assert!(!build(&public, &[]).is_private(Ecosystem::PyPI, "requests"));
}

// --- The switch -----------------------------------------------------------

/// Off means off: no name may be looked up, public or not. On, only the
/// private ones are refused.
#[test]
fn turning_registry_lookups_off_allows_no_lookup() {
    let private = build(
        &UserConfig {
            env: [("GOPRIVATE".to_string(), "github.com/corp".to_string())].into(),
            ..UserConfig::default()
        },
        &[],
    );
    for (eco, name) in [
        ("npm", "react"),
        ("crates.io", "serde"),
        ("pypi", "requests"),
        ("go", "golang.org/x/net"),
    ] {
        assert!(
            !allowed(false, &private, eco, name),
            "{eco} {name} while off"
        );
        assert!(allowed(true, &private, eco, name), "{eco} {name} while on");
    }
    assert!(!allowed(true, &private, "go", "github.com/corp/svc"));
    assert!(registry_lookups_enabled(), "on by default");
}

// --- Every lookup asks first ---------------------------------------------

/// Every production file that builds a request to a public registry with a
/// dependency name asks this module first. A new call site that does not
/// fails here (P1: NETWORK.md promised an off switch the code did not have).
#[test]
fn every_registry_lookup_asks_registry_privacy() {
    const REGISTRY_URLS: &[&str] = &[
        "https://registry.npmjs.org/",
        "https://crates.io/api/",
        "https://static.crates.io/",
        "https://pypi.org/pypi/",
        "https://proxy.golang.org/",
    ];
    // The URL builders only; `release_changelog/mod.rs` gates their caller.
    const NOT_LOOKUPS: &[&str] = &["release_changelog/fetch.rs"];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unguarded = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(&src)
                .expect("under src")
                .to_string_lossy()
                .replace('\\', "/");
            let is_test = rel.ends_with("_tests.rs") || rel.contains("/tests/");
            if !rel.ends_with(".rs") || is_test || NOT_LOOKUPS.contains(&rel.as_str()) {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            // Production code only: stop at the first `#[cfg(test)]` module.
            let code = text.split("#[cfg(test)]").next().unwrap_or("");
            let names_registry = code.lines().any(|l| {
                !l.trim_start().starts_with("//") && REGISTRY_URLS.iter().any(|u| l.contains(u))
            });
            if names_registry && !code.contains("registry_privacy::") {
                unguarded.push(rel);
            }
        }
    }
    unguarded.sort();
    assert!(
        unguarded.is_empty(),
        "registry call sites that do not ask registry_privacy: {unguarded:?}"
    );
}
