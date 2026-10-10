// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The lockfile walk's account of itself: every dependency file it read,
//! failed to read, or does not support, and every directory holding one
//! that it skipped and why. Logged after every walk, persisted in
//! `kv_store` ([`REPORT_KV_KEY`]) and returned with the scan result, so a
//! lockfile 4DA did not read is never a silent gap in what it reports.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{LockFormat, LockfileOutcome};

/// `kv_store` key the latest walk's report is saved under.
pub(crate) const REPORT_KV_KEY: &str = "ace.lockfile_report.v1";

/// Entries kept per list in the persisted / returned report (counts are exact).
const LIST_CAP: usize = 200;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReadEntry {
    pub path: String,
    pub format: String,
    pub ecosystem: String,
    pub packages: usize,
    /// Workspace / link / git / path entries, which name no registry release.
    pub non_registry_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ProblemEntry {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct LockfileReport {
    pub read: Vec<ReadEntry>,
    pub failed: Vec<ProblemEntry>,
    pub unsupported: Vec<ProblemEntry>,
    /// Directories holding a dependency file that the walk did not read
    /// (scaffolding path, nested foreign checkout, user-excluded, depth).
    pub skipped_dirs: Vec<ProblemEntry>,
}

pub(crate) fn format_label(format: LockFormat) -> &'static str {
    match format {
        LockFormat::NpmLock => "package-lock.json",
        LockFormat::NpmShrinkwrap => "npm-shrinkwrap.json",
        LockFormat::Pnpm => "pnpm-lock.yaml",
        LockFormat::YarnClassic => "yarn.lock (v1)",
        LockFormat::YarnBerry => "yarn.lock (berry)",
        LockFormat::Bun => "bun.lock",
        LockFormat::Cargo => "Cargo.lock",
        LockFormat::Poetry => "poetry.lock",
        LockFormat::Uv => "uv.lock",
        LockFormat::Pdm => "pdm.lock",
        LockFormat::Pipfile => "Pipfile.lock",
        LockFormat::Requirements => "requirements",
        LockFormat::GoMod => "go.mod",
        LockFormat::Gemfile => "Gemfile.lock",
        LockFormat::Composer => "composer.lock",
    }
}

impl LockfileReport {
    pub(crate) fn record(&mut self, outcome: &LockfileOutcome) {
        match outcome {
            LockfileOutcome::Read(read) => self.read.push(ReadEntry {
                path: read.path.display().to_string(),
                format: format_label(read.format).to_string(),
                ecosystem: read.format.ecosystem().to_string(),
                packages: read.packages.len(),
                non_registry_entries: read.non_registry_entries,
            }),
            LockfileOutcome::Failed {
                path,
                format,
                reason,
            } => self.failed.push(ProblemEntry {
                path: path.display().to_string(),
                reason: match format {
                    Some(f) => format!("{}: {reason}", format_label(*f)),
                    None => reason.clone(),
                },
            }),
            LockfileOutcome::Unsupported { path, kind } => self.unsupported.push(ProblemEntry {
                path: path.display().to_string(),
                reason: (*kind).to_string(),
            }),
        }
    }

    pub(crate) fn skip_dir(&mut self, dir: &Path, reason: impl Into<String>) {
        self.skipped_dirs.push(ProblemEntry {
            path: dir.display().to_string(),
            reason: reason.into(),
        });
    }

    /// One summary line, plus a warning per file that was not read.
    pub(crate) fn log(&self) {
        let packages: usize = self.read.iter().map(|r| r.packages).sum();
        info!(
            target: "4da::ace",
            lockfiles_read = self.read.len(),
            packages,
            failed = self.failed.len(),
            unsupported = self.unsupported.len(),
            skipped_dirs = self.skipped_dirs.len(),
            "Lockfile walk report"
        );
        for p in &self.failed {
            warn!(target: "4da::ace", path = %p.path, reason = %p.reason, "Lockfile NOT read");
        }
        for p in &self.unsupported {
            warn!(target: "4da::ace", path = %p.path, kind = %p.reason, "Dependency file not supported: its installed versions are not checked");
        }
    }

    /// The report as returned to the UI and saved: exact counts, lists capped.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let cap = |len: usize| len.min(LIST_CAP);
        serde_json::json!({
            "lockfiles_read": self.read.len(),
            "packages_read": self.read.iter().map(|r| r.packages).sum::<usize>(),
            "failed_count": self.failed.len(),
            "unsupported_count": self.unsupported.len(),
            "skipped_dir_count": self.skipped_dirs.len(),
            "read": &self.read[..cap(self.read.len())],
            "failed": &self.failed[..cap(self.failed.len())],
            "unsupported": &self.unsupported[..cap(self.unsupported.len())],
            "skipped_dirs": &self.skipped_dirs[..cap(self.skipped_dirs.len())],
        })
    }
}
