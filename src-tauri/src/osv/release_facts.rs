// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry release facts for the fix-target walk (FX-H5): which releases a
//! package has, and which its registry withdrew. A fix target must never be
//! one of those: lodash 4.17.21 was told "update to >= 4.18.0", a release npm
//! marks deprecated ("Bad release"), and a yanked crate named by
//! `cargo update --precise` fails outright.
//!
//! What counts as withdrawn, per registry:
//! - **crates.io:** `yanked` on the version (`/api/v1/crates/{crate}`).
//! - **PyPI:** a release whose every file is `yanked` (PEP 592; pip skips it
//!   unless pinned exactly), from `/pypi/{package}/json`.
//! - **npm:** `deprecated` on the version, from the abbreviated packument
//!   (`GET /{package}`, `Accept: application/vnd.npm.install-v1+json`), but
//!   only while the package's `latest` release is NOT deprecated. When `latest`
//!   is deprecated too, the whole package is retired ("no longer supported"),
//!   not one release; excluding every release would hide the version that
//!   clears the advisory, so nothing is excluded then.
//! - **Go:** not covered. A module's `retract` directives live in the
//!   `go.mod` of its newest version, which 4DA does not read; Go targets are
//!   computed from the advisory data alone, as before.
//!
//! Fetched only for packages with a version-confirmed advisory, after each
//! OSV sync, from hosts the registry adapters already contact (NETWORK.md
//! §2c). Kept in `kv_store` and refreshed when older than a day. Offline, or
//! for a package never fetched, the engine gets no facts and behaves exactly
//! as it did before this module: targets come from the advisory data.

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Duration, Utc};
use fourda_depengine::registry::{RegistryIndex, RegistryReleases};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::db::Database;

/// `kv_store` key holding every package's facts as one JSON document.
const KV_KEY: &str = "osv_registry_releases";
/// Facts younger than this are not re-fetched.
const FRESH_HOURS: i64 = 24;
/// Facts older than this are dropped rather than used.
const KEEP_DAYS: i64 = 30;
/// Registry reads per sync, so a large vulnerable set spreads over syncs.
const MAX_FETCHES_PER_SYNC: usize = 60;
/// Consecutive failures that end a pass (offline, or a registry down).
const MAX_FAILURES: usize = 3;
const USER_AGENT: &str = "4DA-Developer-OS/1.0 (https://4da.ai)";
const NPM_ABBREVIATED: &str = "application/vnd.npm.install-v1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredPackage {
    /// OSV ecosystem (`npm`, `crates.io`, `PyPI`).
    ecosystem: String,
    /// The matcher's package key.
    key: String,
    /// The registry's own spelling, used for the next fetch.
    name: String,
    fetched_at: DateTime<Utc>,
    #[serde(flatten)]
    releases: RegistryReleases,
}

fn read_stored(db: &Database) -> Vec<StoredPackage> {
    db.get_kv(KV_KEY)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Every package's facts for the matcher, keyed as it groups advisories.
/// Empty (and so exactly the advisory-data-only behaviour) when nothing was
/// ever fetched.
pub(crate) fn load(db: &Database) -> RegistryIndex {
    index(read_stored(db), Utc::now())
}

fn index(stored: Vec<StoredPackage>, now: DateTime<Utc>) -> RegistryIndex {
    stored
        .into_iter()
        .filter(|p| now - p.fetched_at < Duration::days(KEEP_DAYS))
        .map(|p| ((p.key, p.ecosystem), p.releases))
        .collect()
}

/// The OSV ecosystem this module reads a registry for, if any.
fn covered(ecosystem: &str) -> Option<&'static str> {
    match crate::ecosystem::Ecosystem::parse(ecosystem)?.osv_name() {
        "npm" => Some("npm"),
        "crates.io" => Some("crates.io"),
        "PyPI" => Some("PyPI"),
        _ => None,
    }
}

/// (OSV ecosystem, name) of every covered package with a version-confirmed
/// advisory: the only packages whose targets the facts can change.
fn wanted(db: &Database) -> Vec<(String, String)> {
    let matches = super::matching::get_matched_advisories(db).unwrap_or_default();
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    for m in matches.iter().filter(|m| m.is_version_confirmed) {
        if let Some(eco) = covered(&m.ecosystem) {
            out.insert((eco.to_string(), m.package_name.clone()));
        }
    }
    out.into_iter().collect()
}

/// Fetch facts for the packages that need them (missing or older than a
/// day), at most [`MAX_FETCHES_PER_SYNC`]. Best effort: a failed read keeps
/// whatever was stored. Returns how many packages were refreshed.
pub(crate) async fn refresh(db: &Database) -> usize {
    let now = Utc::now();
    let mut stored: HashMap<(String, String), StoredPackage> = read_stored(db)
        .into_iter()
        .filter(|p| now - p.fetched_at < Duration::days(KEEP_DAYS))
        .map(|p| ((p.key.clone(), p.ecosystem.clone()), p))
        .collect();
    let due = due(wanted(db), &stored, now);
    if due.is_empty() {
        return 0;
    }
    let fetched = fetch_all(due, &mut stored).await;
    save(db, stored);
    info!(target: "4da::osv", fetched, "registry release facts refreshed");
    fetched
}

/// The wanted packages with no facts, or facts older than a day, capped.
fn due(
    wanted: Vec<(String, String)>,
    stored: &HashMap<(String, String), StoredPackage>,
    now: DateTime<Utc>,
) -> Vec<(String, String)> {
    wanted
        .into_iter()
        .filter(|(eco, name)| {
            let key = (super::matching::package_key(name, eco), eco.clone());
            stored
                .get(&key)
                .is_none_or(|p| now - p.fetched_at >= Duration::hours(FRESH_HOURS))
        })
        .take(MAX_FETCHES_PER_SYNC)
        .collect()
}

fn save(db: &Database, stored: HashMap<(String, String), StoredPackage>) {
    let mut all: Vec<StoredPackage> = stored.into_values().collect();
    all.sort_by(|a, b| (&a.ecosystem, &a.key).cmp(&(&b.ecosystem, &b.key)));
    match serde_json::to_string(&all) {
        Ok(text) => {
            if let Err(e) = db.set_kv(KV_KEY, &text) {
                warn!(target: "4da::osv", error = %e, "registry release facts: store failed");
            }
        }
        Err(e) => warn!(target: "4da::osv", error = %e, "registry release facts: encode failed"),
    }
}

/// Read each due package's registry; a success replaces its stored facts.
async fn fetch_all(
    due: Vec<(String, String)>,
    stored: &mut HashMap<(String, String), StoredPackage>,
) -> usize {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_default();
    let (mut fetched, mut failures) = (0usize, 0usize);
    for (eco, name) in due {
        match fetch(&client, &eco, &name).await {
            Ok(releases) => {
                failures = 0;
                fetched += 1;
                let key = super::matching::package_key(&name, &eco);
                stored.insert(
                    (key.clone(), eco.clone()),
                    StoredPackage {
                        ecosystem: eco,
                        key,
                        name,
                        fetched_at: Utc::now(),
                        releases,
                    },
                );
            }
            Err(e) => {
                debug!(target: "4da::osv", package = %name, error = %e, "registry release facts unavailable");
                failures += 1;
                if failures >= MAX_FAILURES {
                    warn!(target: "4da::osv", "registry release facts: stopping after {failures} failures");
                    break;
                }
            }
        }
        // crates.io asks for at most one request per second.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
    fetched
}

async fn fetch(
    client: &reqwest::Client,
    eco: &str,
    name: &str,
) -> Result<RegistryReleases, String> {
    let (url, accept) = match eco {
        "npm" => (
            format!("https://registry.npmjs.org/{}", name.replace('/', "%2f")),
            NPM_ABBREVIATED,
        ),
        "crates.io" => (
            format!("https://crates.io/api/v1/crates/{name}"),
            "application/json",
        ),
        "PyPI" => (
            format!("https://pypi.org/pypi/{name}/json"),
            "application/json",
        ),
        other => return Err(format!("no registry read for {other}")),
    };
    let reply = client
        .get(&url)
        .header(reqwest::header::ACCEPT, accept)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !reply.status().is_success() {
        return Err(format!("HTTP {}", reply.status()));
    }
    let doc: serde_json::Value = reply.json().await.map_err(|e| e.to_string())?;
    let releases = match eco {
        "npm" => parse_npm(&doc),
        "crates.io" => parse_crates(&doc),
        _ => parse_pypi(&doc),
    };
    releases.ok_or_else(|| "unreadable registry answer".to_string())
}

/// npm abbreviated packument: every version, and the deprecated ones unless
/// `latest` is deprecated too (a retired package, see the module notes).
fn parse_npm(doc: &serde_json::Value) -> Option<RegistryReleases> {
    let versions = doc.get("versions")?.as_object()?;
    let deprecated = |v: &serde_json::Value| {
        v.get("deprecated")
            .is_some_and(|d| d.as_str().is_some_and(|s| !s.is_empty()) || d.as_bool() == Some(true))
    };
    let retired = doc
        .get("dist-tags")
        .and_then(|t| t.get("latest"))
        .and_then(|l| l.as_str())
        .and_then(|l| versions.get(l))
        .is_some_and(deprecated);
    Some(RegistryReleases {
        published: versions.keys().cloned().collect(),
        excluded: if retired {
            Vec::new()
        } else {
            versions
                .iter()
                .filter(|(_, v)| deprecated(v))
                .map(|(k, _)| k.clone())
                .collect()
        },
    })
}

/// crates.io crate document: every version's `num`, the yanked ones excluded.
fn parse_crates(doc: &serde_json::Value) -> Option<RegistryReleases> {
    let versions = doc.get("versions")?.as_array()?;
    let mut out = RegistryReleases::default();
    for v in versions {
        let Some(num) = v.get("num").and_then(|n| n.as_str()) else {
            continue;
        };
        out.published.push(num.to_string());
        if v.get("yanked").and_then(serde_json::Value::as_bool) == Some(true) {
            out.excluded.push(num.to_string());
        }
    }
    Some(out)
}

/// PyPI JSON API: every release; one whose files are all yanked is excluded.
/// A release with no files at all cannot be installed and is not listed.
fn parse_pypi(doc: &serde_json::Value) -> Option<RegistryReleases> {
    let releases = doc.get("releases")?.as_object()?;
    let mut out = RegistryReleases::default();
    for (version, files) in releases {
        let files = files.as_array().map(Vec::as_slice).unwrap_or_default();
        if files.is_empty() {
            continue;
        }
        out.published.push(version.clone());
        if files
            .iter()
            .all(|f| f.get("yanked").and_then(serde_json::Value::as_bool) == Some(true))
        {
            out.excluded.push(version.clone());
        }
    }
    Some(out)
}

#[cfg(test)]
#[path = "release_facts_tests.rs"]
mod tests;
