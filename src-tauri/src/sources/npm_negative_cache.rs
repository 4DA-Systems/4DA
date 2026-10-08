// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Negative cache for public-npm-registry 404s.
//!
//! A name the public registry does not know is, in practice, a private
//! package (a monorepo member, an internal scope) that the scanner could not
//! prove local. Every lookup sends that name to `registry.npmjs.org`, so the
//! release watch used to re-send it every cycle, forever. Now a 404 is
//! remembered and the name is sent at most once per [`NEGATIVE_TTL_DAYS`]
//! (the window lets a package that later gets published be picked up).
//!
//! Persisted in `kv_store` (one row per name, value = RFC 3339 timestamp of
//! the 404) — no migration.

use chrono::{DateTime, Duration, Utc};

use crate::db::Database;

/// kv_store key prefix; the package name follows verbatim.
const KEY_PREFIX: &str = "sources.npm_registry.not_found.";

/// How long a 404 suppresses re-querying a name.
pub(crate) const NEGATIVE_TTL_DAYS: i64 = 90;

fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

/// True when `name` 404'd on the public registry within the TTL.
pub(crate) fn is_known_missing_at(db: &Database, name: &str, now: DateTime<Utc>) -> bool {
    db.get_kv(&key(name))
        .ok()
        .flatten()
        .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
        .is_some_and(|at| now - at.with_timezone(&Utc) < Duration::days(NEGATIVE_TTL_DAYS))
}

/// Remember that `name` 404'd on the public registry at `now`.
pub(crate) fn record_missing_at(db: &Database, name: &str, now: DateTime<Utc>) {
    if let Err(e) = db.set_kv(&key(name), &now.to_rfc3339()) {
        tracing::debug!(target: "4da::sources", error = %e, package = name, "Could not persist npm 404");
    }
}

/// Remember a public-registry 404 for `name` (no-op without a database).
pub(crate) fn record_missing(name: &str) {
    if let Ok(db) = crate::get_database() {
        record_missing_at(db, name, Utc::now());
    }
}

/// `names` minus every name that 404'd within the TTL (unchanged without a
/// database — a cache that cannot be read suppresses nothing).
pub(crate) fn without_known_missing(names: &[String]) -> Vec<String> {
    let Ok(db) = crate::get_database() else {
        return names.to_vec();
    };
    let now = Utc::now();
    names
        .iter()
        .filter(|n| !is_known_missing_at(db, n, now))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;

    #[test]
    fn a_404_suppresses_the_name_until_the_ttl_lapses() {
        let db = test_db();
        let t0 = Utc::now();
        assert!(!is_known_missing_at(&db, "@4da/agent-framework", t0));

        record_missing_at(&db, "@4da/agent-framework", t0);
        assert!(is_known_missing_at(&db, "@4da/agent-framework", t0));
        assert!(is_known_missing_at(
            &db,
            "@4da/agent-framework",
            t0 + Duration::days(NEGATIVE_TTL_DAYS - 1)
        ));
        assert!(!is_known_missing_at(
            &db,
            "@4da/agent-framework",
            t0 + Duration::days(NEGATIVE_TTL_DAYS + 1)
        ));
        // Other names are untouched.
        assert!(!is_known_missing_at(&db, "react", t0));
    }

    #[test]
    fn garbage_cache_value_suppresses_nothing() {
        let db = test_db();
        db.set_kv(&key("left-pad"), "not a timestamp").unwrap();
        assert!(!is_known_missing_at(&db, "left-pad", Utc::now()));
    }
}
