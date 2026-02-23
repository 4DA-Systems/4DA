# Architecture Reference
## System Structure for 4DA

---

## Canonical Source

The complete architecture specification lives in:
**`/specs/ARCHITECTURE.md`** - Full system design
**`/specs/ACE-STONE-TABLET.md`** - ACE subsystem specification

This file provides a quick reference. For detailed design, read the specs.

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         4DA                                      │
│                 "All Signal. No Feed."                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   Context    │    │    World     │    │  Relevance   │       │
│  │   Membrane   │───▶│   Scanner    │───▶│    Judge     │       │
│  │  (Local)     │    │  (External)  │    │  (Filter)    │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│         │                                       │                │
│         │                                       ▼                │
│         │                              ┌──────────────┐          │
│         │                              │   Delivery   │          │
│         │                              │    Engine    │          │
│         │                              └──────────────┘          │
│         │                                       │                │
│         ▼                                       ▼                │
│  ┌──────────────────────────────────────────────────────┐       │
│  │                   Learning Engine                     │       │
│  │              (Feedback → Model Updates)               │       │
│  └──────────────────────────────────────────────────────┘       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Context Membrane (ACE)
- **Location:** `src-tauri/src/ace/`
- **Purpose:** Understand user's local context
- **Key Files:**
  - `scanner.rs` - File/project scanning
  - `watcher.rs` - Real-time file watching
  - `git.rs` - Git history analysis
  - `behavior.rs` - Implicit learning
  - `db.rs` - ACE database operations
  - `embedding.rs` - Local embedding service
  - `readme_indexing.rs` - README file indexing
  - `topic_embeddings.rs` - Topic embedding cache

### 2. World Scanner
- **Location:** `src-tauri/src/sources/`
- **Purpose:** Fetch external content
- **Key Files:**
  - `hackernews.rs` - HN adapter
  - `arxiv.rs` - arXiv adapter
  - `reddit.rs` - Reddit adapter

### 3. Relevance Judge
- **Location:** `src-tauri/src/scoring/`, `src-tauri/src/commands.rs`, `src-tauri/src/llm.rs`
- **Purpose:** Score items for relevance
- **Key Concepts:**
  - Embedding similarity (primary)
  - LLM re-ranking (optional)
  - Multi-stage filtering

### 4. Delivery Engine
- **Location:** `src-tauri/src/` (notifications), `src/App.tsx` (UI)
- **Purpose:** Surface relevant items
- **Modes:** Feed, Notifications, Digests (planned)

### 5. Learning Engine
- **Location:** `src-tauri/src/ace/behavior.rs`
- **Purpose:** Improve over time
- **Signals:** Clicks, dismissals, saves, explicit feedback

### 6. Void Engine (Ambient Visualization)
- **Location:** `src-tauri/src/void_engine.rs`, `src-tauri/src/void_commands.rs`
- **Frontend:** `src/components/void-engine/`
- **Purpose:** Communicate system state through ambient visual signals
- **Status:**
  - **Heartbeat (Production):** 48px WebGL2/CSS glow in header. Driven by real backend events. Maps pulse/heat/burst/morph/error/staleness to visual changes. Zero-cost when idle (change-driven, not polled).
  - **Universe (Experimental):** Full-screen Three.js 3D visualization. Code-split via React.lazy (~908KB, loads only on click). Projects embeddings to 3D via Johnson-Lindenstrauss random projection. Particle selection, search, camera fly-to, neighbor discovery. **Not actively maintained** - see AD-012 in DECISIONS.md.
- **Key Files:**
  - `void_engine.rs` - Signal system, projection math, universe builder, k-means, 22 tests
  - `void_commands.rs` - 4 Tauri commands (get_void_signal, void_get_universe, void_get_particle_detail, void_get_neighbors)
  - `VoidHeartbeat.tsx` - WebGL2 fragment shader with CSS fallback
  - `VoidEngine.tsx` - Orchestrator (heartbeat click -> lazy-load universe)

### 7. Signal Classifier
- **Location:** `src-tauri/src/signals.rs`
- **Frontend:** `src/components/SignalsPanel.tsx`
- **Purpose:** Classify scored items into actionable signal types (security_alert, breaking_change, tool_discovery, tech_trend, learning, competitive_intel) with priority levels
- **Key Files:**
  - `signals.rs` - Pattern-matching classifier, no external deps
  - `SignalsPanel.tsx` - Filterable, color-coded signal display

---

## Tech Stack Summary

| Layer | Technology |
|-------|------------|
| Application Shell | Tauri 2.0 |
| Backend | Rust |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 |
| Database | SQLite + sqlite-vec |
| Embeddings | OpenAI text-embedding-3-small / Ollama nomic-embed-text (384-dim) |
| LLM (optional) | Claude, OpenAI, Ollama |

---

## Data Flow

```
User Files  ──┐
              ├──▶ Context Membrane ──▶ Interest Model
Git History ──┘                              │
                                             ▼
External    ──▶ World Scanner ──▶ Relevance Judge ──▶ Delivery
Sources                              ▲
                                     │
                              User Feedback
```

---

## Key Directories

```
4DA/
├── .ai/                 # CADE cognition artifacts (YOU ARE HERE)
├── .claude/             # Runtime hooks and state
├── src-tauri/           # Rust backend
│   └── src/
│       ├── ace/         # Autonomic Context Engine
│       ├── scoring/     # Relevance scoring (mod.rs + 10 submodules)
│       ├── sources/     # External adapters
│       ├── lib.rs       # App entry (run, setup, re-exports)
│       ├── commands.rs  # Tauri command handlers + background jobs
│       ├── types.rs     # Shared struct/enum definitions
│       ├── state.rs     # Global statics + accessor functions
│       ├── utils.rs     # Text processing, vector math, topics
│       ├── embeddings.rs # Embedding generation (OpenAI/Ollama)
│       ├── events.rs    # Tauri event emission helpers
│       └── db.rs        # Database layer
├── src/                 # React frontend
├── specs/               # Design documents (ARCHITECTURE.md, ACE-STONE-TABLET.md)
└── mcp-memory-server/   # MCP memory tools
```

---

## Critical Boundaries

### Tauri IPC Boundary
- All Rust↔Frontend communication via IPC commands
- No direct file access from frontend
- Commands defined in `commands.rs` and domain-specific `*_commands.rs` modules
- `lib.rs` re-exports all public types and accessors via `pub use` / `pub(crate) use`

### Privacy Boundary
- Raw data stays local
- Only aggregated/anonymized data may leave (with consent)
- API calls send minimal context

### Embedding Boundary
- Embeddings via OpenAI text-embedding-3-small (384-dim) or Ollama nomic-embed-text (Matryoshka truncation to 384-dim)
- Zero-config fallback: tries Ollama at localhost:11434, then degrades to zero vectors
- Model changes trigger re-embedding

---

## For Deep Dives

| Topic | Document |
|-------|----------|
| Full architecture | `specs/ARCHITECTURE.md` |
| ACE specification | `specs/ACE-STONE-TABLET.md` |
| Phase 0 scope | `specs/PHASE-0-SCOPE.md` |
| Context engine | `specs/CONTEXT-ENGINE.md` |
| Wisdom layer | `.ai/WISDOM.md` |
| Invariants | `.ai/INVARIANTS.md` |
| Decisions | `.ai/DECISIONS.md` |
| Failure modes | `.ai/FAILURE_MODES.md` |

---

*This is a reference. For full details, read the specs directory.*
