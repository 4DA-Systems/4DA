//! Team Intelligence aggregation helpers — pure functions, no DB access.
//!
//! Split from `team_intelligence.rs` to stay under 600-line file limit.
//! These functions operate on in-memory `MemberDna` slices and the
//! `TeamTechEntry`/`TeamBlindSpot`/`OverlapZone`/`UniqueStrength` structs.

use std::collections::{HashMap, HashSet};

use crate::team_intelligence::{OverlapZone, TeamBlindSpot, TeamTechEntry, UniqueStrength};

/// Extracted DNA fields from a single ShareDnaSummary entry.
pub(crate) struct MemberDna {
    pub client_id: String,
    pub display_name: String,
    pub primary_stack: Vec<String>,
    pub interests: Vec<String>,
    #[allow(dead_code)]
    // Reason: populated from team DNA but not yet consumed by aggregation logic
    pub blind_spots: Vec<String>,
}

// ============================================================================
// Adjacent tech map (subset for team-level blind-spot detection)
// ============================================================================

/// Returns a mapping of primary tech to adjacent topics the team should track.
fn team_adjacency_map() -> HashMap<&'static str, &'static [&'static str]> {
    HashMap::from([
        (
            "rust",
            &["cargo", "wasm", "tokio", "serde", "async", "unsafe", "ffi"][..],
        ),
        ("tauri", &["webview", "ipc", "wry", "tao", "desktop"]),
        ("react", &["jsx", "hooks", "vite", "webpack", "nextjs"]),
        ("typescript", &["javascript", "nodejs", "deno", "bun"]),
        (
            "python",
            &["pip", "pytorch", "tensorflow", "django", "flask", "fastapi"],
        ),
        ("go", &["golang", "goroutine", "grpc"]),
        ("java", &["spring", "gradle", "maven", "jvm"]),
        ("kotlin", &["android", "jetpack", "coroutines"]),
        ("swift", &["ios", "swiftui", "xcode", "cocoapods"]),
        ("docker", &["kubernetes", "containers", "k8s", "helm"]),
        ("kubernetes", &["k8s", "helm", "istio", "containers"]),
        (
            "aws",
            &["lambda", "s3", "ec2", "cloudformation", "dynamodb"],
        ),
        ("gcp", &["cloud run", "bigquery", "firebase"]),
        ("azure", &["cosmos", "functions", "devops"]),
        ("vue", &["vuejs", "nuxt", "pinia"]),
        ("svelte", &["sveltekit", "vite"]),
        ("ruby", &["rails", "bundler", "rspec"]),
        ("php", &["laravel", "composer", "symfony"]),
        (
            "csharp",
            &["dotnet", "aspnet", "entity framework", "blazor"],
        ),
        ("sqlite", &["sql", "database", "rusqlite"]),
        ("postgres", &["sql", "database", "pgvector"]),
    ])
}

// ============================================================================
// Aggregation Functions
// ============================================================================

/// Build the collective stack from all member DNA summaries.
pub(crate) fn build_collective_stack(
    dnas: &[MemberDna],
    member_count: usize,
) -> Vec<TeamTechEntry> {
    let mut tech_members: HashMap<String, Vec<String>> = HashMap::new();

    for dna in dnas {
        for tech in &dna.primary_stack {
            let lower = tech.to_lowercase();
            tech_members
                .entry(lower)
                .or_default()
                .push(dna.display_name.clone());
        }
    }

    let effective_count = if member_count > 0 {
        member_count
    } else {
        dnas.len().max(1)
    };

    let mut entries: Vec<TeamTechEntry> = tech_members
        .into_iter()
        .map(|(tech, members)| {
            let confidence = members.len() as f32 / effective_count as f32;
            TeamTechEntry {
                tech,
                members,
                team_confidence: confidence.min(1.0),
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        b.team_confidence
            .partial_cmp(&a.team_confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.tech.cmp(&b.tech))
    });

    entries
}

/// Find adjacent topics nobody on the team covers.
pub(crate) fn compute_blind_spots(
    dnas: &[MemberDna],
    stack: &[TeamTechEntry],
) -> Vec<TeamBlindSpot> {
    let adjacency = team_adjacency_map();

    let mut all_covered: HashSet<String> = HashSet::new();
    for dna in dnas {
        for tech in &dna.primary_stack {
            all_covered.insert(tech.to_lowercase());
        }
        for interest in &dna.interests {
            all_covered.insert(interest.to_lowercase());
        }
    }

    let mut blind_spots: Vec<TeamBlindSpot> = Vec::new();
    let mut seen_topics: HashSet<String> = HashSet::new();

    for entry in stack {
        if let Some(adjacent) = adjacency.get(entry.tech.as_str()) {
            for &adj_topic in *adjacent {
                let lower = adj_topic.to_lowercase();
                if !all_covered.contains(&lower) && seen_topics.insert(lower.clone()) {
                    let related_members = entry.members.len();
                    let severity = if related_members >= 3 {
                        "high"
                    } else {
                        "medium"
                    };
                    blind_spots.push(TeamBlindSpot {
                        topic: adj_topic.to_string(),
                        related_to: vec![entry.tech.clone()],
                        severity: severity.to_string(),
                    });
                }
            }
        }
    }

    blind_spots.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then_with(|| a.topic.cmp(&b.topic))
    });

    blind_spots
}

/// Find topics tracked by 3+ members.
pub(crate) fn compute_overlap_zones(dnas: &[MemberDna]) -> Vec<OverlapZone> {
    let mut tech_members: HashMap<String, Vec<String>> = HashMap::new();

    for dna in dnas {
        let mut all_topics: HashSet<String> = HashSet::new();
        for tech in &dna.primary_stack {
            all_topics.insert(tech.to_lowercase());
        }
        for interest in &dna.interests {
            all_topics.insert(interest.to_lowercase());
        }

        for topic in all_topics {
            tech_members
                .entry(topic)
                .or_default()
                .push(dna.display_name.clone());
        }
    }

    let mut zones: Vec<OverlapZone> = tech_members
        .into_iter()
        .filter(|(_, members)| members.len() >= 3)
        .map(|(topic, members)| {
            let count = members.len();
            OverlapZone {
                topic,
                members,
                member_count: count,
            }
        })
        .collect();

    zones.sort_by(|a, b| b.member_count.cmp(&a.member_count));
    zones
}

/// Find tech known by exactly one member.
pub(crate) fn compute_unique_strengths(dnas: &[MemberDna]) -> Vec<UniqueStrength> {
    let mut tech_owners: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for dna in dnas {
        for tech in &dna.primary_stack {
            let lower = tech.to_lowercase();
            tech_owners
                .entry(lower)
                .or_default()
                .push((dna.client_id.clone(), dna.display_name.clone()));
        }
    }

    let mut strengths: Vec<UniqueStrength> = tech_owners
        .into_iter()
        .filter(|(_, owners)| owners.len() == 1)
        .map(|(tech, owners)| {
            let (_, name) = &owners[0];
            UniqueStrength {
                tech,
                sole_expert: name.clone(),
                risk_level: "high".to_string(),
            }
        })
        .collect();

    strengths.sort_by(|a, b| a.tech.cmp(&b.tech));
    strengths
}

/// Compute stack coverage: fraction of adjacent ecosystem topics that somebody covers.
pub(crate) fn compute_stack_coverage(dnas: &[MemberDna], stack: &[TeamTechEntry]) -> f32 {
    let adjacency = team_adjacency_map();

    let mut all_covered: HashSet<String> = HashSet::new();
    for dna in dnas {
        for tech in &dna.primary_stack {
            all_covered.insert(tech.to_lowercase());
        }
        for interest in &dna.interests {
            all_covered.insert(interest.to_lowercase());
        }
    }

    let mut total_adjacent: usize = 0;
    let mut covered_adjacent: usize = 0;
    let mut counted: HashSet<String> = HashSet::new();

    for entry in stack {
        if let Some(adjacent) = adjacency.get(entry.tech.as_str()) {
            for &adj in *adjacent {
                let lower = adj.to_lowercase();
                if counted.insert(lower.clone()) {
                    total_adjacent += 1;
                    if all_covered.contains(&lower) {
                        covered_adjacent += 1;
                    }
                }
            }
        }
    }

    if total_adjacent == 0 {
        1.0
    } else {
        covered_adjacent as f32 / total_adjacent as f32
    }
}

// ============================================================================
// Signal confidence and priority helpers
// ============================================================================

pub(crate) fn compute_signal_confidence(detector_count: usize) -> f32 {
    let base = 0.5_f32;
    let bonus = (detector_count.saturating_sub(1) as f32) * 0.15;
    (base + bonus).min(1.0)
}

pub(crate) fn priority_rank(priority: &str) -> u8 {
    match priority.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

pub(crate) fn format_unix_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn parse_iso_to_unix(iso: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.timestamp())
}
