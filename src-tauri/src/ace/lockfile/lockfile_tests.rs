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

/// A name that is not valid UTF-8 on this platform: an unpaired surrogate on
/// Windows, a lone `0xff` byte on Unix.
#[cfg(any(windows, unix))]
fn non_utf8_name(prefix: &str) -> std::ffi::OsString {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let mut wide: Vec<u16> = prefix.encode_utf16().collect();
        wide.push(0xD800);
        std::ffi::OsString::from_wide(&wide)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = prefix.as_bytes().to_vec();
        bytes.push(0xff);
        std::ffi::OsString::from_vec(bytes)
    }
}

/// Whether the non-UTF-8 entry was created. `false` when the filesystem
/// refuses the name itself: APFS and HFS+ (macOS) store names as UTF-8 and
/// answer `EILSEQ`, so there such a path cannot exist and nothing can hand it
/// to the readers. Any other error fails the test.
#[cfg(any(windows, unix))]
fn created_non_utf8(result: std::io::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        #[cfg(unix)]
        Err(e) if e.raw_os_error() == Some(libc::EILSEQ) => {
            eprintln!("skipped: this filesystem refuses non-UTF-8 names ({e})");
            false
        }
        Err(e) => panic!("could not create the non-UTF-8 entry: {e}"),
    }
}

/// Review of #1045: a directory whose path is not UTF-8 cannot be handed to
/// the readers. It must still probe positive and come back as one failure
/// per dependency file, never as a directory with nothing in it.
#[cfg(any(windows, unix))]
#[test]
fn a_non_utf8_directory_is_reported_not_dropped() {
    let name = non_utf8_name("app");
    assert!(
        name.to_str().is_none(),
        "the helper must build a non-UTF-8 name"
    );
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(&name);
    if !created_non_utf8(std::fs::create_dir(&dir)) {
        return;
    }
    std::fs::write(dir.join("package-lock.json"), "{}").unwrap();
    std::fs::write(dir.join("requirements.txt"), "flask==3.0.0\n").unwrap();
    std::fs::write(dir.join("README.md"), "# app\n").unwrap();

    assert_eq!(probe(&dir), Some(dir.join("package-lock.json")));
    let outcomes = read_dir(&dir);
    let failed: Vec<(String, String)> = outcomes
        .iter()
        .map(|o| match o {
            LockfileOutcome::Failed { path, reason, .. } => (path.clone(), reason.clone()),
            other => panic!("expected a failure, got {other:?}"),
        })
        .collect();
    assert_eq!(failed.len(), 2, "{outcomes:?}");
    assert!(failed[0].0.ends_with("package-lock.json"), "{failed:?}");
    assert!(failed[1].0.ends_with("requirements.txt"), "{failed:?}");
    assert!(failed.iter().all(|(_, reason)| reason == NON_UTF8_REASON));

    let mut report = LockfileReport::default();
    for outcome in &outcomes {
        report.record(outcome);
    }
    assert_eq!(report.to_json()["failed_count"], 2);
}

/// An entry whose NAME is not UTF-8, in a UTF-8 directory, is skipped as the
/// readers always skipped it, not lossy-spelled into a path that then fails
/// as "unreadable: not found".
#[cfg(any(windows, unix))]
#[test]
fn a_non_utf8_file_name_is_skipped_not_misreported() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut odd = non_utf8_name("requirements");
    odd.push(".txt");
    if !created_non_utf8(std::fs::write(dir.join(&odd), "flask==3.0.0\n")) {
        return;
    }
    std::fs::write(dir.join("requirements.txt"), "click==8.1.7\n").unwrap();

    let listed = StdFs.list(&dir.display().to_string()).unwrap_or_default();
    assert_eq!(listed.len(), 1, "{listed:?}");
    let outcomes = read_dir(dir);
    assert!(
        matches!(outcomes.as_slice(), [LockfileOutcome::Read(r)] if r.path.ends_with("requirements.txt")),
        "{outcomes:?}"
    );
}
