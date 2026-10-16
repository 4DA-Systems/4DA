// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Verdict cache for adversarial deliberation (audit L2, 2026-10-12).
//!
//! Every Preemption refresh (one per engine cycle, about every ten minutes)
//! used to ask the cloud LLM again about the same handful of signal chains
//! and LLM-judged items: 334 calls and $2.49 on 2026-10-11 against a $1.50
//! daily cap, hit every day. A verdict is now remembered under a hash of
//! exactly what the model judged (the item's material fields, the prompt
//! text and version, the provider and the model) and reused until that
//! material changes or [`VERDICT_TTL_SECS`] passes.
//!
//! Persistence: one JSON map in `kv_store`, the existing generic key/value
//! table, so verdicts survive restarts and are shared with any other process
//! on the same database, without a schema migration (a schema bump forces
//! the app and the background engine to be rebuilt together before either
//! opens the database, and parallel branches would collide on the version
//! number). The map is small: one entry per distinct item judged in the last
//! week, capped at [`MAX_ENTRIES`]. Expired entries are pruned on every save.
//! Two processes saving at once can lose the other's newest entries; the
//! loser is re-judged once, never shown unjudged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use super::DeliberationVerdict;
use crate::db::Database;
use crate::evidence::EvidenceItem;

/// Bump when the deliberation prompt's meaning changes in a way the prompt
/// text alone would not show (e.g. how the verdict is parsed or applied).
/// The system prompt text is hashed too, so editing it re-judges on its own.
pub(crate) const PROMPT_VERSION: &str = "adversarial-v1";

/// How long a verdict stands before the item is judged again, even if
/// nothing about it changed: the user's stack and the ecosystem move.
pub(crate) const VERDICT_TTL_SECS: i64 = 7 * 24 * 60 * 60;

/// Upper bound on stored verdicts; the oldest go first.
const MAX_ENTRIES: usize = 512;

const KV_KEY: &str = "adversarial_verdicts_v1";

/// One remembered verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedVerdict {
    pub verdict: DeliberationVerdict,
    /// Unix seconds when the model rendered it.
    pub judged_at: i64,
}

impl CachedVerdict {
    fn is_fresh(&self, now: i64) -> bool {
        now.saturating_sub(self.judged_at) < VERDICT_TTL_SECS && self.judged_at <= now
    }
}

/// The content hash a verdict is filed under: everything the model saw
/// about the item, plus what judged it. Deliberately excludes the item id
/// (chain ids are minted fresh on every refresh) and the user's context
/// summary (its topic ordering shifts from cycle to cycle; the TTL re-judges
/// against a changed profile).
pub(crate) fn material_key(
    item: &EvidenceItem,
    system_prompt: &str,
    provider: &str,
    model: &str,
) -> String {
    let kind = serde_json::to_string(&item.kind).unwrap_or_default();
    let urgency = serde_json::to_string(&item.urgency).unwrap_or_default();
    let fields: [&str; 10] = [
        PROMPT_VERSION,
        system_prompt,
        provider,
        model,
        &kind,
        &urgency,
        &item.title,
        &item.explanation,
        &item.affected_deps.join("\u{1f}"),
        &item.affected_projects.join("\u{1f}"),
    ];
    let mut hasher = Sha256::new();
    for field in fields {
        hasher.update(field.as_bytes());
        // Unit separator: "ab"+"c" never hashes like "a"+"bc".
        hasher.update([0x1e]);
    }
    hex::encode(hasher.finalize())
}

/// The verdicts in memory for one batch; loaded before, saved after.
#[derive(Debug, Default)]
pub(crate) struct VerdictCache {
    entries: BTreeMap<String, CachedVerdict>,
    dirty: bool,
}

impl VerdictCache {
    /// Read the stored verdicts. An unreadable or corrupt store starts empty:
    /// the cost is re-judging, never surfacing anything unjudged.
    pub(crate) fn load(db: &Database) -> Self {
        let entries = match db.get_kv(KV_KEY) {
            Ok(Some(raw)) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                warn!(target: "4da::adversarial", error = %e, "verdict cache unreadable, starting empty");
                BTreeMap::new()
            }),
            Ok(None) => BTreeMap::new(),
            Err(e) => {
                warn!(target: "4da::adversarial", error = %e, "verdict cache read failed, starting empty");
                BTreeMap::new()
            }
        };
        Self {
            entries,
            dirty: false,
        }
    }

    /// A verdict for `key` that is still within its TTL.
    pub(crate) fn fresh(&self, key: &str, now: i64) -> Option<&CachedVerdict> {
        self.entries.get(key).filter(|c| c.is_fresh(now))
    }

    pub(crate) fn insert(&mut self, key: String, verdict: DeliberationVerdict, now: i64) {
        self.entries.insert(
            key,
            CachedVerdict {
                verdict,
                judged_at: now,
            },
        );
        self.dirty = true;
    }

    /// Drop expired entries, then the oldest beyond [`MAX_ENTRIES`].
    /// Returns how many were removed.
    pub(crate) fn prune(&mut self, now: i64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, c| c.is_fresh(now));
        if self.entries.len() > MAX_ENTRIES {
            let mut by_age: Vec<(i64, String)> = self
                .entries
                .iter()
                .map(|(k, c)| (c.judged_at, k.clone()))
                .collect();
            by_age.sort();
            let excess = self.entries.len() - MAX_ENTRIES;
            for (_, key) in by_age.into_iter().take(excess) {
                self.entries.remove(&key);
            }
        }
        let removed = before - self.entries.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    /// Prune and persist when anything changed. Best-effort: a failed write
    /// only costs a re-judge next time.
    pub(crate) fn save(&mut self, db: &Database, now: i64) {
        self.prune(now);
        if !self.dirty {
            return;
        }
        match serde_json::to_string(&self.entries) {
            Ok(json) => match db.set_kv(KV_KEY, &json) {
                Ok(()) => self.dirty = false,
                Err(e) => {
                    warn!(target: "4da::adversarial", error = %e, "verdict cache write failed");
                }
            },
            Err(e) => warn!(target: "4da::adversarial", error = %e, "verdict cache encode failed"),
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
#[path = "adversarial_cache_tests.rs"]
mod tests;
