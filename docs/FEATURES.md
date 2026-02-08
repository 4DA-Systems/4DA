# 4DA Features Guide

## Core Features

### Autonomous Context Engine (ACE)

ACE is 4DA's intelligence layer that understands your work without manual configuration.

#### Project Scanning

ACE detects projects by recognizing manifest files:

| Manifest | Language/Framework |
|----------|-------------------|
| Cargo.toml | Rust |
| package.json | Node.js/JavaScript |
| pyproject.toml | Python |
| go.mod | Go |
| pom.xml | Java (Maven) |
| build.gradle | Java/Kotlin (Gradle) |
| Gemfile | Ruby |
| composer.json | PHP |
| pubspec.yaml | Dart/Flutter |
| CMakeLists.txt | C/C++ |
| Makefile | Various |
| Dockerfile | Docker |

#### File Watching

Real-time monitoring of your context directories:

- Detects file changes with debouncing (prevents spam)
- Extracts topics from modified files
- Updates context weights based on recency
- Persists state across restarts

#### Git Analysis

Extracts context from your Git history:

- Recent commit messages
- Changed file patterns
- Active branches
- Commit frequency

#### Behavior Learning

Learns from your interactions:

- **Clicks**: Moderate positive signal
- **Saves**: Strong positive signal
- **Dismissals**: Strong negative signal
- **Ignores**: Weak negative signal

Over time, 4DA learns which topics resonate with you and which to filter out.

### Multi-Source Analysis

#### Hacker News

- Top stories from front page
- Configurable item limit
- Extracts story URLs for content analysis

#### arXiv

- Latest papers across categories
- Title and abstract analysis
- Author metadata preserved

#### Reddit

- Configurable subreddits
- Post titles and self-text
- Comment summaries available

### Relevance Scoring

The unified scoring formula:

```
combined_score = base_score * affinity_multiplier * (1.0 - anti_penalty)
```

Where:
- `base_score` = Semantic similarity to your interests
- `affinity_multiplier` = 1.0 + (avg_affinity * 0.7), clamped [0.3, 1.7]
- `anti_penalty` = Sum of anti-topic matches, capped at 0.7

### KNN Search

4DA uses sqlite-vec for efficient vector similarity search:

- O(log n) search complexity
- Local embeddings via fastembed
- No external API calls for search
- Incremental indexing

## Privacy Features

### Local-Only Processing

- All file scanning happens locally
- Embeddings computed on-device
- Database stored locally
- No telemetry

### BYOK (Bring Your Own Key)

- You provide API keys
- Keys stored locally only
- Keys never sent to 4DA servers
- Full cost transparency

### Data Isolation

- Raw file contents never leave your machine
- Only titles/summaries sent to LLM (if configured)
- Option for fully local with Ollama

## System Tray

4DA runs in the background:

- Continuous source monitoring
- Scheduled analysis jobs
- System notifications for high-relevance items
- Quick access to settings

## Health Monitoring

ACE monitors its own health:

- Component status tracking
- Automatic fallback chains
- Accuracy metrics
- Anomaly detection

### Anomaly Detectors

7 built-in detectors:

1. **Sudden Interest Shift** - Detects dramatic topic changes
2. **Engagement Drop** - Monitors declining interaction rates
3. **Source Imbalance** - Alerts on source dominance
4. **Relevance Drift** - Tracks scoring distribution shifts
5. **Processing Latency** - Monitors analysis speed
6. **Memory Pressure** - Tracks resource usage
7. **Context Staleness** - Alerts on outdated context

## Cost Management

### Tracking

- Per-provider cost tracking
- Daily cost aggregation
- Usage statistics

### Limits

- Configurable daily cost limits
- Provider-specific limits
- Automatic throttling when limits reached

## Digest System

### Email Digests

Schedule daily/weekly digests:

- Top N items by relevance
- Grouped by source
- Customizable templates

### Notifications

System notifications for:

- High-relevance items (configurable threshold)
- Analysis completion
- Health alerts

## Developer Features

### Tauri Commands

82+ commands exposed for:

- Analysis operations
- Context management
- ACE control
- Settings CRUD
- Health monitoring

See [API Reference](./API_REFERENCE.md) for details.

### MCP Integration

Memory server for AI agents:

- `remember_decision` / `recall_decisions`
- `update_state` / `get_state`
- `search_sessions` / `get_session_messages`
- `record_metric` / `get_quality_report`
