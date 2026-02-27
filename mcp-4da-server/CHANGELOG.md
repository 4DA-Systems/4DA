# Changelog

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
- PASIFA scoring algorithm — 5-axis codebase-aware relevance with confidence weighting
- Privacy-first — all data stays local, zero telemetry
- BYOK — bring your own API keys, never stored remotely
- Works offline with Ollama fallback for embeddings
- Dual transport: stdio (default) and Streamable HTTP
- SQLite storage with automatic migrations
- Compatible with Claude Code, Cursor, Windsurf, VS Code Copilot, and any MCP client
