// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A knowledge gap as a canonical `EvidenceItem`: title, explanation,
//! citations and a confidence that comes from the gap's EVIDENCE.
//!
//! Before 2026-10-07 every gap carried `Confidence::heuristic(0.7)` whatever
//! backed it, and read "Unread version update · never reviewed" — framing
//! about the user's reading habits, attributed to every project that merely
//! declared the package name. A gap now says which projects are behind what.

// UTF-8 safety gate, as in the parent module.
#![deny(clippy::string_slice)]

use serde::{Deserialize, Serialize};

use crate::evidence::{Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints};

use super::{
    build_gap_actions, classify_missed_item, gap_severity_to_urgency, truncate_gap_note,
    truncate_gap_title, KnowledgeGap, MissedItem,
};

/// What makes a gap true — and therefore how much it may be trusted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapBasis {
    /// A stored advisory's affected range still covers a named project's
    /// installed version (deterministic OSV range match).
    Advisory,
    /// A release from the last 30 days that named, live, directly-declaring
    /// projects are behind (lockfile version comparison).
    Release,
    /// Only editorial coverage names the dependency — no version evidence.
    #[default]
    Editorial,
}

impl GapBasis {
    /// Confidence from the evidence tier, never a constant.
    pub fn confidence(self) -> Confidence {
        match self {
            GapBasis::Advisory => Confidence::osv_verified(0.95),
            GapBasis::Release => Confidence::checklist(0.9),
            GapBasis::Editorial => Confidence::heuristic(0.5),
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The gap's subject projects — the attributed set, else the display label.
fn subject_projects(gap: &KnowledgeGap) -> Vec<String> {
    if gap.projects.is_empty() {
        vec![gap.project_path.clone()]
    } else {
        gap.projects.clone()
    }
}

/// "3 projects behind chrono 0.4.45", "jsonwebtoken: advisory affects 1
/// project", or "Knowledge gap: tokio" for editorial-only coverage.
pub(super) fn gap_title(gap: &KnowledgeGap) -> String {
    let n = subject_projects(gap).len();
    let raw = match (gap.basis, gap.latest_release.as_deref()) {
        (GapBasis::Release, Some(latest)) => format!(
            "{} behind {} {latest}",
            plural(n, "project", "projects"),
            gap.dependency
        ),
        (GapBasis::Advisory, _) => format!(
            "{}: advisory affects {}",
            gap.dependency,
            plural(n, "project", "projects")
        ),
        _ => format!("Knowledge gap: {}", gap.dependency),
    };
    truncate_gap_title(&raw)
}

/// Counts by consequence: "1 security advisory, 2 version updates".
fn category_summary(dep: &str, missed: &[MissedItem]) -> String {
    let (mut security, mut breaking, mut updates, mut other) = (0usize, 0usize, 0usize, 0usize);
    for m in missed {
        match classify_missed_item(&m.title, &m.source_type, dep) {
            "security advisory" => security += 1,
            "breaking change" => breaking += 1,
            "version update" => updates += 1,
            _ => other += 1,
        }
    }
    let mut parts: Vec<String> = Vec::with_capacity(3);
    if security > 0 {
        parts.push(plural(security, "security advisory", "security advisories"));
    }
    if breaking > 0 {
        parts.push(plural(breaking, "breaking change", "breaking changes"));
    }
    if updates > 0 {
        parts.push(plural(updates, "version update", "version updates"));
    }
    if other > 0 && parts.is_empty() {
        parts.push(plural(other, "related signal", "related signals"));
    }
    parts.join(", ")
}

/// The most consequential citation: security / breaking first, then a
/// version update, then whatever came first.
fn highlight<'m>(dep: &str, missed: &'m [MissedItem]) -> Option<&'m MissedItem> {
    let class = |m: &MissedItem| classify_missed_item(&m.title, &m.source_type, dep);
    missed
        .iter()
        .find(|m| matches!(class(m), "security advisory" | "breaking change"))
        .or_else(|| missed.iter().find(|m| class(m) == "version update"))
        .or_else(|| missed.first())
}

/// One line, leading with WHO is affected by WHAT.
pub(super) fn build_gap_explanation(gap: &KnowledgeGap) -> String {
    let dep = &gap.dependency;
    let categories = category_summary(dep, &gap.missed_items);
    let projects = subject_projects(gap);
    let location = crate::scoring::release_grade::project_label(&projects)
        .unwrap_or_else(|| gap.project_path.clone());
    let on = gap
        .version
        .as_deref()
        .map(|v| format!(" on {v}"))
        .unwrap_or_default();
    let mut explanation = match (gap.basis, gap.latest_release.as_deref()) {
        (GapBasis::Release, Some(latest)) => format!(
            "{} behind {dep} {latest}: {location}{on} \u{b7} {categories}",
            plural(projects.len(), "project", "projects")
        ),
        (GapBasis::Advisory, _) => format!("{dep}{on}: {categories} affecting {location}"),
        _ => {
            let ver = gap
                .version
                .as_deref()
                .map(|v| format!(" v{v}"))
                .unwrap_or_default();
            format!("{dep}{ver}: {categories}")
        }
    };
    if let Some(item) = highlight(dep, &gap.missed_items) {
        let short_title = crate::utils::truncate_display(&item.title, 80);
        explanation.push_str(&format!(" \u{2014} notably \"{short_title}\""));
    }
    explanation
}

fn missed_item_to_citation(m: &MissedItem, dep_name: &str) -> EvidenceCitation {
    let freshness_days = chrono::NaiveDateTime::parse_from_str(&m.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            let secs = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
            (secs as f32 / 86_400.0).max(0.0)
        })
        .unwrap_or(0.0);
    let category = classify_missed_item(&m.title, &m.source_type, dep_name);
    let mut note = category.to_string();
    if let Some(first) = note.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    EvidenceCitation {
        source: m.source_type.clone(),
        title: truncate_gap_title(&m.title),
        url: m.url.clone(),
        freshness_days,
        relevance_note: truncate_gap_note(&note),
    }
}

impl KnowledgeGap {
    /// Convert a `KnowledgeGap` into the canonical `EvidenceItem`. Used by
    /// `get_knowledge_gaps` (command boundary) and any lens that wants
    /// gap-shaped evidence.
    pub fn to_evidence_item(&self) -> EvidenceItem {
        let evidence: Vec<EvidenceCitation> = self
            .missed_items
            .iter()
            .take(5)
            .map(|m| missed_item_to_citation(m, &self.dependency))
            .collect();
        EvidenceItem {
            id: format!("kg_{}", self.dependency),
            kind: EvidenceKind::Gap,
            title: gap_title(self),
            explanation: build_gap_explanation(self),
            confidence: self.basis.confidence(),
            urgency: gap_severity_to_urgency(&self.gap_severity),
            reversibility: None,
            evidence,
            evidence_total: None,
            affected_projects: subject_projects(self),
            affected_deps: vec![self.dependency.clone()],
            suggested_actions: build_gap_actions(&self.missed_items, &self.dependency),
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints {
                briefing: false,
                preemption: false,
                blind_spots: true,
                evidence: true,
                // Knowledge gaps are not platform-target-scoped (Phase 2c).
                other_build_target: false,
                // Not an upgrade-plan step (Phase 1 dep plan).
                upgrade_plan: false,
                // A gap is about available coverage, never zero coverage.
                no_coverage: false,
                // Host reachability / dormancy hints belong to advisories
                // materialized elsewhere, not to gap rows.
                lockfile_only: false,
                dormant_notice: false,
            },
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
        }
    }
}
