# 4DA - The Internet Searches For You

**4DA (4 Dimensional Autonomy)** is an ambient intelligence layer that monitors your local context, watches external sources continuously, filters ruthlessly (99.9% rejection), and delivers only what matters - before you know you need it.

## Features

- **Autonomous Context Engine (ACE)** - Scans your projects, Git history, and file activity to understand what you're working on
- **Multi-Source Analysis** - Monitors Hacker News, arXiv, and Reddit simultaneously
- **Behavior Learning** - Learns from your interactions to improve relevance over time
- **BYOK (Bring Your Own Key)** - Use your own API keys for LLM providers (Anthropic, OpenAI, Ollama)
- **Privacy First** - All data stays local. Raw files never leave your machine.
- **Cost Conscious** - Transparent cost tracking with configurable daily limits

## Tech Stack

| Component | Technology |
|-----------|------------|
| Application | Tauri 2.0 (Rust + WebView) |
| Frontend | React 18 + TypeScript + Tailwind CSS |
| Database | SQLite 3.45+ with sqlite-vec |
| Embeddings | fastembed (local) or OpenAI |
| LLM | Anthropic Claude / OpenAI / Ollama |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- [pnpm](https://pnpm.io/) (or npm/yarn)

### Development

```bash
# Clone and enter directory
cd 4da-v3

# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

The app will open at `localhost:4444` with hot-reload enabled.

### Production Build

```bash
pnpm tauri build
```

This creates platform-specific installers in `src-tauri/target/release/bundle/`.

## Configuration

### API Keys (BYOK)

4DA requires API keys for LLM functionality. Configure them in Settings:

1. **Anthropic** (recommended) - For Claude-based analysis
2. **OpenAI** - For GPT-based analysis or embeddings
3. **Ollama** - For fully local, free operation

Keys are stored locally and never transmitted except to their respective APIs.

### Context Directories

Add directories you want 4DA to monitor:

- Project directories (Cargo.toml, package.json detected)
- Research folders
- Note collections

4DA will scan these for context signals automatically.

## Architecture

```
4DA/
├── ACE (Autonomous Context Engine)
│   ├── Scanner - Detects projects, tech stacks, topics
│   ├── Watcher - Real-time file change monitoring
│   ├── Behavior - Learns from user interactions
│   └── Health - System monitoring and fallbacks
├── Sources
│   ├── Hacker News - Tech news and discussions
│   ├── arXiv - Academic papers
│   └── Reddit - Community discussions
├── Context Engine - Interest management
├── LLM Integration - Multi-provider support
└── Digest System - Email/notification delivery
```

## How It Works

1. **Context Detection** - ACE scans your monitored directories to understand your tech stack, active projects, and topics
2. **Source Fetching** - Background jobs poll external sources for new content
3. **Relevance Scoring** - Items are scored using:
   - Semantic similarity (KNN via sqlite-vec)
   - Topic affinity (learned from behavior)
   - Anti-topic filtering (learned rejections)
4. **Delivery** - High-relevance items surface in the app or via notifications

## Development

### Running Tests

```bash
# Rust tests
cargo test

# Frontend type checking
pnpm typecheck

# Lint
pnpm lint
```

### Project Structure

```
4da-v3/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── lib.rs      # Main entry, Tauri commands
│   │   ├── ace/        # Autonomous Context Engine
│   │   ├── sources/    # External source adapters
│   │   ├── context_engine.rs
│   │   ├── db.rs       # Database operations
│   │   └── llm.rs      # LLM integration
│   └── Cargo.toml
├── src/                # React frontend
│   ├── App.tsx
│   └── components/
├── specs/              # Architecture specifications
├── .ai/                # CADE engineering docs
└── .claude/            # Session state tracking
```

## Status

| Phase | Status |
|-------|--------|
| Phase 0: POC | Complete |
| Phase 1: Core Loop | ~95% |
| Phase 2: Learning | ~80% |
| Phase 3: Polish | In Progress |

### What's Built

- 82+ Tauri commands wired
- 3 source adapters (HN, arXiv, Reddit)
- Complete ACE scanner (12 manifest types)
- sqlite-vec KNN search
- File watcher with debouncing
- Behavior learning with signal tracking
- LLM integration (Anthropic/OpenAI/Ollama)
- System tray + background monitoring
- All 7 anomaly detectors
- Unified multiplicative relevance scoring

## License

Private - Not for redistribution.

## Contributing

This is a private project. Contact the maintainer for access.

---

*Built with Tauri 2.0, React 18, and SQLite*
