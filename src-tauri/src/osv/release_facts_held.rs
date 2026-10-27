// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What the registry facts keep for the held-dependency rule (audit 6.2.5
//! D-4, `fourda_depengine::held`), read from the SAME npm abbreviated
//! document `release_facts` already fetches for a vulnerable package: per
//! stable release at or above the lowest installed copy, the dependencies it
//! exact-pins or narrowly ranges (`dependencies` + `optionalDependencies`).
//!
//! A narrow range is judged at its dependency's `latest` or highest admitted
//! release, so that dependency's own abbreviated document is read too (name
//! only, the same host, request type, pacing and per-sync budget; NETWORK.md
//! §2c). And its advisories must be in the local mirror: the OSV sync adds
//! every held dependency's name to the npm package set it syncs, and records
//! which names a successful npm sync covered ([`record_synced`]), so "no
//! advisory" is only ever read for a package whose advisories were fetched.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::osv::matching::parse_version;

use super::StoredPackage;

/// `kv_store` key: the held-dependency names the last successful npm OSV
/// sync included.
const SYNCED_KEY: &str = "osv_held_synced_v1";

/// Per release, the held specs (name -> spec) of one npm package.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct HeldSpecs {
    /// The document was read for held specs (a package stored before this
    /// existed, or never read as a vulnerable package, was not).
    #[serde(default)]
    pub read: bool,
    /// The lowest release kept (the lowest installed copy when read).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Release -> its held specs; a release that holds nothing is absent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub specs: BTreeMap<String, BTreeMap<String, String>>,
}

impl HeldSpecs {
    pub(super) fn is_unread(&self) -> bool {
        !self.read
    }

    /// Read for `floor` or lower already.
    pub(super) fn covers(&self, floor: &str) -> bool {
        self.read
            && match (
                self.from.as_deref().and_then(parse_version),
                parse_version(floor),
            ) {
                (Some(from), Some(want)) => from <= want,
                (None, _) => true,
                (Some(_), None) => false,
            }
    }
}

/// The held specs of every stable release of `doc` at or above `floor`.
pub(super) fn parse_npm_held(doc: &serde_json::Value, floor: Option<&str>) -> HeldSpecs {
    let floor_v = floor.and_then(parse_version);
    let specs = doc
        .get("versions")
        .and_then(|v| v.as_object())
        .into_iter()
        .flatten()
        .filter(|(v, _)| {
            parse_version(v)
                .is_some_and(|p| p.pre.is_empty() && floor_v.as_ref().is_none_or(|f| p >= *f))
        })
        .map(|(v, m)| {
            (
                v.clone(),
                fourda_depengine::held::held_specs_of(
                    m.get("dependencies"),
                    m.get("optionalDependencies"),
                ),
            )
        })
        .filter(|(_, held)| !held.is_empty())
        .collect();
    HeldSpecs {
        read: true,
        from: floor.map(str::to_string),
        specs,
    }
}

/// Every name some stored npm package's releases hold, and (second) the
/// subset held through a narrow range, whose own document is needed.
pub(super) fn held_names(
    stored: &HashMap<(String, String), StoredPackage>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut all = BTreeSet::new();
    let mut ranged = BTreeSet::new();
    for p in stored.values().filter(|p| p.ecosystem == "npm") {
        for (name, spec) in p.held.specs.values().flatten() {
            all.insert(name.clone());
            if fourda_depengine::held::exact_pin_version(spec).is_none() {
                ranged.insert(name.clone());
            }
        }
    }
    (all, ranged)
}

/// The held-dependency names the npm OSV sync should include.
pub(crate) fn names_to_sync(db: &Database) -> Vec<String> {
    let stored: HashMap<(String, String), StoredPackage> = super::read_stored(db)
        .into_iter()
        .map(|p| ((p.key.clone(), p.ecosystem.clone()), p))
        .collect();
    held_names(&stored).0.into_iter().collect()
}

/// After a successful npm sync that included `names`: their advisories are
/// in the mirror (or they have none).
pub(crate) fn record_synced(db: &Database, names: &[String]) {
    let keys: BTreeSet<String> = names
        .iter()
        .map(|n| crate::osv::matching::package_key(n, "npm"))
        .collect();
    if let Ok(text) = serde_json::to_string(&keys) {
        if let Err(e) = db.set_kv(SYNCED_KEY, &text) {
            tracing::warn!(target: "4da::osv", error = %e, "held names: store failed");
        }
    }
}

/// The package keys a successful npm sync covered as held dependencies.
pub(crate) fn synced(db: &Database) -> BTreeSet<String> {
    db.get_kv(SYNCED_KEY)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}
