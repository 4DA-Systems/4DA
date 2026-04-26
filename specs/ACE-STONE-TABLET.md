# ACE - Autonomous Context Engine
## The Impenetrable Dome Stone Tablet

**Version:** 1.0.0
**Status:** CANONICAL SPECIFICATION
**Confidence:** 97%
**Author:** Principal Systems Architect

---

## 1. Core Mandate

**ACE always hits its mark.**

The Autonomous Context Engine is the brain of 4DA. It must achieve:
- **Autonomous Detection:** Zero manual input required (manual input enhances, never required)
- **High Accuracy:** >85% precision on relevance scoring
- **True Personalization:** Each profile is unique and deeply understood
- **Seamless Operation:** Users never think about context; it just works

---

## 2. Architecture Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              ACE DOME                                       │
│                    (Impenetrable Context Fortress)                          │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      SIGNAL ACQUISITION LAYER                        │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Project  │ │   File   │ │   Git    │ │ Activity │ │ Behavior │  │   │
│  │  │ Scanner  │ │ Watcher  │ │ Analyzer │ │ Tracker  │ │ Learner  │  │   │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │   │
│  │       │            │            │            │            │         │   │
│  └───────┼────────────┼────────────┼────────────┼────────────┼─────────┘   │
│          │            │            │            │            │              │
│          ▼            ▼            ▼            ▼            ▼              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      SIGNAL VALIDATION LAYER                         │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  Confidence Scoring → Cross-Validation → Anomaly Detection  │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  └──────────────────────────────┬──────────────────────────────────────┘   │
│                                 │                                           │
│                                 ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      CONTEXT SYNTHESIS LAYER                         │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   STATIC    │  │   ACTIVE    │  │   LEARNED   │                  │   │
│  │  │  IDENTITY   │  │   CONTEXT   │  │  BEHAVIOR   │                  │   │
│  │  │ (Explicit)  │  │ (Real-Time) │  │ (Implicit)  │                  │   │
│  │  │ Weight: 1.0 │  │ Weight: 0.8 │  │ Weight: 0.6 │                  │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │   │
│  │         │                │                │                          │   │
│  │         └────────────────┼────────────────┘                          │   │
│  │                          ▼                                           │   │
│  │              ┌───────────────────────┐                               │   │
│  │              │  UNIFIED INTEREST     │                               │   │
│  │              │       MODEL           │                               │   │
│  │              │ (Confidence-Calibrated)│                               │   │
│  │              └───────────┬───────────┘                               │   │
│  └──────────────────────────┼──────────────────────────────────────────┘   │
│                             │                                               │
│                             ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      RELEVANCE SCORING ENGINE                        │   │
│  │                                                                      │   │
│  │   ┌──────────────────────────────────────────────────────────────┐  │   │
│  │   │  EXCLUSION GATE → INTEREST MATCH → CONFIDENCE SCORE → OUTPUT │  │   │
│  │   └──────────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      FEEDBACK & LEARNING LOOP                        │   │
│  │    Implicit Signals ◄── Interactions ──► Explicit Corrections        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Signal Acquisition Layer

### 3.1 Project Scanner (Autonomous)

**Purpose:** Automatically detect technology stack from project manifests.

**Scanned Files:**
```
package.json      → Node.js deps, scripts
Cargo.toml        → Rust crates
pyproject.toml    → Python packages
requirements.txt  → Python deps
go.mod            → Go modules
composer.json     → PHP packages
Gemfile           → Ruby gems
pom.xml           → Java/Maven
build.gradle      → Java/Gradle
CMakeLists.txt    → C/C++ projects
.csproj           → .NET projects
pubspec.yaml      → Dart/Flutter
```

**Extraction Rules:**
```rust
struct ProjectSignal {
    path: PathBuf,
    detected_at: DateTime<Utc>,

    // Extracted data
    languages: Vec<LanguageSignal>,
    frameworks: Vec<FrameworkSignal>,
    dependencies: Vec<DependencySignal>,

    // Confidence metrics
    confidence: f32,           // 0.0-1.0
    evidence_count: usize,     // Number of confirming signals
    last_activity: DateTime<Utc>,
}

struct LanguageSignal {
    language: String,          // "rust", "typescript"
    confidence: f32,           // Based on file count, manifest presence
    file_count: usize,
    primary: bool,             // Is this the main language?
}

struct DependencySignal {
    name: String,              // "tokio", "react"
    version: Option<String>,
    category: DependencyCategory,  // Runtime, Dev, Build
    popularity_rank: Option<u32>,  // For prioritization
}
```

**Confidence Calculation:**
```
Language Confidence =
    0.4 * (has_manifest ? 1.0 : 0.0) +
    0.3 * min(file_count / 10, 1.0) +
    0.2 * (has_config_files ? 1.0 : 0.0) +
    0.1 * recency_factor(last_modified)
```

### 3.2 File Watcher (Real-Time)

**Purpose:** Detect context changes as user works.

**Watch Strategy:**
```rust
struct WatchConfig {
    // Directories to watch (user-authorized)
    watched_dirs: Vec<PathBuf>,

    // File patterns
    include_patterns: Vec<String>,  // ["*.rs", "*.ts", "*.md"]
    exclude_patterns: Vec<String>,  // ["node_modules/**", "target/**"]

    // Performance limits
    max_files_per_dir: usize,       // 10,000
    debounce_ms: u64,               // 500ms
    batch_size: usize,              // Process 50 files at a time
}

struct FileChangeSignal {
    path: PathBuf,
    change_type: FileChangeType,    // Created, Modified, Deleted
    timestamp: DateTime<Utc>,

    // Context extraction
    extracted_topics: Vec<String>,
    extracted_imports: Vec<String>,
    content_hash: String,

    // For relevance weighting
    recency_weight: f32,            // Decays over 7 days
}
```

**Debouncing & Batching:**
- File changes debounced at 500ms (prevent save-storm)
- Batched processing every 5 seconds
- Immediate processing for project manifests

### 3.3 Git Analyzer (Context Depth)

**Purpose:** Understand project history and active work patterns.

**Extracted Signals:**
```rust
struct GitSignal {
    repo_path: PathBuf,

    // Recent activity
    recent_commits: Vec<CommitSignal>,      // Last 30 days
    active_branches: Vec<BranchSignal>,

    // Patterns
    commit_frequency: f32,                   // Commits per day
    active_files: Vec<PathBuf>,              // Files with recent changes
    collaborators: Vec<String>,              // For team context

    // Confidence
    confidence: f32,
    last_analyzed: DateTime<Utc>,
}

struct CommitSignal {
    hash: String,
    message: String,
    timestamp: DateTime<Utc>,

    // Extracted context
    topics: Vec<String>,                     // From commit message
    files_changed: Vec<PathBuf>,
    insertions: usize,
    deletions: usize,
}
```

**Topic Extraction from Commits:**
```
Commit: "fix: resolve tokio runtime panic in async handler"
→ Topics: ["tokio", "async", "runtime", "bug-fix"]

Commit: "feat: add PostgreSQL connection pooling"
→ Topics: ["postgresql", "database", "connection-pooling", "feature"]
```

### 3.4 Activity Tracker (Opt-In Only)

**Purpose:** Understand real-time user focus.

**PRIVACY REQUIREMENTS:**
- OFF by default
- Explicit opt-in with clear explanation
- Easy one-click disable
- No sensitive window title storage (banking, passwords)
- Local processing only

**Implementation:**
```rust
struct ActivitySignal {
    timestamp: DateTime<Utc>,

    // Window context (sanitized)
    app_name: String,                        // "VS Code", "Firefox"
    window_category: WindowCategory,         // IDE, Browser, Terminal, Other

    // Derived context (not raw title)
    detected_project: Option<String>,        // From IDE title
    detected_language: Option<String>,       // From IDE title
    detected_domain: Option<String>,         // From browser domain

    // Duration
    duration_seconds: u64,
    focus_quality: f32,                      // 1.0 = uninterrupted
}

enum WindowCategory {
    IDE,            // VS Code, IntelliJ, Vim
    Terminal,       // iTerm, Windows Terminal
    Browser,        // Firefox, Chrome
    Documentation,  // Dash, DevDocs
    Communication,  // Slack, Discord
    Other,
}
```

**Sanitization Rules:**
```rust
fn sanitize_window_title(raw: &str) -> Option<SanitizedTitle> {
    // Block sensitive patterns
    let blocked = ["password", "banking", "paypal", "login", "1password"];
    if blocked.iter().any(|b| raw.to_lowercase().contains(b)) {
        return None;
    }

    // Extract safe context only
    // "lib.rs - my-project - VS Code" → project: "my-project", file: "lib.rs"
    parse_ide_title(raw)
}
```

### 3.5 Behavior Learner (Implicit Feedback)

**Purpose:** Learn from user actions without asking.

**Tracked Actions:**
```rust
struct BehaviorSignal {
    item_id: i64,
    action: BehaviorAction,
    timestamp: DateTime<Utc>,

    // Context at time of action
    item_topics: Vec<String>,
    item_source: String,

    // Derived signal strength
    signal_strength: f32,        // How confident is this signal?
}

enum BehaviorAction {
    // Positive signals
    Click { dwell_time_seconds: u64 },    // Strength: 0.5 + dwell_bonus
    Save,                                  // Strength: 1.0
    Share,                                 // Strength: 1.0

    // Negative signals
    Dismiss,                               // Strength: -0.8
    MarkIrrelevant,                        // Strength: -1.0

    // Weak signals
    Scroll { visible_seconds: f32 },       // Strength: 0.1 * seconds
    Ignore,                                // Strength: -0.1
}
```

**Signal Strength Calculation:**
```rust
fn compute_signal_strength(action: &BehaviorAction) -> f32 {
    match action {
        Click { dwell_time } => {
            let base = 0.5;
            let dwell_bonus = (*dwell_time as f32 / 60.0).min(0.5);  // Cap at +0.5
            base + dwell_bonus
        }
        Save | Share => 1.0,
        Dismiss => -0.8,
        MarkIrrelevant => -1.0,
        Scroll { visible_seconds } => 0.1 * visible_seconds.min(3.0),
        Ignore => -0.1,
    }
}
```

---

## 4. Signal Validation Layer

### 4.1 Confidence Scoring

Every signal must have a confidence score. No unvalidated data enters the model.

```rust
struct ValidatedSignal {
    raw_signal: RawSignal,

    // Validation metadata
    confidence: f32,                    // 0.0-1.0
    evidence_sources: Vec<String>,      // What confirms this?
    contradictions: Vec<String>,        // What conflicts?

    // Quality metrics
    freshness: f32,                     // Temporal decay applied
    consistency: f32,                   // Matches other signals?
}

fn validate_signal(signal: RawSignal, context: &ValidationContext) -> Option<ValidatedSignal> {
    let confidence = compute_confidence(&signal, context);

    // Reject low-confidence signals
    if confidence < 0.3 {
        return None;
    }

    // Check for contradictions
    let contradictions = find_contradictions(&signal, context);

    Some(ValidatedSignal {
        raw_signal: signal,
        confidence,
        evidence_sources: gather_evidence(&signal, context),
        contradictions,
        freshness: compute_freshness(&signal),
        consistency: compute_consistency(&signal, context),
    })
}
```

**Confidence Thresholds:**

| Confidence | Status | Action |
|------------|--------|--------|
| 0.9 - 1.0 | **Certain** | Full weight |
| 0.7 - 0.9 | **Confident** | Normal weight |
| 0.5 - 0.7 | **Probable** | Reduced weight |
| 0.3 - 0.5 | **Uncertain** | Minimal weight |
| 0.0 - 0.3 | **Rejected** | Discarded |

### 4.2 Cross-Validation

Signals from multiple sources must agree to achieve high confidence.

```rust
struct CrossValidation {
    topic: String,

    // Evidence from each source
    project_evidence: Option<f32>,      // From project scanner
    file_evidence: Option<f32>,         // From file watcher
    git_evidence: Option<f32>,          // From git analyzer
    activity_evidence: Option<f32>,     // From activity tracker
    behavior_evidence: Option<f32>,     // From behavior learner
    explicit_evidence: Option<f32>,     // From user input

    // Computed
    combined_confidence: f32,
    source_count: usize,
}

fn cross_validate(topic: &str, signals: &[ValidatedSignal]) -> CrossValidation {
    let mut cv = CrossValidation::new(topic);

    for signal in signals {
        match signal.source {
            Source::ProjectScanner => cv.project_evidence = Some(signal.confidence),
            Source::FileWatcher => cv.file_evidence = Some(signal.confidence),
            Source::GitAnalyzer => cv.git_evidence = Some(signal.confidence),
            Source::ActivityTracker => cv.activity_evidence = Some(signal.confidence),
            Source::BehaviorLearner => cv.behavior_evidence = Some(signal.confidence),
            Source::UserExplicit => cv.explicit_evidence = Some(signal.confidence),
        }
    }

    // Multi-source confirmation boosts confidence
    cv.source_count = cv.count_sources();
    cv.combined_confidence = compute_combined_confidence(&cv);
    cv
}

fn compute_combined_confidence(cv: &CrossValidation) -> f32 {
    let base = cv.average_evidence();

    // Multi-source bonus: +10% per additional source
    let multi_source_bonus = (cv.source_count - 1) as f32 * 0.1;

    // Explicit user input always wins
    if cv.explicit_evidence.is_some() {
        return 1.0;
    }

    (base + multi_source_bonus).min(0.95)  // Cap at 0.95 for inferred
}
```

### 4.3 Anomaly Detection

Detect sudden context shifts and validate before applying.

```rust
struct AnomalyDetector {
    // Historical baseline
    topic_history: HashMap<String, Vec<TopicObservation>>,

    // Thresholds
    sudden_appearance_threshold: f32,   // 0.7 - new topic with high confidence
    sudden_disappearance_threshold: f32, // 0.3 - active topic vanishes
    drift_threshold: f32,                // 0.5 - gradual context shift
}

struct Anomaly {
    anomaly_type: AnomalyType,
    topic: String,
    confidence: f32,
    recommendation: AnomalyAction,
}

enum AnomalyType {
    SuddenInterestAppearance,    // New topic appears with high confidence
    SuddenInterestDisappearance, // Active topic suddenly gone
    ContextDrift,                // Gradual shift detected
    Contradiction,               // Signals conflict with each other
    StaleContext,                // No updates for extended period
}

enum AnomalyAction {
    AcceptWithCaution,           // Apply but flag for review
    RequestConfirmation,         // Ask user to confirm
    Reject,                      // Don't apply
    RefreshRequired,             // Force context refresh
}

fn detect_anomalies(new_signal: &ValidatedSignal, history: &TopicHistory) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    // Check for sudden appearance
    if new_signal.confidence > 0.7 && !history.contains(&new_signal.topic) {
        anomalies.push(Anomaly {
            anomaly_type: AnomalyType::SuddenInterestAppearance,
            topic: new_signal.topic.clone(),
            confidence: new_signal.confidence,
            recommendation: AnomalyAction::AcceptWithCaution,
        });
    }

    // Check for contradictions
    if let Some(existing) = history.get(&new_signal.topic) {
        if (new_signal.confidence - existing.confidence).abs() > 0.5 {
            anomalies.push(Anomaly {
                anomaly_type: AnomalyType::Contradiction,
                topic: new_signal.topic.clone(),
                confidence: new_signal.confidence,
                recommendation: AnomalyAction::RequestConfirmation,
            });
        }
    }

    anomalies
}
```

---

## 5. Context Synthesis Layer

### 5.1 Three-Layer Context Model

```rust
/// The complete context membrane
pub struct ContextMembrane {
    pub static_identity: StaticIdentity,     // User-declared (highest trust)
    pub active_context: ActiveContext,       // Real-time (high trust)
    pub learned_behavior: LearnedBehavior,   // Implicit (moderate trust)

    // Metadata
    pub last_updated: DateTime<Utc>,
    pub health_score: f32,                   // Overall context quality
}
```

### Layer 1: Static Identity (Weight: 1.0)

**Explicit user declarations. Highest trust. Never overridden by inference.**

```rust
pub struct StaticIdentity {
    // Core identity
    pub role: Option<String>,                // "Backend Developer"
    pub experience_level: Option<String>,    // "Senior", "Junior"

    // Technology preferences
    pub tech_stack: Vec<TechStackItem>,
    pub domains: Vec<String>,                // "distributed systems", "ML"

    // Explicit interests (user typed these)
    pub interests: Vec<Interest>,

    // Hard exclusions (NEVER show these)
    pub exclusions: Vec<Exclusion>,

    // Trust level
    pub confidence: f32,                     // Always 1.0 for explicit
}

pub struct TechStackItem {
    pub name: String,                        // "rust"
    pub proficiency: Proficiency,            // Learning, Competent, Expert
    pub embedding: Vec<f32>,
    pub source: TechSource,                  // Explicit, Inferred, Imported
}

pub struct Interest {
    pub topic: String,
    pub weight: f32,                         // 1.0 for explicit
    pub embedding: Vec<f32>,
    pub source: InterestSource,
    pub added_at: DateTime<Utc>,
}

pub struct Exclusion {
    pub topic: String,
    pub reason: Option<String>,              // "I hate crypto spam"
    pub strength: ExclusionStrength,         // Soft, Hard, Absolute
}

pub enum ExclusionStrength {
    Soft,      // Reduce score by 50%
    Hard,      // Reduce score by 90%
    Absolute,  // Score = 0, never show
}
```

### Layer 2: Active Context (Weight: 0.8)

**Real-time awareness of current work. High trust but decays.**

```rust
pub struct ActiveContext {
    // Current project awareness
    pub current_project: Option<ProjectContext>,
    pub recent_projects: Vec<ProjectContext>,     // Last 7 days

    // File-level context
    pub recently_modified: Vec<FileContext>,      // Last 48 hours
    pub currently_open: Vec<FileContext>,         // If detectable

    // Derived topics from active work
    pub active_topics: Vec<ActiveTopic>,

    // Git context
    pub git_context: Option<GitContext>,

    // Temporal weighting
    pub context_freshness: f32,                   // Decays over time
}

pub struct ProjectContext {
    pub name: String,
    pub path: PathBuf,

    // Detected from manifests
    pub languages: Vec<LanguageSignal>,
    pub frameworks: Vec<String>,
    pub dependencies: Vec<String>,

    // Activity metrics
    pub last_activity: DateTime<Utc>,
    pub activity_score: f32,                      // Recent commit frequency

    // Confidence
    pub detection_confidence: f32,
}

pub struct ActiveTopic {
    pub topic: String,
    pub weight: f32,                              // Decays over 7 days
    pub last_seen: DateTime<Utc>,
    pub source: ActiveTopicSource,
    pub embedding: Vec<f32>,
}

pub enum ActiveTopicSource {
    FileContent,
    GitCommit,
    ImportStatement,
    ProjectManifest,
    ActivityTracker,
}
```

### Layer 3: Learned Behavior (Weight: 0.6)

**Implicit preferences from actions. Moderate trust, requires validation.**

```rust
pub struct LearnedBehavior {
    // Topic affinities from behavior
    pub topic_affinities: HashMap<String, TopicAffinity>,

    // Anti-topics (consistently rejected)
    pub anti_topics: Vec<AntiTopic>,

    // Source preferences
    pub source_preferences: HashMap<String, f32>,

    // Time-of-day patterns
    pub activity_patterns: ActivityPatterns,

    // Confidence metrics
    pub learning_confidence: f32,                 // Based on data volume
    pub last_updated: DateTime<Utc>,
}

pub struct TopicAffinity {
    pub topic: String,
    pub embedding: Vec<f32>,

    // Signal counts
    pub positive_signals: u32,
    pub negative_signals: u32,
    pub total_exposures: u32,

    // Computed score
    pub affinity_score: f32,                      // -1.0 to 1.0
    pub confidence: f32,                          // Based on exposure count

    // Temporal
    pub last_interaction: DateTime<Utc>,
    pub decay_applied: bool,
}

pub struct AntiTopic {
    pub topic: String,
    pub rejection_count: u32,
    pub confidence: f32,
    pub auto_detected: bool,                      // vs user-confirmed
}

impl TopicAffinity {
    pub fn compute_score(&self) -> f32 {
        if self.total_exposures < 5 {
            return 0.0;  // Not enough data
        }

        let raw_score = (self.positive_signals as f32 - self.negative_signals as f32)
            / self.total_exposures as f32;

        let confidence_factor = (self.total_exposures as f32 / 20.0).min(1.0);
        let decay_factor = self.temporal_decay();

        raw_score * confidence_factor * decay_factor
    }

    fn temporal_decay(&self) -> f32 {
        let days_since = (Utc::now() - self.last_interaction).num_days() as f32;
        0.5_f32.powf(days_since / 30.0)  // Half-life of 30 days
    }
}
```

---

## 6. Unified Interest Model

### 6.1 Interest Aggregation

```rust
pub struct UnifiedInterestModel {
    // Aggregated interests with confidence
    pub interests: Vec<AggregatedInterest>,

    // Hard exclusions (union of all layers)
    pub exclusions: Vec<UnifiedExclusion>,

    // Model metadata
    pub model_version: u32,
    pub last_computed: DateTime<Utc>,
    pub health_metrics: ModelHealth,
}

pub struct AggregatedInterest {
    pub topic: String,
    pub embedding: Vec<f32>,

    // Contribution from each layer
    pub static_weight: f32,              // From explicit declaration
    pub active_weight: f32,              // From current context
    pub learned_weight: f32,             // From behavior

    // Final aggregated weight
    pub final_weight: f32,
    pub final_confidence: f32,

    // Audit trail
    pub contributing_sources: Vec<String>,
}

impl AggregatedInterest {
    pub fn compute_final_weight(&mut self) {
        // Layer weights (static > active > learned)
        const STATIC_LAYER_WEIGHT: f32 = 1.0;
        const ACTIVE_LAYER_WEIGHT: f32 = 0.8;
        const LEARNED_LAYER_WEIGHT: f32 = 0.6;

        let weighted_sum =
            self.static_weight * STATIC_LAYER_WEIGHT +
            self.active_weight * ACTIVE_LAYER_WEIGHT +
            self.learned_weight * LEARNED_LAYER_WEIGHT;

        let total_weight =
            (if self.static_weight > 0.0 { STATIC_LAYER_WEIGHT } else { 0.0 }) +
            (if self.active_weight > 0.0 { ACTIVE_LAYER_WEIGHT } else { 0.0 }) +
            (if self.learned_weight > 0.0 { LEARNED_LAYER_WEIGHT } else { 0.0 });

        self.final_weight = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Confidence based on source count and agreement
        self.final_confidence = self.compute_confidence();
    }

    fn compute_confidence(&self) -> f32 {
        let source_count = [
            self.static_weight > 0.0,
            self.active_weight > 0.0,
            self.learned_weight > 0.0,
        ].iter().filter(|&&x| x).count();

        match source_count {
            3 => 0.95,  // All three layers agree
            2 => 0.80,  // Two layers agree
            1 => 0.60,  // Single source
            _ => 0.0,
        }
    }
}
```

### 6.2 Exclusion Handling

```rust
pub struct UnifiedExclusion {
    pub topic: String,
    pub strength: ExclusionStrength,
    pub source: ExclusionSource,
    pub confidence: f32,
}

pub enum ExclusionSource {
    UserExplicit,        // User said "never show crypto"
    LearnedHard,         // Dismissed 10+ items with this topic
    LearnedSoft,         // Dismissed 5+ items with this topic
}

impl UnifiedExclusion {
    pub fn apply_to_score(&self, base_score: f32) -> f32 {
        match self.strength {
            ExclusionStrength::Absolute => 0.0,
            ExclusionStrength::Hard => base_score * 0.1,
            ExclusionStrength::Soft => base_score * 0.5,
        }
    }
}

fn compute_exclusion_strength(topic: &str, behavior: &LearnedBehavior) -> Option<ExclusionStrength> {
    if let Some(anti) = behavior.anti_topics.iter().find(|a| a.topic == topic) {
        match anti.rejection_count {
            0..=4 => None,
            5..=9 => Some(ExclusionStrength::Soft),
            _ => Some(ExclusionStrength::Hard),
        }
    } else {
        None
    }
}
```

---

## 7. Relevance Scoring Engine

### 7.1 The ACE Algorithm

```rust
/// The core relevance scoring algorithm
/// This is the heart of ACE - it must ALWAYS hit its mark
pub fn compute_relevance(
    item: &SourceItem,
    context: &ContextMembrane,
    model: &UnifiedInterestModel,
) -> RelevanceResult {
    let mut result = RelevanceResult::new(item.id);

    // ═══════════════════════════════════════════════════════════════
    // STAGE 1: EXCLUSION GATE (Hard filter - no processing if excluded)
    // ═══════════════════════════════════════════════════════════════
    let exclusion_check = check_exclusions(item, &model.exclusions);
    if let Some(exclusion) = exclusion_check {
        if exclusion.strength == ExclusionStrength::Absolute {
            result.excluded = true;
            result.excluded_by = Some(exclusion.topic.clone());
            result.final_score = 0.0;
            result.explanation = format!("Blocked by exclusion: {}", exclusion.topic);
            return result;
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // STAGE 2: INTEREST MATCHING (Embedding similarity)
    // ═══════════════════════════════════════════════════════════════
    let item_embedding = get_or_compute_embedding(item);

    // Match against all interests
    let mut interest_matches: Vec<InterestMatch> = Vec::new();
    for interest in &model.interests {
        let similarity = cosine_similarity(&item_embedding, &interest.embedding);
        if similarity > 0.5 {  // Minimum threshold
            interest_matches.push(InterestMatch {
                topic: interest.topic.clone(),
                similarity,
                interest_weight: interest.final_weight,
                interest_confidence: interest.final_confidence,
                contribution: similarity * interest.final_weight,
            });
        }
    }

    // Sort by contribution
    interest_matches.sort_by(|a, b| b.contribution.partial_cmp(&a.contribution).unwrap());

    // ═══════════════════════════════════════════════════════════════
    // STAGE 3: CONTEXT MATCHING (Active work relevance)
    // ═══════════════════════════════════════════════════════════════
    let context_score = compute_context_score(item, &context.active_context);

    // ═══════════════════════════════════════════════════════════════
    // STAGE 4: BEHAVIORAL BOOST/PENALTY
    // ═══════════════════════════════════════════════════════════════
    let behavior_modifier = compute_behavior_modifier(item, &context.learned_behavior);

    // ═══════════════════════════════════════════════════════════════
    // STAGE 5: FINAL SCORE COMPUTATION
    // ═══════════════════════════════════════════════════════════════
    let interest_score = interest_matches
        .iter()
        .take(3)  // Top 3 matches
        .map(|m| m.contribution)
        .sum::<f32>()
        / 3.0;

    let base_score =
        interest_score * 0.50 +      // Interest match (50%)
        context_score * 0.35 +        // Context match (35%)
        behavior_modifier * 0.15;     // Behavior (15%)

    // Apply soft exclusion if present
    let final_score = if let Some(exclusion) = &exclusion_check {
        exclusion.apply_to_score(base_score)
    } else {
        base_score
    };

    // ═══════════════════════════════════════════════════════════════
    // STAGE 6: CONFIDENCE SCORING
    // ═══════════════════════════════════════════════════════════════
    let confidence = compute_score_confidence(
        &interest_matches,
        context_score,
        &context.learned_behavior,
    );

    result.final_score = final_score;
    result.confidence = confidence;
    result.interest_matches = interest_matches;
    result.context_score = context_score;
    result.behavior_modifier = behavior_modifier;
    result.explanation = generate_explanation(&result);

    result
}

fn compute_score_confidence(
    matches: &[InterestMatch],
    context_score: f32,
    behavior: &LearnedBehavior,
) -> f32 {
    let match_confidence = if matches.is_empty() {
        0.3  // No matches = low confidence
    } else {
        matches[0].interest_confidence  // Use top match confidence
    };

    let context_confidence = if context_score > 0.5 { 0.9 } else { 0.6 };
    let behavior_confidence = behavior.learning_confidence;

    // Weighted average
    (match_confidence * 0.5 + context_confidence * 0.3 + behavior_confidence * 0.2)
}
```

### 7.2 Relevance Result

```rust
pub struct RelevanceResult {
    pub item_id: i64,

    // Core scores
    pub final_score: f32,              // 0.0 to 1.0
    pub confidence: f32,               // How confident is this score?

    // Component scores
    pub interest_matches: Vec<InterestMatch>,
    pub context_score: f32,
    pub behavior_modifier: f32,

    // Exclusion handling
    pub excluded: bool,
    pub excluded_by: Option<String>,

    // Audit trail
    pub explanation: String,           // Human-readable reason
    pub computed_at: DateTime<Utc>,
}

pub struct InterestMatch {
    pub topic: String,
    pub similarity: f32,               // Embedding similarity
    pub interest_weight: f32,          // Weight of the interest
    pub interest_confidence: f32,      // Confidence in the interest
    pub contribution: f32,             // Final contribution to score
}
```

---

## 8. Accuracy Guarantees

### 8.1 Metrics Definition

```rust
pub struct AccuracyMetrics {
    // Core metrics
    pub precision: f32,                // Relevant items / Shown items
    pub recall: f32,                   // Found relevant / Total relevant
    pub f1_score: f32,                 // Harmonic mean

    // Confidence calibration
    pub calibration_error: f32,        // Predicted confidence vs actual

    // User satisfaction
    pub engagement_rate: f32,          // Clicks / Impressions
    pub explicit_feedback_ratio: f32,  // Thumbs up / (Thumbs up + down)

    // Coverage
    pub cold_start_success_rate: f32,  // Good results within 5 interactions
    pub topic_coverage: f32,           // % of user interests being served
}

// Target metrics for ACE
pub const ACE_TARGETS: AccuracyTargets = AccuracyTargets {
    min_precision: 0.85,               // 85%+ shown items are relevant
    min_recall: 0.70,                  // Find 70%+ of relevant items
    min_engagement: 0.30,              // 30%+ items get clicked
    max_calibration_error: 0.10,       // ±10% confidence accuracy
    cold_start_interactions: 5,        // Good results after 5 interactions
};
```

### 8.2 Accuracy Monitoring

```rust
pub struct AccuracyMonitor {
    // Sliding window metrics
    window_size: usize,                // 100 items
    recent_results: VecDeque<FeedbackResult>,

    // Cumulative metrics
    total_shown: u64,
    total_clicked: u64,
    total_positive_feedback: u64,
    total_negative_feedback: u64,
}

impl AccuracyMonitor {
    pub fn record_feedback(&mut self, result: FeedbackResult) {
        self.recent_results.push_back(result);
        if self.recent_results.len() > self.window_size {
            self.recent_results.pop_front();
        }

        // Update cumulative
        match result.feedback {
            Feedback::Click => self.total_clicked += 1,
            Feedback::ThumbsUp => self.total_positive_feedback += 1,
            Feedback::ThumbsDown => self.total_negative_feedback += 1,
            _ => {}
        }
        self.total_shown += 1;
    }

    pub fn compute_current_metrics(&self) -> AccuracyMetrics {
        let window: Vec<_> = self.recent_results.iter().collect();

        let precision = window.iter()
            .filter(|r| r.feedback.is_positive())
            .count() as f32 / window.len() as f32;

        let engagement = self.total_clicked as f32 / self.total_shown as f32;

        AccuracyMetrics {
            precision,
            engagement_rate: engagement,
            // ... compute other metrics
        }
    }

    pub fn check_accuracy_alerts(&self) -> Vec<AccuracyAlert> {
        let metrics = self.compute_current_metrics();
        let mut alerts = Vec::new();

        if metrics.precision < ACE_TARGETS.min_precision {
            alerts.push(AccuracyAlert::PrecisionBelow {
                current: metrics.precision,
                target: ACE_TARGETS.min_precision,
            });
        }

        if metrics.engagement_rate < ACE_TARGETS.min_engagement {
            alerts.push(AccuracyAlert::EngagementBelow {
                current: metrics.engagement_rate,
                target: ACE_TARGETS.min_engagement,
            });
        }

        alerts
    }
}
```

---

## 9. Graceful Degradation

### 9.1 Fallback Chain

```rust
pub struct FallbackChain {
    levels: Vec<FallbackLevel>,
    current_level: usize,
}

pub enum FallbackLevel {
    Full,           // All systems operational
    NoActivity,     // Activity tracker failed/disabled
    NoGit,          // Git analyzer unavailable
    NoFileWatch,    // File watcher failed
    ManualOnly,     // Only explicit user input works
    Emergency,      // Everything failed, use defaults
}

impl FallbackChain {
    pub fn compute_context(&self, membrane: &mut ContextMembrane) -> ContextQuality {
        match self.current_level {
            FallbackLevel::Full => {
                // All systems go
                self.run_all_scanners(membrane);
                ContextQuality::Excellent
            }
            FallbackLevel::NoActivity => {
                // Skip activity tracker
                self.run_file_scanners(membrane);
                self.run_git_analyzer(membrane);
                self.run_behavior_learner(membrane);
                ContextQuality::Good
            }
            FallbackLevel::NoGit => {
                self.run_file_scanners(membrane);
                self.run_behavior_learner(membrane);
                ContextQuality::Acceptable
            }
            FallbackLevel::NoFileWatch => {
                self.run_project_scanner(membrane);  // One-time scan
                self.run_behavior_learner(membrane);
                ContextQuality::Degraded
            }
            FallbackLevel::ManualOnly => {
                // Only static identity works
                ContextQuality::Minimal
            }
            FallbackLevel::Emergency => {
                // Use hardcoded defaults
                self.apply_emergency_defaults(membrane);
                ContextQuality::Emergency
            }
        }
    }
}

pub enum ContextQuality {
    Excellent,      // 95%+ accuracy expected
    Good,           // 85%+ accuracy expected
    Acceptable,     // 75%+ accuracy expected
    Degraded,       // 60%+ accuracy expected
    Minimal,        // 50%+ accuracy expected
    Emergency,      // Best effort, no guarantees
}
```

### 9.2 Health Monitoring

```rust
pub struct SystemHealth {
    // Component health
    pub project_scanner: ComponentHealth,
    pub file_watcher: ComponentHealth,
    pub git_analyzer: ComponentHealth,
    pub activity_tracker: ComponentHealth,
    pub behavior_learner: ComponentHealth,

    // Overall health
    pub overall_status: HealthStatus,
    pub context_quality: ContextQuality,
    pub last_check: DateTime<Utc>,
}

pub struct ComponentHealth {
    pub status: HealthStatus,
    pub last_success: DateTime<Utc>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
    Disabled,
}

impl SystemHealth {
    pub fn check_all(&mut self) {
        // Check each component
        self.project_scanner = self.check_project_scanner();
        self.file_watcher = self.check_file_watcher();
        self.git_analyzer = self.check_git_analyzer();
        self.activity_tracker = self.check_activity_tracker();
        self.behavior_learner = self.check_behavior_learner();

        // Compute overall status
        self.overall_status = self.compute_overall_status();
        self.context_quality = self.compute_context_quality();
        self.last_check = Utc::now();
    }

    fn compute_overall_status(&self) -> HealthStatus {
        let healthy_count = [
            &self.project_scanner,
            &self.file_watcher,
            &self.git_analyzer,
            &self.behavior_learner,
        ].iter()
         .filter(|c| c.status == HealthStatus::Healthy)
         .count();

        match healthy_count {
            4 => HealthStatus::Healthy,
            2..=3 => HealthStatus::Degraded,
            _ => HealthStatus::Failed,
        }
    }
}
```

---

## 10. Audit Trail

### 10.1 Decision Logging

```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    max_entries: usize,
}

pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub entry_type: AuditEntryType,
    pub details: AuditDetails,
}

pub enum AuditEntryType {
    ContextUpdate,
    RelevanceDecision,
    ExclusionApplied,
    FeedbackReceived,
    AnomalyDetected,
    FallbackActivated,
}

pub struct AuditDetails {
    // What happened
    pub action: String,

    // Why it happened
    pub reason: String,
    pub contributing_factors: Vec<String>,

    // What was the impact
    pub before_state: Option<String>,
    pub after_state: Option<String>,

    // Confidence in decision
    pub confidence: f32,
}

impl AuditLog {
    pub fn log_relevance_decision(&mut self, item: &SourceItem, result: &RelevanceResult) {
        self.entries.push(AuditEntry {
            timestamp: Utc::now(),
            entry_type: AuditEntryType::RelevanceDecision,
            details: AuditDetails {
                action: format!("Scored item {} = {:.2}", item.id, result.final_score),
                reason: result.explanation.clone(),
                contributing_factors: result.interest_matches
                    .iter()
                    .map(|m| format!("{}: {:.2}", m.topic, m.contribution))
                    .collect(),
                before_state: None,
                after_state: Some(format!("score={:.2}, confidence={:.2}",
                    result.final_score, result.confidence)),
                confidence: result.confidence,
            },
        });
    }

    pub fn explain_decision(&self, item_id: i64) -> Option<String> {
        self.entries
            .iter()
            .rev()
            .find(|e| {
                matches!(e.entry_type, AuditEntryType::RelevanceDecision)
                    && e.details.action.contains(&item_id.to_string())
            })
            .map(|e| {
                format!(
                    "Decision: {}\nReason: {}\nFactors: {}\nConfidence: {:.0}%",
                    e.details.action,
                    e.details.reason,
                    e.details.contributing_factors.join(", "),
                    e.details.confidence * 100.0
                )
            })
    }
}
```

---

## 11. Cold Start Strategy

### 11.1 Zero-Input Bootstrap

When a user has provided NO input, ACE can still function:

```rust
pub struct ColdStartStrategy {
    // Bootstrap sources (in priority order)
    pub bootstrap_sources: Vec<BootstrapSource>,

    // Minimum viable context
    pub minimum_signals: usize,        // 3 signals to start scoring
}

pub enum BootstrapSource {
    // Autonomous (no user input needed)
    ProjectManifestScan,               // Scan common project locations
    RecentFileTypes,                   // What file types exist?
    GitConfigEmail,                    // Infer domain from email
    SystemLocale,                      // Language preferences

    // Low-friction user input
    QuickInterestSelection,            // "Pick 3 topics you care about"
    RoleSelection,                     // "What's your role?"

    // External imports (with auth)
    GitHubStars,                       // What repos do they star?
    BrowserBookmarks,                  // What do they save?
}

impl ColdStartStrategy {
    pub async fn bootstrap(&self, context: &mut ContextMembrane) -> BootstrapResult {
        let mut signals_gathered = 0;

        // Try autonomous sources first
        for source in &self.bootstrap_sources {
            match source {
                BootstrapSource::ProjectManifestScan => {
                    if let Ok(projects) = self.scan_common_project_locations().await {
                        for project in projects {
                            context.active_context.recent_projects.push(project);
                            signals_gathered += 1;
                        }
                    }
                }
                BootstrapSource::RecentFileTypes => {
                    if let Ok(types) = self.detect_file_types().await {
                        for (lang, count) in types {
                            if count > 10 {  // Significant presence
                                context.static_identity.tech_stack.push(TechStackItem {
                                    name: lang,
                                    proficiency: Proficiency::Inferred,
                                    source: TechSource::Inferred,
                                    ..Default::default()
                                });
                                signals_gathered += 1;
                            }
                        }
                    }
                }
                // ... handle other sources
            }

            if signals_gathered >= self.minimum_signals {
                break;  // Enough to start
            }
        }

        BootstrapResult {
            signals_gathered,
            ready: signals_gathered >= self.minimum_signals,
            quality: self.compute_bootstrap_quality(signals_gathered),
        }
    }

    fn scan_common_project_locations(&self) -> Result<Vec<ProjectContext>> {
        let common_paths = [
            dirs::home_dir().map(|h| h.join("projects")),
            dirs::home_dir().map(|h| h.join("code")),
            dirs::home_dir().map(|h| h.join("dev")),
            dirs::home_dir().map(|h| h.join("src")),
            dirs::document_dir().map(|d| d.join("GitHub")),
        ];

        let mut projects = Vec::new();
        for path in common_paths.into_iter().flatten() {
            if path.exists() {
                projects.extend(self.scan_directory_for_projects(&path)?);
            }
        }

        Ok(projects)
    }
}
```

### 11.2 Progressive Enhancement

```
COLD START JOURNEY
══════════════════

[First Launch]
├── Autonomous scan of ~/projects, ~/code, etc.
├── Detect: 3 Rust projects, 2 TypeScript projects
├── Infer: User works with Rust, TypeScript, SQL
└── Context Quality: MINIMAL (50% accuracy)

[First Interaction]
├── User clicks on "Async Rust" article
├── Signal: +1 for "rust", +1 for "async"
└── Context Quality: DEGRADED (60% accuracy)

[5 Interactions]
├── Patterns emerge: likes systems programming
├── Dislikes: Web3/crypto articles dismissed
├── Auto-exclusion: "cryptocurrency" (soft)
└── Context Quality: ACCEPTABLE (75% accuracy)

[User Adds Interest]
├── Explicit: "distributed systems"
├── This overrides any inferred context
└── Context Quality: GOOD (85% accuracy)

[Week 1]
├── 50+ interactions logged
├── Topic affinities computed
├── Activity patterns detected
├── File watching active
└── Context Quality: EXCELLENT (90%+ accuracy)
```

---

## 12. Implementation Phases

### Phase A: Foundation (Week 1)

**Goal:** Core autonomous detection without user input

- [ ] Project manifest scanner (package.json, Cargo.toml, etc.)
- [ ] Basic file type detection
- [ ] SQLite schema for all ACE tables
- [ ] Confidence scoring infrastructure
- [ ] Basic relevance scoring with project context

**Success Criteria:**
- Can detect tech stack from 5+ manifest types
- Scores items without any user input
- Confidence scores attached to all signals

### Phase B: Real-Time (Week 2)

**Goal:** Live context awareness

- [ ] File watcher integration (`notify` crate)
- [ ] Debouncing and batching
- [ ] Git integration (recent commits, branches)
- [ ] Topic extraction from code
- [ ] Context freshness decay

**Success Criteria:**
- Context updates within 5 seconds of file change
- Git commits influence relevance within 1 minute
- No performance degradation with 10k+ files

### Phase C: Learning (Week 3)

**Goal:** Implicit preference detection

- [ ] Behavior tracking (clicks, dismissals)
- [ ] Topic affinity computation
- [ ] Anti-topic detection
- [ ] Temporal decay implementation
- [ ] Cross-validation system

**Success Criteria:**
- Learns preferences after 10 interactions
- Auto-detects anti-topics after 5 dismissals
- Accuracy improves measurably week-over-week

### Phase D: Validation (Week 4)

**Goal:** Nuke-proof reliability

- [ ] Anomaly detection
- [ ] Graceful degradation
- [ ] Health monitoring
- [ ] Audit trail
- [ ] Accuracy metrics dashboard

**Success Criteria:**
- System recovers from any single component failure
- Can explain any relevance decision
- Meets 85% precision target

### Phase E: Activity Tracking (Optional)

**Goal:** Real-time focus awareness

- [ ] Window title monitoring (opt-in)
- [ ] Platform-specific implementations
- [ ] Privacy sanitization
- [ ] Focus quality detection

**Success Criteria:**
- Works on Windows (primary)
- Zero sensitive data leakage
- <1% CPU overhead

---

## 13. Database Schema

```sql
-- ═══════════════════════════════════════════════════════════════
-- SIGNAL ACQUISITION TABLES
-- ═══════════════════════════════════════════════════════════════

-- Detected projects from manifest scanning
CREATE TABLE detected_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    languages TEXT,                -- JSON array
    frameworks TEXT,               -- JSON array
    dependencies TEXT,             -- JSON array
    last_activity TEXT,
    detection_confidence REAL DEFAULT 0.5,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- File change signals
CREATE TABLE file_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    change_type TEXT NOT NULL,     -- 'created', 'modified', 'deleted'
    extracted_topics TEXT,         -- JSON array
    content_hash TEXT,
    timestamp TEXT DEFAULT (datetime('now')),
    processed INTEGER DEFAULT 0
);

-- Git signals
CREATE TABLE git_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_path TEXT NOT NULL,
    commit_hash TEXT,
    commit_message TEXT,
    extracted_topics TEXT,         -- JSON array
    files_changed TEXT,            -- JSON array
    timestamp TEXT DEFAULT (datetime('now'))
);

-- ═══════════════════════════════════════════════════════════════
-- CONTEXT LAYER TABLES
-- ═══════════════════════════════════════════════════════════════

-- Layer 1: Static Identity (user-declared)
CREATE TABLE user_identity (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton
    role TEXT,
    experience_level TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE tech_stack (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    proficiency TEXT DEFAULT 'inferred',  -- 'learning', 'competent', 'expert', 'inferred'
    embedding BLOB,
    source TEXT DEFAULT 'inferred',       -- 'explicit', 'inferred', 'imported'
    confidence REAL DEFAULT 0.5,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE explicit_interests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    weight REAL DEFAULT 1.0,
    embedding BLOB,
    source TEXT DEFAULT 'explicit',
    confidence REAL DEFAULT 1.0,          -- Always 1.0 for explicit
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE exclusions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    strength TEXT DEFAULT 'hard',         -- 'soft', 'hard', 'absolute'
    reason TEXT,
    source TEXT DEFAULT 'explicit',       -- 'explicit', 'learned'
    created_at TEXT DEFAULT (datetime('now'))
);

-- Layer 2: Active Context
CREATE TABLE watched_directories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    enabled INTEGER DEFAULT 1,
    last_indexed TEXT,
    file_count INTEGER DEFAULT 0,
    chunk_count INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE active_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    weight REAL DEFAULT 0.5,
    embedding BLOB,
    source TEXT NOT NULL,                 -- 'file', 'git', 'manifest', 'activity'
    last_seen TEXT DEFAULT (datetime('now')),
    decay_applied INTEGER DEFAULT 0
);

-- Layer 3: Learned Behavior
CREATE TABLE interactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_item_id INTEGER NOT NULL,
    action TEXT NOT NULL,                 -- 'click', 'save', 'dismiss', 'ignore'
    dwell_time_seconds INTEGER,
    item_topics TEXT,                     -- JSON array
    timestamp TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (source_item_id) REFERENCES source_items(id)
);

CREATE TABLE topic_affinities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    embedding BLOB,
    positive_signals INTEGER DEFAULT 0,
    negative_signals INTEGER DEFAULT 0,
    total_exposures INTEGER DEFAULT 0,
    affinity_score REAL DEFAULT 0.0,
    confidence REAL DEFAULT 0.0,
    last_interaction TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE anti_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    rejection_count INTEGER DEFAULT 0,
    confidence REAL DEFAULT 0.0,
    auto_detected INTEGER DEFAULT 1,
    user_confirmed INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now'))
);

-- ═══════════════════════════════════════════════════════════════
-- VALIDATION & MONITORING TABLES
-- ═══════════════════════════════════════════════════════════════

-- Signal validation
CREATE TABLE validated_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_type TEXT NOT NULL,
    signal_data TEXT NOT NULL,            -- JSON
    confidence REAL NOT NULL,
    evidence_sources TEXT,                -- JSON array
    contradictions TEXT,                  -- JSON array
    freshness REAL,
    timestamp TEXT DEFAULT (datetime('now'))
);

-- Audit trail
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_type TEXT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT,
    contributing_factors TEXT,            -- JSON array
    before_state TEXT,
    after_state TEXT,
    confidence REAL,
    timestamp TEXT DEFAULT (datetime('now'))
);

-- Accuracy metrics
CREATE TABLE accuracy_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric_date TEXT NOT NULL,
    precision_score REAL,
    recall_score REAL,
    engagement_rate REAL,
    items_shown INTEGER,
    items_clicked INTEGER,
    positive_feedback INTEGER,
    negative_feedback INTEGER,
    created_at TEXT DEFAULT (datetime('now'))
);

-- System health
CREATE TABLE system_health (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    component TEXT NOT NULL,
    status TEXT NOT NULL,                 -- 'healthy', 'degraded', 'failed', 'disabled'
    last_success TEXT,
    error_count INTEGER DEFAULT 0,
    last_error TEXT,
    checked_at TEXT DEFAULT (datetime('now'))
);

-- ═══════════════════════════════════════════════════════════════
-- INDEXES FOR PERFORMANCE
-- ═══════════════════════════════════════════════════════════════

CREATE INDEX idx_file_signals_timestamp ON file_signals(timestamp);
CREATE INDEX idx_git_signals_timestamp ON git_signals(timestamp);
CREATE INDEX idx_interactions_timestamp ON interactions(timestamp);
CREATE INDEX idx_active_topics_last_seen ON active_topics(last_seen);
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_type ON audit_log(entry_type);
```

---

## 14. Success Criteria

### Hard Requirements (Must Hit)

| Requirement | Target | Measurement |
|-------------|--------|-------------|
| **Precision** | >85% | Relevant / Shown |
| **Cold Start** | 5 interactions | Until good results |
| **Autonomy** | 100% | Works without user input |
| **Latency** | <100ms | Context lookup time |
| **Memory** | <100MB | ACE overhead |
| **Recovery** | <5s | From any single failure |

### Soft Requirements (Should Hit)

| Requirement | Target | Measurement |
|-------------|--------|-------------|
| **Recall** | >70% | Found / Total relevant |
| **Engagement** | >30% | Click-through rate |
| **Learning** | Week 1 | Visible improvement |
| **Explanation** | 100% | Can explain any decision |

---

## 15. The ACE Guarantee

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│                     THE ACE GUARANTEE                            │
│                                                                  │
│   1. ACE ALWAYS HITS ITS MARK                                   │
│      - 85%+ precision or alert triggered                        │
│      - Every decision is explainable                            │
│      - Confidence scores never lie                              │
│                                                                  │
│   2. ACE NEVER REQUIRES USER INPUT                              │
│      - Works from first launch                                  │
│      - User input enhances, never required                      │
│      - Zero-config for basic functionality                      │
│                                                                  │
│   3. ACE NEVER FAILS SILENTLY                                   │
│      - All errors logged and reported                           │
│      - Graceful degradation, not crashes                        │
│      - Health status always visible                             │
│                                                                  │
│   4. ACE RESPECTS PRIVACY ABSOLUTELY                            │
│      - No data leaves machine without explicit consent          │
│      - Activity tracking off by default                         │
│      - User can delete all data anytime                         │
│                                                                  │
│   5. ACE LEARNS BUT DOESN'T CREEP                               │
│      - User always understands why items shown                  │
│      - No unexplainable "magic"                                 │
│      - Transparent learning signals                             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

*This specification is the canonical reference for ACE implementation. All code must trace back to this document. When in doubt, ACE hits its mark.*

**Approved: Stone Tablet Status**
**Confidence: 97%**
**Remaining 3%: Implementation edge cases to be discovered**
