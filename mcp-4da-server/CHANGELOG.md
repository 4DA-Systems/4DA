# Changelog

## 4.6.0 (2026-04-24)

### Breaking: Tool consolidation — 39 → 14 tools

Removed 25 tools that returned empty, broken, or low-value data through MCP.
Every remaining tool reliably returns useful, actionable information.

**Kept (14):**
- `vulnerability_scan` — live CVE scanning (standalone)
- `dependency_health` — health score + version freshness (standalone)
- `upgrade_planner` — ranked upgrade recommendations (standalone)
- `what_should_i_know` — pre-task intelligence briefing (standalone)
- `ecosystem_pulse` — filtered ecosystem news (standalone)
- `get_context` — tech stack + interests (standalone)
- `get_relevant_content` — scored content feed (full mode)
- `get_actionable_signals` — classified alerts (full mode)
- `knowledge_gaps` — dependency blind spots (full mode)
- `record_feedback` — save/dismiss to teach the system (full mode)
- `decision_memory` — persistent architectural decisions (standalone)
- `check_decision_alignment` — verify tech choices (standalone)
- `agent_memory` — cross-session persistent memory (standalone)
- `developer_dna` — tech identity profile (full mode)

**Removed:** explain_relevance, score_autopsy, trend_analysis, daily_briefing,
context_analysis, topic_connections, signal_chains, semantic_shifts,
attention_report, source_health, config_validator, llm_status,
export_context_packet, reverse_mentions, project_health, tech_radar,
agent_session_brief, delegation_score, autophagy_status, decision_windows,
compound_advantage, record_agent_feedback, get_agent_feedback_stats,
trust_summary, preemption_feed

**Fixed:**
- `get_relevant_content` now uses Rust-computed PASIFA scores when the desktop
  app database is present, instead of the simplified TypeScript keyword scorer.
  Results are dramatically more accurate.

## 1.0.0 (2026-02-27)

Initial public release.

### Tools (27)

**Content & Scoring**
- `get_relevant_content` — Query filtered content by relevance, source, time
- `explain_relevance` — Understand why an item scored the way it did
- `record_feedback` — Teach 4DA what you like/dislike (click, save, dismiss)
- `score_autopsy` — Deep forensic analysis of relevance scores

**Intelligence & Analysis**
- `daily_briefing` — Executive summary of discoveries
- `trend_analysis` — Statistical patterns, anomalies, and predictions
- `get_actionable_signals` — Classify content into actionable signals with priority levels
- `signal_chains` — Get causal signal chains connecting related events over time
- `semantic_shifts` — Detect narrative shifts in topics you follow
- `topic_connections` — Build knowledge graphs from content

**Developer Context**
- `get_context` — Get user's interests, tech stack, learned affinities
- `context_analysis` — Optimize your context for better relevance
- `knowledge_gaps` — Detect knowledge gaps in your project dependencies
- `project_health` — Project health radar for dependency freshness and security
- `reverse_mentions` — Find where your projects are mentioned in sources
- `attention_report` — Analyze attention allocation vs codebase needs
- `developer_dna` — Export your Developer DNA — tech identity, dependencies, engagement, blind spots

**Decision & Memory**
- `decision_memory` — Manage developer decisions (record, list, check, update, supersede)
- `tech_radar` — Generate tech radar from decisions and content signals
- `check_decision_alignment` — Check if a technology aligns with active decisions
- `decision_windows` — View time-bounded opportunities requiring attention
- `compound_advantage` — Measures intelligence leverage for decisions

**Agent Integration**
- `agent_memory` — Cross-agent persistent memory — store and recall across sessions
- `agent_session_brief` — Tailored session startup context for AI agents
- `delegation_score` — Should the agent proceed or ask the human?
- `export_context_packet` — Generate portable context packet for session handoff

**System**
- `source_health` — Diagnose source fetching and data quality issues
- `config_validator` — Validate configuration and detect issues
- `llm_status` — Check LLM/Ollama configuration and availability
- `autophagy_status` — Intelligence metabolism status — calibration accuracy, anti-patterns

### Features

- 11 content sources: Hacker News, Reddit, Twitter/X, GitHub, RSS, YouTube, arXiv, Dev.to, Lobsters, Product Hunt, custom feeds
- PASIFA scoring algorithm — 5-axis codebase-aware relevance with confidence calibration
- Privacy-first — local SQLite reads; the only outbound call is vulnerability_scan (package names + versions to OSV.dev), zero telemetry
- BYOK — bring your own API keys, never stored remotely
- Works offline with Ollama fallback for embeddings
- Dual transport: stdio (default) and Streamable HTTP
- SQLite storage with automatic migrations
- Compatible with Claude Code, Cursor, Windsurf, VS Code Copilot, and any MCP client
