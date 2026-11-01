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
//! - `{"op":"range_check","eco":"npm","name":"ws","constraint":"7.x","from":"7.4.6",
//!   "registry":{"published":[..],"excluded":[..]},"osv":[verbatim OSV records],
//!   "records":[AdvisoryRecord..],"advisories":[RangeAdvisory..],"held_unsafe":[..]}`
//!   (C0.5) → a [`crate::range_check::RangeCheck`]: the safe releases the
//!   constraint admits. The package's advisories are `advisories` plus the
//!   rows of `records` and `osv` (stored as the mirror stores them) that name
//!   `name`; at least one of the three must be given, so "no advisories" is
//!   always stated, never a missing field. `from` and `held_unsafe` (releases
//!   the host's held-dependency check ruled out) are optional. An unreadable
//!   constraint is an error.

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
use crate::range_check::{package_advisories, range_check, RangeAdvisory, RangeQuery};
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
    RangeCheck(Box<RangeCheckRequest>),
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

/// A `range_check` request (C0.5).
#[derive(Deserialize)]
struct RangeCheckRequest {
    eco: String,
    name: String,
    constraint: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    registry: RegistryReleases,
    osv: Option<Vec<OsvRecord>>,
    records: Option<Vec<AdvisoryRecord>>,
    advisories: Option<Vec<RangeAdvisory>>,
    #[serde(default)]
    held_unsafe: Vec<String>,
}

/// The `range_check` op: the package's advisories gathered, then the rule.
fn range_check_op(request: RangeCheckRequest) -> Result<Value, String> {
    let RangeCheckRequest {
        eco,
        name,
        constraint,
        from,
        registry,
        osv,
        records,
        advisories,
        held_unsafe,
    } = request;
    if osv.is_none() && records.is_none() && advisories.is_none() {
        return Err(
            "range_check: give the package's advisories (`advisories`, `records` or `osv`; an empty list means none)"
                .to_string(),
        );
    }
    let mut rows = records.unwrap_or_default();
    rows.extend(osv.iter().flatten().flat_map(stored_records));
    let mut all = advisories.unwrap_or_default();
    all.extend(package_advisories(&rows, &eco, &name));
    let answer = range_check(&RangeQuery {
        ecosystem: &eco,
        constraint: &constraint,
        from: from.as_deref(),
        registry: &registry,
        advisories: &all,
        held_unsafe: &held_unsafe,
    })
    .map_err(|e| format!("range_check: {e}"))?;
    serde_json::to_value(answer).map_err(|e| format!("range_check: {e}"))
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
        Request::RangeCheck(request) => range_check_op(*request)?,
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
#[path = "api_tests.rs"]
mod tests;
