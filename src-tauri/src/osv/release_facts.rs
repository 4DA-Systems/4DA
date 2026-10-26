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
//!   only when a LATER, non-deprecated, stable release exists in the same
//!   caret range (same major; same minor for `0.x`). npm's `deprecated` is
//!   often a whole-major "please upgrade" notice, not a withdrawn release:
//!   glob deprecates 160 of 168 versions ("Old versions of glob are not
//!   supported"), 10.5.0 included, and 12.0.0 is its first clean release.
//!   Excluding them all sent glob 10.4.5 (GHSA-5j98-mcp5-4vw2, fixed 10.5.0)
//!   to 12.0.0, a major jump that turned "a lockfile refresh is enough" into
//!   "the parent must upgrade" (review of #976, 2026-10-17). lodash 4.18.0 is
//!   still excluded: 4.18.1 is clean in 4.x. A package whose `latest` is
//!   deprecated (retired whole) excludes nothing.
//! - Each ecosystem is read only while its registry source (`npm_registry`,
//!   `crates_io`, `pypi`) is enabled: a disabled source makes no calls.
//! - **Go:** not covered. A module's `retract` directives live in the
//!   `go.mod` of its newest version, which 4DA does not read; Go targets are
//!   computed from the advisory data alone, as before.
//!
//! Fetched only for packages with a version-confirmed advisory, after each
//! OSV sync, from hosts the registry adapters already contact (NETWORK.md
//! §2c). Kept in `kv_store` and refreshed when older than a day. Offline, or
//! for a package never fetched, the engine gets no facts and behaves exactly
//! as it did before this module: targets come from the advisory data.
//!
//! The same npm document also carries each release's `dependencies` (audit
//! H1, `osv::parent_release`): for the installed npm packages that pin a
//! vulnerable copy (and the declared dependency such a parent arrives
//! through), the read keeps, per release, its requirement on that one
//! package, and the `latest` tag. No other request is made for it: one
//! abbreviated document per package, as before, still name only.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Duration, Utc};
use fourda_depengine::registry::{RegistryIndex, RegistryReleases};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::osv::matching::parse_version;
use crate::osv::parent_release::{ReleaseReq, Releases, RequirementHistory};

/// `kv_store` key holding every package's facts as one JSON document.
const KV_KEY: &str = "osv_registry_releases";
/// Facts younger than this are not re-fetched.
const FRESH_HOURS: i64 = 24;
/// Facts older than this are dropped rather than used.
const KEEP_DAYS: i64 = 30;
/// Registry reads per sync. It was 60, read one at a time, so a first sync
/// over a large vulnerable set left most packages without facts and their
/// targets came from the advisory data alone, which named withdrawn releases
/// (audit A-2: lodash 4.18.0, get-func-name 2.0.1, express-fileupload
/// 1.1.9). The reads are now bounded by [`MAX_IN_FLIGHT`] and
/// [`SYNC_BUDGET_SECS`] as well, and missing facts go first (`due`).
const MAX_FETCHES_PER_SYNC: usize = 300;
/// npm and PyPI reads in flight at once (crates.io stays one per second).
const MAX_IN_FLIGHT: usize = 4;
/// No new read starts after this long; what is left waits for the next sync.
const SYNC_BUDGET_SECS: u64 = 150;
/// Failed batches in a row that end a lane (offline, or a registry down).
const MAX_FAILURES: usize = 3;
/// Whole-request budget; see `fetch_all` for the document sizes behind it.
const REQUEST_TIMEOUT_SECS: u64 = 90;
/// Gap between two crates.io reads (its crawler policy: one per second).
const CRATES_PAUSE_MS: u64 = 1100;
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
    #[serde(flatten)]
    parents: ParentFacts,
}

/// What the parent-release walk (H1) reads from an npm document beyond the
/// release list. Empty for the other registries.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ParentFacts {
    /// The `latest` dist-tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
    /// The `latest` release is deprecated: the package is retired whole.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    retired: bool,
    /// Per package this one was read for: what each release requires of it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    requires: BTreeMap<String, RequirementHistory>,
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

/// The registry source whose toggle governs reads for an OSV ecosystem.
fn source_type(ecosystem: &str) -> Option<&'static str> {
    match ecosystem {
        "npm" => Some("npm_registry"),
        "crates.io" => Some("crates_io"),
        "PyPI" => Some("pypi"),
        _ => None,
    }
}

/// Per (OSV ecosystem, name): the packages whose per-release requirement is
/// wanted from its document (empty: the release list only).
type Wants = BTreeMap<(String, String), BTreeSet<String>>;

/// Every covered package with a version-confirmed advisory, plus (npm, H1)
/// each installed package that pins such a package's vulnerable copy and the
/// declared dependency it arrives through, keeping only those whose registry
/// source is enabled: the only packages whose advice the facts can change,
/// and the only registries 4DA may contact.
fn wanted(db: &Database) -> Wants {
    let matches = super::matching::get_matched_advisories(db).unwrap_or_default();
    let mut out: Wants = BTreeMap::new();
    for m in matches.iter().filter(|m| m.is_version_confirmed) {
        if let Some(eco) = covered(&m.ecosystem) {
            out.entry((eco.to_string(), m.package_name.clone()))
                .or_default();
        }
    }
    for (parent, child) in super::parent_release_load::wanted_parents(db, &matches) {
        out.entry(("npm".to_string(), parent))
            .or_default()
            .insert(child);
    }
    let kept: BTreeSet<(String, String)> = gate(out.keys().cloned().collect(), |source| {
        db.is_source_enabled(source)
    })
    .into_iter()
    .collect();
    out.retain(|k, _| kept.contains(k));
    out
}

/// Drop the packages whose registry source is disabled.
fn gate(wanted: Vec<(String, String)>, enabled: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    wanted
        .into_iter()
        .filter(|(eco, _)| source_type(eco).is_some_and(&enabled))
        .collect()
}

/// Whether the stored registry facts list `version` as a published release
/// of `package` (any covered ecosystem). Read straight from `kv_store` on
/// `conn`, for the morning audit (G4): a target only the registry names,
/// such as lodash 4.18.1, is a real version.
pub(crate) fn registry_lists(conn: &rusqlite::Connection, package: &str, version: &str) -> bool {
    let text: Option<String> = conn
        .query_row(
            "SELECT CAST(value AS TEXT) FROM kv_store WHERE key = ?1",
            rusqlite::params![KV_KEY],
            |row| row.get(0),
        )
        .ok();
    let stored: Vec<StoredPackage> = text
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    stored.iter().any(|p| {
        (p.name.eq_ignore_ascii_case(package)
            || p.key == super::matching::package_key(package, &p.ecosystem))
            && p.releases
                .published
                .iter()
                .any(|v| v.trim() == version.trim())
    })
}

/// Fetch facts for the packages that need them (missing or older than a
/// day), at most [`MAX_FETCHES_PER_SYNC`] within [`SYNC_BUDGET_SECS`]. Best effort: a failed read keeps
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

/// One package to read, and the packages whose requirement it is read for.
type Due = ((String, String), BTreeSet<String>);

/// The wanted packages with no facts, facts older than a day, or facts that
/// lack a requirement now wanted, capped. Order (audit A-2): the vulnerable
/// packages themselves before the parents read for them; within each, a
/// package with NO facts first (its target is otherwise read from the
/// advisory data alone, which can name a withdrawn release: lodash 4.18.0),
/// then the oldest facts first, so a large set never starves the same tail
/// of the alphabet sync after sync.
fn due(
    wanted: Wants,
    stored: &HashMap<(String, String), StoredPackage>,
    now: DateTime<Utc>,
) -> Vec<Due> {
    let mut out: Vec<(Option<DateTime<Utc>>, Due)> = wanted
        .into_iter()
        .filter_map(|((eco, name), children)| {
            let key = (super::matching::package_key(&name, &eco), eco.clone());
            let held = stored.get(&key);
            let is_due = held.is_none_or(|p| {
                now - p.fetched_at >= Duration::hours(FRESH_HOURS)
                    || children.iter().any(|c| !p.parents.requires.contains_key(c))
            });
            is_due.then(|| (held.map(|p| p.fetched_at), ((eco, name), children)))
        })
        .collect();
    // `None` (never fetched) sorts before any fetch time.
    out.sort_by_key(|(fetched, (_, children))| (!children.is_empty(), *fetched));
    out.truncate(MAX_FETCHES_PER_SYNC);
    out.into_iter().map(|(_, due)| due).collect()
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
    // After the write: a reader that sees the new generation reads the new
    // document (review of #981).
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Read each due package's registry; a success replaces its stored facts.
async fn fetch_all(due: Vec<Due>, stored: &mut HashMap<(String, String), StoredPackage>) -> usize {
    // Large documents are normal here: `next`'s abbreviated packument is
    // ~26 MB and boto3's PyPI JSON ~3.3 MB (2026-10-17), so the budget is
    // per-request generous rather than the 30 s the small reads use.
    let client = match reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(crate::http_client::ssrf_guarded_redirect_policy())
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            warn!(target: "4da::osv", error = %e, "registry release facts: no HTTP client");
            return 0;
        }
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(SYNC_BUDGET_SECS);
    // crates.io asks for at most one request per second, so it is a paced
    // lane of its own; npm and PyPI run beside it, a few at a time.
    let (crates, others): (Vec<Due>, Vec<Due>) = due
        .into_iter()
        .partition(|((eco, _), _)| eco == "crates.io");
    let (paced, parallel) = futures::join!(
        lane(&client, crates, 1, CRATES_PAUSE_MS, deadline),
        lane(&client, others, MAX_IN_FLIGHT, 0, deadline),
    );
    let mut fetched = 0;
    for ((eco, name), (releases, parents)) in paced.into_iter().chain(parallel) {
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
                parents,
            },
        );
    }
    fetched
}

/// One package's read facts, keyed (OSV ecosystem, registry name).
type Fetched = ((String, String), (RegistryReleases, ParentFacts));

/// Read `due` in order, `width` at a time with `pause_ms` between batches,
/// starting no batch after `deadline`; [`MAX_FAILURES`] failed batches in a
/// row (offline, or the registry down) end the lane.
async fn lane(
    client: &reqwest::Client,
    due: Vec<Due>,
    width: usize,
    pause_ms: u64,
    deadline: tokio::time::Instant,
) -> Vec<Fetched> {
    let mut out = Vec::new();
    let mut failures = 0usize;
    for (i, batch) in due.chunks(width.max(1)).enumerate() {
        if tokio::time::Instant::now() >= deadline {
            info!(target: "4da::osv", left = due.len() - i * width.max(1), "registry release facts: sync budget spent, the rest next sync");
            break;
        }
        if i > 0 && pause_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(pause_ms)).await;
        }
        let reads = batch
            .iter()
            .map(|((eco, name), children)| fetch(client, eco, name, children));
        let mut any = false;
        for (((eco, name), _), result) in batch.iter().zip(futures::future::join_all(reads).await) {
            match result {
                Ok(facts) => {
                    any = true;
                    out.push(((eco.clone(), name.clone()), facts));
                }
                Err(e) => {
                    debug!(target: "4da::osv", package = %name, error = %e, "registry release facts unavailable");
                }
            }
        }
        failures = if any { 0 } else { failures + 1 };
        if failures >= MAX_FAILURES {
            warn!(target: "4da::osv", "registry release facts: stopping a lane after {failures} failed batches");
            break;
        }
    }
    out
}

async fn fetch(
    client: &reqwest::Client,
    eco: &str,
    name: &str,
    children: &BTreeSet<String>,
) -> Result<(RegistryReleases, ParentFacts), String> {
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
    let parents = if eco == "npm" {
        parse_npm_parent(&doc, children)
    } else {
        ParentFacts::default()
    };
    releases
        .map(|r| (r, parents))
        .ok_or_else(|| "unreadable registry answer".to_string())
}

/// npm abbreviated packument, for H1: the `latest` tag, whether it is
/// deprecated, and per release its requirement on each of `children`, from
/// `dependencies` / `optionalDependencies`, else `peerDependencies` (flagged:
/// npm 7+ installs a missing peer). A release that names none of them gets an
/// empty entry: it does not depend on the package.
fn parse_npm_parent(doc: &serde_json::Value, children: &BTreeSet<String>) -> ParentFacts {
    let Some(versions) = doc.get("versions").and_then(|v| v.as_object()) else {
        return ParentFacts::default();
    };
    let latest = doc
        .get("dist-tags")
        .and_then(|t| t.get("latest"))
        .and_then(|l| l.as_str())
        .map(str::to_string);
    let retired = latest
        .as_deref()
        .and_then(|l| versions.get(l))
        .is_some_and(npm_deprecated);
    let field = |m: &serde_json::Value, section: &str, child: &str| {
        m.get(section)?.get(child)?.as_str().map(str::to_string)
    };
    let requires = children
        .iter()
        .map(|child| {
            let history: RequirementHistory = versions
                .iter()
                // Pre-releases are never a parent release to name.
                .filter(|(v, _)| parse_version(v).is_some_and(|p| p.pre.is_empty()))
                .map(|(v, m)| {
                    let direct = field(m, "dependencies", child)
                        .or_else(|| field(m, "optionalDependencies", child));
                    let req = match (direct, field(m, "peerDependencies", child)) {
                        (Some(r), _) => ReleaseReq {
                            reqs: vec![r],
                            peer: false,
                        },
                        (None, Some(p)) => ReleaseReq {
                            reqs: vec![p],
                            peer: true,
                        },
                        (None, None) => ReleaseReq::default(),
                    };
                    (v.clone(), req)
                })
                .collect();
            (child.clone(), history)
        })
        .collect();
    ParentFacts {
        latest,
        retired,
        requires,
    }
}

/// The stored registry facts, parsed once, for the parent-release walk (H1).
pub(crate) struct ReleaseBook {
    by_key: HashMap<(String, String), StoredPackage>,
}

/// Bumped by every write of the stored document in this process.
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// How long a parsed document is reused without re-reading it: one plan or
/// feed build reads it once (review of #981, N8). A write from another
/// process (the headless engine) is seen within this long.
const BOOK_REUSE_SECS: u64 = 60;

type CachedBook = (usize, u64, std::time::Instant, std::sync::Arc<ReleaseBook>);

/// Every package's facts, parsed once per build: reused while the same
/// database has seen no write here and the copy is younger than
/// [`BOOK_REUSE_SECS`]. Empty when nothing was fetched.
pub(crate) fn book(db: &Database) -> std::sync::Arc<ReleaseBook> {
    static CACHE: std::sync::Mutex<Option<CachedBook>> = std::sync::Mutex::new(None);
    if cfg!(test) {
        // Tests run many short-lived databases in parallel; an address can
        // be reused, so they always read (the cache itself is tested through
        // `book_cached`).
        return std::sync::Arc::new(book_from(read_stored(db), Utc::now()));
    }
    let generation = GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    book_cached(db, &CACHE, std::time::Instant::now(), generation)
}

/// [`book`] with its cache, clock and write generation passed in.
fn book_cached(
    db: &Database,
    cache: &std::sync::Mutex<Option<CachedBook>>,
    now: std::time::Instant,
    generation: u64,
) -> std::sync::Arc<ReleaseBook> {
    let at = std::ptr::from_ref(db) as usize;
    if let Ok(cache) = cache.lock() {
        let reusable = cache.as_ref().filter(|(addr, g, when, _)| {
            *addr == at
                && *g == generation
                && now.saturating_duration_since(*when).as_secs() < BOOK_REUSE_SECS
        });
        if let Some((.., b)) = reusable {
            return b.clone();
        }
    }
    let fresh = std::sync::Arc::new(book_from(read_stored(db), Utc::now()));
    if let Ok(mut cache) = cache.lock() {
        *cache = Some((at, generation, now, fresh.clone()));
    }
    fresh
}

fn book_from(stored: Vec<StoredPackage>, now: DateTime<Utc>) -> ReleaseBook {
    ReleaseBook {
        by_key: stored
            .into_iter()
            .filter(|p| now - p.fetched_at < Duration::days(KEEP_DAYS))
            .map(|p| ((p.key.clone(), p.ecosystem.clone()), p))
            .collect(),
    }
}

impl ReleaseBook {
    fn get(&self, eco: &str, name: &str) -> Option<&StoredPackage> {
        self.by_key
            .get(&(super::matching::package_key(name, eco), eco.to_string()))
    }

    /// The installable releases (listed, not withdrawn), `latest` and
    /// whether it is deprecated. `None` when never read.
    pub(crate) fn releases(&self, eco: &str, name: &str) -> Option<Releases> {
        let p = self.get(eco, name)?;
        let withdrawn = |v: &String| p.releases.excluded.iter().any(|e| e == v);
        Some(Releases {
            versions: p
                .releases
                .published
                .iter()
                .filter(|v| !withdrawn(v))
                .cloned()
                .collect(),
            latest: p.parents.latest.clone(),
            deprecated: p.parents.retired,
        })
    }

    /// What each release of `parent` requires of `child`, when read.
    pub(crate) fn history(
        &self,
        eco: &str,
        parent: &str,
        child: &str,
    ) -> Option<&RequirementHistory> {
        let requires = &self.get(eco, parent)?.parents.requires;
        requires.get(child).or_else(|| {
            requires
                .iter()
                .find(|(c, _)| c.eq_ignore_ascii_case(child))
                .map(|(_, h)| h)
        })
    }

    /// The packages `parent`'s requirements were read for.
    pub(crate) fn children(&self, eco: &str, parent: &str) -> Vec<String> {
        self.get(eco, parent)
            .map(|p| p.parents.requires.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The release facts the fix-target walk reads for `name`.
    pub(crate) fn registry(&self, eco: &str, name: &str) -> RegistryReleases {
        self.get(eco, name)
            .map(|p| p.releases.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "release_facts_test_support.rs"]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::{store_docs_for_tests, store_for_tests};

fn npm_deprecated(manifest: &serde_json::Value) -> bool {
    manifest
        .get("deprecated")
        .is_some_and(|d| d.as_str().is_some_and(|s| !s.is_empty()) || d.as_bool() == Some(true))
}

/// Same caret range (`^a` admits `b`'s line): same major, and for `0.x` the
/// same minor.
fn same_caret_line(a: &semver::Version, b: &semver::Version) -> bool {
    a.major == b.major && (a.major > 0 || a.minor == b.minor)
}

/// npm abbreviated packument: every version, and a deprecated one excluded
/// only when a later, non-deprecated, stable release exists on its caret line
/// (see the module notes). A package whose `latest` is deprecated (retired
/// whole) excludes nothing.
fn parse_npm(doc: &serde_json::Value) -> Option<RegistryReleases> {
    let versions = doc.get("versions")?.as_object()?;
    let retired = doc
        .get("dist-tags")
        .and_then(|t| t.get("latest"))
        .and_then(|l| l.as_str())
        .and_then(|l| versions.get(l))
        .is_some_and(npm_deprecated);
    let clean: Vec<semver::Version> = versions
        .iter()
        .filter(|(_, m)| !npm_deprecated(m))
        .filter_map(|(v, _)| parse_version(v))
        .filter(|v| v.pre.is_empty())
        .collect();
    let superseded = |v: &semver::Version| clean.iter().any(|c| same_caret_line(v, c) && c > v);
    let excluded = if retired {
        Vec::new()
    } else {
        versions
            .iter()
            .filter(|(_, m)| npm_deprecated(m))
            .filter(|(v, _)| parse_version(v).is_some_and(|p| superseded(&p)))
            .map(|(v, _)| v.clone())
            .collect()
    };
    Some(RegistryReleases {
        published: versions.keys().cloned().collect(),
        excluded,
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
