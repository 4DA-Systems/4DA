# MUSE Context Protocol — Specification

> An open standard for communicating creative context to AI generation providers.

**Status:** Draft | **Version:** 0.1.0 | **Date:** 2026-03-26
**License:** Apache 2.0 (protocol specification and reference implementations)

---

## 1. Purpose

The MUSE Context Protocol (MCP — not to be confused with Model Context Protocol) defines a structured format for describing a creator's aesthetic identity, preferences, and anti-patterns in a way that AI generation systems can consume to produce more aligned outputs.

**Note on naming:** To avoid confusion with Anthropic's Model Context Protocol, this specification uses "MUSE Protocol" or "MCP-C" (Context) in shorthand. The full name is always "MUSE Context Protocol."

### Goals

1. **Provider-agnostic** — any generation system (image, video, audio, text-to-X) can consume this format
2. **Privacy-preserving** — the protocol transmits derived signals, never source material
3. **Composable** — profiles can be blended, layered, and filtered
4. **Versionable** — profiles evolve over time; the protocol supports snapshots and drift
5. **Lightweight** — a full creative profile fits in <10KB JSON

### Non-Goals

- Prescribing how providers should USE the context (that's their implementation choice)
- Replacing provider-specific style reference systems (this complements them)
- Transmitting images, audio, or any source media

---

## 2. Protocol Overview

A MUSE Context payload is a JSON document containing:

```
MuseContext
├── meta           — protocol version, profile ID, timestamps
├── visual         — color, composition, style signals (image/video creators)
├── sonic          — timbre, rhythm, harmonic signals (audio creators)
├── motion         — pacing, transition, energy signals (video creators)
├── semantic       — thematic topics, anti-patterns, domain vocabulary
├── embeddings     — style vectors for semantic matching
└── confidence     — overall profile strength and per-signal confidence
```

Providers consume whichever sections are relevant to their modality. An image generator reads `visual` + `semantic`. A music generator reads `sonic` + `semantic`. A video generator reads all four.

---

## 3. Schema Definition

### 3.1 Root Object

```json
{
  "muse_protocol": "0.1.0",
  "profile_id": "uuid-v4",
  "profile_name": "Meridian Album Art",
  "created_at": "2026-03-15T10:30:00Z",
  "updated_at": "2026-03-26T14:22:00Z",
  "source_count": 47,
  "visual": { ... },
  "sonic": { ... },
  "motion": { ... },
  "semantic": { ... },
  "embeddings": { ... },
  "confidence": { ... }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `muse_protocol` | string | Yes | Protocol version (semver) |
| `profile_id` | string | Yes | Unique profile identifier (UUID v4) |
| `profile_name` | string | Yes | Human-readable pack/profile name |
| `created_at` | string (ISO 8601) | Yes | Initial creation timestamp |
| `updated_at` | string (ISO 8601) | Yes | Last update timestamp |
| `source_count` | integer | Yes | Number of source files that contributed |
| `visual` | object | No | Visual signals (see 3.2) |
| `sonic` | object | No | Audio signals (see 3.3) |
| `motion` | object | No | Motion/video signals (see 3.4) |
| `semantic` | object | Yes | Thematic/conceptual signals (see 3.5) |
| `embeddings` | object | No | Style vectors (see 3.6) |
| `confidence` | object | Yes | Confidence scores (see 3.7) |

### 3.2 Visual Signals

```json
{
  "visual": {
    "color": {
      "dominant": [
        { "hex": "#D4AF37", "weight": 0.35 },
        { "hex": "#1A1A1A", "weight": 0.30 },
        { "hex": "#2D1B69", "weight": 0.20 },
        { "hex": "#FFFFFF", "weight": 0.10 },
        { "hex": "#8B4513", "weight": 0.05 }
      ],
      "temperature": 0.72,
      "contrast": 0.89,
      "saturation": 0.45,
      "harmony": "complementary"
    },
    "composition": {
      "symmetry": 0.30,
      "negative_space": 0.70,
      "focal_point": "golden_ratio",
      "depth": 0.80,
      "edge_density": 0.35
    },
    "texture": {
      "grain": 0.65,
      "smoothness": 0.20,
      "organic_vs_geometric": 0.75
    }
  }
}
```

**Color fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `dominant` | array of {hex, weight} | weights sum to 1.0 | Top colors by prevalence, ordered by weight |
| `temperature` | float | 0.0 (cool) to 1.0 (warm) | Overall color temperature tendency |
| `contrast` | float | 0.0 (flat) to 1.0 (high contrast) | Luminance range characteristic |
| `saturation` | float | 0.0 (monochrome) to 1.0 (vivid) | Overall saturation level |
| `harmony` | string | enum | Detected palette relationship: `complementary`, `analogous`, `triadic`, `split_complementary`, `monochromatic`, `mixed` |

**Composition fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `symmetry` | float | 0.0 (asymmetric) to 1.0 (symmetric) | Bilateral symmetry tendency |
| `negative_space` | float | 0.0 (dense) to 1.0 (spacious) | Background-to-foreground ratio |
| `focal_point` | string | enum | Dominant focal positioning: `center`, `rule_of_thirds`, `golden_ratio`, `edge`, `distributed`, `none` |
| `depth` | float | 0.0 (flat) to 1.0 (deep) | Perceived depth/layering |
| `edge_density` | float | 0.0 (minimal) to 1.0 (busy) | Visual complexity |

**Texture fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `grain` | float | 0.0 to 1.0 | Film grain / noise tendency |
| `smoothness` | float | 0.0 to 1.0 | Clean/polished tendency |
| `organic_vs_geometric` | float | 0.0 (geometric) to 1.0 (organic) | Shape language tendency |

### 3.3 Sonic Signals

```json
{
  "sonic": {
    "timbre": {
      "brightness": 0.45,
      "warmth": 0.70,
      "density": 0.55,
      "texture": "analog_warm"
    },
    "rhythm": {
      "tempo_range": [85, 110],
      "tempo_center": 95,
      "grid_strictness": 0.40,
      "syncopation": 0.60
    },
    "harmony": {
      "key_affinities": ["Dm", "Am", "F", "Gm"],
      "mode_preference": "minor",
      "harmonic_complexity": 0.55,
      "dissonance_tolerance": 0.40
    },
    "production": {
      "stereo_width": 0.80,
      "dynamic_range": 0.65,
      "low_end_emphasis": 0.70,
      "reverb_amount": 0.55,
      "clarity": 0.60
    }
  }
}
```

**Timbre fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `brightness` | float | 0.0 (dark) to 1.0 (bright) | Spectral centroid tendency |
| `warmth` | float | 0.0 (cold/digital) to 1.0 (warm/analog) | Low-mid frequency emphasis |
| `density` | float | 0.0 (sparse) to 1.0 (dense) | Spectral occupancy |
| `texture` | string | descriptive | Timbral character label |

**Rhythm fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `tempo_range` | [float, float] | BPM | Min and max tempo across body of work |
| `tempo_center` | float | BPM | Most common tempo |
| `grid_strictness` | float | 0.0 (free) to 1.0 (quantized) | How rigidly beats align to grid |
| `syncopation` | float | 0.0 (on-beat) to 1.0 (syncopated) | Off-beat emphasis tendency |

**Harmony fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `key_affinities` | string[] | Musical keys | Most frequently used keys |
| `mode_preference` | string | enum | `major`, `minor`, `modal`, `atonal`, `mixed` |
| `harmonic_complexity` | float | 0.0 (simple) to 1.0 (complex) | Chord vocabulary richness |
| `dissonance_tolerance` | float | 0.0 (consonant) to 1.0 (dissonant) | Tension level preference |

**Production fields:**

| Field | Type | Range | Description |
|---|---|---|---|
| `stereo_width` | float | 0.0 (mono) to 1.0 (wide) | Stereo image characteristic |
| `dynamic_range` | float | 0.0 (compressed) to 1.0 (dynamic) | Loudness variation |
| `low_end_emphasis` | float | 0.0 (thin) to 1.0 (bass-heavy) | Sub/bass frequency prominence |
| `reverb_amount` | float | 0.0 (dry) to 1.0 (wet) | Spatial effect amount |
| `clarity` | float | 0.0 (muddy) to 1.0 (pristine) | Mix definition |

### 3.4 Motion Signals

```json
{
  "motion": {
    "pacing": {
      "cuts_per_minute": 8.5,
      "shot_duration_median": 4.2,
      "shot_duration_variance": 0.65,
      "energy_arc": "build_to_climax"
    },
    "transitions": {
      "hard_cut_ratio": 0.70,
      "dissolve_ratio": 0.20,
      "fade_ratio": 0.05,
      "other_ratio": 0.05,
      "preferred_style": "hard_cut"
    },
    "camera": {
      "movement_intensity": 0.40,
      "stability": 0.75,
      "preferred_angles": ["eye_level", "low_angle"],
      "zoom_tendency": 0.20
    }
  }
}
```

### 3.5 Semantic Signals

```json
{
  "semantic": {
    "topics": [
      { "label": "organic forms", "weight": 0.85 },
      { "label": "nocturnal", "weight": 0.72 },
      { "label": "ethereal", "weight": 0.68 },
      { "label": "texture", "weight": 0.65 },
      { "label": "grain", "weight": 0.60 }
    ],
    "anti_patterns": [
      { "label": "corporate", "strength": 0.95 },
      { "label": "flat illustration", "strength": 0.88 },
      { "label": "oversaturated", "strength": 0.82 },
      { "label": "centered composition", "strength": 0.70 }
    ],
    "domain": "album_art",
    "vocabulary": ["warp", "drift", "echo", "void", "pulse"]
  }
}
```

| Field | Type | Description |
|---|---|---|
| `topics` | array of {label, weight} | Thematic concepts present in the work, ordered by weight |
| `anti_patterns` | array of {label, strength} | Explicitly undesired characteristics, ordered by strength |
| `domain` | string | Creative domain label (freeform, for provider context) |
| `vocabulary` | string[] | Recurring words/concepts from the creator's work (naming patterns, project titles) |

### 3.6 Embeddings

```json
{
  "embeddings": {
    "style_centroid": "base64-encoded-384-dim-float32-vector",
    "cluster_centers": [
      "base64-encoded-384-dim-float32-vector",
      "base64-encoded-384-dim-float32-vector",
      "base64-encoded-384-dim-float32-vector"
    ],
    "dimension": 384,
    "model": "nomic-embed-text",
    "generated_at": "2026-03-26T14:22:00Z"
  }
}
```

| Field | Type | Description |
|---|---|---|
| `style_centroid` | string (base64) | Weighted average embedding of all source files |
| `cluster_centers` | string[] (base64) | K-means cluster centers (captures style diversity within the pack) |
| `dimension` | integer | Embedding dimensionality (384 for nomic-embed-text) |
| `model` | string | Model used to generate embeddings (for compatibility checking) |
| `generated_at` | string (ISO 8601) | When embeddings were computed |

**Encoding:** Embeddings are float32 arrays encoded as base64. To decode:
```python
import base64, numpy as np
vec = np.frombuffer(base64.b64decode(encoded), dtype=np.float32)
```

### 3.7 Confidence

```json
{
  "confidence": {
    "overall": 0.87,
    "visual": 0.92,
    "sonic": 0.78,
    "motion": 0.45,
    "semantic": 0.90,
    "source_diversity": 0.75,
    "temporal_depth_days": 45,
    "feedback_loops": 127
  }
}
```

| Field | Type | Range | Description |
|---|---|---|---|
| `overall` | float | 0.0-1.0 | Aggregate confidence in the profile |
| `visual` | float | 0.0-1.0 | Confidence in visual signals |
| `sonic` | float | 0.0-1.0 | Confidence in audio signals |
| `motion` | float | 0.0-1.0 | Confidence in motion signals |
| `semantic` | float | 0.0-1.0 | Confidence in thematic signals |
| `source_diversity` | float | 0.0-1.0 | How diverse the source material is (many different works vs. variations on one piece) |
| `temporal_depth_days` | integer | 0+ | How many days of creative work the profile spans |
| `feedback_loops` | integer | 0+ | Number of generation→feedback cycles completed |

Providers should weight their use of MUSE signals by confidence. A profile with `overall: 0.3` should have less influence than one with `overall: 0.9`.

---

## 4. Usage Patterns

### 4.1 Text Enrichment (any provider)

The simplest integration. Convert MUSE context to natural language prepended to the user's prompt.

```python
def enrich_prompt(muse_context: dict, user_prompt: str) -> str:
    parts = []

    if visual := muse_context.get("visual"):
        color = visual.get("color", {})
        if dominant := color.get("dominant"):
            hex_list = ", ".join(c["hex"] for c in dominant[:3])
            parts.append(f"Color palette: {hex_list}")
        if (temp := color.get("temperature")) is not None:
            parts.append(f"Color temperature: {'warm' if temp > 0.6 else 'cool' if temp < 0.4 else 'neutral'}")
        if (contrast := color.get("contrast")) is not None:
            parts.append(f"Contrast: {'high' if contrast > 0.7 else 'low' if contrast < 0.3 else 'moderate'}")

    if semantic := muse_context.get("semantic"):
        if topics := semantic.get("topics"):
            topic_labels = ", ".join(t["label"] for t in topics[:5])
            parts.append(f"Themes: {topic_labels}")
        if anti := semantic.get("anti_patterns"):
            anti_labels = ", ".join(a["label"] for a in anti[:4])
            parts.append(f"Avoid: {anti_labels}")

    style_guide = ". ".join(parts) + "." if parts else ""
    return f"Style context: {style_guide}\n\n{user_prompt}" if style_guide else user_prompt
```

### 4.2 Parameter Influence (native integration)

Providers with native support can map MUSE signals directly to generation parameters:

```python
# Example: mapping MUSE visual signals to Runway Gen-3 parameters
def muse_to_runway_params(muse_context: dict) -> dict:
    params = {}
    if visual := muse_context.get("visual"):
        color = visual.get("color", {})
        # Map temperature to Runway's style_preset
        if (temp := color.get("temperature")) is not None:
            if temp > 0.7:
                params["color_grade"] = "warm"
            elif temp < 0.3:
                params["color_grade"] = "cool"

    if motion := muse_context.get("motion"):
        pacing = motion.get("pacing", {})
        if energy := pacing.get("energy_arc"):
            params["motion_amount"] = {
                "build_to_climax": "moderate",
                "high_energy": "high",
                "contemplative": "low"
            }.get(energy, "moderate")

    return params
```

### 4.3 Embedding Similarity (advanced)

Providers with their own embedding space can compute similarity between MUSE embeddings and their style references:

```python
import numpy as np

def find_closest_style(muse_centroid: np.ndarray, provider_styles: dict) -> str:
    """Find the provider's built-in style closest to the MUSE centroid."""
    best_style = None
    best_sim = -1
    for name, embedding in provider_styles.items():
        sim = np.dot(muse_centroid, embedding) / (
            np.linalg.norm(muse_centroid) * np.linalg.norm(embedding)
        )
        if sim > best_sim:
            best_sim = sim
            best_style = name
    return best_style
```

---

## 5. Blending

Multiple MUSE profiles can be blended with custom weights:

```json
{
  "blend": {
    "sources": [
      { "profile_id": "abc-123", "weight": 0.6 },
      { "profile_id": "def-456", "weight": 0.4 }
    ]
  }
}
```

**Blending rules:**
- Numeric fields: weighted average
- Color arrays: merge and re-weight
- Topic arrays: union with weight adjustment
- Anti-patterns: union (conservative — if either profile rejects it, the blend rejects it)
- Embeddings: weighted average, re-normalized to unit length
- Confidence: minimum of source confidences × blend coherence factor

---

## 6. Profile Export & Marketplace Format

For marketplace distribution, profiles are exported in a stripped format:

```json
{
  "muse_protocol": "0.1.0",
  "export_format": "marketplace",
  "listing": {
    "title": "Neon Noir",
    "creator": "cybervoid",
    "description": "High-contrast cyberpunk aesthetic...",
    "tags": ["cyberpunk", "neon", "dark", "cinematic"],
    "modalities": ["image", "video"],
    "signal_strength": 0.94,
    "source_count": 2400,
    "build_duration_days": 240,
    "feedback_loops": 1847
  },
  "visual": { ... },
  "sonic": { ... },
  "motion": { ... },
  "semantic": { ... },
  "embeddings": { ... },
  "confidence": { ... }
}
```

**What's stripped:**
- `profile_id` (replaced with marketplace listing ID)
- File paths, timestamps, and any personally identifiable metadata
- Individual source file data (only aggregated signals)

**Provenance indicators (anti-fraud):**
- `source_count` — profiles from very few files are suspicious
- `build_duration_days` — profiles built over months are more authentic than overnight creations
- `feedback_loops` — high count indicates real usage, not synthetic generation

---

## 7. Versioning & Compatibility

### Protocol Versioning

The protocol follows semantic versioning:
- **Major** (1.0 → 2.0): Breaking changes to required fields
- **Minor** (1.0 → 1.1): New optional fields or sections
- **Patch** (1.0.0 → 1.0.1): Clarifications, documentation changes

Providers should check `muse_protocol` and handle unknown fields gracefully (ignore, don't error).

### Profile Versioning

Profiles evolve as the creator adds files and provides feedback. The `updated_at` timestamp indicates freshness. Providers may cache profiles but should respect `updated_at` for cache invalidation.

### Embedding Compatibility

Different embedding models produce different vector spaces. The `embeddings.model` field allows providers to:
1. Use the embeddings directly if they support the same model
2. Ignore embeddings and use only structured signals if the model is unknown
3. Request re-embedding in their preferred model space (future API extension)

---

## 8. Security Considerations

- **No source material transmission.** The protocol explicitly excludes file content, image data, audio samples, or any reconstructible media.
- **Embedding irreversibility.** 384-dim embeddings cannot be reversed to reconstruct source material. The information loss is fundamental, not obscured.
- **Profile authentication.** Marketplace profiles should be signed by the creating 4DA instance. Verification prevents tampering during distribution.
- **Prompt injection.** Text enrichment must sanitize MUSE context to prevent injection into generation prompts. Topics and anti-patterns should be treated as data, not executable instructions.

---

## 9. Reference Implementations

### Planned Implementations

| Language | Package | Status | Notes |
|---|---|---|---|
| Python | `muse-context` | Planned | Primary — most generation APIs are Python |
| JavaScript/TypeScript | `@4da/muse-context` | Planned | Web services, ComfyUI extensions |
| Rust | `muse-context` | Planned | 4DA native, high-performance applications |

### Minimum Viable Implementation

A provider can support MUSE with just text enrichment (Section 4.1) — approximately 30 lines of Python. The barrier to integration is intentionally near-zero.

---

## 10. Specification Governance

This specification is maintained by 4DA Systems Pty Ltd and published under Apache 2.0.

Community contributions are welcome via the specification's GitHub repository. Breaking changes require an RFC process with a minimum 30-day comment period.

The specification is authoritative. Implementations that diverge from the specification are non-conforming. "MUSE-compatible" or "MUSE-enabled" labels require passing the conformance test suite (included in reference implementations).
