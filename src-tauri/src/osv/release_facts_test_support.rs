// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Test helpers that store registry facts as a fetch would.

use std::collections::{BTreeSet, HashMap};

use chrono::Utc;
use fourda_depengine::registry::RegistryReleases;

use super::{parse_npm, parse_npm_parent, save, ParentFacts, StoredPackage};
use crate::db::Database;
use crate::osv::parent_release::RequirementHistory;

/// One package's stored facts, for tests: (ecosystem, name, published,
/// latest, retired, requirement history per child).
pub(crate) type TestFacts<'a> = (
    &'a str,
    &'a str,
    Vec<&'a str>,
    Option<&'a str>,
    bool,
    Vec<(&'a str, RequirementHistory)>,
);

/// Store npm abbreviated documents as a fetch would, through the same
/// parsers (withdrawn releases and requirements read from the raw document),
/// for tests and the graph rule vectors. Each document's requirements are
/// kept for every dependency it names.
pub(crate) fn store_docs_for_tests(db: &Database, docs: &[(String, serde_json::Value)]) {
    let stored: HashMap<(String, String), StoredPackage> = docs
        .iter()
        .map(|(name, doc)| {
            let children: BTreeSet<String> = doc
                .get("versions")
                .and_then(|v| v.as_object())
                .into_iter()
                .flat_map(|vs| vs.values())
                .flat_map(|m| {
                    ["dependencies", "optionalDependencies", "peerDependencies"]
                        .into_iter()
                        .filter_map(|s| m.get(s).and_then(|d| d.as_object()))
                        .flat_map(|d| d.keys().cloned())
                        .collect::<Vec<_>>()
                })
                .collect();
            let key = crate::osv::matching::package_key(name, "npm");
            let p = StoredPackage {
                ecosystem: "npm".to_string(),
                key: key.clone(),
                name: name.clone(),
                fetched_at: Utc::now(),
                releases: parse_npm(doc).unwrap_or_default(),
                parents: parse_npm_parent(doc, &children),
            };
            ((key, "npm".to_string()), p)
        })
        .collect();
    save(db, stored);
}

/// Store facts as a fetch would, for tests.
pub(crate) fn store_for_tests(db: &Database, packages: Vec<TestFacts<'_>>) {
    let stored: HashMap<(String, String), StoredPackage> = packages
        .into_iter()
        .map(|(eco, name, published, latest, retired, requires)| {
            let key = crate::osv::matching::package_key(name, eco);
            let p = StoredPackage {
                ecosystem: eco.to_string(),
                key: key.clone(),
                name: name.to_string(),
                fetched_at: Utc::now(),
                releases: RegistryReleases {
                    published: published.into_iter().map(str::to_string).collect(),
                    excluded: Vec::new(),
                },
                parents: ParentFacts {
                    latest: latest.map(str::to_string),
                    retired,
                    requires: requires
                        .into_iter()
                        .map(|(c, h)| (c.to_string(), h))
                        .collect(),
                },
            };
            ((key, eco.to_string()), p)
        })
        .collect();
    save(db, stored);
}
