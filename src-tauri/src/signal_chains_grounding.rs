// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency grounding and confidence policy for signal chains.

use std::collections::HashSet;

use rusqlite::params;

use crate::db::table_exists;

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
    /// Grounded items that ARE a published advisory for the package: a linker
    /// row of kind `advisory`, or an osv/cve row whose `Affected:` line names
    /// it. A title that merely contains a CVE id is not one.
    pub(super) advisory_item_ids: HashSet<i64>,
}

/// The user's own vocabulary a chain topic may be grounded in: declared tech
/// stack, ACE-detected tech, declared interests, and the names of notable
/// non-dev dependencies. Lowercased, each also stored in a separator-free form
/// so `node.js` / `nodejs` and `vector-search` / `vector search` meet.
///
/// Fresh-profile E2E 2026-10-09: with an EMPTY profile Preemption showed
/// chains on `game`, `cloud`, `code`, `backend`, `aws` ("God of War on PSP", a
/// hypoglycemia article). `extract_topics` is a generic vocabulary; a topic
/// recurring across days says the internet talks about it, not that the user
/// does. Empty set (no profile) => no chains.
///
/// Only interests are split on `/ , & |` ("AI/LLM" is two interests). A
/// package name is one term: splitting `@tauri-apps/api` or `@google/genai`
/// minted the bare words `api` and `google` (measured on the live corpus).
/// Dependency names that are generic words (`http`, `api`, `core` — the
/// domain profile's AMBIGUOUS / UTILITY lists, or under four characters) do not
/// ground a topic on name alone; they still form a chain through structured
/// dependency proof (`verified_dep`).
pub(super) fn user_topic_terms(conn: &rusqlite::Connection) -> HashSet<String> {
    let mut terms = HashSet::new();
    for value in string_column(conn, "SELECT topic FROM explicit_interests") {
        for part in value.split(['/', ',', '&', '|']) {
            insert_term(&mut terms, part);
        }
    }
    for value in string_column(conn, "SELECT technology FROM tech_stack") {
        insert_term(&mut terms, &value);
    }
    if table_exists(conn, "detected_tech") {
        let sql = if table_has_column(conn, "detected_tech", "confidence") {
            "SELECT name FROM detected_tech WHERE confidence >= 0.5"
        } else {
            "SELECT name FROM detected_tech"
        };
        for value in string_column(conn, sql) {
            insert_term(&mut terms, &value);
        }
    }
    for table in ["user_dependencies", "project_dependencies"] {
        if !table_exists(conn, table) {
            continue;
        }
        let sql = if table_has_column(conn, table, "is_dev") {
            format!("SELECT package_name FROM {table} WHERE COALESCE(is_dev, 0) = 0")
        } else {
            format!("SELECT package_name FROM {table}")
        };
        for value in string_column(conn, &sql) {
            if is_distinctive_dependency_name(&value) {
                insert_term(&mut terms, &value);
            }
        }
    }
    terms
}

fn insert_term(terms: &mut HashSet<String>, raw: &str) {
    let lower = raw.trim().to_lowercase();
    if lower.is_empty() {
        return;
    }
    let compact = compact_term(&lower);
    // The canonical spelling too, so a declared `next` meets a chain grouped
    // as `nextjs` (`signal_chains_topics`).
    if let Some(canonical) = crate::scoring::aliases::canonical_spelling(&lower) {
        terms.insert(canonical.to_string());
    }
    terms.insert(lower);
    if !compact.is_empty() {
        terms.insert(compact);
    }
}

/// A dependency name specific enough to stand for the user's stack on its own:
/// not a short or generic word (`http`, `api`, `log`), not a utility crate.
fn is_distinctive_dependency_name(name: &str) -> bool {
    use crate::domain_profile_data::{AMBIGUOUS_DEPS, UTILITY_DEPS};
    let lower = name.trim().to_lowercase();
    lower.len() >= 4
        && !AMBIGUOUS_DEPS.contains(&lower.as_str())
        && !UTILITY_DEPS.contains(&lower.as_str())
}

/// Is this chain topic one of the user's own terms?
pub(super) fn topic_in_user_terms(terms: &HashSet<String>, topic: &str) -> bool {
    let lower = topic.trim().to_lowercase();
    terms.contains(&lower) || {
        let compact = compact_term(&lower);
        !compact.is_empty() && terms.contains(&compact)
    }
}

fn compact_term(lower: &str) -> String {
    lower.chars().filter(|c| c.is_alphanumeric()).collect()
}

fn string_column(conn: &rusqlite::Connection, sql: &str) -> Vec<String> {
    let Ok(mut stmt) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
        return Vec::new();
    };
    rows.filter_map(std::result::Result::ok).collect()
}

/// Ground a topic group on whichever of its spellings is an installed
/// package: a chain shown as `next.js` is grounded by the `next` package.
/// Returns the grounding spelling (the real package name) and its evidence;
/// the strongest spelling wins, ties to the first in sorted order.
pub(super) fn best_dependency_evidence(
    conn: &rusqlite::Connection,
    spellings: &[String],
    topic_items: &[TopicChainItem],
) -> (Option<String>, DependencyEvidence) {
    let mut best: (Option<String>, DependencyEvidence) = (None, DependencyEvidence::none());
    for spelling in spellings {
        let evidence = dependency_evidence(conn, spelling, topic_items);
        if evidence.score > best.1.score {
            best = (Some(spelling.to_lowercase()), evidence);
        }
    }
    best
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
    let linked = load_linked_item_ids(conn, &topic_lower, &["exact_registry", "advisory"]);
    let linked_advisories = load_linked_item_ids(conn, &topic_lower, &["advisory"]);

    let mut qualifying_hits: HashSet<(String, bool)> = HashSet::new();
    let mut grounded_item_ids = HashSet::new();
    let mut advisory_item_ids = HashSet::new();
    let mut grounded_dates = HashSet::new();
    let mut security_signal = false;
    let mut breaking_signal = false;

    for hit in hits.iter().filter(|hit| !hit.is_dev) {
        let mut hit_qualifies = false;
        for (id, title, source_type, timestamp, content) in topic_items {
            // Item-level STRUCTURED proof only (2026-09-06 live audit): a
            // linker row of registry/advisory kind, or the same proof read
            // off the item itself. A bare title word never grounds — `which`
            // grounded on "Which app should I use?", `openai` on a Reuters
            // legal story, `typescript` on a calendar library, `axum` and
            // `tauri` on their own plugins' release rows, every one a
            // medium-urgency "learning" chain on Preemption.
            if !linked.contains(id)
                && !item_is_about_package(source_type, title, content, &topic_lower)
            {
                continue;
            }

            hit_qualifies = true;
            grounded_item_ids.insert(*id);
            if linked_advisories.contains(id)
                || item_is_advisory_for_package(source_type, content, &topic_lower)
            {
                advisory_item_ids.insert(*id);
            }
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
        advisory_item_ids,
    }
}

/// Items the dependency linker has already bound to the package with
/// STRUCTURED proof of the given kinds — a registry row whose subject is the
/// package (`exact_registry`) or an advisory naming it in `Affected:`
/// (`advisory`). Title-heuristic links are deliberately never asked for: they
/// are the bare title words this policy exists to reject. Empty when the table
/// does not exist (older schemas, hermetic tests).
fn load_linked_item_ids(
    conn: &rusqlite::Connection,
    topic_lower: &str,
    match_types: &[&str],
) -> HashSet<i64> {
    if match_types.is_empty() || !table_exists(conn, "source_item_dependencies") {
        return HashSet::new();
    }
    let placeholders = (0..match_types.len())
        .map(|i| format!("?{}", i + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT source_item_id FROM source_item_dependencies
         WHERE LOWER(package_name) = ?1
           AND match_type IN ({placeholders})"
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return HashSet::new();
    };
    let mut bound: Vec<&dyn rusqlite::ToSql> = vec![&topic_lower];
    for match_type in match_types {
        bound.push(match_type);
    }
    let Ok(rows) = stmt.query_map(bound.as_slice(), |row| row.get::<_, i64>(0)) else {
        return HashSet::new();
    };
    rows.filter_map(std::result::Result::ok).collect()
}

/// An osv/cve registry row whose `Affected:` line names the package — a
/// published advisory FOR it, read off the item itself.
fn item_is_advisory_for_package(source_type: &str, content: &str, topic_lower: &str) -> bool {
    matches!(source_type.to_lowercase().as_str(), "osv" | "cve")
        && crate::dep_linker::advisory_affected_package_match(content, topic_lower)
}

/// The same proof computed from the item itself, for rows the linker has not
/// visited yet (it runs after each fetch and links known project deps only):
/// a registry row whose subject IS the package, or an osv/cve row whose
/// `Affected:` line names it. Editorial sources never qualify here.
fn item_is_about_package(source_type: &str, title: &str, content: &str, topic_lower: &str) -> bool {
    let st = source_type.to_lowercase();
    if crate::dep_linker::is_registry_source(&st) {
        return registry_subject_matches(title, topic_lower);
    }
    matches!(st.as_str(), "osv" | "cve")
        && crate::dep_linker::advisory_affected_package_match(content, topic_lower)
}

/// Registry titles are "<registry>: <package> v<version>" (crates.io, npm,
/// PyPI, Go). The subject is the first token after the registry prefix; a
/// release OF `axum-extra` is not a release OF `axum`.
fn registry_subject_matches(title: &str, topic_lower: &str) -> bool {
    let lower = title.to_lowercase();
    let body = lower
        .split_once(':')
        .map_or(lower.as_str(), |(_, rest)| rest.trim_start());
    let Some(subject) = body.split_whitespace().next() else {
        return false;
    };
    subject.replace('_', "-") == topic_lower.replace('_', "-")
}

impl DependencyEvidence {
    fn none() -> Self {
        Self {
            score: 0.0,
            security_signal: false,
            breaking_signal: false,
            grounded_item_ids: HashSet::new(),
            advisory_item_ids: HashSet::new(),
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

fn table_has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> bool {
    let sql = format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?1");
    conn.query_row(&sql, params![column], |row| row.get::<_, i64>(0))
        .map(|count| count > 0)
        .unwrap_or(false)
}
