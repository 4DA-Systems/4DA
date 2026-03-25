//! MUSE Influence Formatter
//!
//! Transforms MUSE context packs into provider-consumable formats.
//! Phase 0: Text enrichment (Layer 0 — works with every generation API).
//! Phase 1+: Structured protocol output, provider-specific adapters.

use super::{MusePack, VisualProfile, SonicProfile, MotionProfile};

// ============================================================================
// Layer 0: Text Enrichment
// ============================================================================

/// Enrich a generation prompt with MUSE context from the active pack.
///
/// This is the simplest and most universal integration — works with any
/// text-accepting generation API (Sora, Runway, Midjourney, DALL-E, etc.)
/// by prepending structured style guidance to the user's prompt.
pub fn enrich_prompt(pack: &MusePack, original_prompt: &str) -> String {
    let mut parts = Vec::new();

    // Visual context
    if let Some(ref visual) = pack.visual {
        parts.extend(describe_visual(visual));
    }

    // Sonic context
    if let Some(ref sonic) = pack.sonic {
        parts.extend(describe_sonic(sonic));
    }

    // Motion context
    if let Some(ref motion) = pack.motion {
        parts.extend(describe_motion(motion));
    }

    // Thematic topics
    if !pack.topics.is_empty() {
        let labels: Vec<&str> = pack.topics.iter().take(5).map(|t| t.label.as_str()).collect();
        parts.push(format!("Themes: {}", labels.join(", ")));
    }

    // Anti-patterns (critical — what to avoid)
    if !pack.anti_patterns.is_empty() {
        let labels: Vec<&str> = pack
            .anti_patterns
            .iter()
            .take(4)
            .map(|t| t.label.as_str())
            .collect();
        parts.push(format!("Avoid: {}", labels.join(", ")));
    }

    if parts.is_empty() {
        return original_prompt.to_string();
    }

    let context = parts.join(". ");
    format!("Style context: {context}.\n\n{original_prompt}")
}

/// Describe visual characteristics as natural language
fn describe_visual(v: &VisualProfile) -> Vec<String> {
    let mut parts = Vec::new();

    // Color language
    if !v.dominant_colors.is_empty() {
        let hex_list: Vec<&str> = v.dominant_colors.iter().take(3).map(|c| c.hex.as_str()).collect();
        parts.push(format!("Color palette: {}", hex_list.join(", ")));
    }

    // Temperature
    let temp = if v.temperature > 0.65 {
        "warm"
    } else if v.temperature < 0.35 {
        "cool"
    } else {
        "neutral"
    };
    parts.push(format!("Color temperature: {temp}"));

    // Contrast
    if v.contrast > 0.7 {
        parts.push("High contrast".to_string());
    } else if v.contrast < 0.3 {
        parts.push("Low contrast, flat tones".to_string());
    }

    // Composition
    if v.negative_space > 0.6 {
        parts.push("Generous negative space".to_string());
    } else if v.negative_space < 0.3 {
        parts.push("Dense composition".to_string());
    }

    if v.symmetry <= 0.3 {
        parts.push("Asymmetric framing".to_string());
    } else if v.symmetry >= 0.7 {
        parts.push("Symmetrical composition".to_string());
    }

    if let Some(ref fp) = v.focal_point {
        parts.push(format!("Focal point: {fp}"));
    }

    // Texture
    if v.grain > 0.5 {
        parts.push("Textural grain".to_string());
    }
    if v.organic_vs_geometric > 0.6 {
        parts.push("Organic flowing forms".to_string());
    } else if v.organic_vs_geometric < 0.3 {
        parts.push("Geometric, structured forms".to_string());
    }

    parts
}

/// Describe sonic characteristics as natural language
fn describe_sonic(s: &SonicProfile) -> Vec<String> {
    let mut parts = Vec::new();

    // Timbre
    if s.warmth > 0.6 {
        parts.push("Warm, analog character".to_string());
    } else if s.warmth < 0.3 {
        parts.push("Cool, digital character".to_string());
    }

    if s.brightness > 0.6 {
        parts.push("Bright, present highs".to_string());
    } else if s.brightness <= 0.3 {
        parts.push("Dark, subdued highs".to_string());
    }

    // Tempo
    if let Some(center) = s.tempo_center {
        parts.push(format!("Tempo: ~{center:.0} BPM"));
    }

    // Key
    if let Some(ref mode) = s.mode_preference {
        parts.push(format!("Mode: {mode}"));
    }
    if !s.key_affinities.is_empty() {
        let keys: Vec<&str> = s.key_affinities.iter().take(3).map(|k| k.as_str()).collect();
        parts.push(format!("Key affinities: {}", keys.join(", ")));
    }

    // Production
    if s.stereo_width > 0.7 {
        parts.push("Wide stereo image".to_string());
    }
    if s.dynamic_range > 0.7 {
        parts.push("Dynamic, uncompressed".to_string());
    } else if s.dynamic_range < 0.3 {
        parts.push("Compressed, punchy".to_string());
    }

    parts
}

/// Describe motion characteristics as natural language
fn describe_motion(m: &MotionProfile) -> Vec<String> {
    let mut parts = Vec::new();

    // Pacing
    if m.cuts_per_minute > 15.0 {
        parts.push("Fast-paced editing".to_string());
    } else if m.cuts_per_minute < 5.0 {
        parts.push("Slow, contemplative pacing".to_string());
    } else {
        parts.push(format!("Moderate pacing (~{:.0} cuts/min)", m.cuts_per_minute));
    }

    // Transitions
    if m.hard_cut_ratio > 0.8 {
        parts.push("Hard cuts preferred".to_string());
    } else if m.dissolve_ratio > 0.4 {
        parts.push("Dissolve transitions".to_string());
    }

    // Camera
    if m.stability > 0.7 {
        parts.push("Stable, locked camera".to_string());
    } else if m.stability < 0.3 {
        parts.push("Handheld, dynamic camera".to_string());
    }

    if let Some(ref arc) = m.energy_arc {
        parts.push(format!("Energy arc: {arc}"));
    }

    parts
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::muse::{ColorWeight, MusePackType, WeightedTopic};

    fn make_test_pack() -> MusePack {
        MusePack {
            id: "test-001".to_string(),
            name: "Test Pack".to_string(),
            description: None,
            pack_type: MusePackType::Custom,
            is_active: true,
            source_count: 10,
            confidence: 0.85,
            visual: Some(VisualProfile {
                dominant_colors: vec![
                    ColorWeight { hex: "#D4AF37".to_string(), weight: 0.35 },
                    ColorWeight { hex: "#1A1A1A".to_string(), weight: 0.30 },
                    ColorWeight { hex: "#2D1B69".to_string(), weight: 0.20 },
                ],
                temperature: 0.72,
                contrast: 0.89,
                saturation: 0.45,
                harmony: Some("complementary".to_string()),
                symmetry: 0.3,
                negative_space: 0.7,
                focal_point: Some("golden_ratio".to_string()),
                depth: 0.8,
                grain: 0.65,
                organic_vs_geometric: 0.75,
            }),
            sonic: None,
            motion: None,
            topics: vec![
                WeightedTopic { label: "organic forms".to_string(), weight: 0.85 },
                WeightedTopic { label: "nocturnal".to_string(), weight: 0.72 },
            ],
            anti_patterns: vec![
                WeightedTopic { label: "corporate".to_string(), weight: 0.95 },
                WeightedTopic { label: "flat illustration".to_string(), weight: 0.88 },
            ],
            style_centroid: None,
            created_at: "2026-03-15T00:00:00Z".to_string(),
            updated_at: "2026-03-26T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_enrich_prompt_with_visual() {
        let pack = make_test_pack();
        let result = enrich_prompt(&pack, "cinematic sunset over water");

        assert!(result.contains("Style context:"));
        assert!(result.contains("#D4AF37"));
        assert!(result.contains("warm"));
        assert!(result.contains("High contrast"));
        assert!(result.contains("Generous negative space"));
        assert!(result.contains("Asymmetric framing"));
        assert!(result.contains("Organic flowing forms"));
        assert!(result.contains("organic forms"));
        assert!(result.contains("Avoid: corporate, flat illustration"));
        assert!(result.contains("cinematic sunset over water"));
    }

    #[test]
    fn test_enrich_prompt_empty_pack() {
        let pack = MusePack {
            id: "empty".to_string(),
            name: "Empty".to_string(),
            description: None,
            pack_type: MusePackType::Custom,
            is_active: false,
            source_count: 0,
            confidence: 0.0,
            visual: None,
            sonic: None,
            motion: None,
            topics: Vec::new(),
            anti_patterns: Vec::new(),
            style_centroid: None,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let result = enrich_prompt(&pack, "just a prompt");
        assert_eq!(result, "just a prompt");
    }

    #[test]
    fn test_enrich_prompt_preserves_original() {
        let pack = make_test_pack();
        let original = "a very specific prompt with special chars: <>&\"'";
        let result = enrich_prompt(&pack, original);
        assert!(result.ends_with(original));
    }

    #[test]
    fn test_sonic_description() {
        let sonic = SonicProfile {
            brightness: 0.3,
            warmth: 0.8,
            density: 0.5,
            tempo_range: Some((85.0, 110.0)),
            tempo_center: Some(95.0),
            grid_strictness: 0.4,
            key_affinities: vec!["Dm".to_string(), "Am".to_string()],
            mode_preference: Some("minor".to_string()),
            stereo_width: 0.85,
            dynamic_range: 0.65,
        };

        let parts = describe_sonic(&sonic);
        assert!(parts.iter().any(|p| p.contains("Warm")));
        assert!(parts.iter().any(|p| p.contains("Dark")));
        assert!(parts.iter().any(|p| p.contains("95")));
        assert!(parts.iter().any(|p| p.contains("Wide stereo")));
    }
}
