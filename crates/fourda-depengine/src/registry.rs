// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What a package's registry says about its releases, beyond the advisory
//! data: which releases exist, and which the registry has withdrawn or marked
//! unfit (crates.io and PyPI `yanked`, npm `deprecated`). The host fetches
//! these; the engine only reads them.
//!
//! Live 2026-10-16 (FX-H5): every advisory lodash 4.17.21 still had was fixed
//! in 4.18.0, so 4DA said "update lodash to >= 4.18.0". npm marks 4.18.0
//! deprecated ("Bad release. Please use lodash@4.17.21 instead."); 4.18.1 is
//! the release to take. The walk now steps over a withdrawn release exactly
//! as it steps over a pre-release, and names what it stepped over.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::version_order::OrderedVersion;

/// One package's releases as its registry reports them. Both lists empty is
/// "no registry facts": the engine then knows only the releases the advisory
/// data names, as it did before this input existed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryReleases {
    /// Every release the registry lists (any order; pre-releases are left out
    /// of targets as always). Lets the walk name a release no advisory
    /// mentions, such as the one after a withdrawn fix.
    #[serde(default)]
    pub published: Vec<String>,
    /// Releases the registry withdrew or marked unfit. Never a target.
    #[serde(default)]
    pub excluded: Vec<String>,
}

impl RegistryReleases {
    /// No facts at all.
    pub fn is_empty(&self) -> bool {
        self.published.is_empty() && self.excluded.is_empty()
    }

    /// The excluded releases, parsed in `ecosystem`. Unparseable spellings
    /// are dropped (they cannot equal a version the walk can name).
    pub fn excluded_in(&self, ecosystem: &str) -> Vec<OrderedVersion> {
        self.excluded
            .iter()
            .filter_map(|v| OrderedVersion::parse_in(v, ecosystem))
            .collect()
    }

    /// Whether the registry withdrew `version` (equal as versions, so build
    /// metadata such as Go's `+incompatible` does not hide it).
    pub fn excludes(&self, ecosystem: &str, version: &str) -> bool {
        OrderedVersion::parse_in(version, ecosystem)
            .is_some_and(|v| is_listed(&self.excluded_in(ecosystem), &v))
    }
}

/// Whether `version` equals any of `listed`.
pub(crate) fn is_listed(listed: &[OrderedVersion], version: &OrderedVersion) -> bool {
    listed
        .iter()
        .any(|l| version.compare(l) == Some(Ordering::Equal))
}

/// Registry facts per package, keyed by (package key as
/// [`crate::matching::package_key`] forms it, OSV ecosystem), the key the
/// matcher groups a package's advisories by.
pub type RegistryIndex = HashMap<(String, String), RegistryReleases>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_compares_as_versions() {
        let r = RegistryReleases {
            published: vec![],
            excluded: vec!["4.18.0".into(), "v25.0.13+incompatible".into()],
        };
        assert!(r.excludes("npm", "4.18.0"));
        assert!(!r.excludes("npm", "4.18.1"));
        assert!(r.excludes("Go", "v25.0.13"));
        assert!(!r.excludes("npm", "not a version"));
        assert!(!r.is_empty());
        assert!(RegistryReleases::default().is_empty());
    }
}
