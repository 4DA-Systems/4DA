// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;
use crate::lockfile::{probe_manifest, read_dir, LockfileOutcome, MemFs};

fn manifest_only(outcomes: &[LockfileOutcome]) -> Vec<(String, &'static str, usize)> {
    outcomes
        .iter()
        .filter_map(|o| match o {
            LockfileOutcome::ManifestOnly {
                path,
                ecosystem,
                dependencies,
            } => Some((path.clone(), *ecosystem, *dependencies)),
            _ => None,
        })
        .collect()
}

const PACKAGE_JSON: &str = r#"{
  "name": "app",
  "dependencies": { "react": "^18.2.0", "lib": "workspace:*", "local": "file:../local" },
  "devDependencies": { "vite": "^5.0.0", "react": "^18.2.0" },
  "optionalDependencies": { "fsevents": "^2.3.0" },
  "peerDependencies": { "react-dom": "^18" }
}"#;

#[test]
fn a_package_json_without_a_lockfile_is_reported_with_its_count() {
    let fs = MemFs::new().with("/p/app/package.json", PACKAGE_JSON);
    // react, vite, fsevents: workspace/file specs are local, peers are the
    // consumer's, and a name declared twice counts once.
    assert_eq!(
        manifest_only(&read_dir(&fs, "/p/app")),
        [("/p/app/package.json".to_string(), "javascript", 3)]
    );
    assert_eq!(
        probe_manifest(&fs, "/p/app"),
        Some("/p/app/package.json".into())
    );
    assert_eq!(
        crate::lockfile::probe(&fs, "/p/app"),
        None,
        "probe stays lockfiles-only"
    );
}

#[test]
fn a_lockfile_beside_or_above_the_manifest_covers_it() {
    let beside = MemFs::new()
        .with("/p/app/package.json", PACKAGE_JSON)
        .with("/p/app/pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    assert!(manifest_only(&read_dir(&beside, "/p/app")).is_empty());

    // A workspace member resolves through the root's lockfile.
    let member = MemFs::new().with("/repo/Cargo.lock", "version = 3\n").with(
        "/repo/crates/core/Cargo.toml",
        "[dependencies]\nserde = \"1\"\n",
    );
    assert!(manifest_only(&read_dir(&member, "/repo/crates/core")).is_empty());

    // The search stops at the repository root: a lockfile above it belongs
    // to something else.
    let outside = MemFs::new()
        .with("/home/package-lock.json", "{}")
        .with("/home/proj/.git/HEAD", "ref: refs/heads/main\n")
        .with("/home/proj/package.json", PACKAGE_JSON);
    assert_eq!(manifest_only(&read_dir(&outside, "/home/proj")).len(), 1);

    // Another ecosystem's lockfile does not cover it.
    let other = MemFs::new()
        .with("/p/Cargo.lock", "version = 3\n")
        .with("/p/package.json", PACKAGE_JSON);
    assert_eq!(manifest_only(&read_dir(&other, "/p")).len(), 1);
}

#[test]
fn a_manifest_declaring_nothing_is_not_reported_and_a_broken_one_fails() {
    let empty = MemFs::new().with("/p/package.json", r#"{"name":"x","scripts":{}}"#);
    assert!(read_dir(&empty, "/p").is_empty());

    let broken = MemFs::new().with("/p/package.json", "{ not json");
    let outcomes = read_dir(&broken, "/p");
    assert!(
        matches!(&outcomes[..], [LockfileOutcome::Failed { path, reason, .. }]
            if path == "/p/package.json" && reason.contains("not valid JSON")),
        "{outcomes:?}"
    );
}

#[test]
fn cargo_toml_counts_every_dependency_table_and_skips_local_crates() {
    let toml = r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = { version = "1", features = ["derive"] }
tokio = "1"
core = { path = "../core" }
published-local = { path = "../pl", version = "0.2" }
from-git = { git = "https://example.com/x.git" }
shared = { workspace = true }

[dependencies.reqwest]
version = "0.12"
default-features = false

[dependencies.sibling]
path = "../sibling"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
insta = "1"

[target.'cfg(windows)'.dependencies]
windows-sys = "0.59"

[build-dependencies]
cc = "1"
"#;
    assert_eq!(
        declared_dependencies("Cargo.toml", toml),
        // serde tokio published-local reqwest insta windows-sys cc
        Ok(7)
    );
}

#[test]
fn pyproject_counts_pep621_groups_poetry_and_uv() {
    let toml = r#"
[project]
name = "svc"
dependencies = [
    "fastapi>=0.110",   # web
    "Pydantic_Settings[dotenv]>=2",
    "local-pkg @ file:///src/local",
]

[project.optional-dependencies]
test = ["pytest>=8", "fastapi"]

[dependency-groups]
dev = ["ruff", { include-group = "test" }]

[tool.uv]
dev-dependencies = ["mypy>=1"]

[tool.poetry.dependencies]
python = "^3.11"
httpx = "^0.27"
mylib = { path = "../mylib" }

[tool.poetry.group.lint.dependencies]
black = "^24"

[build-system]
requires = ["hatchling"]
"#;
    // fastapi pydantic-settings pytest ruff mypy httpx black; the direct
    // URL, the local path, `python` and the build backend are not counted.
    assert_eq!(declared_dependencies("pyproject.toml", toml), Ok(7));
}

#[test]
fn pipfile_gemfile_and_composer_json() {
    let pipfile = "[packages]\nrequests = \"*\"\nmine = {path = \".\", editable = true}\n\n[dev-packages]\npytest = \"*\"\n\n[requires]\npython_version = \"3.11\"\n";
    assert_eq!(declared_dependencies("Pipfile", pipfile), Ok(2));

    let gemfile = "source 'https://rubygems.org'\ngem 'rails', '~> 7.1'\ngem \"pg\"\ngem 'local', path: '../local'\ngem 'edge', github: 'x/edge'\n  gem 'rspec' # test\n";
    assert_eq!(declared_dependencies("Gemfile", gemfile), Ok(3));

    let composer = r#"{"require":{"php":">=8.1","ext-json":"*","symfony/console":"^6"},"require-dev":{"phpunit/phpunit":"^10"}}"#;
    assert_eq!(declared_dependencies("composer.json", composer), Ok(2));
}

#[test]
fn python_pins_in_the_directory_lock_its_pyproject() {
    let fs = MemFs::new()
        .with(
            "/p/pyproject.toml",
            "[project]\ndependencies = [\"flask\"]\n",
        )
        .with("/p/requirements.txt", "flask==3.0.0\n");
    assert!(manifest_only(&read_dir(&fs, "/p")).is_empty());

    // Requirements that pin nothing do not.
    let ranged = MemFs::new()
        .with(
            "/p/pyproject.toml",
            "[project]\ndependencies = [\"flask\"]\n",
        )
        .with("/p/requirements.txt", "flask>=3\n");
    assert_eq!(
        manifest_only(&read_dir(&ranged, "/p")),
        [("/p/pyproject.toml".to_string(), "python", 1)]
    );
}
