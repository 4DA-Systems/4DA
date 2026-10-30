// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The provisional JSON call surface, one string in and one string out. The
//! wasm module exports it (`wasm.rs`); it also keeps every rule reachable from
//! the module, so the module's size in CI measures the real engine.
//!
//! Provisional on purpose: the schema is `depengine/0` and nothing outside
//! this repository calls it. The versioned `depengine/1` contract
//! (single-engine design §3.2: `scan`, `fix_paths`, `range_check`,
//! `version_cmp`) replaces it once the engine owns inventory (C0.2+).
//!
//! Requests (`ranges` is an OSV `ranges` array as the app stores it, with
//! enumerated `versions` as an `ENUMERATED` range):
//! - `{"op":"version_cmp","eco":"PyPI","a":"1.0.post1","b":"1.0"}` → `"<" | "=" | ">" | null`
//! - `{"op":"affected","eco":"npm","version":"1.2.3","ranges":[...]}` → `{"affected":bool,"confirmed":bool}`
//! - `{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[[...],[...]],
//!   "registry":{"published":[..],"excluded":[..]}}` (`registry` optional: the
//!   package's releases and the ones its registry withdrew, `RegistryReleases`)
//!   → `{"line_fixes":[..],"walk":{"version":"..","clears_all":bool}|null,"withheld":[..]}`
//! - `{"op":"match_inventory","inventory":[Package..],"osv":[verbatim OSV records],
//!   "records":[AdvisoryRecord..]}` → `{"records":n,"findings":[{"advisory_id",
//!   "package_name","ecosystem","installed_version","fixed_version",
//!   "is_version_confirmed","project_paths","dependency_instances":[..]}]}`.
//!   `osv` records are stored the way one store pass of the app's mirror
//!   stores them ([`crate::osv::stored_records`]; within this request the
//!   first row per (id, package, ecosystem) wins, after any `records` given
//!   as already-stored rows). Across syncs the app's table is an upsert
//!   (`ON CONFLICT DO UPDATE`), so there the LAST write of a row wins; give
//!   the current rows as `records` to reproduce a mirror state.
//!   Every `inventory` row must say `primary`; copies with no primary row
//!   are an error, not an empty result. Optional `registry`:
//!   `[{"ecosystem","name","published":[..],"excluded":[..]}]`.
//! - `{"op":"read_lockfiles","dir":"/p","files":{"/p/package-lock.json":"..."}}`
//!   → `{"outcomes":[LockfileOutcome..]}`: every dependency file in `dir`, read
//!   by the lockfile readers (C0.3) from `files` held as a
//!   [`crate::lockfile::MemFs`] (companions such as `go.sum` or `-r` includes
//!   are read from `files` too). An outcome is `{"read":{..}}`,
//!   `{"failed":{..}}` or `{"unsupported":{..}}`. At most
//!   [`MAX_LOCKFILE_FILES`] files and [`MAX_LOCKFILE_BYTES`] bytes of them;
//!   a request over either cap is an error.
//! - `{"op":"parent_links","eco":"npm","package":"braces","round":0,"lines":[{"installed_version",
//!   "target_version","sites":[{"project_path","is_direct"}]}],"edges":[EdgeObs..],
//!   "installed":[InstalledObs..],"ranges":[..],"managers":{"<project>":"pnpm"},
//!   "dirs":{"<project>":"<dir>"},"provided":{..}}` (C0.4) → either
//!   `{"needs":[Need..]}` or `{"links":[ParentLink..],"unlinked":[UnlinkedSite..]}`,
//!   plus `"unanswered":[Need..]` when needs were left unanswered.
//!   The parents that bring each transitive copy in and whether a lockfile
//!   refresh reaches the target ([`crate::parent_hint::parent_links_lookup`]).
//!   `ranges` (required, never empty) are the package's advisories: a
//!   resolution elsewhere is cited only when clear of all of them, so with
//!   none nothing could be checked and the request is an error. `managers` is
//!   the tool the host detected per project; `dirs` maps a project to its
//!   directory (default: the project itself), joined with `/`. With `needs`,
//!   read them and call again with `round` one higher and the answers in
//!   `provided`: `{"manifests":{path:text|null},"listings":{dir:[names]|null},
//!   "crate_manifests":{"name@version":text|null},"npm_locks":{dir:text|null},
//!   "exhausted":bool}` (`null`: looked, not there), each keyed exactly as its
//!   need names it. `round` (required: 0 on the first call) is the engine's
//!   own cap: from round [`crate::needs::MAX_ROUNDS`] on, or once the host
//!   sets `exhausted`, it asks for nothing more and answers from what it was
//!   given. A need never answered by then (answered under another key counts
//!   as never answered) is listed in `unanswered` and read as absent, so what
//!   rests on it was NOT CHECKED: an unanswered manifest leaves the refresh
//!   verdict inferred from semver, and an unanswered `package-lock.json`
//!   tree names no parent from a requirement range alone (unless another
//!   parent is evidenced, the copy gets the semver route in `unlinked`). A
//!   tree answered `null` keeps the range-based attribution. Provided texts
//!   and listings are capped at [`MAX_LOCKFILE_BYTES`] in all.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::ecosystem::normalize as normalize_ecosystem;
use crate::fix_path::Manager;
use crate::fix_target::{installable_line_fix, walk_target_kinds, AdvisoryKind};
use crate::lines::{InstallSite, LineTarget};
use crate::lockfile::source::join_slash;
use crate::lockfile::{read_dir, MemFs};
use crate::matching::{inventory_problem, match_inventory_with, package_key, Package};
use crate::needs::{Lookup, Provided, MAX_ROUNDS};
use crate::osv::{stored_records, AdvisoryRecord, OsvRecord};
use crate::parent_hint::{parent_links_lookup, unlinked_sites, EdgeObs, InstalledObs, Projects};
use crate::range::check_version_affected;
use crate::registry::{RegistryIndex, RegistryReleases};
use crate::version_order::OrderedVersion;

/// The schema every response carries.
pub const SCHEMA: &str = "depengine/0";

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    VersionCmp {
        eco: String,
        a: String,
        b: String,
    },
    Affected {
        eco: String,
        version: Option<String>,
        ranges: Option<Value>,
    },
    FixTarget {
        eco: String,
        installed: String,
        ranges: Vec<Option<Value>>,
        #[serde(default)]
        registry: RegistryReleases,
        /// Parallel to `ranges`: malware / maintenance-notice records.
        #[serde(default)]
        kinds: Vec<AdvisoryKind>,
    },
    ReadLockfiles {
        dir: String,
        #[serde(default)]
        files: std::collections::BTreeMap<String, String>,
    },
    ParentLinks(Box<ParentLinksRequest>),
    MatchInventory {
        inventory: Vec<Package>,
        #[serde(default)]
        records: Vec<AdvisoryRecord>,
        #[serde(default)]
        osv: Vec<OsvRecord>,
        #[serde(default)]
        registry: Vec<PackageReleases>,
    },
}

/// A `parent_links` request (C0.4).
#[derive(Deserialize)]
struct ParentLinksRequest {
    eco: String,
    package: String,
    /// The calls made before this one: 0, then one higher per round of needs.
    round: usize,
    lines: Vec<LineIn>,
    #[serde(default)]
    edges: Vec<EdgeObs>,
    #[serde(default)]
    installed: Vec<InstalledObs>,
    /// Required, with no default: an absent list would read every version
    /// as clear of the package's advisories.
    ranges: Vec<Option<Value>>,
    #[serde(default)]
    managers: BTreeMap<String, Manager>,
    #[serde(default)]
    dirs: BTreeMap<String, String>,
    #[serde(default)]
    provided: ProvidedIn,
}

/// One release line of the package: what a link reads of a `LineTarget`.
#[derive(Deserialize)]
struct LineIn {
    installed_version: String,
    target_version: Option<String>,
    sites: Vec<SiteIn>,
}

#[derive(Deserialize)]
struct SiteIn {
    project_path: String,
    is_direct: bool,
    #[serde(default)]
    is_dev: bool,
}

impl LineIn {
    fn into_line(self) -> LineTarget {
        LineTarget {
            installed_version: self.installed_version,
            target_version: self.target_version,
            clears_all_known: false,
            upgrade_type: None,
            sites: self
                .sites
                .into_iter()
                .map(|s| InstallSite {
                    project_path: s.project_path,
                    is_direct: s.is_direct,
                    is_dev: s.is_dev,
                })
                .collect(),
            introduces: Vec::new(),
            malicious: Vec::new(),
            held: None,
        }
    }
}

/// The host's answers to earlier `needs`, as JSON carries them.
#[derive(Deserialize, Default)]
struct ProvidedIn {
    #[serde(default)]
    manifests: BTreeMap<String, Option<String>>,
    #[serde(default)]
    listings: BTreeMap<String, Option<Vec<String>>>,
    #[serde(default)]
    crate_manifests: BTreeMap<String, Option<String>>,
    /// `package-lock.json` text; text that does not parse reads as absent.
    #[serde(default)]
    npm_locks: BTreeMap<String, Option<String>>,
    #[serde(default)]
    exhausted: bool,
}

impl ProvidedIn {
    fn into_provided(self) -> Result<Provided, String> {
        let texts: usize = [&self.manifests, &self.crate_manifests, &self.npm_locks]
            .iter()
            .flat_map(|m| m.iter())
            .map(|(k, v)| k.len() + v.as_ref().map_or(0, String::len))
            .sum();
        let names: usize = self
            .listings
            .iter()
            .map(|(dir, names)| dir.len() + names.iter().flatten().map(String::len).sum::<usize>())
            .sum();
        let bytes = texts + names;
        if bytes > MAX_LOCKFILE_BYTES {
            return Err(format!(
                "parent_links: {bytes} bytes of provided files and listings, more than the {MAX_LOCKFILE_BYTES} allowed"
            ));
        }
        let npm_locks = self
            .npm_locks
            .into_iter()
            .map(|(dir, text)| {
                let tree = text.and_then(|t| serde_json::from_str::<Value>(&t).ok());
                (dir, tree.map(std::sync::Arc::new))
            })
            .collect();
        Ok(Provided {
            manifests: self.manifests,
            listings: self.listings,
            crate_manifests: self.crate_manifests,
            npm_locks,
            exhausted: self.exhausted,
        })
    }
}

/// The `parent_links` op: the links, or what the host must read first.
fn parent_links_op(request: ParentLinksRequest) -> Result<Value, String> {
    let ParentLinksRequest {
        eco,
        package,
        round,
        lines,
        edges,
        installed,
        ranges,
        managers,
        dirs,
        provided,
    } = request;
    if ranges.is_empty() {
        return Err(
            "parent_links: `ranges` is empty; with no advisory ranges no version can be checked as clear"
                .to_string(),
        );
    }
    let mut provided = provided.into_provided()?;
    // The engine's own cap, whatever the host does with `exhausted`.
    provided.exhausted |= round >= MAX_ROUNDS;
    let lines: Vec<LineTarget> = lines.into_iter().map(LineIn::into_line).collect();
    let stored: Vec<Option<String>> = ranges.iter().map(|r| stored(r.as_ref())).collect();
    let is_clear = |version: &str| {
        stored
            .iter()
            .all(|r| !check_version_affected(&eco, Some(version), r).0)
    };
    let manager = |project: &str| managers.get(project).copied();
    let dir = |project: &str| {
        dirs.get(project)
            .cloned()
            .unwrap_or_else(|| project.to_string())
    };
    let projects = Projects {
        manager: &manager,
        dir: &dir,
        join: &join_slash,
    };
    let found = parent_links_lookup(
        &package, &lines, &edges, &installed, &is_clear, &projects, &provided,
    );
    let (links, unanswered) = match found {
        Lookup::Needs(needs) => return Ok(json!({ "needs": needs })),
        Lookup::Done(links) => (links, Vec::new()),
        Lookup::Partial { value, unanswered } => (value, unanswered),
    };
    let unlinked = unlinked_sites(&package, &lines, &links, &manager);
    let mut out = json!({ "links": links, "unlinked": unlinked });
    // Only when set, so a complete answer keeps its exact shape.
    if !unanswered.is_empty() {
        out["unanswered"] = json!(unanswered);
    }
    Ok(out)
}

/// One package's registry facts in a `match_inventory` request.
#[derive(Deserialize)]
struct PackageReleases {
    ecosystem: String,
    name: String,
    #[serde(flatten)]
    releases: RegistryReleases,
}

fn registry_index(rows: Vec<PackageReleases>) -> RegistryIndex {
    rows.into_iter()
        .map(|r| {
            let key = (
                package_key(&r.name, &r.ecosystem),
                normalize_ecosystem(&r.ecosystem).to_string(),
            );
            (key, r.releases)
        })
        .collect()
}

/// The `match_inventory` op: stored rows plus verbatim records, matched. An
/// inventory with copies but no primary rows is an error, never an empty
/// all-clear ([`inventory_problem`]).
fn match_op(
    inventory: &[Package],
    mut records: Vec<AdvisoryRecord>,
    osv: &[OsvRecord],
    registry: &RegistryIndex,
) -> Result<Value, String> {
    if let Some(problem) = inventory_problem(inventory) {
        return Err(problem);
    }
    let key = |r: &AdvisoryRecord| {
        (
            r.advisory_id.clone(),
            r.package_name.clone(),
            r.ecosystem.clone(),
        )
    };
    let mut seen: std::collections::HashSet<(String, String, String)> =
        records.iter().map(key).collect();
    for row in osv.iter().flat_map(stored_records) {
        if seen.insert(key(&row)) {
            records.push(row);
        }
    }
    let findings: Vec<Value> = match_inventory_with(inventory, &records, registry, |_, _, _| {})
        .into_iter()
        .map(|f| {
            let r = &records[f.record];
            json!({
                "advisory_id": r.advisory_id,
                "package_name": r.package_name,
                "ecosystem": r.ecosystem,
                "installed_version": f.installed_version,
                "fixed_version": f.fixed_version,
                "is_version_confirmed": f.is_version_confirmed,
                "project_paths": f.project_paths,
                "dependency_instances": f.dependency_instances,
            })
        })
        .collect();
    Ok(json!({ "records": records.len(), "findings": findings }))
}

/// The most files one `read_lockfiles` request may carry.
pub const MAX_LOCKFILE_FILES: usize = 4096;

/// The most text, in bytes summed over every file, one `read_lockfiles`
/// request may carry. The largest lockfile in the conformance corpus is a few
/// MB; this leaves headroom for a monorepo's without letting a request
/// make the module hold an unbounded copy.
pub const MAX_LOCKFILE_BYTES: usize = 64 * 1024 * 1024;

/// A `read_lockfiles` request over the caps is an error, not a read.
fn check_file_bounds(files: &std::collections::BTreeMap<String, String>) -> Result<(), String> {
    if files.len() > MAX_LOCKFILE_FILES {
        return Err(format!(
            "read_lockfiles: {} files, more than the {MAX_LOCKFILE_FILES} allowed",
            files.len()
        ));
    }
    let bytes: usize = files.iter().map(|(p, t)| p.len() + t.len()).sum();
    if bytes > MAX_LOCKFILE_BYTES {
        return Err(format!(
            "read_lockfiles: {bytes} bytes of files, more than the {MAX_LOCKFILE_BYTES} allowed"
        ));
    }
    Ok(())
}

/// A ranges array as the stored JSON text the rules read.
fn stored(ranges: Option<&Value>) -> Option<String> {
    ranges.filter(|v| !v.is_null()).map(Value::to_string)
}

fn answer(request: Request) -> Result<Value, String> {
    Ok(match request {
        Request::VersionCmp { eco, a, b } => {
            let order = OrderedVersion::parse_in(&a, &eco)
                .zip(OrderedVersion::parse_in(&b, &eco))
                .and_then(|(x, y)| x.compare(&y));
            match order {
                Some(Ordering::Less) => json!("<"),
                Some(Ordering::Equal) => json!("="),
                Some(Ordering::Greater) => json!(">"),
                None => Value::Null,
            }
        }
        Request::Affected {
            eco,
            version,
            ranges,
        } => {
            let (affected, confirmed) =
                check_version_affected(&eco, version.as_deref(), &stored(ranges.as_ref()));
            json!({ "affected": affected, "confirmed": confirmed })
        }
        Request::FixTarget {
            eco,
            installed,
            ranges,
            registry,
            kinds,
        } => {
            let stored: Vec<Option<String>> = ranges.iter().map(|r| stored(r.as_ref())).collect();
            let line_fixes: Vec<String> = stored
                .iter()
                .filter_map(|r| installable_line_fix(&eco, &installed, r, &registry).fix)
                .collect();
            let refs: Vec<&Option<String>> = stored.iter().collect();
            let outcome = walk_target_kinds(&eco, &installed, &refs, &kinds, &registry);
            let walk = outcome
                .target
                .map(|w| json!({ "version": w.version, "clears_all": w.clears_all }));
            let mut out =
                json!({ "line_fixes": line_fixes, "walk": walk, "withheld": outcome.withheld });
            // Only when set, so every earlier answer keeps its exact shape.
            if !outcome.introduces.is_empty() {
                out["introduces"] = json!(outcome.introduces);
            }
            if !outcome.malware.is_empty() {
                out["malware"] = json!(outcome.malware);
            }
            out
        }
        Request::MatchInventory {
            inventory,
            records,
            osv,
            registry,
        } => match_op(&inventory, records, &osv, &registry_index(registry))?,
        Request::ParentLinks(request) => parent_links_op(*request)?,
        Request::ReadLockfiles { dir, files } => {
            check_file_bounds(&files)?;
            let mut fs = MemFs::new();
            for (path, text) in &files {
                fs.insert(path, text);
            }
            json!({ "outcomes": read_dir(&fs, &dir) })
        }
    })
}

/// Answer one JSON request. Never panics on bad input: a request that does
/// not parse, or one the engine cannot answer truthfully, comes back as
/// `{"ok": false, "error": ...}`.
pub fn call(request: &str) -> String {
    let error = |e: String| json!({ "schema": SCHEMA, "ok": false, "error": e });
    let response = match serde_json::from_str::<Request>(request) {
        Ok(req) => match answer(req) {
            Ok(result) => json!({ "schema": SCHEMA, "ok": true, "result": result }),
            Err(e) => error(e),
        },
        Err(e) => error(e.to_string()),
    };
    response.to_string()
}

#[cfg(test)]
#[path = "api_parent_links_tests.rs"]
mod parent_links_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn result(request: &str) -> Value {
        let v: Value = serde_json::from_str(&call(request)).unwrap_or_default();
        assert_eq!(v["schema"], SCHEMA);
        assert_eq!(v["ok"], true, "{v}");
        v["result"].clone()
    }

    #[test]
    fn version_cmp_reads_pypi_as_pep440() {
        let r = result(r#"{"op":"version_cmp","eco":"PyPI","a":"1.0.0-1","b":"1.0.0"}"#);
        assert_eq!(r, json!(">"));
        let r = result(r#"{"op":"version_cmp","eco":"npm","a":"x","b":"1.0.0"}"#);
        assert_eq!(r, Value::Null);
    }

    #[test]
    fn affected_and_fix_target_answer_from_the_rules() {
        let ranges = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.4"}]}]"#;
        let r = result(&format!(
            r#"{{"op":"affected","eco":"npm","version":"1.2.3","ranges":{ranges}}}"#
        ));
        assert_eq!(r, json!({"affected": true, "confirmed": true}));
        let r = result(&format!(
            r#"{{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[{ranges}]}}"#
        ));
        assert_eq!(
            r,
            json!({"line_fixes": ["1.2.4"], "walk": {"version": "1.2.4", "clears_all": true}, "withheld": []})
        );
        // The registry withdrew 1.2.4: the next release it offers is the fix.
        let r = result(&format!(
            r#"{{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[{ranges}],
                "registry":{{"published":["1.2.3","1.2.4","1.2.5"],"excluded":["1.2.4"]}}}}"#
        ));
        assert_eq!(
            r,
            json!({"line_fixes": ["1.2.5"], "walk": {"version": "1.2.5", "clears_all": true}, "withheld": ["1.2.4"]})
        );
    }

    #[test]
    fn match_inventory_reads_registry_facts_per_package() {
        let r = result(
            r#"{"op":"match_inventory",
                "inventory":[{"ecosystem":"rust","project":"/p","name":"h2","version":"0.3.16","primary":true}],
                "osv":[{"id":"RUSTSEC-2023-0034","affected":[{"package":{"name":"h2","ecosystem":"crates.io"},
                  "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"0.3.17"}]}]}]}],
                "registry":[{"ecosystem":"crates.io","name":"H2","published":["0.3.16","0.3.17","0.3.18"],"excluded":["0.3.17"]}]}"#,
        );
        let copy = &r["findings"][0]["dependency_instances"][0];
        assert_eq!(copy["target_version"], "0.3.18");
        assert_eq!(copy["withheld"], json!(["0.3.17"]));
    }

    #[test]
    fn match_inventory_stores_verbatim_records_and_matches_them() {
        let r = result(
            r#"{"op":"match_inventory",
                "inventory":[{"ecosystem":"javascript","project":"/p","name":"ws","version":"7.5.9","primary":true}],
                "osv":[{"id":"GHSA-ws","modified":"x","affected":[{"package":{"name":"ws","ecosystem":"npm"},
                  "ranges":[{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.10"}]}]}]},
                       {"id":"GHSA-ws","affected":[{"package":{"name":"ws","ecosystem":"npm"}}]}]}"#,
        );
        assert_eq!(
            r["records"], 1,
            "a repeated (id, package, ecosystem) row is stored once"
        );
        let f = &r["findings"][0];
        assert_eq!(f["advisory_id"], "GHSA-ws");
        assert_eq!(f["is_version_confirmed"], true);
        assert_eq!(f["fixed_version"], "7.5.10");
        assert_eq!(f["dependency_instances"][0]["clean_version"], "7.5.10");
        assert_eq!(f["project_paths"], json!(["/p"]));
    }

    #[test]
    fn copies_without_a_primary_row_are_an_error_not_an_all_clear() {
        let request = r#"{"op":"match_inventory",
            "inventory":[{"ecosystem":"npm","project":"/p","name":"ws","version":"7.5.9","primary":false}],
            "osv":[{"id":"GHSA-ws","affected":[{"package":{"name":"ws","ecosystem":"npm"},
              "ranges":[{"type":"SEMVER","events":[{"introduced":"0"}]}]}]}]}"#;
        let v: Value = serde_json::from_str(&call(request)).unwrap_or_default();
        assert_eq!(v["ok"], false, "{v}");
        assert!(v["error"]
            .as_str()
            .is_some_and(|e| e.contains("no primary")));
        // `primary` is required: leaving it out is a parse error too.
        let v: Value = serde_json::from_str(&call(
            r#"{"op":"match_inventory","inventory":[{"ecosystem":"npm","project":"/p","name":"ws","version":"7.5.9"}]}"#,
        ))
        .unwrap_or_default();
        assert_eq!(v["ok"], false, "{v}");
        // An empty inventory is a truthful empty answer.
        let r = result(r#"{"op":"match_inventory","inventory":[]}"#);
        assert_eq!(r["findings"], json!([]));
    }

    #[test]
    fn read_lockfiles_reads_the_files_given() {
        let r = result(
            r#"{"op":"read_lockfiles","dir":"/p","files":{
                "/p/requirements.txt":"-r base.txt\nflask==3.0.0\n",
                "/p/base.txt":"click==8.1.7\n",
                "/p/pom.xml":"<project/>"}}"#,
        );
        let outcomes = r["outcomes"].as_array().cloned().unwrap_or_default();
        // base.txt is not a requirements file by name; it is read as the include.
        assert_eq!(outcomes.len(), 2, "{r}");
        let req = &outcomes[0]["read"];
        assert_eq!(req["path"], "/p/requirements.txt");
        assert_eq!(req["format"], "requirements");
        assert_eq!(req["packages"][0]["name"], "click", "the include is read");
        assert_eq!(req["packages"][1]["name"], "flask");
        assert_eq!(outcomes[1]["unsupported"]["path"], "/p/pom.xml");
    }

    #[test]
    fn read_lockfiles_over_its_caps_is_an_error() {
        let many: serde_json::Map<String, Value> = (0..=MAX_LOCKFILE_FILES)
            .map(|i| (format!("/p/{i}.txt"), json!("")))
            .collect();
        let v: Value = serde_json::from_str(&call(
            &json!({"op": "read_lockfiles", "dir": "/p", "files": many}).to_string(),
        ))
        .unwrap_or_default();
        assert_eq!(v["ok"], false, "{}", v["error"]);
        assert!(v["error"].as_str().is_some_and(|e| e.contains("files")));

        let big = "x".repeat(MAX_LOCKFILE_BYTES);
        let v: Value = serde_json::from_str(&call(
            &json!({"op": "read_lockfiles", "dir": "/p",
                    "files": {"/p/requirements.txt": big}})
            .to_string(),
        ))
        .unwrap_or_default();
        assert_eq!(v["ok"], false);
        assert!(v["error"].as_str().is_some_and(|e| e.contains("bytes")));
    }

    #[test]
    fn a_bad_request_is_an_error_not_a_panic() {
        let v: Value = serde_json::from_str(&call("{\"op\":\"nope\"}")).unwrap_or_default();
        assert_eq!(v["ok"], false);
        let v: Value = serde_json::from_str(&call("not json")).unwrap_or_default();
        assert_eq!(v["ok"], false);
    }
}
