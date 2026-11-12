// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One ecosystem's sync pass, and what it is allowed to record.
//!
//! A pass ends one of three ways, and only the first is a sync:
//!
//! - **Verified**: the advisory data was confirmed current against the
//!   remote: the archive's ETag matched, or a download completed.
//! - **Unverified**: the remote could not confirm it, and the pass served
//!   from the archive already on disk. Its advisories stay matched (dated
//!   data beats none), but the status keeps the last verified time and
//!   carries the reason, so every reader sees a check that did not happen.
//! - **Failed**: no advisory data could be read at all.
//!
//! Until 2026-11-11 the second case recorded "synced now, no error": with the
//! network blocked, `--engine-once` re-read its cached archives, logged
//! "OSV sync complete" and exited 0, and the app reported advisories as
//! freshly checked for as long as it stayed offline.

use std::collections::HashSet;

use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::db::Database;
use crate::error::Result;
use crate::osv::cache::{Freshness, Mirror};
use crate::osv::types::SyncResult;

/// How one ecosystem's pass ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EcosystemOutcome {
    /// The advisory data is confirmed current; `stored` rows were written.
    Verified { stored: usize },
    /// Served from the archive on disk without confirmation. `stored` is
    /// `None` when the mirror already held that archive's rows and nothing
    /// was re-read; `data_as_of` is when the archive was downloaded.
    Unverified {
        stored: Option<usize>,
        reason: String,
        data_as_of: Option<String>,
    },
    /// Nothing could be read.
    Failed { error: String },
}

/// What a pass over one ecosystem's archive did.
#[derive(Debug)]
struct ArchivePass {
    /// Advisory rows stored, or `None` when nothing needed re-reading.
    stored: Option<usize>,
    /// Why the archive could not be confirmed current, when it could not.
    unverified: Option<String>,
}

/// Run one ecosystem's pass over its archive mirror. `packages` are matched
/// on this machine; none of them is sent anywhere (audit P2).
pub(super) async fn sync_one_ecosystem(
    db: &Database,
    mirror: &Mirror,
    ecosystem: &str,
    packages: &[String],
) -> EcosystemOutcome {
    match populate_from_zip_mirror(db, mirror, ecosystem, packages).await {
        Ok(ArchivePass {
            stored,
            unverified: None,
        }) => {
            info!(
                target: "4da::osv",
                ecosystem,
                cached_advisories = stored.unwrap_or(0),
                "Populated from the verified ZIP mirror"
            );
            EcosystemOutcome::Verified {
                stored: stored.unwrap_or(0),
            }
        }
        Ok(ArchivePass {
            stored,
            unverified: Some(cause),
        }) => EcosystemOutcome::Unverified {
            stored,
            reason: format!(
                "could not check for newer advisories ({cause}); matched against the archive already on disk"
            ),
            data_as_of: mirror.archive_downloaded_at(ecosystem),
        },
        Err(e) => EcosystemOutcome::Failed {
            error: e.to_string(),
        },
    }
}

/// Populate advisories for one ecosystem from the locally cached OSV ZIP
/// mirror, (re)downloading the ZIP when it is missing or when OSV has published
/// a newer advisory set (cheap ETag HEAD check) so routine syncs pick up
/// newly-disclosed CVEs.
///
/// The per-ecosystem ZIP carries the full advisory JSON and is matched
/// locally: it is the only path that fills `osv_advisories`, and nothing
/// about the user's dependency set leaves the machine.
async fn populate_from_zip_mirror(
    db: &Database,
    mirror: &Mirror,
    ecosystem: &str,
    packages: &[String],
) -> Result<ArchivePass> {
    let pkg_set: HashSet<String> = packages
        .iter()
        .map(|p| crate::osv::matching::package_key(p, ecosystem))
        .collect();

    // Ask the remote whether the archive on disk is still the current one, and
    // download only when it is not (the check is a cheap HTTP HEAD; npm's ZIP
    // is ~200MB) — that is what lets a routine sync pick up newly-published
    // CVEs instead of serving a frozen snapshot. The three answers are kept
    // apart: "could not ask" (offline, a proxy) is NOT "fresh". A failed check
    // or a failed download falls back to the ZIP already on disk, which a
    // download replaces only once it has completed and opens as an archive
    // (`cache::replace_archive`), and the pass is then unverified.
    let unverified = match mirror.freshness(ecosystem).await {
        Freshness::Current => None,
        Freshness::Outdated => match mirror.download(ecosystem).await {
            Ok(_) => None,
            Err(e) => {
                warn!(
                    target: "4da::osv",
                    ecosystem,
                    error = %e,
                    "ZIP refresh failed; falling back to the existing cached ZIP if present"
                );
                Some(e.to_string())
            }
        },
        Freshness::Unknown(reason) => {
            warn!(
                target: "4da::osv",
                ecosystem,
                error = %reason,
                "Could not ask whether the cached ZIP is current; falling back to it"
            );
            Some(reason)
        }
    };

    // An unverified pass is retried (every engine cycle; every 5 to 30
    // minutes in the app) for as long as the remote stays away. Reading the
    // npm archive is minutes of CPU, so a retry that has the same file and
    // the same packages as the read already in the mirror reads nothing.
    let fingerprint = archive_fingerprint(db, mirror, ecosystem, &pkg_set);
    if unverified.is_some()
        && fingerprint.is_some()
        && db.osv_archive_ingested(ecosystem) == fingerprint
    {
        return Ok(ArchivePass {
            stored: None,
            unverified,
        });
    }

    match mirror.sync_from_zip(db, ecosystem, &pkg_set) {
        Ok(count) => {
            remember_ingested(db, ecosystem, fingerprint.as_deref());
            Ok(ArchivePass {
                stored: Some(count),
                unverified,
            })
        }
        Err(_) => {
            // No usable ZIP on disk (first run offline, or a file that does
            // not open) — make a final attempt to download, then match
            // locally. A download that completes is a verified pass.
            mirror.download(ecosystem).await?;
            let count = mirror.sync_from_zip(db, ecosystem, &pkg_set)?;
            let fingerprint = archive_fingerprint(db, mirror, ecosystem, &pkg_set);
            remember_ingested(db, ecosystem, fingerprint.as_deref());
            Ok(ArchivePass {
                stored: Some(count),
                unverified: None,
            })
        }
    }
}

/// Identifies "this archive file, read for these packages, by this build and
/// schema": equal fingerprints mean reading the archive again would store the
/// rows the mirror already holds. The file is identified by its size and
/// modification time (a download always writes a new file), not by its
/// metadata sidecar, which can be missing or behind. `None` without a file.
fn archive_fingerprint(
    db: &Database,
    mirror: &Mirror,
    ecosystem: &str,
    packages: &HashSet<String>,
) -> Option<String> {
    let file = std::fs::metadata(mirror.zip_path(ecosystem)).ok()?;
    let modified = file
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let mut names: Vec<&str> = packages.iter().map(String::as_str).collect();
    names.sort_unstable();

    let mut hasher = Sha256::new();
    // A new build or a migrated schema may store a record differently (a new
    // column), so it reads the archive again.
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update([0u8]);
    hasher.update(db.get_schema_version().unwrap_or(0).to_le_bytes());
    hasher.update(file.len().to_le_bytes());
    hasher.update(modified.to_le_bytes());
    for name in names {
        hasher.update(name.as_bytes());
        hasher.update(b"\n");
    }
    Some(hex::encode(hasher.finalize()))
}

fn remember_ingested(db: &Database, ecosystem: &str, fingerprint: Option<&str>) {
    let Some(fingerprint) = fingerprint else {
        return;
    };
    if let Err(e) = db.set_osv_archive_ingested(ecosystem, fingerprint) {
        warn!(target: "4da::osv", ecosystem, error = %e, "Could not record the archive read into the mirror");
    }
}

/// Advisory rows were deleted: every archive is read again on its next pass.
pub(super) fn forget_ingested_archives(db: &Database) {
    if let Err(e) = db.clear_osv_archives_ingested() {
        warn!(target: "4da::osv", error = %e, "Could not clear the ingested-archive marks");
    }
}

/// Write one ecosystem's outcome to the sync status and the pass result.
/// Only a verified outcome counts as synced and moves the verified time.
pub(super) fn record_outcome(
    db: &Database,
    ecosystem: &str,
    outcome: EcosystemOutcome,
    result: &mut SyncResult,
) {
    let written = match outcome {
        EcosystemOutcome::Verified { stored } => {
            result.ecosystems_synced.push(ecosystem.to_string());
            result.advisories_stored += stored;
            db.update_osv_sync_status(ecosystem, i64::try_from(stored).unwrap_or(i64::MAX), None)
        }
        EcosystemOutcome::Unverified {
            stored,
            reason,
            data_as_of,
        } => {
            warn!(
                target: "4da::osv",
                ecosystem,
                reason = %reason,
                data_as_of = data_as_of.as_deref().unwrap_or("unknown"),
                "Advisories served from the archive on disk; they could not be verified as current"
            );
            result.advisories_stored += stored.unwrap_or(0);
            result.errors.push(format!("{ecosystem}: {reason}"));
            db.record_osv_sync_unverified(
                ecosystem,
                stored.map(|n| i64::try_from(n).unwrap_or(i64::MAX)),
                &reason,
                data_as_of.as_deref(),
            )
        }
        EcosystemOutcome::Failed { error } => {
            let msg = format!("{ecosystem}: {error}");
            warn!(target: "4da::osv", error = %msg, "Ecosystem sync failed");
            result.errors.push(msg);
            // The rows of the last verified sync, if any, are still in the
            // mirror: its time and count stay.
            db.record_osv_sync_unverified(ecosystem, None, &error, None)
        }
    };
    if let Err(e) = written {
        warn!(target: "4da::osv", ecosystem, error = %e, "Could not record the sync status");
    }
}

#[cfg(test)]
#[path = "sync_pass_tests.rs"]
mod tests;
