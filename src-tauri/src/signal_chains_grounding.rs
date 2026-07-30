// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency grounding and confidence policy for signal chains.

use std::collections::HashSet;

use rusqlite::params;

use super::{classify_chain_signal, TopicChainItem, UNGROUNDED_CONFIDENCE_CAP};

/// Pure urgency/confidence policy for a detected chain, separated from DB access so the
/// grounding rules are unit-testable without a live database.
///
/// `dep_match` is the installed-dependency relevance (0.0 when the chain's topic is not a
/// tracked dependency). `has_security` / `has_breaking` must already be dependency-grounded
/// before entering this policy. Raw title keywords are useful for link labeling and copy, but
/// they must not escalate urgency without item-level proof that the signal touches the user's
/// tracked dependency.
pub(super) struct ChainPolicy {
    pub(super) priority: &'static str,
    pub(super) confidence: f64,
}

pub(super) fn chain_policy(
    has_security: bool,
    has_breaking: bool,
    dep_match: f64,
    links_len: usize,
) -> ChainPolicy {
    let has_dep = dep_match > 0.0;

    let priority = if has_security && has_dep {
        "critical"
    } else if has_breaking && has_dep {
        "alert"
    } else if has_dep && links_len >= 3 {
        "advisory"
    } else {
        // Ungrounded (no installed dep), or a thin grounded signal: awareness only.
        "watch"
    };

    let corroboration = (links_len as f64 / 5.0).min(1.0);
    let severity = if has_security {
        1.0
    } else if has_breaking {
        0.7
    } else {
        0.3
    };
    // Weighted confidence: dep relevance matters most (50%), corroboration from
    // multiple sources adds credibility (30%), keyword-inferred severity is least
    // reliable (20%).
    let mut confidence = dep_match * 0.5 + corroboration * 0.3 + severity * 0.2;
    if !has_dep {
        confidence = confidence.min(UNGROUNDED_CONFIDENCE_CAP);
    }

    ChainPolicy {
        priority,
        confidence,
    }
}

pub(super) struct DependencyEvidence {
    pub(super) score: f64,
    pub(super) security_signal: bool,
    pub(super) breaking_signal: bool,
    pub(super) grounded_item_ids: HashSet<i64>,
}

pub(super) fn dependency_evidence(
    conn: &rusqlite::Connection,
    topic: &str,
    topic_items: &[TopicChainItem],
) -> DependencyEvidence {
    let topic_lower = topic.to_lowercase();
    let hits = load_dependency_hits(conn, &topic_lower);
    if hits.is_empty() {
        return DependencyEvidence::none();
    }

    let mut qualifying_hits: HashSet<(String, bool)> = HashSet::new();
    let mut grounded_item_ids = HashSet::new();
    let mut grounded_dates = HashSet::new();
    let mut security_signal = false;
    let mut breaking_signal = false;

    for hit in hits.iter().filter(|hit| !hit.is_dev) {
        let mut hit_qualifies = false;
        for (id, title, _, timestamp, content) in topic_items {
            let title_lower = title.to_lowercase();
            let content_lower = content.to_lowercase();
            let language = match hit.language.trim() {
                "" => None,
                lang => Some(lang),
            };
            if !crate::package_ambiguity::has_word_boundary_match(&title_lower, &topic_lower) {
                continue;
            }
            if !crate::package_ambiguity::dep_grounded_match_for_ecosystem(
                &title_lower,
                &content_lower,
                &topic_lower,
                language,
            ) {
                continue;
            }

            hit_qualifies = true;
            grounded_item_ids.insert(*id);
            grounded_dates.insert(timestamp.chars().take(10).collect::<String>());
            match classify_chain_signal(title).as_str() {
                "security_alert" => security_signal = true,
                "breaking_change" => breaking_signal = true,
                _ => {}
            }
        }
        if hit_qualifies {
            qualifying_hits.insert((hit.language.trim().to_lowercase(), hit.is_direct));
        }
    }

    if grounded_item_ids.len() < 2 || grounded_dates.len() < 2 {
        return DependencyEvidence::none();
    }

    let has_direct = qualifying_hits.iter().any(|(_, is_direct)| *is_direct);
    let base = if has_direct { 0.62 } else { 0.50 };
    let score =
        (base + ((qualifying_hits.len().saturating_sub(1)) as f64 * 0.08).min(0.20)).min(0.90);

    DependencyEvidence {
        score,
        security_signal,
        breaking_signal,
        grounded_item_ids,
    }
}

impl DependencyEvidence {
    fn none() -> Self {
        Self {
            score: 0.0,
            security_signal: false,
            breaking_signal: false,
            grounded_item_ids: HashSet::new(),
        }
    }
}

struct DependencyHit {
    language: String,
    is_dev: bool,
    is_direct: bool,
}

fn load_dependency_hits(conn: &rusqlite::Connection, topic_lower: &str) -> Vec<DependencyHit> {
    let mut hits = Vec::new();
    append_dependency_hits(
        conn,
        "user_dependencies",
        "ecosystem",
        topic_lower,
        &mut hits,
    );
    append_dependency_hits(
        conn,
        "project_dependencies",
        "language",
        topic_lower,
        &mut hits,
    );
    hits
}

fn append_dependency_hits(
    conn: &rusqlite::Connection,
    table: &'static str,
    language_column: &'static str,
    topic_lower: &str,
    hits: &mut Vec<DependencyHit>,
) {
    if !table_exists(conn, table) {
        return;
    }
    let language_expr = if table_has_column(conn, table, language_column) {
        format!("COALESCE({language_column}, '')")
    } else {
        "''".to_string()
    };
    let is_dev_expr = if table_has_column(conn, table, "is_dev") {
        "COALESCE(is_dev, 0)"
    } else {
        "0"
    };
    let is_direct_expr = if table_has_column(conn, table, "is_direct") {
        "COALESCE(is_direct, 1)"
    } else {
        "1"
    };
    let sql = format!(
        "SELECT {language_expr}, {is_dev_expr}, {is_direct_expr}
         FROM {table}
         WHERE LOWER(package_name) = ?1"
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return;
    };
    let Ok(rows) = stmt.query_map(params![topic_lower], |row| {
        Ok(DependencyHit {
            language: row.get(0)?,
            is_dev: row.get::<_, i64>(1).unwrap_or(0) != 0,
            is_direct: row.get::<_, i64>(2).unwrap_or(1) != 0,
        })
    }) else {
        return;
    };
    hits.extend(rows.filter_map(std::result::Result::ok));
}

fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1");
    conn.query_row(&sql, params![column], |row| row.get::<_, i64>(0))
        .map(|count| count > 0)
        .unwrap_or(false)
}
