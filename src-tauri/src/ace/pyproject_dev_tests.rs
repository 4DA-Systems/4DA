// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `pyproject_dev.rs`.

use super::*;

/// The 6.2.5 audit's `x-f6c-py-uvlock` manifest, verbatim.
const F6C: &str = r#"[project]
name = "f6c"
version = "0.1.0"
requires-python = ">=3.8"
dependencies = [
  "django==3.2.0",
  "jinja2>=2.10,<3",
  "pyyaml~=5.3",
]

[dependency-groups]
dev = ["urllib3==1.26.4"]
"#;

#[test]
fn pep735_dev_group_is_read() {
    assert_eq!(parse_pyproject_dev_dependencies(F6C), ["urllib3"]);
}

#[test]
fn include_group_is_followed_and_cycles_stop() {
    let content = r#"
[dependency-groups]
test = [
    "pytest>=8",  # the runner
    "coverage[toml]>=7.4 ; python_version >= '3.9'",
    { include-group = "Typing" },
]
typing = ["mypy==1.10.0"]
dev = [{include-group = "test"}, "ruff"]
loop-a = [{ include-group = "loop_b" }, "a-only"]
loop-b = [{ include-group = "loop-a" }]
"#;
    let groups = dependency_groups(content);
    assert_eq!(groups["dev"], ["ruff", "pytest", "coverage", "mypy"]);
    assert_eq!(groups["test"], ["pytest", "coverage", "mypy"]);
    // A cycle terminates and still collects what it reaches.
    assert_eq!(groups["loop-b"], ["a-only"]);
    // `include-group`'s value is a group name, never a package.
    let all = parse_pyproject_dev_dependencies(content);
    assert!(!all.iter().any(|n| n.eq_ignore_ascii_case("typing")));
    assert!(!all.iter().any(|n| n == "test"));
}

#[test]
fn uv_legacy_pdm_and_poetry_dev_tables_are_read() {
    let content = r#"
[tool.uv]
default-groups = ["dev", "lint"]
dev-dependencies = [
  "black==24.1.0",
]

[tool.pdm.dev-dependencies]
lint = ["flake8>=7"]

[tool.poetry.dependencies]
python = "^3.10"
requests = "^2.31"

[tool.poetry.dev-dependencies]
pytest = "^7.0"

[tool.poetry.group.docs.dependencies]
mkdocs = "^1.5"
python = "^3.10"

[tool.poetry.group.docs.dependencies."sphinx-rtd"]
version = "^2"

[tool.poetry.group.docs]
optional = true
"#;
    let dev = parse_pyproject_dev_dependencies(content);
    for name in ["black", "flake8", "pytest", "mkdocs", "sphinx-rtd"] {
        assert!(dev.iter().any(|d| d == name), "{name} in {dev:?}");
    }
    // Runtime and interpreter pins are never dev; `optional` and
    // `default-groups` are settings, not packages.
    for name in [
        "requests",
        "python",
        "optional",
        "default-groups",
        "version",
    ] {
        assert!(!dev.iter().any(|d| d == name), "{name} in {dev:?}");
    }
}

#[test]
fn optional_dependencies_and_comments_are_not_dev() {
    let content = r#"
[project.optional-dependencies]
server = ["uvicorn[standard]>=0.29"]

[dependency-groups]
# lint = ["pylint"]
dev = [
  # "commented-out",
  "ruff",
]
"#;
    assert_eq!(parse_pyproject_dev_dependencies(content), ["ruff"]);
}

#[test]
fn group_names_normalise() {
    assert_eq!(normalize("Typing_Extra.Group"), "typing-extra-group");
    assert_eq!(normalize("a--b"), "a-b");
}
