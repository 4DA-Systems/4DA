# Changelog

All notable changes to 4DA will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [1.0.0] - 2026-03-08

### Highlights

4DA (4 Dimensional Autonomy) is a privacy-first desktop app that surfaces developer-relevant content from the internet — scored against your actual codebase, running entirely on your machine. No telemetry, no cloud dependency, no configuration required. See useful content within 60 seconds of first launch.

### Features

**Intelligence & Scoring**
- 5-axis relevance scoring (PASIFA) evaluates content against your actual codebase and tech stack
- 10 built-in sources: Hacker News, Reddit, arXiv, GitHub Trending, Product Hunt, RSS, YouTube, Twitter/X, DevTo, and Lobsters
- Taste Test calibration: 15-card interactive session tunes scoring to your preferences
- Developer DNA: auto-detected tech stack profiles your development identity across graduated tiers
- Temporal clustering groups related articles across sources and time
- Novelty detection filters seen-before content and boosts new releases
- Content quality analysis evaluates title quality, content depth, and source authority

**AI Briefings (Free)**
- AI-generated daily intelligence briefings summarize your top signals
- AI-generated weekly digest of the most important developments across all sources

**Intelligence Layer (Signal)**
- Score Autopsy explains exactly why each article received its score
- Natural language queries against your content stream
- Signal cards surface critical and high-priority items above the briefing
- Persistent briefing state with auto-refresh and freshness indicators

**STREETS Playbook (Free)**
- All 7 STREETS modules included free inside 4DA
- Interactive lessons, templates, and sovereign developer profile
- Suns Dashboard for strategic technology tracking

**Developer Tools**
- Essential Toolkit with 7 micro-tools for daily development
- Command Deck with git operations and project management
- Decision Journal for tracking and reviewing technical decisions
- Knowledge gap detection identifies blind spots in your project dependencies
- MCP integration: 33 tools for Claude Code, Cursor, and Copilot

**Privacy & Security**
- Runs 100% locally — zero telemetry, zero data collection
- BYOK (Bring Your Own Key) — API keys never leave your machine
- All AI processing happens on-device with Ollama support
- Restrictive CSP blocks unauthorized network requests
- Keyword-only mode available without any AI provider

**Localization**
- Full i18n support with built-in translation editor
- Auto-detected locale with manual override
- Translation override system for custom terminology

### Technical
- Built with Tauri 2.0 (Rust) + React 19 + TypeScript + SQLite
- sqlite-vec for local vector search and semantic matching
- 2,435 tests (1,618 Rust + 817 frontend)
- 11MB installer (Windows NSIS)
- Auto-updater with Minisign signature verification
- Standalone CLI binary (`4da`) for terminal workflows
- FSL-1.1-Apache-2.0 license (converts to Apache 2.0 after 2 years)

### Known Limitations
- Ollama required for local AI features (optional — keyword-only mode available without it)
- First analysis may take 60-90 seconds depending on number of configured sources
- Twitter/X source requires API key
