//! MUSE Pack Extraction Pipeline
//!
//! Processes source files in a pack through the appropriate extractors,
//! updates extraction status, and aggregates results into the pack profile.

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::Path;
use tracing::{info, warn};

use crate::error::{Result, ResultExt};

use super::image_analysis;
use super::{classify_media_type, MuseMediaType, VisualProfile, WeightedTopic};

// ============================================================================
// Pack Extraction
// ============================================================================

/// Process all pending source files in a pack.
///
/// For each pending source:
/// 1. Classify the file type
/// 2. Run the appropriate extractor
/// 3. Store per-file results
/// 4. Update extraction status
///
/// After all files, aggregate results into the pack profile.
pub fn extract_pack(conn: &Connection, pack_id: &str) -> Result<ExtractionReport> {
    let mut report = ExtractionReport::default();

    // Get all pending sources for this pack
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, file_type FROM muse_pack_sources
             WHERE pack_id = ?1 AND extraction_status = 'pending'",
        )
        .context("Failed to query pending sources")?;

    let sources: Vec<(i64, String, String)> = stmt
        .query_map([pack_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .context("Failed to read sources")?
        .filter_map(|r| r.ok())
        .collect();

    report.total = sources.len() as u32;

    for (source_id, file_path, file_type) in &sources {
        // Mark as processing
        conn.execute(
            "UPDATE muse_pack_sources SET extraction_status = 'processing' WHERE id = ?1",
            [source_id],
        )
        .ok();

        let path = Path::new(file_path);

        // Check file exists
        if !path.exists() {
            mark_failed(conn, *source_id, "File not found");
            report.failed += 1;
            continue;
        }

        // Compute file hash for change detection
        let file_hash = compute_file_hash(path).ok();

        // Extract based on media type
        let media_type = classify_media_type(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        );

        match media_type {
            Some(MuseMediaType::Image) => {
                match image_analysis::analyze_image(path) {
                    Ok(profile) => {
                        // Store per-file color/composition data as JSON
                        let color_json = serde_json::to_string(&ProfileData {
                            dominant_colors: profile.dominant_colors.clone(),
                            temperature: profile.temperature,
                            contrast: profile.contrast,
                            saturation: profile.saturation,
                        })
                        .ok();

                        let comp_json = serde_json::to_string(&CompositionData {
                            symmetry: profile.symmetry,
                            negative_space: profile.negative_space,
                            focal_point: profile.focal_point.clone(),
                            depth: profile.depth,
                            grain: profile.grain,
                            organic_vs_geometric: profile.organic_vs_geometric,
                        })
                        .ok();

                        conn.execute(
                            "UPDATE muse_pack_sources SET
                                extraction_status = 'done',
                                extracted_at = datetime('now'),
                                color_data = ?1,
                                composition_data = ?2,
                                confidence = 0.9,
                                file_hash = ?3
                             WHERE id = ?4",
                            params![color_json, comp_json, file_hash, source_id],
                        )
                        .context("Failed to update source with extraction")?;

                        report.succeeded += 1;
                        info!(target: "muse::extract", source_id = source_id, path = %file_path, "Image extracted");
                    }
                    Err(e) => {
                        mark_failed(conn, *source_id, &e.to_string());
                        report.failed += 1;
                        warn!(target: "muse::extract", source_id = source_id, error = %e, "Image extraction failed");
                    }
                }
            }
            Some(MuseMediaType::Audio) | Some(MuseMediaType::Video) => {
                // Phase 2 — mark as pending for now
                conn.execute(
                    "UPDATE muse_pack_sources SET extraction_status = 'pending', file_hash = ?1 WHERE id = ?2",
                    params![file_hash, source_id],
                )
                .ok();
                report.skipped += 1;
            }
            _ => {
                mark_failed(conn, *source_id, "Unsupported media type");
                report.failed += 1;
            }
        }
    }

    // Aggregate extracted results into the pack profile
    if report.succeeded > 0 {
        aggregate_pack_profile(conn, pack_id)?;
    }

    info!(
        target: "muse::extract",
        pack_id = pack_id,
        total = report.total,
        succeeded = report.succeeded,
        failed = report.failed,
        skipped = report.skipped,
        "Pack extraction complete"
    );

    Ok(report)
}

/// Aggregate individual source extractions into the pack's unified profile.
fn aggregate_pack_profile(conn: &Connection, pack_id: &str) -> Result<()> {
    // Collect all successful image extractions
    let mut stmt = conn
        .prepare(
            "SELECT color_data, composition_data FROM muse_pack_sources
             WHERE pack_id = ?1 AND extraction_status = 'done' AND color_data IS NOT NULL",
        )
        .context("Failed to query extracted sources")?;

    let mut color_profiles: Vec<ProfileData> = Vec::new();
    let mut comp_profiles: Vec<CompositionData> = Vec::new();

    let rows: Vec<(Option<String>, Option<String>)> = stmt
        .query_map([pack_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("Failed to read extraction data")?
        .filter_map(|r| r.ok())
        .collect();

    for (color_json, comp_json) in &rows {
        if let Some(json) = color_json {
            if let Ok(data) = serde_json::from_str::<ProfileData>(json) {
                color_profiles.push(data);
            }
        }
        if let Some(json) = comp_json {
            if let Ok(data) = serde_json::from_str::<CompositionData>(json) {
                comp_profiles.push(data);
            }
        }
    }

    if color_profiles.is_empty() {
        return Ok(());
    }

    let n = color_profiles.len() as f64;

    // Average color metrics
    let avg_temp: f64 = color_profiles.iter().map(|p| p.temperature).sum::<f64>() / n;
    let avg_contrast: f64 = color_profiles.iter().map(|p| p.contrast).sum::<f64>() / n;
    let avg_saturation: f64 = color_profiles.iter().map(|p| p.saturation).sum::<f64>() / n;

    // Merge dominant colors (collect all, re-cluster)
    let all_colors: Vec<super::ColorWeight> = color_profiles
        .iter()
        .flat_map(|p| p.dominant_colors.clone())
        .collect();

    // Take top 5 by weight
    let mut sorted_colors = all_colors;
    sorted_colors.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
    sorted_colors.truncate(5);

    // Average composition metrics
    let cn = comp_profiles.len() as f64;
    let avg_symmetry = if cn > 0.0 { comp_profiles.iter().map(|p| p.symmetry).sum::<f64>() / cn } else { 0.5 };
    let avg_neg_space = if cn > 0.0 { comp_profiles.iter().map(|p| p.negative_space).sum::<f64>() / cn } else { 0.5 };
    let avg_depth = if cn > 0.0 { comp_profiles.iter().map(|p| p.depth).sum::<f64>() / cn } else { 0.5 };
    let avg_grain = if cn > 0.0 { comp_profiles.iter().map(|p| p.grain).sum::<f64>() / cn } else { 0.0 };
    let avg_organic = if cn > 0.0 { comp_profiles.iter().map(|p| p.organic_vs_geometric).sum::<f64>() / cn } else { 0.5 };

    // Build the unified visual profile
    let profile = VisualProfile {
        dominant_colors: sorted_colors,
        temperature: avg_temp,
        contrast: avg_contrast,
        saturation: avg_saturation,
        harmony: None, // Recomputed when needed
        symmetry: avg_symmetry,
        negative_space: avg_neg_space,
        focal_point: most_common_focal(&comp_profiles),
        depth: avg_depth,
        grain: avg_grain,
        organic_vs_geometric: avg_organic,
    };

    // Store as JSON in the pack's color_profile and composition_profile
    let color_json = serde_json::to_string(&profile).unwrap_or_default();

    // Derive confidence from source count (more sources = higher confidence)
    let confidence = (1.0 - 1.0 / (n + 1.0)).min(0.95);

    // Generate thematic topics from visual analysis
    let topics = derive_visual_topics(&profile);
    let topics_json = serde_json::to_string(&topics).unwrap_or_default();

    conn.execute(
        "UPDATE muse_packs SET
            color_profile = ?1,
            confidence = ?2,
            thematic_topics = ?3,
            updated_at = datetime('now')
         WHERE id = ?4",
        params![color_json, confidence, topics_json, pack_id],
    )
    .context("Failed to update pack profile")?;

    info!(target: "muse::extract", pack_id = pack_id, sources = n as u32, confidence = confidence, "Pack profile aggregated");

    Ok(())
}

/// Derive thematic topics from visual analysis
fn derive_visual_topics(profile: &VisualProfile) -> Vec<WeightedTopic> {
    let mut topics = Vec::new();

    // Temperature
    if profile.temperature > 0.65 {
        topics.push(WeightedTopic { label: "warm tones".to_string(), weight: profile.temperature });
    } else if profile.temperature < 0.35 {
        topics.push(WeightedTopic { label: "cool tones".to_string(), weight: 1.0 - profile.temperature });
    }

    // Contrast
    if profile.contrast > 0.7 {
        topics.push(WeightedTopic { label: "high contrast".to_string(), weight: profile.contrast });
    } else if profile.contrast < 0.3 {
        topics.push(WeightedTopic { label: "flat tones".to_string(), weight: 1.0 - profile.contrast });
    }

    // Composition
    if profile.negative_space > 0.6 {
        topics.push(WeightedTopic { label: "negative space".to_string(), weight: profile.negative_space });
    }
    if profile.symmetry > 0.7 {
        topics.push(WeightedTopic { label: "symmetrical".to_string(), weight: profile.symmetry });
    } else if profile.symmetry < 0.3 {
        topics.push(WeightedTopic { label: "asymmetric".to_string(), weight: 1.0 - profile.symmetry });
    }

    // Texture
    if profile.grain > 0.5 {
        topics.push(WeightedTopic { label: "textural grain".to_string(), weight: profile.grain });
    }
    if profile.organic_vs_geometric > 0.6 {
        topics.push(WeightedTopic { label: "organic forms".to_string(), weight: profile.organic_vs_geometric });
    } else if profile.organic_vs_geometric < 0.3 {
        topics.push(WeightedTopic { label: "geometric forms".to_string(), weight: 1.0 - profile.organic_vs_geometric });
    }

    // Sort by weight descending
    topics.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
    topics
}

fn most_common_focal(profiles: &[CompositionData]) -> Option<String> {
    if profiles.is_empty() {
        return None;
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for p in profiles {
        if let Some(ref fp) = p.focal_point {
            *counts.entry(fp.clone()).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k)
}

// ============================================================================
// Helper Types
// ============================================================================

/// Per-file color extraction data (stored as JSON in muse_pack_sources.color_data)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ProfileData {
    dominant_colors: Vec<super::ColorWeight>,
    temperature: f64,
    contrast: f64,
    saturation: f64,
}

/// Per-file composition data (stored as JSON in muse_pack_sources.composition_data)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CompositionData {
    symmetry: f64,
    negative_space: f64,
    focal_point: Option<String>,
    depth: f64,
    grain: f64,
    organic_vs_geometric: f64,
}

/// Report from a pack extraction run
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ExtractionReport {
    pub total: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub skipped: u32,
}

fn mark_failed(conn: &Connection, source_id: i64, _reason: &str) {
    conn.execute(
        "UPDATE muse_pack_sources SET extraction_status = 'failed' WHERE id = ?1",
        [source_id],
    )
    .ok();
}

fn compute_file_hash(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).context("Failed to read file for hashing")?;
    let hash = Sha256::digest(&bytes);
    Ok(format!("{:x}", hash))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_warm_topics() {
        let profile = VisualProfile {
            dominant_colors: vec![],
            temperature: 0.8,
            contrast: 0.85,
            saturation: 0.5,
            harmony: None,
            symmetry: 0.2,
            negative_space: 0.7,
            focal_point: None,
            depth: 0.5,
            grain: 0.6,
            organic_vs_geometric: 0.75,
        };

        let topics = derive_visual_topics(&profile);
        let labels: Vec<&str> = topics.iter().map(|t| t.label.as_str()).collect();

        assert!(labels.contains(&"warm tones"));
        assert!(labels.contains(&"high contrast"));
        assert!(labels.contains(&"negative space"));
        assert!(labels.contains(&"asymmetric"));
        assert!(labels.contains(&"textural grain"));
        assert!(labels.contains(&"organic forms"));
    }

    #[test]
    fn test_derive_cool_geometric_topics() {
        let profile = VisualProfile {
            dominant_colors: vec![],
            temperature: 0.2,
            contrast: 0.2,
            saturation: 0.5,
            harmony: None,
            symmetry: 0.85,
            negative_space: 0.3,
            focal_point: None,
            depth: 0.5,
            grain: 0.1,
            organic_vs_geometric: 0.15,
        };

        let topics = derive_visual_topics(&profile);
        let labels: Vec<&str> = topics.iter().map(|t| t.label.as_str()).collect();

        assert!(labels.contains(&"cool tones"));
        assert!(labels.contains(&"flat tones"));
        assert!(labels.contains(&"symmetrical"));
        assert!(labels.contains(&"geometric forms"));
    }

    #[test]
    fn test_extraction_report_default() {
        let report = ExtractionReport::default();
        assert_eq!(report.total, 0);
        assert_eq!(report.succeeded, 0);
    }
}
