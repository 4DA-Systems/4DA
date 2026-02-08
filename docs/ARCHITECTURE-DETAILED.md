# 4DA Architecture - Technical Deep Dive

**Version**: 3.0 (PASIFA Edition)
**Last Updated**: 2026-02-03
**Status**: Production-Ready (Phase 1 ~95% complete)

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Components](#core-components)
3. [Data Flow](#data-flow)
4. [Database Schema](#database-schema)
5. [Relevance Scoring Algorithm](#relevance-scoring-algorithm)
6. [ACE (Autonomous Context Engine)](#ace-autonomous-context-engine)
7. [Source Adapters](#source-adapters)
8. [Performance Characteristics](#performance-characteristics)
9. [Security Model](#security-model)
10. [Extension Points](#extension-points)

---

## System Overview

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                          USER'S MACHINE                              │
│                                                                       │
│  ┌──────────────────┐                    ┌────────────────────────┐ │
│  │  Tauri Frontend  │◄───IPC Commands───►│   Rust Backend         │ │
│  │  (React + TS)    │                    │   (Tokio Async)        │ │
│  └──────────────────┘                    └────────────────────────┘ │
│          │                                         │                 │
│          │                                         ▼                 │
│          │                              ┌─────────────────────────┐ │
│          │                              │  SQLite + sqlite-vec    │ │
│          │                              │  (Local Database)       │ │
│          │                              └─────────────────────────┘ │
│          │                                         │                 │
│          │                                         ▼                 │
│          │                              ┌─────────────────────────┐ │
│          │                              │  ACE Context Engine     │ │
│          │                              │  - Scanner              │ │
│          │                              │  - Watcher              │ │
│          │                              │  - Behavior Learning    │ │
│          │                              └─────────────────────────┘ │
│          │                                         │                 │
│          │                                         ▼                 │
│          │                              ┌─────────────────────────┐ │
│          │                              │  Source Adapters        │ │
│          │                              │  - HackerNews           │ │
│          │                              │  - arXiv                │ │
│          │                              │  - Reddit               │ │
│          │                              │  - RSS                  │ │
│          │                              └─────────────────────────┘ │
│          │                                         │                 │
│          │                                         ▼                 │
│          │                              ┌─────────────────────────┐ │
│          │                              │  Relevance Judge        │ │
│          │                              │  - Embedding Similarity │ │
│          │                              │  - Interest Matching    │ │
│          │                              │  - ACE Boost            │ │
│          │                              │  - Affinity Multiplier  │ │
│          │                              └─────────────────────────┘ │
│          │                                         │                 │
│          └─────────────────────────────────────────┘                 │
│                                                                       │
└───────────────────────────────────────────────────────────────────────┘
                            │                        │
                            ▼                        ▼
                   ┌───────────────────┐    ┌───────────────────┐
                   │  Anthropic API    │    │  OpenAI API       │
                   │  (Claude, BYOK)   │    │  (GPT, Embeddings)│
                   └───────────────────┘    └───────────────────┘
```

### Key Design Principles

1. **Privacy First**: All data processing happens locally. Raw files never leave the machine.
2. **BYOK (Bring Your Own Key)**: Users provide API keys, we never store them remotely.
3. **Autonomous Operation**: Minimal user configuration required. System auto-discovers context.
4. **Explainability**: Every relevance decision includes a human-readable explanation.
5. **Cost Conscious**: Hard daily limits, transparent cost tracking, efficient batching.

---

## Core Components

### 1. Tauri Frontend (React + TypeScript)

**Purpose**: User interface for configuration, result viewing, and feedback.

**Key Features**:
- **Matte black minimalist UI**: Dark theme (#0A0A0A), generous whitespace
- **Real-time updates**: WebSocket-like Tauri IPC for progress tracking
- **Batch operations**: "Dismiss <30%", "Save >60%" buttons
- **Confidence indicators**: Visual feedback for score certainty

**Tech Stack**:
- React 18 (hooks-based, functional components)
- TypeScript (strict mode, full type safety)
- Tailwind CSS (utility-first styling)
- Tauri 2.0 IPC (command invocation, event listening)

**Component Structure**:
```
src/
├── App.tsx               # Main app shell
├── components/
│   ├── ResultItem.tsx    # Single relevance result display
│   ├── ConfidenceIndicator.tsx  # Confidence score UI
│   ├── Settings.tsx      # Configuration panel
│   └── DigestView.tsx    # Daily digest display
├── hooks/
│   ├── use-tauri.ts      # Tauri command wrappers
│   └── use-feedback.ts   # Feedback state management
└── types.ts              # Shared TypeScript interfaces
```

---

### 2. Rust Backend (Tokio Async Runtime)

**Purpose**: Core logic, database operations, LLM integration, source fetching.

**Key Features**:
- **82+ Tauri commands**: Full IPC surface for frontend
- **Async/await**: Non-blocking I/O with Tokio
- **Error handling**: `thiserror` for structured errors
- **Logging**: `tracing` with structured logging

**Module Structure**:
```
src-tauri/src/
├── lib.rs                # Main entry, Tauri commands
├── main.rs               # Application entrypoint
├── db.rs                 # Database operations
├── llm.rs                # LLM integration (Anthropic/OpenAI/Ollama)
├── context_engine.rs     # Interest management, embeddings
├── digest.rs             # Daily digest generation
├── ace/
│   ├── mod.rs            # ACE coordination
│   ├── scanner.rs        # Project manifest scanner
│   ├── db.rs             # ACE database operations
│   └── watcher.rs        # File system watching
└── sources/
    ├── mod.rs            # Source registry
    ├── hackernews.rs     # HN adapter
    ├── arxiv.rs          # arXiv adapter
    ├── reddit.rs         # Reddit adapter
    └── rss.rs            # RSS adapter
```

---

### 3. SQLite + sqlite-vec (Vector Database)

**Purpose**: Local-first persistence with KNN vector search.

**Key Features**:
- **sqlite-vec extension**: O(log n) KNN search for semantic matching
- **384-dim embeddings**: MiniLM-L6-v2 or OpenAI text-embedding-3-small
- **Single file database**: Portable, no external dependencies
- **ACID transactions**: Reliable state management

**Performance**:
- KNN search: ~10ms for 10k vectors (384-dim)
- Embedding generation: ~50ms per text (local fastembed)
- Database size: ~100MB for 10k items + embeddings

---

## Data Flow

### Analysis Flow (User-Initiated)

```
1. User clicks "Analyze" button
   │
   ├──> Frontend: Invokes `analyze_relevance` Tauri command
   │
   ├──> Backend: Fetches latest items from all sources
   │     ├──> HackerNews: GET https://hacker-news.firebaseio.com/v0/topstories.json
   │     ├──> arXiv: RSS feed fetch
   │     ├──> Reddit: JSON API fetch
   │     └──> RSS: Parse custom feeds
   │
   ├──> Backend: For each item:
   │     ├──> Embed item title + content (fastembed or OpenAI)
   │     ├──> KNN search in context_chunks (sqlite-vec)
   │     ├──> Calculate interest score (keyword matching)
   │     ├──> Calculate ACE boost (detected tech overlap)
   │     ├──> Apply affinity multiplier (learned behavior)
   │     ├──> Apply anti-topic penalty (dismissals)
   │     └──> Generate explanation (LLM or template)
   │
   ├──> Backend: Sort by score, filter >0.6 threshold
   │
   └──> Frontend: Display results with confidence indicators
```

### Background Monitoring Flow (Automated)

```
1. Cron job triggers every N minutes (configurable)
   │
   ├──> Backend: Fetch new items since last check
   │
   ├──> Backend: Score new items (same as above)
   │
   ├──> Backend: Filter for high-relevance (>0.8)
   │
   ├──> If high-relevance items found:
   │     ├──> System tray notification
   │     └──> Store in digest queue
   │
   └──> Daily: Email digest with top 10 items
```

### Feedback Loop (Learning)

```
1. User action (save/dismiss/mark_irrelevant)
   │
   ├──> Frontend: Invokes `record_feedback` Tauri command
   │
   ├──> Backend: Store feedback in database
   │
   ├──> Backend: Update topic affinities:
   │     ├──> "Save" → Increase affinity by 0.1
   │     ├──> "Dismiss" → Decrease affinity by 0.05
   │     └──> "Mark Irrelevant" → Add to anti-topics
   │
   └──> Backend: Re-score future items with updated model
```

---

## Database Schema

### Tables Overview

```sql
-- User-added context files (embeddings for semantic search)
CREATE TABLE context_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    chunk_text TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,  -- NEW: Section weighting
    added_at TEXT NOT NULL,
    UNIQUE(source_file, chunk_text)
);

-- Virtual table for KNN search
CREATE VIRTUAL TABLE context_vec USING vec0(
    embedding FLOAT[384]
);

-- External source items
CREATE TABLE source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    url TEXT,
    content TEXT,
    score INTEGER,           -- Upvotes, stars, etc.
    created_at TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    UNIQUE(source_type, external_id)
);

-- User interests (manual or auto-inferred)
CREATE TABLE interests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    weight REAL NOT NULL DEFAULT 1.0,
    source TEXT NOT NULL,  -- 'user' or 'inferred'
    created_at TEXT NOT NULL
);

-- Virtual table for interest embeddings
CREATE VIRTUAL TABLE interest_vec USING vec0(
    embedding FLOAT[384]
);

-- Anti-topics (topics to filter out)
CREATE TABLE anti_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    confidence REAL NOT NULL,
    user_confirmed BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

-- Learned topic affinities (from behavior)
CREATE TABLE topic_affinities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    affinity_score REAL NOT NULL,  -- -1.0 to 1.0
    confidence REAL NOT NULL,       -- 0.0 to 1.0
    updated_at TEXT NOT NULL
);

-- User feedback on relevance
CREATE TABLE feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,
    action TEXT NOT NULL,  -- 'save', 'dismiss', 'mark_irrelevant'
    created_at TEXT NOT NULL,
    FOREIGN KEY(item_id) REFERENCES source_items(id)
);

-- ACE discovered projects
CREATE TABLE discovered_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_path TEXT NOT NULL UNIQUE,
    manifest_type TEXT NOT NULL,
    project_name TEXT,
    detected_tech TEXT,  -- JSON array
    discovered_at TEXT NOT NULL
);

-- ACE active topics (from manifests, commits)
CREATE TABLE ace_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL UNIQUE,
    confidence REAL NOT NULL,
    last_seen TEXT NOT NULL
);
```

---

## Relevance Scoring Algorithm

### PASIFA Formula (Privacy-Aware Semantic Intelligence for File Analysis)

```rust
// Step 1: Context Similarity (KNN Search)
let context_score = sqlite_vec_knn_search(item_embedding, context_embeddings, k=5)
    .average_distance();

// Step 2: Interest Score (Keyword Matching)
let interest_score = interests
    .iter()
    .filter(|i| item_text.contains(&i.topic.to_lowercase()))
    .map(|i| i.weight)
    .sum::<f32>()
    .min(1.0);

// Step 3: Base Score (Weighted Average)
let base_score = (context_score * 0.5) + (interest_score * 0.5);

// Step 4: ACE Boost (Detected Tech Overlap)
let ace_boost = detected_tech
    .iter()
    .filter(|t| item_text.contains(&t.to_lowercase()))
    .map(|t| ace_topics.get(t).map_or(0.0, |at| at.confidence * 0.1))
    .sum::<f32>()
    .min(0.3);  // Cap at 0.3

// Step 5: Affinity Multiplier (Learned Behavior)
let affinity_mult = 1.0 + topic_affinities
    .iter()
    .filter(|(topic, _)| item_text.contains(&topic.to_lowercase()))
    .map(|(_, (affinity, confidence))| affinity * confidence * 0.3)
    .sum::<f32>();

// Step 6: Anti-Topic Penalty
let anti_penalty = anti_topics
    .iter()
    .filter(|at| item_text.contains(&at.topic.to_lowercase()))
    .map(|at| 1.0 - (at.confidence * 0.5))
    .product::<f32>();

// Step 7: Final Score
let combined_score = base_score + ace_boost;
let final_score = (combined_score * affinity_mult * anti_penalty).clamp(0.0, 1.0);
```

### Confidence Calculation

```rust
fn calculate_confidence(ace_ctx: &ACEContext) -> f32 {
    let mut signals = Vec::new();

    // Signal 1: Context chunks exist
    if context_count > 0 {
        signals.push(1.0);
    }

    // Signal 2: Interests configured
    if !interests.is_empty() {
        signals.push(0.8);
    }

    // Signal 3: ACE topics detected
    for (topic, conf) in &ace_ctx.topic_confidence {
        signals.push(*conf);
    }

    // Signal 4: Anti-topics confirmed
    for (topic, conf) in &ace_ctx.anti_topic_confidence {
        if user_confirmed {
            signals.push(*conf);
        }
    }

    // Aggregate: Average of all signals, bonus for agreement
    let avg = signals.iter().sum::<f32>() / signals.len() as f32;
    let bonus = if signals.len() >= 3 { 0.1 } else { 0.0 };

    (avg + bonus).clamp(0.0, 1.0)
}
```

---

## ACE (Autonomous Context Engine)

### Scanner Component

**Purpose**: Discovers projects and extracts technology stack without user input.

**Supported Manifests** (12 types):
1. `Cargo.toml` → Rust
2. `package.json` → JavaScript/TypeScript
3. `pyproject.toml` → Python
4. `requirements.txt` → Python
5. `go.mod` → Go
6. `composer.json` → PHP
7. `Gemfile` → Ruby
8. `pom.xml` → Java
9. `build.gradle` → Java
10. `CMakeLists.txt` → C++
11. `*.csproj` → C#
12. `pubspec.yaml` → Dart

**Discovery Algorithm**:
```rust
fn discover_projects_recursive(dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    visit_dirs(dir, &mut projects, 0, max_depth);
    projects
}

fn visit_dirs(dir: &Path, projects: &mut Vec<PathBuf>, depth: usize, max: usize) {
    if depth > max { return; }

    // Check if this dir has a manifest
    if has_manifest(dir) {
        projects.push(dir.to_path_buf());
        return;  // Don't recurse into projects
    }

    // Continue searching subdirectories
    for entry in fs::read_dir(dir).filter(|e| is_not_ignored(e)) {
        visit_dirs(&entry.path(), projects, depth + 1, max);
    }
}
```

**README Indexing (PASIFA)**:
```rust
// NEW: Section-aware weighting
fn index_readme(readme_path: &Path) -> Result<Vec<(String, Vec<f32>, f32)>, Error> {
    let content = fs::read_to_string(readme_path)?;
    let sections = parse_readme_sections(&content);

    let mut chunks = Vec::new();
    for section in sections {
        let weight = section_weight(&section.heading);  // 0.3-1.0
        let section_chunks = chunk_text(&section.content, readme_path);

        for chunk in section_chunks {
            let embedding = embed_text(&chunk)?;
            chunks.push((chunk, embedding, weight));
        }
    }

    Ok(chunks)
}

fn section_weight(heading: &str) -> f32 {
    match heading.to_lowercase().as_str() {
        h if h.contains("feature") || h.contains("overview") => 1.0,
        h if h.contains("api") || h.contains("usage") => 0.9,
        h if h.contains("architecture") => 0.85,
        h if h.contains("example") => 0.8,
        h if h.contains("install") => 0.7,
        h if h.contains("license") => 0.3,
        _ => 0.6,
    }
}
```

### Watcher Component

**Purpose**: Real-time monitoring of file changes to update context.

**Technology**: `notify` crate with debouncing (500ms).

**Watched Events**:
- Manifest changes (Cargo.toml, package.json, etc.)
- README updates
- Git commits (via .git/logs/HEAD)

**Incremental Updates**:
```rust
async fn handle_file_change(event: notify::Event) {
    match event.kind {
        EventKind::Modify(ModifyKind::Data(_)) => {
            if is_manifest(&event.path) {
                rescan_project(&event.path).await;
            } else if is_readme(&event.path) {
                reindex_readme(&event.path).await;
            }
        }
        _ => {}
    }
}
```

---

## Source Adapters

### Common Interface

All source adapters implement the `Source` trait:

```rust
#[async_trait]
pub trait Source {
    fn source_type(&self) -> &'static str;
    fn name(&self) -> &'static str;
    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>>;
    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String>;
    fn should_fetch(&self, last_fetch: Option<SystemTime>) -> bool;
}
```

### Hacker News Adapter

**API**: Firebase Realtime Database (`https://hacker-news.firebaseio.com/v0/`)

**Endpoints**:
- `topstories.json`: Top 500 story IDs
- `item/{id}.json`: Story details

**Fetch Strategy**:
- Fetch top 30 stories every 30 minutes
- Rate limit: 1 request per second
- Cache story details for 24 hours

**Example**:
```rust
async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
    let url = "https://hacker-news.firebaseio.com/v0/topstories.json";
    let story_ids: Vec<u64> = self.client.get(url).send().await?.json().await?;

    let mut items = Vec::new();
    for id in story_ids.iter().take(30) {
        let story = self.fetch_story(*id).await?;
        items.push(SourceItem::from_hn_story(story));
    }

    Ok(items)
}
```

### arXiv Adapter

**API**: RSS feed (`http://export.arxiv.org/rss/{category}`)

**Categories**:
- cs.AI (Artificial Intelligence)
- cs.LG (Machine Learning)
- cs.PL (Programming Languages)
- stat.ML (Statistics - Machine Learning)

**Fetch Strategy**:
- Fetch daily (new papers posted at 00:00 UTC)
- Parse RSS XML manually (no extra dependencies)

**Example**:
```rust
async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
    let url = "http://export.arxiv.org/rss/cs.AI";
    let xml = self.client.get(url).send().await?.text().await?;

    let items = parse_arxiv_rss(&xml)?
        .into_iter()
        .map(|entry| SourceItem::from_arxiv_entry(entry))
        .collect();

    Ok(items)
}
```

### Reddit Adapter

**API**: JSON endpoint (`https://www.reddit.com/r/{subreddit}.json`)

**Subreddits**:
- r/programming
- r/rust
- r/javascript
- r/MachineLearning

**Fetch Strategy**:
- Fetch top 25 posts every hour
- No authentication required (public API)

**Example**:
```rust
async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
    let url = format!("https://www.reddit.com/r/{}.json?limit=25", self.subreddit);
    let resp: RedditResponse = self.client
        .get(&url)
        .header("User-Agent", "4DA/0.1")
        .send()
        .await?
        .json()
        .await?;

    let items = resp.data.children
        .into_iter()
        .map(|post| SourceItem::from_reddit_post(post.data))
        .collect();

    Ok(items)
}
```

### RSS Adapter (Generic)

**Purpose**: Support for any custom RSS or Atom feed.

**Parsing**: Manual XML parsing (both RSS 2.0 and Atom formats).

**Fetch Strategy**:
- User-configurable interval (default: hourly)
- Supports HTTP Basic Auth for private feeds

**Example**:
```rust
async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
    let xml = self.client.get(&self.feed_url).send().await?.text().await?;

    // Auto-detect format
    let items = if xml.contains("<rss") {
        parse_rss_feed(&xml)?
    } else if xml.contains("<feed") {
        parse_atom_feed(&xml)?
    } else {
        return Err(SourceError::InvalidFormat);
    };

    Ok(items.into_iter()
        .map(|entry| SourceItem::from_rss_entry(entry))
        .collect())
}
```

---

## Performance Characteristics

### Benchmarks (Intel i7-12700K, 32GB RAM, NVMe SSD)

| Operation | Time | Notes |
|-----------|------|-------|
| **Cold start** | 2.5s | App launch to UI ready |
| **ACE scan (10 projects)** | 1.2s | Manifest parsing + README indexing |
| **Embedding generation (local)** | 50ms/item | fastembed (ONNX) |
| **Embedding generation (OpenAI)** | 200ms/item | Network latency |
| **KNN search (10k vectors)** | 10ms | sqlite-vec, k=5 |
| **Full analysis (100 items)** | 8s | Fetch + embed + score + explain |
| **Database size** | ~100MB | 10k items + embeddings |

### Optimization Strategies

1. **Batch Embedding**: Embed 10 items at once to reduce API overhead
2. **Incremental Indexing**: Only re-index changed files
3. **Smart Caching**: Cache embeddings for 30 days, context for 7 days
4. **Lazy Loading**: Load results page-by-page (25 items per page)
5. **Background Jobs**: Schedule heavy tasks during idle time

---

## Security Model

### Threat Model

**Assumptions**:
- User's machine is trusted (4DA is not a security tool)
- Network is untrusted (HTTPS for all external requests)
- LLM providers are semi-trusted (send only embeddings, not raw files)

**Threats Mitigated**:
1. **Data exfiltration**: Raw files never leave machine
2. **API key theft**: Keys stored in OS-level secure storage (Keychain, Credential Manager)
3. **Prompt injection**: User prompts are sanitized before LLM calls
4. **Malicious source items**: Content scraping sandboxed (no code execution)

**Threats Not Mitigated**:
1. **Local malware**: 4DA cannot protect against malware on the user's machine
2. **Supply chain attacks**: Dependencies are not verified beyond cargo/npm audit
3. **LLM backdoors**: If Anthropic/OpenAI APIs are compromised, 4DA is vulnerable

### API Key Storage

**Platform-Specific**:
- **macOS**: Keychain Services
- **Windows**: Windows Credential Manager
- **Linux**: Secret Service API (gnome-keyring, KWallet)

**Implementation**:
```rust
use keyring::Entry;

// Store API key securely
fn store_api_key(provider: &str, key: &str) -> Result<(), Error> {
    let entry = Entry::new("4da", provider)?;
    entry.set_password(key)?;
    Ok(())
}

// Retrieve API key
fn get_api_key(provider: &str) -> Result<String, Error> {
    let entry = Entry::new("4da", provider)?;
    entry.get_password()
}
```

### Prompt Sanitization

**Strategy**: Whitelist allowed tokens, escape special characters.

**Implementation**:
```rust
fn sanitize_prompt(user_input: &str) -> String {
    user_input
        .replace("```", "")  // Remove code blocks
        .replace("![]", "")  // Remove images
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,;:!?-".contains(*c))
        .collect()
}
```

---

## Extension Points

### Adding a New Source Adapter

1. Implement the `Source` trait:
```rust
pub struct MySource {
    config: SourceConfig,
    client: reqwest::Client,
}

#[async_trait]
impl Source for MySource {
    fn source_type(&self) -> &'static str { "my_source" }
    fn name(&self) -> &'static str { "My Source" }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        // Fetch from API
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Scrape full content
    }
}
```

2. Register in `src-tauri/src/sources/mod.rs`:
```rust
pub fn get_source_registry() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(HackerNewsSource::new()),
        Box::new(MySource::new()),  // Add here
    ]
}
```

3. Add Tauri commands for configuration:
```rust
#[tauri::command]
async fn get_my_source_config() -> Result<MySourceConfig, String> {
    // Load config from database
}

#[tauri::command]
async fn set_my_source_config(config: MySourceConfig) -> Result<(), String> {
    // Save config to database
}
```

### Adding a New Manifest Type

1. Add to `ManifestType` enum in `src-tauri/src/ace/scanner.rs`:
```rust
pub enum ManifestType {
    // ... existing types ...
    MyManifest,  // Add here
}

impl ManifestType {
    fn filename(&self) -> &'static str {
        match self {
            ManifestType::MyManifest => "my_manifest.ext",
            // ...
        }
    }

    fn language(&self) -> &'static str {
        match self {
            ManifestType::MyManifest => "my_language",
            // ...
        }
    }
}
```

2. Add parser in `scanner.rs`:
```rust
fn parse_my_manifest(path: &Path) -> Result<ProjectSignal, String> {
    let content = fs::read_to_string(path)?;
    // Parse manifest, extract dependencies
}
```

### Adding a New LLM Provider

1. Implement the `LLMProvider` trait in `src-tauri/src/llm.rs`:
```rust
#[async_trait]
pub trait LLMProvider {
    async fn generate(&self, prompt: &str) -> Result<String, LLMError>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LLMError>;
}

pub struct MyLLMProvider {
    api_key: String,
    base_url: String,
}

#[async_trait]
impl LLMProvider for MyLLMProvider {
    async fn generate(&self, prompt: &str) -> Result<String, LLMError> {
        // Call API
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LLMError> {
        // Call embedding API
    }
}
```

2. Register in `get_llm_provider()`:
```rust
pub fn get_llm_provider(provider: &str) -> Result<Box<dyn LLMProvider>, LLMError> {
    match provider {
        "anthropic" => Ok(Box::new(AnthropicProvider::new())),
        "my_provider" => Ok(Box::new(MyLLMProvider::new())),  // Add here
        _ => Err(LLMError::UnknownProvider),
    }
}
```

---

## Future Enhancements

### Short-Term (Next 3 Months)

1. **GitHub Source Adapter**: Trending repos, releases
2. **Product Hunt Adapter**: New products, launches
3. **Twitter/X Adapter**: Tech influencers, announcements
4. **Score Autopsy UI**: Visual breakdown of relevance decisions
5. **Multi-Modal Scoring**: Social signals (upvotes, stars), temporal signals (trending velocity)

### Medium-Term (3-6 Months)

1. **Team Support**: Shared context across team members
2. **Mobile App**: iOS/Android companion with push notifications
3. **Email Digests**: Daily summaries sent to inbox
4. **Browser Extension**: Inline relevance scores on HN/Reddit
5. **Public API**: REST API for third-party integrations

### Long-Term (6-12 Months)

1. **Visual Signals**: CLIP-based screenshot analysis
2. **Cross-File Relationships**: Import graph modeling
3. **Recurring Patterns**: Time-of-day relevance, deadline awareness
4. **Collaborative Filtering**: "Users like you also found X relevant"
5. **On-Prem Enterprise**: Single-tenant deployment for companies

---

## Appendix: Key Algorithms

### Chunking Algorithm

```rust
fn chunk_text(text: &str, source: &str) -> Vec<(String, String)> {
    const CHUNK_SIZE: usize = 500;  // tokens
    const OVERLAP: usize = 50;

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();

    let mut i = 0;
    while i < words.len() {
        let end = (i + CHUNK_SIZE).min(words.len());
        let chunk = words[i..end].join(" ");

        let source_info = format!("{}:chunk_{}", source, chunks.len());
        chunks.push((source_info, chunk));

        i += CHUNK_SIZE - OVERLAP;
    }

    chunks
}
```

### Behavior Decay Algorithm

```rust
fn decay_affinities() {
    let now = SystemTime::now();

    for affinity in get_all_affinities() {
        let days_since = (now - affinity.last_updated).as_secs() / 86400;
        let decay_factor = 0.5_f32.powf(days_since as f32 / 30.0);

        let new_score = affinity.affinity_score * decay_factor;
        let new_confidence = affinity.confidence * decay_factor;

        update_affinity(affinity.id, new_score, new_confidence);
    }
}
```

---

**End of Technical Architecture Document**

*For implementation details, see source code at D:\4da-v3\src-tauri\src\*
*For API documentation, see docs/API.md*
*For user guide, see docs/USER_GUIDE.md*
