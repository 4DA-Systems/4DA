// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! npm fix targets judged by what they install (audit 6.2.5 D-4), applied to
//! every matched copy after the engine's walk (`osv::matching`).
//!
//! The walk names the smallest release the package's OWN advisories leave
//! clean. For npm that is not yet a clean fix: express 4.20.0 is clear of
//! express's advisories and exact-pins qs 6.11.0 and path-to-regexp 0.1.10,
//! both affected, so NodeGoat was told "update express to 4.20.0". Here each
//! npm copy's target is re-judged by the pure rule
//! (`fourda_depengine::held`): when what it holds is affected, the next
//! releases on its line that its own advisories leave clean are tried, and
//! the smallest that holds nothing affected becomes the target. None clean:
//! the target stays, flagged NOT a clean fix. The check cannot run (registry
//! facts not read, a held package's advisories not in the mirror): the
//! target stays, flagged NOT CHECKED. Never moved on a guess.
//!
//! Inputs, all local: the registry facts `release_facts` keeps (what each
//! release holds; the held ranges' release lists and `latest`) and the
//! advisory rows the matcher already read. A held package's advisories are
//! read only when the mirror covers it: it is installed in a project, the
//! mirror holds a record for it, or the npm sync that included it as a held
//! name succeeded (`release_facts::held_synced`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use fourda_depengine::fix_target::AdvisoryKind;
use fourda_depengine::held::{
    check_release, held_candidates, judge_target, HeldFacts, HeldOutcome, HeldSources,
    OwnAdvisories,
};
use fourda_depengine::matching::Finding;
use fourda_depengine::osv::AdvisoryRecord;

use super::matching::{check_version_affected, package_key};
use super::release_facts::ReleaseBook;
use super::types::StoredAdvisory;

/// What the rule reads, gathered once per matching run.
pub(crate) struct HeldContext<'a> {
    book: &'a ReleaseBook,
    /// npm package key -> indices into `records`.
    by_key: HashMap<String, Vec<usize>>,
    records: &'a [AdvisoryRecord],
    advisories: &'a [StoredAdvisory],
    /// npm package keys whose advisories the mirror covers.
    covered: BTreeSet<String>,
    /// Memos (review of #1039, N1): the same dependency and range recur in
    /// every candidate of every copy, and this runs inside every
    /// `get_matched_advisories` call.
    judged: RefCell<HashMap<(String, String), Option<String>>>,
    hits: RefCell<HashMap<(String, String), Option<Vec<String>>>>,
}

fn is_npm(ecosystem: &str) -> bool {
    crate::ecosystem::Ecosystem::parse(ecosystem).is_some_and(|e| e.osv_name() == "npm")
}

impl<'a> HeldContext<'a> {
    /// `installed`: the npm names the projects hold (the set the mirror is
    /// synced for); `synced`: the held names a successful npm sync included.
    pub(crate) fn new(
        book: &'a ReleaseBook,
        records: &'a [AdvisoryRecord],
        advisories: &'a [StoredAdvisory],
        installed: impl IntoIterator<Item = String>,
        synced: BTreeSet<String>,
    ) -> Self {
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in records
            .iter()
            .enumerate()
            .filter(|(_, r)| is_npm(&r.ecosystem))
        {
            by_key
                .entry(package_key(&r.package_name, "npm"))
                .or_default()
                .push(i);
        }
        let mut covered = synced;
        covered.extend(installed.into_iter().map(|n| package_key(&n, "npm")));
        covered.extend(by_key.keys().cloned());
        Self {
            book,
            by_key,
            records,
            advisories,
            covered,
            judged: RefCell::default(),
            hits: RefCell::default(),
        }
    }

    fn rows(&self, name: &str) -> &[usize] {
        self.by_key
            .get(&package_key(name, "npm"))
            .map_or(&[], Vec::as_slice)
    }

    /// The outcome for one copy: `None` when the rule has nothing to say.
    pub(crate) fn judge(&self, name: &str, installed: &str, target: &str) -> Option<HeldOutcome> {
        let unchecked = |reason: String| {
            Some(HeldOutcome::Unchecked {
                target: target.to_string(),
                reason,
            })
        };
        let Some(read) = self.book.held_specs(name) else {
            return unchecked(format!(
                "the npm registry has not been read for what {name}'s releases depend on"
            ));
        };
        // Read from a higher release than the target (a lower copy was
        // installed since): the target's own entry is simply absent, which
        // must not read as "holds nothing" (review of #1039, B1).
        if !read.covers(target) {
            return unchecked(format!(
                "the npm registry was read for {name} from {} up; {target} is below it",
                read.from.as_deref().unwrap_or("its newest releases")
            ));
        }
        let held = &read.specs;
        let at = |v: &str| held.get(v);
        at(target)?;
        let releases = self
            .book
            .releases("npm", name)
            .map(|r| r.versions)
            .unwrap_or_default();
        let own: Vec<(&Option<String>, AdvisoryKind)> = self
            .rows(name)
            .iter()
            .map(|&i| (&self.records[i].affected_ranges, self.records[i].kind()))
            .collect();
        let view = OwnAdvisories::new(Some(installed), &releases, &own);
        let candidates = held_candidates(target, &releases, |v| view.clean(v));
        let empty = BTreeMap::new();
        judge_target(&candidates, |v| {
            check_release(at(v).unwrap_or(&empty), self)
        })
    }

    /// The package's own advisories leave nothing at all on `version`.
    fn clears_all(&self, name: &str, installed: &str, version: &str) -> bool {
        let releases = self
            .book
            .releases("npm", name)
            .map(|r| r.versions)
            .unwrap_or_default();
        let own: Vec<(&Option<String>, AdvisoryKind)> = self
            .rows(name)
            .iter()
            .map(|&i| (&self.records[i].affected_ranges, self.records[i].kind()))
            .collect();
        OwnAdvisories::new(Some(installed), &releases, &own).clears_all(version)
    }
}

impl HeldSources for HeldContext<'_> {
    fn facts(&self, name: &str) -> Option<HeldFacts> {
        self.book.held_facts(name)
    }

    fn judged(&self, name: &str, spec: &str) -> Option<String> {
        let key = (package_key(name, "npm"), spec.to_string());
        if let Some(hit) = self.judged.borrow().get(&key) {
            return hit.clone();
        }
        let got = fourda_depengine::held::judged_version(spec, self.facts(name).as_ref());
        self.judged.borrow_mut().insert(key, got.clone());
        got
    }

    fn affected(&self, name: &str, version: &str) -> Option<Vec<String>> {
        let key = (package_key(name, "npm"), version.to_string());
        if let Some(hit) = self.hits.borrow().get(&key) {
            return hit.clone();
        }
        let got = self.read_affected(name, version);
        self.hits.borrow_mut().insert(key, got.clone());
        got
    }
}

impl HeldContext<'_> {
    fn read_affected(&self, name: &str, version: &str) -> Option<Vec<String>> {
        if !self.covered.contains(&package_key(name, "npm")) {
            return None;
        }
        // One id per alias group, maintenance notices left out.
        let mut ids: Vec<String> = Vec::new();
        let mut aliases: BTreeSet<&str> = BTreeSet::new();
        for &i in self.rows(name) {
            let r = &self.records[i];
            if r.informational
                || !check_version_affected("npm", Some(version), &r.affected_ranges).0
            {
                continue;
            }
            let own = self
                .advisories
                .get(i)
                .map(|a| a.aliases.as_slice())
                .unwrap_or_default();
            if ids.contains(&r.advisory_id) || aliases.contains(r.advisory_id.as_str()) {
                continue;
            }
            if own.iter().any(|a| ids.contains(a)) {
                continue;
            }
            aliases.extend(own.iter().map(String::as_str));
            ids.push(r.advisory_id.clone());
        }
        ids.sort();
        Some(ids)
    }
}

/// Judge one release of an npm package outside a matching run, reading what
/// the matcher reads (the raise in `raised_lines` names a release the walk
/// never judged; review of #1039, S1). Unchecked when the inputs cannot be
/// read.
pub(crate) fn judge_in_db(
    db: &crate::db::Database,
    name: &str,
    installed: &str,
    target: &str,
) -> Option<HeldOutcome> {
    let read = || -> Result<_, String> {
        let advisories = db.get_all_osv_advisories().map_err(|e| e.to_string())?;
        let mut deps = db
            .get_auditable_user_dependencies()
            .map_err(|e| e.to_string())?;
        deps.extend(
            db.get_auditable_scanned_dependencies()
                .map_err(|e| e.to_string())?,
        );
        Ok((advisories, deps))
    };
    let (advisories, deps) = match read() {
        Ok(inputs) => inputs,
        Err(e) => {
            return Some(HeldOutcome::Unchecked {
                target: target.to_string(),
                reason: format!("the local advisory data could not be read ({e})"),
            })
        }
    };
    let records: Vec<AdvisoryRecord> = advisories
        .iter()
        .map(super::matching::advisory_record)
        .collect();
    let book = super::release_facts::book(db);
    let installed_npm = deps
        .into_iter()
        .filter(|d| is_npm(&d.ecosystem))
        .map(|d| d.package_name);
    let ctx = HeldContext::new(
        &book,
        &records,
        &advisories,
        installed_npm,
        super::release_facts::held_synced(db),
    );
    ctx.judge(name, installed, target)
}

/// Re-judge every confirmed npm copy's target in `findings`.
pub(crate) fn apply(ctx: &HeldContext<'_>, findings: &mut [Finding]) {
    let mut cache: HashMap<(String, String, String), Option<HeldOutcome>> = HashMap::new();
    for f in findings.iter_mut() {
        let Some(record) = ctx.records.get(f.record) else {
            continue;
        };
        if !is_npm(&record.ecosystem) {
            continue;
        }
        let name = record.package_name.as_str();
        for copy in f
            .dependency_instances
            .iter_mut()
            .filter(|c| c.is_version_confirmed)
        {
            let (Some(installed), Some(target)) =
                (copy.installed_version.clone(), copy.target_version.clone())
            else {
                continue;
            };
            let outcome = cache
                .entry((package_key(name, "npm"), installed.clone(), target.clone()))
                .or_insert_with(|| ctx.judge(name, &installed, &target))
                .clone();
            let Some(outcome) = outcome else {
                continue;
            };
            if let HeldOutcome::Moved { to, .. } = &outcome {
                copy.clean_version = ctx.clears_all(name, &installed, to).then(|| to.clone());
                copy.target_version = Some(to.clone());
            }
            copy.held = Some(outcome);
        }
    }
}

#[cfg(test)]
#[path = "held_targets_tests.rs"]
mod tests;
