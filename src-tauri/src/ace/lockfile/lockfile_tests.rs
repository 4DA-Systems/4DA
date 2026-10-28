// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The host side of the lockfile readers: [`StdFs`] on a real directory, the
//! walk, and the walk's report. The readers' parsing is tested in the engine
//! crate (`crates/fourda-depengine/src/lockfile/lockfile_tests.rs`) over an
//! in-memory file source.

use std::path::Path;

use super::*;

fn read(path: &Path, format: LockFormat, content: &str) -> LockfileRead {
    match read_text(path, format, content) {
        LockfileOutcome::Read(read) => read,
        other => panic!("{} did not read: {other:?}", path.display()),
    }
}

fn ids(read: &LockfileRead) -> Vec<String> {
    let mut v: Vec<String> = read
        .packages
        .iter()
        .map(|p| format!("{}@{}", p.name, p.version))
        .collect();
    v.sort();
    v
}

#[test]
fn requirements_includes_are_read_from_disk_beside_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("base.txt"),
        "click==8.1.7\n-r requirements-dev.txt\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("requirements-dev.txt"),
        "-r ./base.txt\npytest==8.0.0\n",
    )
    .unwrap();
    let main = tmp.path().join("requirements.txt");
    let content = "-r base.txt\nfastapi[all]==0.111.0\n";
    std::fs::write(&main, content).unwrap();
    let read = read(&main, LockFormat::Requirements, content);
    assert_eq!(
        ids(&read),
        ["click@8.1.7", "fastapi@0.111.0", "pytest@8.0.0"]
    );
    assert_eq!(read.path, main.display().to_string());
}

#[test]
fn requirements_variants_and_requirements_dir_are_found() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for name in [
        "requirements.txt",
        "requirements-dev.txt",
        "_test_minimum_requirements.txt",
        "notes.txt",
    ] {
        std::fs::write(dir.join(name), "x==1.0\n").unwrap();
    }
    std::fs::create_dir_all(dir.join("requirements")).unwrap();
    std::fs::write(dir.join("requirements").join("base.txt"), "y==2.0\n").unwrap();
    let files = requirements_files(dir);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "_test_minimum_requirements.txt",
            "base.txt",
            "requirements-dev.txt",
            "requirements.txt"
        ]
    );
    assert_eq!(files[1], dir.join("requirements").join("base.txt"));
    assert!(
        requirements_files(&dir.join("requirements")).is_empty(),
        "the parent owns requirements/"
    );
}

#[test]
fn go_mod_pre_117_reads_the_go_sum_beside_it() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("go.mod");
    let go_mod = "module example.com/old\n\ngo 1.16\n\nrequire github.com/a/a v1.2.0\n";
    std::fs::write(
        tmp.path().join("go.sum"),
        "github.com/a/a v1.2.0 h1:x=\n\
         github.com/b/b v0.3.0/go.mod h1:x=\n\
         github.com/b/b v0.10.0 h1:x=\n",
    )
    .unwrap();
    let read = read(&path, LockFormat::GoMod, go_mod);
    assert_eq!(
        ids(&read),
        ["github.com/a/a@1.2.0", "github.com/b/b@0.10.0"]
    );
}

#[test]
fn read_dir_reports_unsupported_and_orphan_go_sum() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("pom.xml"), "<project/>").unwrap();
    std::fs::write(dir.join("bun.lockb"), [0u8, 1, 2]).unwrap();
    std::fs::write(dir.join("go.sum"), "github.com/a/a v1.0.0 h1:x=\n").unwrap();
    std::fs::write(dir.join("package-lock.json"), "{ broken").unwrap();
    let outcomes = read_dir(dir);
    let unsupported = outcomes
        .iter()
        .filter(|o| matches!(o, LockfileOutcome::Unsupported { .. }))
        .count();
    let failed = outcomes
        .iter()
        .filter(|o| matches!(o, LockfileOutcome::Failed { .. }))
        .count();
    assert_eq!((unsupported, failed), (2, 2), "{outcomes:?}");
    assert_eq!(probe(dir), Some(dir.join("package-lock.json")));

    let mut report = LockfileReport::default();
    for outcome in &outcomes {
        report.record(outcome);
    }
    let json = report.to_json();
    assert_eq!(json["failed_count"], 2);
    assert_eq!(json["unsupported_count"], 2);
    assert_eq!(
        json["unsupported"][0]["path"],
        dir.join("bun.lockb").display().to_string(),
        "the report names the file as the platform spells it"
    );
}

/// Audit A2 (2026-10-12): a .NET project with only a `.csproj` was never
/// reported, so it read as having no dependencies at all.
#[test]
fn read_dir_reports_a_csproj_only_project_as_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(
        dir.join("Api.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk\"/>",
    )
    .unwrap();
    assert!(probe(dir).is_some(), "the walk must see the project");
    let outcomes = read_dir(dir);
    assert!(
        matches!(
            outcomes.as_slice(),
            [LockfileOutcome::Unsupported { kind, .. }] if kind.starts_with("NuGet")
        ),
        "{outcomes:?}"
    );

    // With packages.lock.json present the lockfile entry stands alone.
    std::fs::write(dir.join("packages.lock.json"), "{}").unwrap();
    assert_eq!(read_dir(dir).len(), 1);
}

#[test]
fn parse_lockfiles_under_flattens_every_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::create_dir_all(root.join("web").join("node_modules").join("x")).unwrap();
    std::fs::write(root.join("requirements.txt"), "flask==3.0.0\n").unwrap();
    std::fs::write(
        root.join("web").join("package-lock.json"),
        r#"{"lockfileVersion":3,"packages":{"node_modules/hono":{"version":"4.6.0"}}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("web")
            .join("node_modules")
            .join("x")
            .join("package-lock.json"),
        r#"{"lockfileVersion":3,"packages":{"node_modules/ignored":{"version":"1.0.0"}}}"#,
    )
    .unwrap();
    let (packages, problems) = parse_lockfiles_under(root);
    assert!(problems.is_empty(), "{problems:?}");
    let mut got: Vec<String> = packages
        .iter()
        .map(|p| format!("{}|{}|{}", p.ecosystem, p.name, p.version))
        .collect();
    got.sort();
    assert_eq!(got, ["PyPI|flask|3.0.0", "npm|hono|4.6.0"]);
    let hono = packages.iter().find(|p| p.name == "hono").unwrap();
    assert_eq!(
        hono.lockfile,
        root.join("web").join("package-lock.json"),
        "outcome paths are the host's own spelling"
    );
}
