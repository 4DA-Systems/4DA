# 4DA Context Engine Architecture

> **Implicit capture removed (noted 2026-08-17, v20b / AD-031).** The "Learned
> Behavior" layer this document designs — `topic_affinities`, `anti_topics`,
> `source_preferences`, and the `Ignore` (scrolled-past) signal — was removed;
> only explicit engagement capture remains. The layer's scoring authority had
> already been removed in v19 (AD-029). This document stands as the historical
> design record.

## The Vision

The Context Engine is the brain of 4DA. It transforms generic information filtering into deeply personalized relevance by understanding **who you are**, **what you're working on**, and **what patterns emerge from your behavior**.

Current state: Static markdown files in a folder.
Target state: A living, learning interest model that adapts to you.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      CONTEXT MEMBRANE                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    STATIC    │  │    ACTIVE    │  │   LEARNED    │          │
│  │   IDENTITY   │  │   CONTEXT    │  │   BEHAVIOR   │          │
│  │   (Layer 1)  │  │   (Layer 2)  │  │   (Layer 3)  │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                    │
│         └────────────┬────┴────────────────┘                    │
│                      ▼                                           │
│         ┌────────────────────────────────┐                      │
│         │    UNIFIED INTEREST MODEL      │                      │
│         │  (Weighted Vector Aggregation) │                      │
│         └────────────────────────────────┘                      │
│                      │                                           │
│                      ▼                                           │
│         ┌────────────────────────────────┐                      │
│         │    RELEVANCE SCORING ENGINE    │                      │
│         └────────────────────────────────┘                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Static Identity

**Purpose:** Explicit, user-declared interests and preferences.

**Data Sources:**
- Onboarding questionnaire (job role, tech stack, domains)
- Explicit topic declarations ("I care about: Rust, AI, databases")
- Explicit exclusions ("I never want to see: crypto, sports")
- Imported interests (GitHub stars, Twitter follows, Pocket saves)

**Implementation:**
```rust
struct StaticIdentity {
    // Core identity
    role: Option<String>,           // "Backend Developer", "Data Scientist"
    tech_stack: Vec<String>,        // ["Rust", "TypeScript", "PostgreSQL"]
    domains: Vec<String>,           // ["distributed systems", "ML infra"]

    // Explicit interests (user-declared topics)
    interests: Vec<Interest>,       // Topics with confidence 1.0

    // Explicit exclusions (never show these)
    exclusions: Vec<String>,        // ["cryptocurrency", "NFT", "sports"]

    // Imported from external sources
    github_languages: Vec<String>,  // From starred repos
    github_topics: Vec<String>,     // From starred repos
}

struct Interest {
    topic: String,
    embedding: Vec<f32>,            // Pre-computed embedding
    weight: f32,                    // 1.0 for explicit, lower for inferred
    source: InterestSource,         // Explicit, GitHub, Twitter, etc.
}
```

**UI Requirements:**
- Onboarding flow for first-time setup
- Settings panel for editing interests
- GitHub OAuth for importing stars (optional)
- Simple text input for quick topic adding

---

## Layer 2: Active Context

**Purpose:** Real-time awareness of what the user is currently working on.

**Data Sources:**
- Watched directories (code projects)
- Recent file modifications (last 7 days)
- Git commit messages and branch names
- Currently open project (if detectable)

**Implementation:**
```rust
struct ActiveContext {
    // Watched directories
    watched_dirs: Vec<WatchedDirectory>,

    // Recent activity
    recent_files: Vec<RecentFile>,
    recent_commits: Vec<CommitInfo>,

    // Current project detection
    current_project: Option<ProjectContext>,

    // Derived topics from active work
    active_topics: Vec<TopicWeight>,
}

struct WatchedDirectory {
    path: PathBuf,
    file_types: Vec<String>,        // ["rs", "ts", "md"]
    last_indexed: DateTime<Utc>,
    chunk_count: usize,
}

struct RecentFile {
    path: PathBuf,
    modified_at: DateTime<Utc>,
    content_hash: String,
    extracted_topics: Vec<String>,  // Keywords, imports, etc.
}

struct ProjectContext {
    name: String,
    root_path: PathBuf,
    detected_stack: Vec<String>,    // From package.json, Cargo.toml, etc.
    readme_summary: Option<String>, // Extracted from README
}
```

**Features:**
- File system watcher for real-time updates
- Git integration for commit analysis
- Project detection (package.json, Cargo.toml, pyproject.toml)
- Automatic topic extraction from code (imports, dependencies)

---

## Layer 3: Learned Behavior

**Purpose:** Implicit preferences learned from user actions over time.

**Data Sources:**
- Item clicks (opened in browser)
- Item saves (bookmarked)
- Item dismissals (marked not relevant)
- Dwell time (how long item stayed visible)
- Search queries within 4DA

**Implementation:**
```rust
struct LearnedBehavior {
    // Interaction history
    interactions: Vec<Interaction>,

    // Aggregated topic scores
    topic_affinities: HashMap<String, TopicAffinity>,

    // Anti-topics (consistently rejected)
    anti_topics: Vec<String>,

    // Source preferences
    source_preferences: HashMap<String, f32>,  // "reddit" -> 0.8
}

struct Interaction {
    item_id: i64,
    action: InteractionType,
    timestamp: DateTime<Utc>,

    // Context at time of interaction
    item_embedding: Vec<f32>,
    item_topics: Vec<String>,
}

enum InteractionType {
    Click,                          // Opened the item
    Save,                           // Bookmarked
    Dismiss,                        // Marked not relevant
    Ignore,                         // Scrolled past without action
}

struct TopicAffinity {
    topic: String,
    positive_signals: u32,          // Clicks, saves
    negative_signals: u32,          // Dismisses
    total_exposures: u32,           // Times shown
    last_interaction: DateTime<Utc>,

    // Computed score with decay
    affinity_score: f32,            // -1.0 to 1.0
}
```

**Learning Algorithm:**
```
affinity_score = (positive - negative) / total_exposures
                 * temporal_decay(last_interaction)
                 * confidence_factor(total_exposures)

Where:
- temporal_decay halves every 30 days
- confidence_factor increases with more data (min 10 interactions)
```

---

## Unified Interest Model

**Purpose:** Combine all context layers into a single relevance scoring function.

**Algorithm:**
```rust
fn compute_relevance(item: &SourceItem, context: &ContextMembrane) -> f32 {
    let item_embedding = embed(item);

    // Layer 1: Static identity match
    let static_score = context.static_identity
        .interests
        .iter()
        .map(|i| cosine_similarity(&item_embedding, &i.embedding) * i.weight)
        .max()
        .unwrap_or(0.0);

    // Check exclusions (hard filter)
    if context.static_identity.exclusions.iter().any(|e| item.contains_topic(e)) {
        return 0.0;
    }

    // Layer 2: Active context match
    let active_score = context.active_context
        .active_topics
        .iter()
        .map(|t| topic_similarity(&item, &t.topic) * t.weight)
        .max()
        .unwrap_or(0.0);

    // Layer 3: Learned behavior match
    let learned_score = context.learned_behavior
        .topic_affinities
        .iter()
        .filter(|(_, a)| a.affinity_score > 0.0)
        .map(|(topic, a)| topic_similarity(&item, topic) * a.affinity_score)
        .sum::<f32>();

    // Anti-topic penalty
    let anti_penalty = context.learned_behavior
        .anti_topics
        .iter()
        .filter(|t| item.contains_topic(t))
        .count() as f32 * 0.3;

    // Weighted combination
    let combined = static_score * 0.4      // Explicit intent
                 + active_score * 0.35     // Current work
                 + learned_score * 0.25    // Behavioral signal
                 - anti_penalty;

    combined.clamp(0.0, 1.0)
}
```

---

## Database Schema Extensions

```sql
-- User identity and explicit interests
CREATE TABLE user_identity (
    id INTEGER PRIMARY KEY,
    role TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE explicit_interests (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    weight REAL DEFAULT 1.0,
    embedding BLOB,
    source TEXT DEFAULT 'explicit',  -- 'explicit', 'github', 'import'
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE exclusions (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT (datetime('now'))
);

-- Watched directories
CREATE TABLE watched_directories (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    enabled INTEGER DEFAULT 1,
    last_indexed TEXT,
    chunk_count INTEGER DEFAULT 0
);

-- Interactions for learning
CREATE TABLE interactions (
    id INTEGER PRIMARY KEY,
    source_item_id INTEGER NOT NULL,
    action TEXT NOT NULL,  -- 'click', 'save', 'dismiss', 'ignore'
    timestamp TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (source_item_id) REFERENCES source_items(id)
);

-- Learned topic affinities
CREATE TABLE topic_affinities (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL UNIQUE,
    positive_signals INTEGER DEFAULT 0,
    negative_signals INTEGER DEFAULT 0,
    total_exposures INTEGER DEFAULT 0,
    last_interaction TEXT,
    affinity_score REAL DEFAULT 0.0
);
```

---

## Implementation Phases

### Phase 1: Static Identity (Foundation)
- [ ] Create onboarding UI for role/tech stack selection
- [ ] Add explicit interest input (topics)
- [ ] Add exclusion list
- [ ] Store interests as embeddings
- [ ] Integrate into relevance scoring

### Phase 2: Active Context (Real-Time)
- [ ] Directory watcher for watched folders
- [ ] Index recent file changes (last 7 days)
- [ ] Project detection from config files
- [ ] Git integration for commit analysis
- [ ] Extract topics from code (imports, deps)

### Phase 3: Learned Behavior (Intelligence)
- [ ] Track click/save/dismiss actions
- [ ] Compute topic affinities from interactions
- [ ] Implement temporal decay
- [ ] Detect anti-topics from patterns
- [ ] Integrate learned scores into relevance

### Phase 4: Refinement
- [ ] Confidence calibration
- [ ] A/B testing different weights
- [ ] User feedback on relevance quality
- [ ] Export/import interest profiles

---

## Privacy Considerations

All context data stays LOCAL:
- No interest data leaves the device
- No behavioral data is transmitted
- Embeddings are computed locally
- User has full control to delete any data

This is not optional - it's core to 4DA's value proposition.

---

## Success Metrics

The context engine is successful when:
1. **Precision increases**: Higher % of shown items are actually relevant
2. **Recall improves**: Fewer relevant items are missed
3. **Cold start is fast**: New users get good results within 5 interactions
4. **Adaptation is visible**: User sees relevance improve over time
5. **No creepiness**: User understands why items are shown

---

## Open Questions

1. Should we support multiple "contexts" (work vs personal)?
2. How aggressive should temporal decay be?
3. Should we show users their learned interest model?
4. How do we handle topic extraction from code files?
5. Should GitHub import be opt-in during onboarding?
