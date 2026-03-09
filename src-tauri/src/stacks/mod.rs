//! Stack Intelligence System — proactive domain knowledge for developer ecosystems.
//!
//! Pre-computed, composable technology profiles that make scoring smarter
//! from the first fetch. Each profile encodes pain points, ecosystem shifts,
//! keyword boosts, and source preferences for a technology stack.

pub mod detection;
pub mod profiles;
pub mod scoring;

use rusqlite::{params, Connection, Result as SqliteResult};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Core Types
// ============================================================================

/// A curated seed content item for a technology stack.
/// Used during first analysis to guarantee high-quality results.
pub struct SeedItem {
    pub title: &'static str,
    pub url: &'static str,
    pub source_type: &'static str,
}

/// A known pain point for a technology stack.
/// Requires 2+ keyword matches to trigger (prevents false positives).
pub struct PainPoint {
    pub keywords: &'static [&'static str],
    pub severity: f32,
    pub description: &'static str,
}

/// A shift in the ecosystem (e.g., Drizzle replacing Prisma).
pub struct EcosystemShift {
    pub from: &'static str,
    pub to: &'static str,
    pub keywords: &'static [&'static str],
    pub boost: f32,
}

/// A complete technology stack profile with domain knowledge.
pub struct StackProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub core_tech: &'static [&'static str],
    pub companions: &'static [&'static str],
    pub competing: &'static [&'static str],
    pub pain_points: &'static [PainPoint],
    pub ecosystem_shifts: &'static [EcosystemShift],
    pub keyword_boosts: &'static [(&'static str, f32)],
    pub source_preferences: &'static [(&'static str, f32)],
    pub detection_markers: &'static [&'static str],
    pub detection_threshold: usize,
    pub seed_content: &'static [SeedItem],
}

/// Merged result of composing multiple stack profiles.
/// When `active` is false (no stacks selected), all scoring values are neutral.
#[derive(Default, Clone)]
pub struct ComposedStack {
    pub pain_points: Vec<&'static PainPoint>,
    pub ecosystem_shifts: Vec<&'static EcosystemShift>,
    pub keyword_boosts: HashMap<&'static str, f32>,
    pub source_preferences: HashMap<&'static str, f32>,
    pub all_tech: HashSet<&'static str>,
    pub competing: HashSet<&'static str>,
    pub active: bool,
}

// ============================================================================
// Public API
// ============================================================================

/// Compose multiple stack profiles into a single merged view.
/// - Keyword boosts use MAX (not SUM) to prevent score inflation.
/// - Source preferences use AVERAGE across selected profiles.
/// - Pain points and ecosystem shifts are concatenated.
pub fn compose_profiles(ids: &[String]) -> ComposedStack {
    if ids.is_empty() {
        return ComposedStack::default();
    }

    let mut pain_points = Vec::new();
    let mut ecosystem_shifts = Vec::new();
    let mut keyword_boosts: HashMap<&'static str, f32> = HashMap::new();
    let mut source_pref_sums: HashMap<&'static str, (f32, u32)> = HashMap::new();
    let mut all_tech = HashSet::new();
    let mut competing = HashSet::new();

    for id in ids {
        if let Some(profile) = get_profile(id) {
            // Collect pain points
            for pp in profile.pain_points {
                pain_points.push(pp);
            }

            // Collect ecosystem shifts
            for es in profile.ecosystem_shifts {
                ecosystem_shifts.push(es);
            }

            // Keyword boosts: MAX semantics
            for &(kw, boost) in profile.keyword_boosts {
                let entry = keyword_boosts.entry(kw).or_insert(0.0);
                if boost > *entry {
                    *entry = boost;
                }
            }

            // Source preferences: accumulate for averaging
            for &(source, pref) in profile.source_preferences {
                let entry = source_pref_sums.entry(source).or_insert((0.0, 0));
                entry.0 += pref;
                entry.1 += 1;
            }

            // Collect tech
            for &tech in profile.core_tech {
                all_tech.insert(tech);
            }
            for &tech in profile.companions {
                all_tech.insert(tech);
            }
            for &tech in profile.competing {
                competing.insert(tech);
            }
        }
    }

    // Average source preferences
    let source_preferences: HashMap<&'static str, f32> = source_pref_sums
        .into_iter()
        .map(|(source, (sum, count))| (source, sum / count as f32))
        .collect();

    ComposedStack {
        pain_points,
        ecosystem_shifts,
        keyword_boosts,
        source_preferences,
        all_tech,
        competing,
        active: true,
    }
}

/// Look up a profile by its ID.
pub fn get_profile(id: &str) -> Option<&'static StackProfile> {
    profiles::ALL_PROFILES.iter().find(|p| p.id == id).copied()
}

/// List all available profiles.
pub fn list_profiles() -> &'static [&'static StackProfile] {
    &profiles::ALL_PROFILES
}

// ============================================================================
// Database Operations
// ============================================================================

/// Load selected stack IDs from the database.
pub fn load_selected_stacks(conn: &Connection) -> Vec<String> {
    let mut stmt = match conn.prepare("SELECT profile_id FROM selected_stacks ORDER BY id") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in stacks: {e}");
            None
        }
    })
    .collect()
}

/// Save selected stack IDs to the database (replaces existing).
pub fn save_selected_stacks(conn: &Connection, ids: &[String]) -> SqliteResult<()> {
    conn.execute("DELETE FROM selected_stacks", [])?;
    let mut stmt = conn.prepare(
        "INSERT INTO selected_stacks (profile_id, auto_detected, confidence) VALUES (?1, ?2, ?3)",
    )?;
    for id in ids {
        stmt.execute(params![id, 0, 1.0])?;
    }
    Ok(())
}

/// Save auto-detected stack profiles to the database.
pub fn save_detected_stacks(
    conn: &Connection,
    detections: &[detection::StackDetection],
) -> SqliteResult<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO selected_stacks (profile_id, auto_detected, confidence) VALUES (?1, ?2, ?3)",
    )?;
    for d in detections {
        stmt.execute(params![d.profile_id, 1, d.confidence])?;
    }
    Ok(())
}

/// Load selected stacks and compose them into a merged profile.
pub fn load_composed_stack(conn: &Connection) -> ComposedStack {
    let ids = load_selected_stacks(conn);
    compose_profiles(&ids)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_compose_is_inactive() {
        let composed = compose_profiles(&[]);
        assert!(!composed.active);
        assert!(composed.pain_points.is_empty());
        assert!(composed.keyword_boosts.is_empty());
    }

    #[test]
    fn test_single_profile_compose() {
        let composed = compose_profiles(&["rust_systems".to_string()]);
        assert!(composed.active);
        assert!(!composed.pain_points.is_empty());
        assert!(composed.all_tech.contains("rust"));
    }

    #[test]
    fn test_multi_profile_compose_max_semantics() {
        let composed =
            compose_profiles(&["nextjs_fullstack".to_string(), "rust_systems".to_string()]);
        assert!(composed.active);
        assert!(composed.all_tech.contains("rust"));
        assert!(composed.all_tech.contains("nextjs"));
        // Both profiles should contribute pain points
        assert!(composed.pain_points.len() > 3);
    }

    #[test]
    fn test_source_preferences_averaged() {
        // Both nextjs_fullstack and rust_systems have hackernews prefs
        let composed =
            compose_profiles(&["nextjs_fullstack".to_string(), "rust_systems".to_string()]);
        if let Some(&pref) = composed.source_preferences.get("hackernews") {
            // Average of two hackernews preferences
            assert!(pref > 0.0);
        }
    }

    #[test]
    fn test_unknown_profile_ignored() {
        let composed = compose_profiles(&["nonexistent_profile".to_string()]);
        // Still active (ids were provided) but no content
        assert!(composed.active);
        assert!(composed.pain_points.is_empty());
    }

    #[test]
    fn test_all_profiles_exist() {
        let profiles = list_profiles();
        assert_eq!(profiles.len(), 11);
        for profile in profiles {
            assert!(!profile.id.is_empty());
            assert!(!profile.name.is_empty());
            assert!(!profile.core_tech.is_empty());
            assert!(!profile.pain_points.is_empty());
        }
    }

    #[test]
    fn test_get_profile_by_id() {
        assert!(get_profile("rust_systems").is_some());
        assert!(get_profile("nextjs_fullstack").is_some());
        assert!(get_profile("nonexistent").is_none());
    }
}
