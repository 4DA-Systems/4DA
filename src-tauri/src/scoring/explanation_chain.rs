// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Explanation evidence chain — the single source for "why did this surface?".
//!
//! Every scored item gets an ORDERED `Vec<ExplanationFactor>` derived from the
//! same values the pipeline actually scored with. Doctrine ("never show
//! intelligence the system can't stand behind"):
//!
//! 1. **Every factor names concrete evidence.** A factor that cannot name the
//!    package / advisory id / project / interest term / decision-window title
//!    is NOT emitted. No templates, no bare counts.
//! 2. **Ordering leads with the strongest evidence.** The chain is sorted by
//!    evidence trust tier first (machine-verifiable grounding — security /
//!    dependency / your-own-context — leads soft topical/interest overlap),
//!    then by axis magnitude within a tier. Raw axis scores are NOT comparable
//!    across axes (each contributes to the final score through a different
//!    downstream coefficient), so a bare cross-axis magnitude sort would rank a
//!    strong keyword hit above a corroborated dependency match — the opposite
//!    of the product thesis (grounding beats keywords). `weight_share` is each
//!    factor's share of the total axis magnitude (the bar shows relative
//!    signal strength; the order shows trust).
//! 3. **Dependency evidence is corroborated-only.** The caller passes only
//!    display-worthy matches (name-corroborated text matches, or matches the
//!    advisory's own affected-package metadata confirms) — the raw alias
//!    expansion family never reaches the UI.
//! 4. **Semantic-only items self-disclose.** If the chain carries no
//!    DependencyMatch / SecurityAdvisory / ContextMatch factor, an honesty
//!    tail is appended: "Topic similarity only — no dependency link".
//!
//! The card subtitle (`SourceRelevance.explanation`) is rendered FROM this
//! chain (`render_subtitle`), so collapsed and expanded views agree by
//! construction.

use super::ace_context::ACEContext;
use super::aliases;
use super::context::is_low_quality_topic;
use super::dependencies::DepMatch;
use super::utils::has_word_boundary_match;
use crate::{context_engine, scoring_config, ExplanationFactor, FactorKind, RelevanceMatch};

/// Everything the chain builder needs, gathered from values the pipeline
/// already computed. All evidence must come from here — the builder performs
/// no scoring of its own.
pub(crate) struct ChainInputs<'a> {
    pub title: &'a str,
    pub item_topics: &'a [String],
    pub ace_ctx: &'a ACEContext,
    pub interests: &'a [context_engine::Interest],
    pub declared_tech: &'a [String],
    pub matches: &'a [RelevanceMatch],
    /// Display-worthy dependency matches ONLY (corroborated / advisory-
    /// confirmed), pre-sorted by confidence descending.
    pub display_deps: &'a [DepMatch],
    pub dep_match_score: f32,
    pub context_score: f32,
    pub interest_score: f32,
    pub keyword_score: f32,
    pub ace_boost: f32,
    pub window_boost: f32,
    pub matched_window_label: Option<&'a str>,
    pub skill_gap_boost: f32,
    pub matched_skill_gaps: &'a [String],
    /// True when the item is a security advisory (necessity security path).
    pub is_security: bool,
    pub necessity_score: f32,
    pub advisory_id: Option<&'a str>,
    pub cvss_score: Option<f32>,
    pub cvss_severity: Option<&'a str>,
    pub fixed_version: Option<&'a str>,
    pub installed_version: Option<&'a str>,
    /// The grounding verdict came from the registry-subject route: the item is
    /// a release OF the user's dependency (subject match), not a text mention.
    pub via_registry_subject: bool,
}

/// Word-boundary-aware topic match (same rule the score path applies): the
/// item topic equals the candidate, or contains it as a whole delimited
/// segment. Prevents infix artifacts ("macos" must not match "os").
fn topic_word_match(item_topic: &str, candidate: &str) -> bool {
    item_topic == candidate
        || item_topic
            .split(|c: char| matches!(c, '-' | '.' | '/' | '_' | ' '))
            .any(|seg| seg == candidate)
}

fn dedup_preserve_order(hits: &mut Vec<&str>) {
    let mut seen: Vec<&str> = Vec::with_capacity(hits.len());
    hits.retain(|h| {
        if seen.contains(h) {
            false
        } else {
            seen.push(h);
            true
        }
    });
}

/// Evidence trust tier — lower ranks lead the chain. Ordered by how
/// machine-verifiable / grounding-strong the evidence is: a CVE on your
/// dependency is the highest-stakes, most-verifiable claim; a bare topical
/// overlap is the weakest. This is the primary sort key so hard grounding can
/// never be visually outranked by soft interest/topic overlap regardless of
/// raw (non-comparable) axis magnitudes.
fn trust_rank(kind: FactorKind) -> u8 {
    match kind {
        FactorKind::SecurityAdvisory => 0,
        FactorKind::DependencyMatch => 1,
        FactorKind::ContextMatch => 2,
        FactorKind::DecisionWindow => 3,
        FactorKind::SkillGap => 4,
        FactorKind::InterestMatch => 5,
        FactorKind::TopicMatch => 7,
        FactorKind::CommunitySignal => 8,
    }
}

/// Shorten a source path to its last two segments (`db/vector.rs`), enough to
/// identify the file without a wall of absolute path. The full path always
/// travels in the factor's `evidence`, so nothing is lost.
fn short_source_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let segments: Vec<&str> = normalized
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    match segments.as_slice() {
        [] => normalized.clone(),
        [only] => (*only).to_string(),
        [.., parent, file] => format!("{parent}/{file}"),
    }
}

fn provenance(d: &DepMatch) -> &'static str {
    if d.is_dev {
        "dev-only"
    } else if d.is_direct {
        "direct"
    } else {
        "transitive"
    }
}

/// Weighted (unnormalized) factor collected during the build phase.
struct WeightedFactor {
    kind: FactorKind,
    display: String,
    evidence: String,
    weight: f32,
}

/// Build the ranked evidence chain. See module docs for the invariants.
pub(crate) fn build_explanation_chain(inp: &ChainInputs<'_>) -> Vec<ExplanationFactor> {
    let mut factors: Vec<WeightedFactor> = Vec::new();
    // Topics already cited by a stronger tier — weaker tiers must not repeat them.
    let mut used_topics: Vec<String> = Vec::new();

    // ── 1. Security advisory (highest-trust evidence when nameable) ──────
    if inp.is_security {
        let mut evidence_parts: Vec<String> = Vec::new();
        if let Some(id) = inp.advisory_id.map(str::trim).filter(|s| !s.is_empty()) {
            evidence_parts.push(id.to_string());
        }
        if let Some(cvss) = inp.cvss_score {
            evidence_parts.push(format!("CVSS {cvss:.1}"));
        } else if let Some(sev) = inp.cvss_severity.map(str::trim).filter(|s| !s.is_empty()) {
            evidence_parts.push(format!("severity {sev}"));
        }
        if let Some(iv) = inp
            .installed_version
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            evidence_parts.push(format!("installed v{iv}"));
        }
        if let Some(fv) = inp.fixed_version.map(str::trim).filter(|s| !s.is_empty()) {
            evidence_parts.push(format!("fixed in v{fv}"));
        }

        let named_dep = inp.display_deps.first().map(|d| d.package_name.as_str());
        // Emit ONLY when the factor can name something concrete: an advisory
        // id / severity, or the affected dependency. A generic "security in
        // your ecosystem" line is exactly the un-evidenced template this
        // module exists to kill.
        if !evidence_parts.is_empty() || named_dep.is_some() {
            let display = match named_dep {
                Some(dep) => format!("Security advisory affects your dependency {dep}"),
                None => match inp.advisory_id.map(str::trim).filter(|s| !s.is_empty()) {
                    Some(id) => format!("Security advisory {id}"),
                    None => String::new(),
                },
            };
            if !display.is_empty() {
                if evidence_parts.is_empty() {
                    // named_dep is Some here (display would be empty otherwise)
                    if let (Some(dep), Some(dm)) = (named_dep, inp.display_deps.first()) {
                        evidence_parts.push(format!("{dep} ({})", provenance(dm)));
                    }
                }
                factors.push(WeightedFactor {
                    kind: FactorKind::SecurityAdvisory,
                    display,
                    evidence: evidence_parts.join(" \u{b7} "),
                    weight: inp.necessity_score.max(inp.dep_match_score),
                });
            }
        }
    }

    // ── 2. Dependency match (corroborated evidence only) ─────────────────
    if !inp.display_deps.is_empty() && inp.dep_match_score > 0.0 {
        let named: Vec<&DepMatch> = inp.display_deps.iter().take(3).collect();
        let names: Vec<&str> = named.iter().map(|d| d.package_name.as_str()).collect();
        let noun = if names.len() == 1 {
            "dependency"
        } else {
            "dependencies"
        };
        let evidence = named
            .iter()
            .map(|d| {
                let mut s = format!("{} ({}", d.package_name, provenance(d));
                if let Some(v) = d
                    .version
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    s.push_str(&format!(", installed v{v}"));
                }
                s.push(')');
                s
            })
            .collect::<Vec<_>>()
            .join("; ");
        for n in &names {
            used_topics.push(n.to_lowercase());
        }
        // Registry-subject releases get the honest, stronger claim: the item
        // IS a release of the user's dependency, not merely text naming it.
        let (display, evidence_tail) = if inp.via_registry_subject {
            (
                format!("Release of your {noun} {}", names.join(", ")),
                "the subject of this release",
            )
        } else {
            (
                format!("Names your {noun} {}", names.join(", ")),
                "named in the item text",
            )
        };
        factors.push(WeightedFactor {
            kind: FactorKind::DependencyMatch,
            display,
            evidence: format!("{evidence} \u{2014} {evidence_tail}"),
            weight: inp.dep_match_score,
        });
    }

    // ── 3. Declared stack (the user's explicit tech choices) ─────────────
    let mut declared_hits: Vec<&str> = inp
        .item_topics
        .iter()
        .filter_map(|t| {
            inp.declared_tech
                .iter()
                // Word-boundary, never infix — same rule as the interest/topic
                // tiers. `.contains()` would false-match "django"→"go",
                // minting a phantom hard-evidence ContextMatch that suppresses
                // the honesty tail. "react-native"→"react" still matches
                // (segment split), which is the case we want to keep.
                .find(|tech| topic_word_match(t, &tech.to_lowercase()))
                .map(String::as_str)
        })
        .filter(|t| !used_topics.iter().any(|u| u == &t.to_lowercase()))
        .collect();
    dedup_preserve_order(&mut declared_hits);
    if !declared_hits.is_empty() {
        let names: Vec<String> = declared_hits
            .iter()
            .copied()
            .take(3)
            .map(|n| {
                let nl = n.to_lowercase();
                if let Some(info) = inp.ace_ctx.dependency_info.get(&nl) {
                    if let Some(ref v) = info.version {
                        return format!("{n} v{v}");
                    }
                }
                n.to_string()
            })
            .collect();
        for hit in &declared_hits {
            used_topics.push(hit.to_lowercase());
        }
        factors.push(WeightedFactor {
            kind: FactorKind::ContextMatch,
            display: format!("Uses {} (your stack)", names.join(", ")),
            evidence: format!("your declared stack: {}", names.join(", ")),
            // Declared-tech topical hits land on the keyword axis.
            weight: inp.keyword_score.max(0.01),
        });
    }

    // ── 4. Detected tech, named to a scanned project when possible ───────
    let mut detected_hits: Vec<&str> = inp
        .item_topics
        .iter()
        .filter_map(|t| {
            inp.ace_ctx
                .detected_tech
                .iter()
                // Word-boundary, never infix (detected_tech is already lowercase).
                .find(|tech| topic_word_match(t, tech))
                .map(String::as_str)
        })
        .filter(|t| !used_topics.iter().any(|u| u == &t.to_lowercase()))
        .collect();
    dedup_preserve_order(&mut detected_hits);
    if !detected_hits.is_empty() {
        let names: Vec<&str> = detected_hits.iter().copied().take(2).collect();
        let project = names.iter().find_map(|&tech| {
            inp.ace_ctx.tech_projects.get(tech).and_then(|projects| {
                if projects.len() == 1 {
                    Some(projects[0].clone())
                } else {
                    None
                }
            })
        });
        for &n in &names {
            used_topics.push(n.to_lowercase());
        }
        let (display, evidence) = match project {
            Some(p) => (
                format!("Related to {} (in {p})", names.join(", ")),
                format!("detected in your project {p}: {}", names.join(", ")),
            ),
            None => (
                format!("Related to {} (active project)", names.join(", ")),
                format!("detected in your scanned projects: {}", names.join(", ")),
            ),
        };
        factors.push(WeightedFactor {
            kind: FactorKind::ContextMatch,
            display,
            evidence,
            weight: inp.ace_boost.max(0.01),
        });
    }

    // ── 5. Project-context similarity (KNN against the user's own files) ─
    //
    // TRUTHFULNESS GATE (2026-08-25 live Signal audit). Three defects together
    // made this factor read as fabricated evidence on real security advisories:
    //
    //  1. The claim was gated on the AGGREGATE `context_score >= 0.3` while the
    //     evidence quoted the individual match's own similarity — so a weak
    //     match displayed whenever the aggregate cleared the bar. Worse, 0.3 is
    //     BELOW `CONTEXT_THRESHOLD`, the bar this same pipeline uses to call a
    //     context axis confirmed: the chain asserted a relationship the scorer
    //     itself would not have counted.
    //
    //  2. `extract_short_phrase` returns the chunk's opening sentence, and a
    //     Rust chunk opens with its doc comment — so the quoted "code" was
    //     systematically a natural-language COMMENT. Live output paired the
    //     axios `maxBodyLength` bypass with `/// Maximum content length per
    //     feed item (100KB)`. `context_admission` is right that the FILE is
    //     code; the quoted LINE is the one part of it that is not, and prose
    //     embeddings match arbitrary text by construction — the same mechanism
    //     that produced `Similar to your code: "Nunca perca entregas"`.
    //
    //  3. It fired alongside hard evidence. An advisory already grounded in a
    //     real dependency gains nothing from a prose resemblance, and a
    //     visibly weak one discredits the hard factor sitting beside it.
    //
    // So: require the MATCH itself to clear the confirmation bar, name the file
    // (identity-bearing and openable by the reader) instead of quoting a
    // comment, and stay silent when harder evidence already carries the item.
    let already_grounded = factors.iter().any(|f| {
        matches!(
            f.kind,
            FactorKind::SecurityAdvisory | FactorKind::DependencyMatch
        )
    });
    if !already_grounded && inp.context_score >= scoring_config::CONTEXT_THRESHOLD {
        if let Some(m) = inp.matches.iter().find(|m| {
            m.similarity >= scoring_config::CONTEXT_THRESHOLD && !m.source_file.trim().is_empty()
        }) {
            factors.push(WeightedFactor {
                kind: FactorKind::ContextMatch,
                display: format!(
                    "Similar to your code in {}",
                    short_source_path(&m.source_file)
                ),
                evidence: format!(
                    "{} ({}% similar)",
                    m.source_file,
                    (m.similarity * 100.0).round() as i32
                ),
                weight: inp.context_score,
            });
        }
    }

    // ── 6. Interest match (word-boundary or curated alias, never infix) ──
    if inp.interest_score > 0.15 {
        let mut interest_hits: Vec<&str> = inp
            .item_topics
            .iter()
            .filter_map(|t| {
                inp.interests
                    .iter()
                    .find(|i| {
                        let il = i.topic.to_lowercase();
                        topic_word_match(t, &il) || aliases::are_aliases(t, &il)
                    })
                    .map(|i| i.topic.as_str())
            })
            .filter(|t| !used_topics.iter().any(|u| u == &t.to_lowercase()))
            .collect();
        dedup_preserve_order(&mut interest_hits);
        if !interest_hits.is_empty() {
            let names: Vec<&str> = interest_hits.iter().copied().take(2).collect();
            let title_lower = inp.title.to_lowercase();
            let evidence = names
                .iter()
                .map(|n| {
                    let loc = if has_word_boundary_match(&title_lower, &n.to_lowercase()) {
                        "in the title"
                    } else {
                        "in the item topics"
                    };
                    format!("'{n}' {loc}")
                })
                .collect::<Vec<_>>()
                .join("; ");
            for &n in &names {
                used_topics.push(n.to_lowercase());
            }
            factors.push(WeightedFactor {
                kind: FactorKind::InterestMatch,
                display: format!("Matches interest: {}", names.join(", ")),
                evidence,
                weight: inp.interest_score,
            });
        }
    }

    // ── 7. Active project topics (quality-gated, word-boundary) ──────────
    let mut topic_hits: Vec<&str> = inp
        .item_topics
        .iter()
        .filter_map(|t| {
            inp.ace_ctx
                .active_topics
                .iter()
                .find(|at| !is_low_quality_topic(at) && topic_word_match(t, at))
                .map(String::as_str)
        })
        .filter(|t| !used_topics.iter().any(|u| u == &t.to_lowercase()))
        .collect();
    dedup_preserve_order(&mut topic_hits);
    if !topic_hits.is_empty() {
        let names: Vec<&str> = topic_hits.iter().copied().take(2).collect();
        for &n in &names {
            used_topics.push(n.to_lowercase());
        }
        factors.push(WeightedFactor {
            kind: FactorKind::TopicMatch,
            display: format!("Overlaps your recent work: {}", names.join(", ")),
            evidence: format!("active topics from your commits: {}", names.join(", ")),
            weight: (inp.ace_boost * 0.9).max(0.005),
        });
    }

    // ── 8. Decision window (post-#214: must name the window) ─────────────
    if inp.window_boost > 0.0 {
        if let Some(label) = inp
            .matched_window_label
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            factors.push(WeightedFactor {
                kind: FactorKind::DecisionWindow,
                display: format!("Relevant to open decision: {label}"),
                evidence: format!("open decision window \"{label}\""),
                weight: inp.window_boost,
            });
        }
    }

    // ── 9. Skill gap (must name the gap) ──────────────────────────────────
    if inp.skill_gap_boost > 0.0 && !inp.matched_skill_gaps.is_empty() {
        let gaps: Vec<&str> = inp
            .matched_skill_gaps
            .iter()
            .map(String::as_str)
            .filter(|g| !used_topics.iter().any(|u| u == &g.to_lowercase()))
            .take(3)
            .collect();
        if !gaps.is_empty() {
            factors.push(WeightedFactor {
                kind: FactorKind::SkillGap,
                display: format!("Closes skill gap: {}", gaps.join(", ")),
                evidence: format!(
                    "you use {} but haven't engaged with recent updates",
                    gaps.join(", ")
                ),
                weight: inp.skill_gap_boost,
            });
        }
    }

    // ── Rank + normalize ──────────────────────────────────────────────────
    // Trust tier is the PRIMARY key (hard grounding leads); axis magnitude
    // breaks ties within a tier. Raw axis scores are not comparable across
    // axes, so magnitude alone would let a strong keyword hit outrank a
    // corroborated dependency — see invariant #2.
    factors.sort_by(|a, b| {
        trust_rank(a.kind).cmp(&trust_rank(b.kind)).then_with(|| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    let total: f32 = factors.iter().map(|f| f.weight.max(0.0)).sum();
    let mut chain: Vec<ExplanationFactor> = factors
        .into_iter()
        .map(|f| ExplanationFactor {
            kind: f.kind,
            display: f.display,
            evidence: f.evidence,
            weight_share: if total > 0.0 {
                (f.weight.max(0.0) / total).clamp(0.0, 1.0)
            } else {
                0.0
            },
        })
        .collect();

    // ── Honesty tail: semantic-only items self-disclose ──────────────────
    let has_hard_evidence = chain.iter().any(|f| {
        matches!(
            f.kind,
            FactorKind::DependencyMatch | FactorKind::SecurityAdvisory | FactorKind::ContextMatch
        )
    });
    if !has_hard_evidence {
        chain.push(ExplanationFactor {
            kind: FactorKind::TopicMatch,
            display: "Topic similarity only \u{2014} no dependency link".to_string(),
            evidence: "no dependency, advisory, or project-context evidence".to_string(),
            weight_share: 0.0,
        });
    }

    chain
}

/// Render the card subtitle from the chain: the top factor's display, plus the
/// second factor's when present. The honesty tail is included only when it is
/// the entire chain (a semantic-only item with no other factors).
pub(crate) fn render_subtitle(chain: &[ExplanationFactor]) -> Option<String> {
    if chain.is_empty() {
        return None;
    }
    let is_tail = |f: &ExplanationFactor| f.display.starts_with("Topic similarity only");
    let real: Vec<&ExplanationFactor> = chain.iter().filter(|f| !is_tail(f)).collect();
    if real.is_empty() {
        return chain.first().map(|f| f.display.clone());
    }
    let parts: Vec<&str> = real.iter().take(2).map(|f| f.display.as_str()).collect();
    Some(parts.join(" \u{b7} "))
}

#[cfg(test)]
#[path = "explanation_chain_tests.rs"]
mod tests;
