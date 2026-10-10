// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Fixture-corpus audit of the lockfile walk + OSV sync + matcher, end to end
//! on real repositories. `#[ignore]`d: it needs network (OSV) and a corpus on
//! disk, and must point at a scratch database.
//!
//! ```text
//! FX_ROOT=<dir of cloned repos>  FX_NAMES=a,b,c  FX_OUT=<out dir>
//! FX_DB=<scratch>/fx.db  FOURDA_DATA_DIR=<scratch>  FOURDA_DB_PATH=<scratch>/fx.db
//! cargo test --lib fixture_audit -- --ignored --nocapture
//! ```
//! Writes `rust-<name>.json` per repo: packages, findings (with aliases and
//! the version-confirmed flag) and the walk report. Every repo is stored
//! under a synthetic `C:/fxaudit/<name>` project path so nothing depends on
//! where the corpus is cloned.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn walk_repo(db: &Database, name: &str, repo: &Path) -> serde_json::Value {
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let started = std::time::Instant::now();
    let gated = collect_lockfile_dirs(std::slice::from_ref(&repo.to_path_buf()));
    let mut report = LockfileReport::default();
    let dirs = lockfile::walk_dirs(repo);
    for dir in &dirs {
        let rel = dir
            .strip_prefix(repo)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let project = if rel.is_empty() {
            format!("C:/fxaudit/{name}")
        } else {
            format!("C:/fxaudit/{name}/{rel}")
        };
        process_lockfile_dir(db, &scanner, dir, &project, &mut report);
    }
    eprintln!(
        "{name}: {} dirs ({} via gated walk) in {}ms",
        dirs.len(),
        gated.len(),
        started.elapsed().as_millis()
    );
    serde_json::json!({
        "gated_walk_dirs": gated.len(),
        "dirs": dirs.len(),
        "report": report.to_json(),
    })
}

fn repo_findings(
    name: &str,
    matches: &[crate::osv::types::MatchedAdvisory],
    instances: &[crate::db::DependencyInstanceRow],
) -> (Vec<serde_json::Value>, Vec<String>) {
    let prefix = format!("c:/fxaudit/{}", name.to_lowercase());
    let in_repo = |p: &str| {
        let p = p.replace('\\', "/").to_lowercase();
        p == prefix || p.starts_with(&format!("{prefix}/"))
    };
    let mut findings = Vec::new();
    for m in matches {
        for inst in m
            .dependency_instances
            .iter()
            .filter(|i| in_repo(&i.project_path))
        {
            findings.push(serde_json::json!({
                "eco": m.ecosystem, "name": m.package_name,
                "version": inst.installed_version, "id": m.advisory_id,
                "aliases": m.aliases, "confirmed": inst.is_version_confirmed,
                "project_path": inst.project_path, "fixed": inst.fixed_version,
            }));
        }
    }
    let packages: BTreeSet<String> = instances
        .iter()
        .filter(|r| in_repo(&r.project_path))
        .map(|r| {
            format!(
                "{}|{}|{}|{}",
                r.ecosystem, r.package_name, r.version, r.project_path
            )
        })
        .collect();
    (findings, packages.into_iter().collect())
}

#[test]
#[ignore = "fixture corpus audit: needs FX_* env, a scratch DB and network (OSV)"]
fn fixture_audit() {
    let root = PathBuf::from(env("FX_ROOT"));
    let out_dir = PathBuf::from(env("FX_OUT"));
    let names: Vec<String> = env("FX_NAMES")
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let db = Database::new(Path::new(&env("FX_DB"))).expect("open scratch db");
    let mut meta: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for name in &names {
        meta.insert(name.clone(), walk_repo(&db, name, &root.join(name)));
    }

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let started = std::time::Instant::now();
    let sync = rt.block_on(crate::osv::sync::sync(&db));
    eprintln!(
        "sync in {}ms: {:?}",
        started.elapsed().as_millis(),
        sync.as_ref()
            .map(|s| (s.advisories_stored, s.errors.clone()))
    );
    let (matches, _) =
        crate::osv::matching::get_matched_advisories_with_not_compiled(&db).expect("match");
    let instances = db.get_all_dependency_instances().expect("instances");

    for name in &names {
        let (findings, packages) = repo_findings(name, &matches, &instances);
        eprintln!(
            "{name}: packages={} findings={}",
            packages.len(),
            findings.len()
        );
        let out = serde_json::json!({
            "repo": name,
            "meta": meta.get(name),
            "findings": findings,
            "packages": packages,
        });
        std::fs::write(
            out_dir.join(format!("rust-{name}.json")),
            serde_json::to_string_pretty(&out).expect("serialize"),
        )
        .expect("write");
    }
}
