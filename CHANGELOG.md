# Changelog

## [1.0.0] - 2026-02-17

### First release

Privacy-first developer intelligence that surfaces what matters from the noise. Runs entirely on your machine.

### Features

- **5-axis relevance scoring** — context, interest, ace, learned, and dependency signals with confirmation gate
- **Domain profile** — graduated tech identity (primary, dependency, detected, interest, off-domain)
- **Content quality analysis** — title quality, content depth, source authority scoring
- **Novelty detection** — intro content penalty, release boost, seen-before filtering
- **Intent scoring** — work topics from recent git/file activity boost relevant content
- **AI briefings** — persistent daily briefings with auto-refresh and freshness indicators
- **Signal cards** — critical and high-priority items surfaced above the briefing
- **Article reader** — in-app content viewing with AI summaries
- **Saved items** — bookmark and collect items for later
- **Content type badges** — visual indicators for content categories
- **Zero-click launch** — auto-analysis on startup with persistent briefing state
- **Background monitoring** — auto-enabled after first successful analysis
- **CLI binary** — standalone `4da` command (briefing, signals, gaps, health, status)
- **MCP server** — Model Context Protocol integration for Claude Code and other MCP hosts
- **Knowledge gaps** — detect gaps in your project dependencies
- **Dependency intelligence** — match content against your project's actual dependencies

### Sources

- Hacker News, Reddit, GitHub Trending, DevTo, Lobsters, Product Hunt, ArXiv, TechCrunch

### Stack

- Tauri 2.0 (Rust + React + TypeScript + SQLite)
- Local-first with Ollama fallback
- BYOK (bring your own key) for AI features
