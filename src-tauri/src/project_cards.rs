// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Project cards: the user context the feed-demoting judges (ingest judge,
//! pending-verdict drain) read in place of a flat tech list.
//!
//! Judges decide "is this useful for one of the user's projects", so they
//! need to know what each project IS. Measured on the 230-item Label Bench
//! (14 project-positive, AI-labelled gold, production v6 prompt):
//!
//! | judge context                                   | AUC   |
//! |-------------------------------------------------|-------|
//! | production (10 tech names + 5 topics), Haiku    | 0.721 |
//! | cards, nouns only, Haiku                        | 0.778 |
//! | cards + README purpose line, Haiku              | 0.904 |
//! | cards + fixed-vocabulary purpose terms, Haiku   | 0.877 |
//! | cards + fixed-vocabulary purpose terms, gemma4  | 0.877 |
//!
//! Privacy (decision B, 2026-09-25; NETWORK.md "nouns, never your prose"):
//! a judge on this machine (`llm_egress::provider_is_on_machine`) gets the
//! full card, README purpose line and hardware included. A cloud judge gets
//! nouns only: project name, languages, frameworks, dependencies.
//!
//! Only projects active in the last [`ACTIVE_WITHIN_DAYS`] get a card: the
//! live scan still lists projects untouched since 2025 (an old monorepo, an
//! abandoned MVP, a superseded checkout), and a card tells the judge "this
//! matters to the user".

use std::path::{Path, PathBuf};

use crate::db::Database;

const ACTIVE_WITHIN_DAYS: i64 = 90;
const MAX_DEPS_PER_CARD: usize = 30;
const PURPOSE_MAX_CHARS: usize = 400;

#[derive(Debug, Clone)]
pub(crate) struct ProjectRow {
    pub path: String,
    pub name: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub dependencies: Vec<String>,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// One card per active project root; nested packages fold into their root.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Card {
    pub root: PathBuf,
    pub name: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Group active rows into root cards. Pure, so the rules are testable.
pub(crate) fn build_cards(rows: &[ProjectRow], now: chrono::DateTime<chrono::Utc>) -> Vec<Card> {
    let cutoff = now - chrono::Duration::days(ACTIVE_WITHIN_DAYS);
    let mut active: Vec<&ProjectRow> = rows
        .iter()
        .filter(|r| r.last_activity.is_some_and(|t| t >= cutoff))
        .collect();
    active.sort_by(|a, b| a.path.len().cmp(&b.path.len()).then(a.path.cmp(&b.path)));

    let mut cards: Vec<Card> = Vec::new();
    for row in active {
        let path = PathBuf::from(&row.path);
        let card = match cards.iter_mut().find(|c| path.starts_with(&c.root)) {
            Some(card) => card,
            None => {
                cards.push(Card {
                    root: path.clone(),
                    name: row.name.clone(),
                    languages: Vec::new(),
                    frameworks: Vec::new(),
                    dependencies: Vec::new(),
                });
                cards.last_mut().expect("just pushed")
            }
        };
        merge_unique(&mut card.languages, &row.languages);
        merge_unique(&mut card.frameworks, &row.frameworks);
        for dep in &row.dependencies {
            let dep = clean_dep(dep);
            if !dep.is_empty() && !card.dependencies.iter().any(|d| d == dep) {
                card.dependencies.push(dep.to_string());
            }
        }
    }
    cards.sort_by(|a, b| a.root.cmp(&b.root));
    for card in &mut cards {
        card.languages.sort();
        card.frameworks.sort();
        card.dependencies.truncate(MAX_DEPS_PER_CARD);
    }
    cards
}

fn merge_unique(into: &mut Vec<String>, from: &[String]) {
    for v in from {
        if !into.contains(v) {
            into.push(v.clone());
        }
    }
}

/// `serde_json.workspace` / `tokio 1.40` -> `serde_json` / `tokio`.
fn clean_dep(dep: &str) -> &str {
    dep.split(' ')
        .next()
        .unwrap_or("")
        .split(".workspace")
        .next()
        .unwrap_or("")
}

/// The first prose paragraph of the project's README (headings, lists,
/// tables, badges and code fences skipped), or `None`.
pub(crate) fn readme_purpose(readme: &str) -> Option<String> {
    let mut in_fence = false;
    let mut paragraph: Vec<String> = Vec::new();
    for line in readme.lines().chain(std::iter::once("")) {
        let t = line.trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if t.is_empty() {
            let text = paragraph.join(" ");
            paragraph.clear();
            let text: String = strip_markup(&text);
            if text.chars().count() > 80 {
                return Some(text.chars().take(PURPOSE_MAX_CHARS).collect());
            }
            continue;
        }
        if t.starts_with(['#', '|', '>', '!', '[', '-', '*', '<']) {
            continue;
        }
        paragraph.push(t.to_string());
    }
    None
}

fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            '*' | '_' | '`' => {}
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render cards for a judge. `on_machine` adds the README purpose line and
/// the hardware line; a cloud judge receives nouns only.
pub(crate) fn render(
    cards: &[Card],
    on_machine: bool,
    read_readme: impl Fn(&Path) -> Option<String>,
) -> String {
    let mut out = String::from("The developer works on these projects:\n");
    for card in cards {
        out.push('\n');
        out.push_str(&format!("Project {}", card.name));
        if on_machine {
            if let Some(purpose) = read_readme(&card.root).as_deref().and_then(readme_purpose) {
                out.push_str(&format!(": {purpose}"));
            }
        }
        out.push_str(&format!(
            "\n  Languages: {}. Frameworks: {}.\n  Direct dependencies: {}\n",
            card.languages.join(", "),
            if card.frameworks.is_empty() {
                "none".to_string()
            } else {
                card.frameworks.join(", ")
            },
            card.dependencies.join(", ")
        ));
    }
    if on_machine {
        if let Some(line) = hardware_line() {
            out.push('\n');
            out.push_str(&line);
        }
    }
    out
}

fn hardware_line() -> Option<String> {
    let hw = crate::hardware_detect::detect_hardware();
    let gpu = hw.gpu.as_ref().map(|g| match g.vram_mb {
        Some(mb) => format!(", {} ({} GB VRAM)", g.name, mb / 1024),
        None => format!(", {}", g.name),
    });
    Some(format!(
        "Machine: {:.0} GB RAM{}.",
        hw.ram_total_gb,
        gpu.unwrap_or_default()
    ))
}

fn load_rows(db: &Database) -> Vec<ProjectRow> {
    let conn = db.read_conn();
    let Ok(mut stmt) = conn.prepare(
        "SELECT path, name, languages, frameworks, dependencies, last_activity
         FROM detected_projects WHERE scratch = 0",
    ) else {
        return Vec::new();
    };
    let parse_list = |s: Option<String>| -> Vec<String> {
        s.and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    };
    stmt.query_map([], |r| {
        Ok(ProjectRow {
            path: r.get(0)?,
            name: r.get(1)?,
            languages: parse_list(r.get(2)?),
            frameworks: parse_list(r.get(3)?),
            dependencies: parse_list(r.get(4)?),
            last_activity: r
                .get::<_, Option<String>>(5)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|t| t.with_timezone(&chrono::Utc)),
        })
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

/// The judge's user context: cards when any project is active, else the
/// flat tech summary (`adversarial::build_user_context_summary`).
pub(crate) fn judge_context(db: &Database, on_machine: bool) -> String {
    let cards = build_cards(&load_rows(db), chrono::Utc::now());
    if cards.is_empty() {
        return crate::adversarial::build_user_context_summary();
    }
    render(&cards, on_machine, |root| {
        ["README.md", "readme.md", "Readme.md"]
            .iter()
            .find_map(|n| std::fs::read_to_string(root.join(n)).ok())
    })
}

#[cfg(test)]
#[path = "project_cards_tests.rs"]
mod tests;
