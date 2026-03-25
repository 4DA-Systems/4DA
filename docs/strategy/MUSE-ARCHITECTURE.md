# MUSE — Technical Architecture

> System design for integrating MUSE into the 4DA platform.

**Status:** Design Draft | **Version:** 0.1.0 | **Date:** 2026-03-26
**Depends on:** `MUSE-STRATEGY.md`, `MUSE-CONTEXT-PROTOCOL.md`

---

## 1. Design Principles

1. **Extend, don't fork** — MUSE reuses ACE's infrastructure. No parallel embedding pipeline, no separate database, no duplicated affinity system.
2. **Extractor abstraction** — creative media extractors follow the same `Extractor` trait pattern as PDF/Office. MUSE doesn't need a new extraction framework.
3. **Pack-first** — Context Packs are the atomic unit. Everything flows through packs — creation, blending, activation, export, marketplace.
4. **Feedback-native** — generation outcomes (kept/rejected) feed back into MUSE from day one. The learning loop is not a v2 feature.
5. **Provider-agnostic** — MUSE produces context. It doesn't know or care which generation API consumes it. The influence formatter adapts output per provider.

---

## 2. Module Structure

```
src-tauri/src/
├── ace/                          # Existing — shared infrastructure
│   ├── mod.rs                    # ACE orchestrator (extend for MUSE awareness)
│   ├── context.rs                # File change processing (reused by MUSE)
│   ├── db.rs                     # Database schema (extend for MUSE tables)
│   ├── embedding.rs              # Embedding service (add CLIP model support)
│   ├── scanner.rs                # Project scanner (unchanged)
│   ├── watcher.rs                # File watcher (extend for creative file types)
│   └── git.rs                    # Git analysis (unchanged)
│
├── muse/                         # NEW — MUSE-specific logic
│   ├── mod.rs                    # MUSE system orchestrator
│   ├── pack.rs                   # Context Pack lifecycle (create, update, blend, export)
│   ├── profile.rs                # Creative DNA profile builder (aggregates across packs)
│   ├── influence.rs              # Influence formatter (context → provider-specific format)
│   ├── feedback.rs               # Generation feedback processing (kept/rejected → learning)
│   ├── drift.rs                  # Creative drift detection + temporal analysis
│   ├── crossmodal.rs             # Cross-modal correlation engine
│   └── marketplace.rs            # Profile export/import for marketplace (v2)
│
├── extractors/                   # Existing — extend with creative types
│   ├── mod.rs                    # ExtractorRegistry (register MUSE extractors)
│   ├── pdf.rs                    # Existing
│   ├── office.rs                 # Existing
│   ├── image.rs                  # NEW/EXTEND — CLIP embeddings + color + composition
│   ├── video.rs                  # NEW — keyframe extraction + cut detection
│   └── audio.rs                  # NEW — spectral fingerprint + tempo/key
│
src/
├── components/
│   ├── muse/                     # NEW — MUSE frontend
│   │   ├── MuseDashboard.tsx     # Main MUSE view
│   │   ├── PackManager.tsx       # Create, manage, blend packs
│   │   ├── ProfileView.tsx       # Visualize creative DNA
│   │   ├── InfluencePreview.tsx  # Preview how context enriches a prompt
│   │   ├── DriftTimeline.tsx     # Creative evolution over time
│   │   └── GenerationFlow.tsx    # The generation → feedback loop UI
```

---

## 3. Database Schema Extensions

These tables extend the existing ACE database (`ace/db.rs`). Same `data/4da.db` file. Same migration system.

```sql
-- Context Packs
CREATE TABLE IF NOT EXISTS muse_packs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    pack_type TEXT NOT NULL DEFAULT 'custom',   -- 'custom', 'auto', 'imported', 'marketplace'
    is_active INTEGER NOT NULL DEFAULT 0,
    source_count INTEGER NOT NULL DEFAULT 0,
    confidence REAL NOT NULL DEFAULT 0.0,
    color_profile TEXT,                          -- JSON: dominant colors, temperature, contrast, saturation
    composition_profile TEXT,                    -- JSON: symmetry, negative_space, focal_point, depth
    style_embedding BLOB,                       -- 384-dim centroid of all source embeddings
    cluster_centers TEXT,                        -- JSON array of cluster center embeddings
    thematic_topics TEXT,                        -- JSON array of topic strings
    anti_patterns TEXT,                          -- JSON array of anti-pattern strings
    sonic_profile TEXT,                          -- JSON: tempo, key, timbre clusters (audio packs)
    motion_profile TEXT,                         -- JSON: pacing, transitions, energy (video packs)
    metadata TEXT,                               -- JSON: arbitrary pack metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Pack source files (what files contributed to this pack)
CREATE TABLE IF NOT EXISTS muse_pack_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pack_id TEXT NOT NULL REFERENCES muse_packs(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    file_type TEXT NOT NULL,                     -- 'image', 'video', 'audio', 'document', 'project_file'
    media_type TEXT,                             -- MIME type
    extraction_status TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'processing', 'done', 'failed'
    extracted_at TEXT,
    embedding BLOB,                             -- 384-dim individual file embedding
    color_data TEXT,                             -- JSON: per-file color analysis
    composition_data TEXT,                       -- JSON: per-file composition analysis
    spectral_data TEXT,                          -- JSON: per-file audio analysis
    confidence REAL NOT NULL DEFAULT 0.0,
    file_hash TEXT,                              -- SHA-256 for change detection
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_pack_sources_pack ON muse_pack_sources(pack_id);
CREATE INDEX idx_pack_sources_type ON muse_pack_sources(file_type);

-- Generation history (for feedback learning)
CREATE TABLE IF NOT EXISTS muse_generations (
    id TEXT PRIMARY KEY,
    pack_id TEXT REFERENCES muse_packs(id),
    provider TEXT NOT NULL,                      -- 'runway', 'midjourney', 'dalle', 'sora', 'udio', etc.
    prompt TEXT NOT NULL,                        -- Original user prompt
    enriched_prompt TEXT,                        -- After MUSE enrichment
    influence_payload TEXT,                      -- JSON: full context sent to provider
    outcome TEXT,                                -- 'kept', 'rejected', 'modified', 'unknown'
    outcome_signal REAL,                         -- -1.0 to 1.0 strength
    outcome_notes TEXT,                          -- Optional user annotation
    generation_params TEXT,                      -- JSON: provider-specific params used
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    outcome_at TEXT
);

CREATE INDEX idx_generations_pack ON muse_generations(pack_id);
CREATE INDEX idx_generations_outcome ON muse_generations(outcome);

-- Creative drift snapshots
CREATE TABLE IF NOT EXISTS muse_drift_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pack_id TEXT REFERENCES muse_packs(id),
    snapshot_type TEXT NOT NULL,                 -- 'color', 'composition', 'style', 'sonic', 'overall'
    metric_name TEXT NOT NULL,                   -- e.g., 'color_temperature', 'symmetry_score'
    metric_value REAL NOT NULL,
    embedding_snapshot BLOB,                    -- 384-dim style centroid at this point in time
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_drift_pack ON muse_drift_snapshots(pack_id, snapshot_type);
CREATE INDEX idx_drift_time ON muse_drift_snapshots(created_at);

-- Pack blends (composite packs)
CREATE TABLE IF NOT EXISTS muse_pack_blends (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    blend_pack_id TEXT NOT NULL REFERENCES muse_packs(id) ON DELETE CASCADE,
    source_pack_id TEXT NOT NULL REFERENCES muse_packs(id),
    weight REAL NOT NULL DEFAULT 0.5,           -- 0.0-1.0 weight of this source in the blend
    UNIQUE(blend_pack_id, source_pack_id)
);
```

---

## 4. Extractor Design

### 4.1 Image Extractor

The most critical extractor — ships first, must be excellent.

**Extraction Pipeline:**
```
Image file (PSD/PNG/TIFF/JPG/RAW)
    │
    ├──→ CLIP embedding (via Ollama multimodal model)
    │    └── 384-dim style vector capturing semantic content
    │
    ├──→ Color analysis (pure Rust, no external deps)
    │    ├── Dominant colors (k-means clustering on pixel data, k=5)
    │    ├── Color temperature (warm/cool ratio from HSL analysis)
    │    ├── Contrast ratio (luminance range / standard deviation)
    │    ├── Saturation distribution (mean + std dev)
    │    └── Palette harmony (complementary/analogous/triadic detection)
    │
    ├──→ Composition analysis (pure Rust)
    │    ├── Rule of thirds / golden ratio focal point detection (edge detection + saliency)
    │    ├── Symmetry score (mirror correlation along vertical/horizontal axes)
    │    ├── Negative space ratio (background vs. foreground segmentation)
    │    ├── Depth estimation (blur gradient analysis)
    │    └── Edge density (Sobel filter → busy vs. minimal)
    │
    └──→ Metadata extraction
         ├── EXIF data (camera, lens, settings — for photographers)
         ├── Color profile (sRGB, Adobe RGB, etc.)
         └── Layer/group names (PSD — reveals workflow vocabulary)
```

**Crate dependencies:**
- `image` (already in Cargo.toml) — pixel access, format decoding
- CLIP via Ollama's `/api/embed` with multimodal model (e.g., `llava`, `bakllava`)
- Color analysis: pure Rust using `image` crate pixel iterators
- Composition: pure Rust (Sobel kernels, correlation functions)
- PSD: `psd` crate for layer metadata (optional)

**Output:** `ImageExtraction` struct containing all signals, stored in `muse_pack_sources`.

### 4.2 Audio Extractor

**Extraction Pipeline:**
```
Audio file (WAV/FLAC/MP3/AIFF/stems)
    │
    ├──→ Spectral fingerprint
    │    ├── FFT → spectrogram
    │    ├── Mel-frequency cepstral coefficients (MFCCs) — timbre fingerprint
    │    ├── Spectral centroid over time — brightness trajectory
    │    └── Spectral rolloff — frequency distribution character
    │
    ├──→ Temporal analysis
    │    ├── Onset detection → tempo estimation (BPM)
    │    ├── Beat grid stability (quantized vs. human feel)
    │    ├── Dynamic range (loudness variation, compression detection)
    │    └── Stereo width analysis (correlation coefficient per frequency band)
    │
    ├──→ Harmonic analysis
    │    ├── Chromagram → key detection (Krumhansl-Schmuckler algorithm)
    │    ├── Chord progression patterns (basic: major/minor/dominant)
    │    └── Harmonic complexity score
    │
    └──→ Embedding
         └── Aggregate spectral features → 384-dim vector (custom projection or Ollama audio model when available)
```

**Crate dependencies:**
- `symphonia` — audio decoding (pure Rust, many formats)
- `rustfft` — FFT computation
- Custom MFCC/chromagram computation (standard DSP, implementable in Rust)

**Note:** Audio extraction is computationally heavier than image. Process in background with progress reporting.

### 4.3 Video Extractor

**Extraction Pipeline:**
```
Video file (MP4/MOV/ProRes/MKV)
    │
    ├──→ Keyframe extraction
    │    ├── Scene change detection (histogram difference between frames)
    │    ├── Extract 1 representative frame per scene
    │    └── Each keyframe → Image extractor pipeline (CLIP + color + composition)
    │
    ├──→ Temporal analysis
    │    ├── Cut frequency → pacing profile (cuts per minute over time)
    │    ├── Shot duration distribution (histogram: quick cuts vs. long takes)
    │    ├── Motion intensity (optical flow magnitude per scene)
    │    └── Transition detection (hard cut vs. dissolve vs. fade)
    │
    ├──→ Color trajectory
    │    ├── Color palette per scene (not per frame — too expensive)
    │    ├── Color consistency score (how stable is the palette across the video)
    │    └── Color arc (warm→cool transitions, brightness trajectory)
    │
    └──→ Embedding
         └── Average keyframe CLIP embeddings → 384-dim video style vector
```

**Crate dependencies:**
- `ffmpeg-next` or shell out to `ffmpeg` for keyframe extraction (ffmpeg is ubiquitous)
- Image analysis reuses the image extractor
- Optical flow: simplified block matching (pure Rust) or defer to ffmpeg's scene detection

**Note:** Video extraction is the heaviest operation. Phase 2 feature. Initial implementation: keyframes only (scene detection + CLIP per keyframe). Full motion analysis in Phase 3.

---

## 5. Embedding Pipeline Extensions

### CLIP Model Support

The existing `EmbeddingService` in `ace/embedding.rs` handles text embeddings via Ollama. For images, MUSE needs CLIP (Contrastive Language-Image Pre-training) embeddings.

**Approach:**
- Ollama supports multimodal models (`llava`, `bakllava`, `llama3.2-vision`)
- These models can generate embeddings for images via the same `/api/embed` endpoint
- The embedding dimension may differ from text models — normalize to 384-dim (same Matryoshka truncation)

**Fallback chain:**
1. Ollama multimodal model (local, private) — preferred
2. OpenAI CLIP endpoint (if user has key) — higher quality but sends data
3. Zero vector (384-dim) — graceful degradation, context still works from non-embedding signals

**Implementation:**
```rust
// In ace/embedding.rs — extend EmbeddingService
impl EmbeddingService {
    /// Generate embedding for an image file
    pub async fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>> {
        // Read image, base64 encode
        // Send to Ollama multimodal model
        // Truncate + normalize to 384-dim
        // Cache result
    }

    /// Generate embedding for audio features
    pub async fn embed_audio_features(&self, features: &AudioFeatures) -> Result<Vec<f32>> {
        // Serialize audio features to descriptive text
        // "Dark, minor key, 95 BPM, wide stereo, warm harmonics, sparse instrumentation"
        // Embed the text description (text embedding captures semantic meaning)
        // This is a bridge until dedicated audio embedding models are available
    }
}
```

### Audio Embedding Strategy

No mainstream local audio embedding model exists yet. Bridge approach:
1. Extract spectral/temporal/harmonic features (numbers)
2. Convert to natural language description
3. Embed the description using text embedding model
4. This works because MUSE's cross-modal correlations happen in the same embedding space

When dedicated audio embedding models become available via Ollama, swap in directly.

---

## 6. Context Pack Lifecycle

### Creation Flow

```
User selects files/folder
        │
        ▼
ExtractorRegistry routes each file to appropriate extractor
        │
        ├── .png/.jpg/.psd/.tiff → ImageExtractor
        ├── .mp4/.mov → VideoExtractor (keyframes → ImageExtractor)
        ├── .wav/.flac/.mp3 → AudioExtractor
        ├── .pdf → PdfExtractor (existing)
        └── .docx/.xlsx → OfficeExtractor (existing)
        │
        ▼
Each extraction produces:
  - File embedding (384-dim)
  - Media-specific signals (color, composition, spectral, etc.)
  - Confidence score
        │
        ▼
Pack aggregation:
  - Compute centroid embedding (weighted average of file embeddings)
  - Compute cluster centers (k-means, k=3-5 depending on source count)
  - Aggregate color profiles (dominant colors across all sources)
  - Aggregate composition profiles (central tendencies)
  - Extract thematic topics (from embeddings → nearest topic labels)
  - Infer anti-patterns (from outlier detection — what's conspicuously absent)
        │
        ▼
Store in muse_packs + muse_pack_sources
        │
        ▼
Pack available for activation + enrichment
```

### Update Flow (incremental)

When files change or new files are added:
1. Hash-check against `muse_pack_sources.file_hash`
2. Re-extract only changed/new files
3. Recompute pack aggregates (centroid, clusters, profiles)
4. Record drift snapshot before updating

### Blend Flow

```
Pack A (weight 0.6) + Pack B (weight 0.4)
        │
        ▼
Weighted average of:
  - Style embeddings (0.6*A_centroid + 0.4*B_centroid, re-normalize)
  - Color profiles (weighted merge of dominant colors)
  - Composition profiles (weighted average of scores)
  - Thematic topics (union, with weights)
  - Anti-patterns (union — if either pack rejects it, the blend rejects it)
        │
        ▼
New "blend" pack stored with pack_type='blend'
References to source packs in muse_pack_blends
```

---

## 7. Influence Formatting

The influence formatter transforms MUSE context into provider-consumable formats.

### Format: Text Enrichment (Layer 0)

```rust
pub fn format_text_enrichment(pack: &MusePack, original_prompt: &str) -> String {
    let mut parts = Vec::new();

    // Style guidance from composition + color
    if let Some(color) = &pack.color_profile {
        parts.push(format!("Color palette: {}", color.describe()));
    }
    if let Some(comp) = &pack.composition_profile {
        parts.push(format!("Composition: {}", comp.describe()));
    }

    // Thematic context
    if !pack.thematic_topics.is_empty() {
        parts.push(format!("Themes: {}", pack.thematic_topics.join(", ")));
    }

    // Anti-patterns (critical — what to avoid)
    if !pack.anti_patterns.is_empty() {
        parts.push(format!("Avoid: {}", pack.anti_patterns.join(", ")));
    }

    format!("Style context: {}.\n\n{}", parts.join(". "), original_prompt)
}
```

### Format: MUSE Context Protocol (Layer 1)

Structured JSON per the protocol spec. See `MUSE-CONTEXT-PROTOCOL.md`.

### Format: Provider-Specific (Layer 2)

Each partner gets an adapter:
```rust
pub trait ProviderAdapter {
    fn format_context(&self, pack: &MusePack) -> ProviderPayload;
    fn parse_feedback(&self, response: &ProviderResponse) -> GenerationOutcome;
}

// Implementations per partner
pub struct RunwayAdapter;
pub struct ReplicateAdapter;
pub struct ComfyUIAdapter;
```

---

## 8. MCP Integration

### New MUSE MCP Tools

```
muse_create_pack        — Create a context pack from files/folders
muse_list_packs         — List all packs with status and confidence
muse_activate_pack      — Set pack(s) as active context
muse_get_profile        — Get the creative DNA profile for a pack
muse_enrich_prompt      — Enrich a generation prompt with active pack context
muse_blend_packs        — Create a weighted blend of multiple packs
muse_record_feedback    — Record generation outcome (kept/rejected)
muse_creative_drift     — Get drift analysis for a pack over time
muse_export_profile     — Export pack as portable MUSE profile
muse_cross_modal        — Get cross-modal correlations between packs
```

These tools follow the same pattern as existing 4DA MCP tools — registered in `mcp-4da-server/`, backed by Rust functions.

### Integration with Existing Tools

| Existing Tool | MUSE Enhancement |
|---|---|
| `get_context` | Include active MUSE pack summary in context response |
| `developer_dna` | Add creative dimensions if MUSE packs exist |
| `compound_advantage` | Include MUSE profile richness in compound score |
| `what_should_i_know` | Surface creative drift alerts alongside dev briefing |

---

## 9. Frontend Components

### MuseDashboard

The main MUSE view, accessible from 4DA's primary navigation.

**Sections:**
1. **Active Packs** — currently active context packs with confidence indicators
2. **Quick Generate** — prompt input with real-time enrichment preview
3. **Creative DNA** — visual summary of overall creative profile
4. **Recent Generations** — history with outcome tracking (kept/rejected)
5. **Drift Alerts** — "your palette has shifted warmer this month"

### PackManager

Create and manage context packs.

**Features:**
- Drag-and-drop file import (or folder picker)
- Real-time extraction progress (per-file status)
- Pack visualization (color swatches, composition diagrams, topic clouds)
- Blend controls (sliders for weight adjustment)
- Source file list with individual contribution indicators

### ProfileView

Visualize what MUSE understands about your creative identity.

**Visualizations:**
- Color palette wheel (dominant colors positioned by temperature/saturation)
- Composition heatmap (where visual weight tends to concentrate)
- Style embedding 2D projection (t-SNE/UMAP of file embeddings, showing clusters)
- Topic affinity radar chart (themes ranked by strength)
- Anti-pattern list (what MUSE learned to avoid)

### InfluencePreview

Real-time preview of how MUSE enriches a prompt.

**UX:**
- Left panel: original prompt (user types)
- Right panel: enriched prompt (updates live as user types)
- Highlight diff: show exactly what MUSE added
- Pack selector: switch active pack to see how enrichment changes
- Confidence indicator: how confident MUSE is in its enrichment

---

## 10. Performance Considerations

| Operation | Expected Time | Strategy |
|---|---|---|
| Image extraction (CLIP + color + composition) | 2-5s per image | Background queue, batch Ollama calls |
| Audio extraction (spectral + tempo + key) | 5-15s per track | Background, show progress |
| Video keyframe extraction | 10-30s per minute of video | Background, ffmpeg scene detection |
| Pack aggregation (50 files) | 1-3s | After all extractions complete |
| Prompt enrichment | <50ms | Pre-computed, cache active pack context |
| Drift computation | 100-500ms | On-demand from snapshots table |

**Memory management:**
- CLIP model loaded on first MUSE use, unloaded after 10min idle
- Audio FFT buffers allocated per-file, freed after extraction
- Video keyframes extracted to temp directory, cleaned after processing
- Pack embeddings cached in memory for active packs only

---

## 11. Privacy Architecture

MUSE inherits 4DA's privacy guarantees and adds creative-specific protections.

### What STAYS local (always)

- Source files (images, audio, video — NEVER uploaded)
- Raw extraction data (pixel values, spectral data, waveforms)
- Pack source file paths
- Generation history (prompts, outcomes)

### What CAN leave (only with explicit user action)

- MUSE profile export (embeddings + weights, NO source data)
- Enriched prompts to generation APIs (text only, controlled by user)
- Marketplace listings (anonymized embeddings, no file references)

### Mathematical guarantee

MUSE profiles contain 384-dimensional embedding vectors and statistical aggregates (means, standard deviations, cluster centers). Reconstructing source images/audio from these representations is mathematically infeasible — it's a many-to-one mapping with astronomical information loss.

### Transparency panel

The MUSE UI includes a "What MUSE Knows" panel showing exactly:
- Which files were processed
- What signals were extracted (in human-readable form)
- What would be included in an export
- A clear "Delete All MUSE Data" button

---

## 12. Migration Path

### Phase 0 Prep (during 4DA v1 development)

Minimal changes to existing code:

1. **ExtractorRegistry** — ensure the trait is generic enough for creative types (it already is)
2. **Database migration system** — ensure it can add MUSE tables without disrupting ACE tables
3. **Embedding service** — add `embed_image` stub that returns zero vector (graceful degradation)
4. **MCP server** — reserve `muse_*` tool namespace
5. **Frontend routing** — add `/muse` route placeholder

### Phase 1 Implementation

1. Add MUSE database tables (migration)
2. Implement image extractor (CLIP + color + composition)
3. Implement pack lifecycle (create, update, activate)
4. Implement text enrichment formatter
5. Build frontend (PackManager, ProfileView, InfluencePreview)
6. Add MCP tools
7. Wire generation feedback loop

### Phase 2 Expansion

1. Audio extractor
2. Video extractor (keyframes)
3. Cross-modal engine
4. Drift detection
5. Pack blending
6. Provider adapters (partners)

This architecture ensures MUSE is deeply integrated with 4DA's existing systems while maintaining clean separation of concerns. Every component is independently testable, and the phased approach means partial MUSE is still valuable.
