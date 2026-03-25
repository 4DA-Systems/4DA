//! 4DA MUSE — Multimodal Understanding & Semantic Engine
//!
//! Creative context system for digital artists (image, video, audio).
//! Extracts creative signals from local files, builds compound profiles,
//! and projects them into AI generation pipelines.
//!
//! # Architecture
//!
//! MUSE extends ACE's infrastructure (embeddings, sqlite-vec, affinities)
//! with creative-specific extractors and the Context Pack system.
//!
//! ```text
//! Files → Extractors → Pack Aggregation → Context Packs → Influence Formatting
//!                                              ↓
//!                                     Generation Feedback
//!                                              ↓
//!                                     Compound Learning
//! ```
//!
//! # Phase 0 (Current)
//!
//! Foundation layer: types, database schema, pack lifecycle stubs.
//! No creative extractors yet — those ship in Phase 1 (image) and Phase 2 (audio/video).

pub mod db;
pub mod influence;
pub mod pack;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// Core MUSE Types
// ============================================================================

/// A MUSE Context Pack — the atomic unit of creative context.
///
/// Derived from user files, not a folder of files. Contains semantic
/// distillation: embeddings, color profiles, compositional tendencies,
/// thematic topics, and anti-patterns.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MusePack {
    /// Unique identifier
    pub id: String,
    /// Human-readable name (e.g., "Meridian Album Art")
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Pack type
    pub pack_type: MusePackType,
    /// Whether this pack is currently active for generation influence
    pub is_active: bool,
    /// Number of source files that contributed to this pack
    pub source_count: u32,
    /// Overall confidence in the pack's signals (0.0-1.0)
    pub confidence: f64,
    /// Visual signals: color, composition, texture
    pub visual: Option<VisualProfile>,
    /// Sonic signals: timbre, rhythm, harmony, production
    pub sonic: Option<SonicProfile>,
    /// Motion signals: pacing, transitions, camera
    pub motion: Option<MotionProfile>,
    /// Thematic topics derived from the work
    pub topics: Vec<WeightedTopic>,
    /// Anti-patterns — what to explicitly avoid
    pub anti_patterns: Vec<WeightedTopic>,
    /// Style embedding centroid (384-dim, base64 encoded for serialization)
    pub style_centroid: Option<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
}

/// Pack type classification
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum MusePackType {
    /// User-created from selected files
    Custom,
    /// Auto-detected from creative activity
    Auto,
    /// Imported from another user's export
    Imported,
    /// Purchased from MUSE Marketplace
    Marketplace,
    /// Blend of multiple packs
    Blend,
}

impl std::fmt::Display for MusePackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom => write!(f, "custom"),
            Self::Auto => write!(f, "auto"),
            Self::Imported => write!(f, "imported"),
            Self::Marketplace => write!(f, "marketplace"),
            Self::Blend => write!(f, "blend"),
        }
    }
}

// ============================================================================
// Visual Profile (Image + Video visual characteristics)
// ============================================================================

/// Visual characteristics extracted from image/video content
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VisualProfile {
    /// Dominant colors with weights (sum to 1.0)
    pub dominant_colors: Vec<ColorWeight>,
    /// Color temperature: 0.0 (cool) to 1.0 (warm)
    pub temperature: f64,
    /// Contrast level: 0.0 (flat) to 1.0 (high contrast)
    pub contrast: f64,
    /// Saturation level: 0.0 (monochrome) to 1.0 (vivid)
    pub saturation: f64,
    /// Palette harmony type
    pub harmony: Option<String>,
    /// Symmetry tendency: 0.0 (asymmetric) to 1.0 (symmetric)
    pub symmetry: f64,
    /// Negative space: 0.0 (dense) to 1.0 (spacious)
    pub negative_space: f64,
    /// Dominant focal point positioning
    pub focal_point: Option<String>,
    /// Perceived depth: 0.0 (flat) to 1.0 (deep)
    pub depth: f64,
    /// Grain/noise tendency: 0.0 (clean) to 1.0 (heavy grain)
    pub grain: f64,
    /// Organic vs geometric: 0.0 (geometric) to 1.0 (organic)
    pub organic_vs_geometric: f64,
}

/// A color with its weight in the palette
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ColorWeight {
    /// Hex color code (e.g., "#D4AF37")
    pub hex: String,
    /// Weight in the palette (0.0-1.0)
    pub weight: f64,
}

// ============================================================================
// Sonic Profile (Audio characteristics)
// ============================================================================

/// Audio characteristics extracted from music/sound files
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SonicProfile {
    /// Spectral brightness: 0.0 (dark) to 1.0 (bright)
    pub brightness: f64,
    /// Warmth: 0.0 (cold/digital) to 1.0 (warm/analog)
    pub warmth: f64,
    /// Sonic density: 0.0 (sparse) to 1.0 (dense)
    pub density: f64,
    /// Tempo range in BPM [min, max]
    pub tempo_range: Option<(f64, f64)>,
    /// Most common tempo
    pub tempo_center: Option<f64>,
    /// Grid strictness: 0.0 (free) to 1.0 (quantized)
    pub grid_strictness: f64,
    /// Key affinities (e.g., ["Dm", "Am", "F"])
    pub key_affinities: Vec<String>,
    /// Mode preference
    pub mode_preference: Option<String>,
    /// Stereo width: 0.0 (mono) to 1.0 (wide)
    pub stereo_width: f64,
    /// Dynamic range: 0.0 (compressed) to 1.0 (dynamic)
    pub dynamic_range: f64,
}

// ============================================================================
// Motion Profile (Video temporal characteristics)
// ============================================================================

/// Motion/temporal characteristics extracted from video content
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MotionProfile {
    /// Average cuts per minute
    pub cuts_per_minute: f64,
    /// Median shot duration in seconds
    pub shot_duration_median: f64,
    /// Energy arc description
    pub energy_arc: Option<String>,
    /// Hard cut ratio (0.0-1.0)
    pub hard_cut_ratio: f64,
    /// Dissolve ratio (0.0-1.0)
    pub dissolve_ratio: f64,
    /// Camera movement intensity: 0.0 (static) to 1.0 (dynamic)
    pub movement_intensity: f64,
    /// Camera stability: 0.0 (handheld) to 1.0 (locked)
    pub stability: f64,
}

// ============================================================================
// Shared Types
// ============================================================================

/// A topic or anti-pattern with a weight/strength
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WeightedTopic {
    /// Topic label (e.g., "organic forms", "corporate")
    pub label: String,
    /// Weight or strength (0.0-1.0)
    pub weight: f64,
}

/// A source file contributing to a MUSE pack
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MusePackSource {
    /// Unique ID
    pub id: i64,
    /// Pack this source belongs to
    pub pack_id: String,
    /// Path to the source file
    pub file_path: String,
    /// Media type category
    pub file_type: MuseMediaType,
    /// Extraction status
    pub extraction_status: ExtractionStatus,
    /// Individual file confidence
    pub confidence: f64,
    /// File content hash for change detection
    pub file_hash: Option<String>,
}

/// Media type categories MUSE understands
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum MuseMediaType {
    Image,
    Video,
    Audio,
    Document,
    ProjectFile,
}

impl std::fmt::Display for MuseMediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image => write!(f, "image"),
            Self::Video => write!(f, "video"),
            Self::Audio => write!(f, "audio"),
            Self::Document => write!(f, "document"),
            Self::ProjectFile => write!(f, "project_file"),
        }
    }
}

/// Extraction pipeline status
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Pending,
    Processing,
    Done,
    Failed,
}

impl std::fmt::Display for ExtractionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Generation record for feedback learning
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MuseGeneration {
    /// Unique generation ID
    pub id: String,
    /// Pack used for this generation
    pub pack_id: Option<String>,
    /// Generation provider (e.g., "runway", "midjourney")
    pub provider: String,
    /// Original user prompt
    pub prompt: String,
    /// Enriched prompt after MUSE influence
    pub enriched_prompt: Option<String>,
    /// User outcome: kept, rejected, modified
    pub outcome: Option<GenerationOutcome>,
    /// Outcome signal strength (-1.0 to 1.0)
    pub outcome_signal: Option<f64>,
    /// Timestamp
    pub created_at: String,
}

/// Generation outcome for feedback loop
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum GenerationOutcome {
    Kept,
    Rejected,
    Modified,
    Unknown,
}

impl std::fmt::Display for GenerationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kept => write!(f, "kept"),
            Self::Rejected => write!(f, "rejected"),
            Self::Modified => write!(f, "modified"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// ============================================================================
// Media Type Detection
// ============================================================================

/// Classify a file extension into a MUSE media type
pub fn classify_media_type(extension: &str) -> Option<MuseMediaType> {
    match extension.to_lowercase().as_str() {
        // Image formats
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "tiff" | "tif" | "bmp" | "psd"
        | "raw" | "cr2" | "nef" | "arw" | "dng" | "heic" | "heif" | "exr" => {
            Some(MuseMediaType::Image)
        }
        // Video formats
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv" | "m4v" | "prores" => {
            Some(MuseMediaType::Video)
        }
        // Audio formats
        "wav" | "flac" | "mp3" | "aiff" | "aif" | "ogg" | "m4a" | "aac" | "wma" | "opus" => {
            Some(MuseMediaType::Audio)
        }
        // Creative project files
        "aep" | "prproj" | "drp" | "als" | "flp" | "logicx" | "blend" | "c4d" | "ma"
        | "mb" | "hip" | "nk" | "sketch" | "fig" | "xd" => Some(MuseMediaType::ProjectFile),
        _ => None,
    }
}

/// All file extensions MUSE can potentially process
pub const MUSE_EXTENSIONS: &[&str] = &[
    // Image
    "png", "jpg", "jpeg", "gif", "webp", "svg", "tiff", "tif", "bmp", "psd", "raw", "cr2",
    "nef", "arw", "dng", "heic", "heif", "exr",
    // Video
    "mp4", "mov", "avi", "mkv", "webm", "flv", "wmv", "m4v",
    // Audio
    "wav", "flac", "mp3", "aiff", "aif", "ogg", "m4a", "aac", "wma", "opus",
    // Project files
    "aep", "prproj", "drp", "als", "flp", "blend", "c4d", "sketch", "fig", "xd",
];

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_image_types() {
        assert_eq!(classify_media_type("png"), Some(MuseMediaType::Image));
        assert_eq!(classify_media_type("jpg"), Some(MuseMediaType::Image));
        assert_eq!(classify_media_type("PSD"), Some(MuseMediaType::Image));
        assert_eq!(classify_media_type("JPEG"), Some(MuseMediaType::Image));
    }

    #[test]
    fn test_classify_video_types() {
        assert_eq!(classify_media_type("mp4"), Some(MuseMediaType::Video));
        assert_eq!(classify_media_type("mov"), Some(MuseMediaType::Video));
        assert_eq!(classify_media_type("webm"), Some(MuseMediaType::Video));
    }

    #[test]
    fn test_classify_audio_types() {
        assert_eq!(classify_media_type("wav"), Some(MuseMediaType::Audio));
        assert_eq!(classify_media_type("flac"), Some(MuseMediaType::Audio));
        assert_eq!(classify_media_type("mp3"), Some(MuseMediaType::Audio));
    }

    #[test]
    fn test_classify_project_types() {
        assert_eq!(
            classify_media_type("aep"),
            Some(MuseMediaType::ProjectFile)
        );
        assert_eq!(
            classify_media_type("blend"),
            Some(MuseMediaType::ProjectFile)
        );
        assert_eq!(
            classify_media_type("als"),
            Some(MuseMediaType::ProjectFile)
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(classify_media_type("rs"), None);
        assert_eq!(classify_media_type("txt"), None);
        assert_eq!(classify_media_type("unknown"), None);
    }

    #[test]
    fn test_pack_type_display() {
        assert_eq!(MusePackType::Custom.to_string(), "custom");
        assert_eq!(MusePackType::Blend.to_string(), "blend");
        assert_eq!(MusePackType::Marketplace.to_string(), "marketplace");
    }

    #[test]
    fn test_muse_extensions_coverage() {
        // Every extension in MUSE_EXTENSIONS should be classifiable
        for ext in MUSE_EXTENSIONS {
            assert!(
                classify_media_type(ext).is_some(),
                "Extension '{ext}' should be classifiable"
            );
        }
    }
}
