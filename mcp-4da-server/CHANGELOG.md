# Changelog

## 5.0.2 (2026-08-17)

### Docs: republish so npm serves the corrected README

No code changes. 5.0.1 was published to npm on 2026-08-16 minutes before the
AD-030 copy retirement (#469) landed the corrected README, so the npm package
page still described "Compound intelligence (learns over time)" and content that "compounds over time" — claims the product retired. <!-- retired-ok: quotes the retired claims verbatim to document exactly what this republish removed from npm -->
This release exists to put the current README (and the corrected app-vs-MCP
comparison) on npm.

## 5.0.1 (2026-08-16)

### Security: HTTP transport accepted forged authentication tokens

**Affects `--http` only. The default stdio transport was never exposed.**

`extractAuthClaims` decoded the JWT payload and validated the claims inside it
but never read the signature segment — no HMAC was ever computed, despite the
function's own docstring claiming "HMAC-SHA256 verification against the shared
relay secret". Any request could present
`Authorization: Bearer x.<base64url({"team_id":"x","client_id":"y","role":"admin","exp":9999999999})>.x`
and be accepted as a team admin.

Three defects compounded it:

- The transport's header claimed "Binds to 127.0.0.1 only", but `--host`
  accepted any address, so `--http --host 0.0.0.0` served all 14 tools to the
  network.
- The DNS rebinding guard was written as `if (origin) { ...check... }`, so a
  request with **no** `Origin` header — every non-browser client, and any
  attacker — skipped it entirely.
- `hasPermission()` existed but was never called, so even legitimate tokens got
  no role enforcement: a `viewer` could invoke every write tool.

Fixed:

- **Real signature verification.** HMAC-SHA256 over
  `base64url(header).base64url(payload)`, constant-time compared
  (`crypto.timingSafeEqual`) against the shared secret, **before** any claim is
  read. The algorithm is pinned to `HS256`, so `alg: none`, other HMAC widths,
  and asymmetric-algorithm confusion are all refused. `exp` is now mandatory
  (a signed token with no expiry is a permanent credential) and `nbf` is
  honoured, both with the issuer's 60s leeway. No new dependency — Node's
  built-in `node:crypto`.
- **Fail closed.** With no secret configured (`MCP_AUTH_SECRET`, falling back
  to `JWT_SECRET` for parity with a co-deployed relay), *every* token is
  rejected. A server that cannot verify a signature must not trust claims.
- **Host-header DNS rebinding guard on every request**, using the SDK's
  `validateHostHeader`/`validateOriginHeader`. `Host` is mandatory, so the
  check can no longer be skipped by omitting a header; a foreign `Origin` is
  still refused when present.
- **Safe `--host`.** 127.0.0.1 remains the default. A non-loopback bind is
  **refused at startup** unless a secret is configured, prints a warning
  banner, and forces authentication on for every request regardless of
  `MCP_AUTH_REQUIRED`.
- **Role enforcement wired up.** Every tool call is checked against the
  verified role via the registry's existing `readOnlyHint` annotation:
  `viewer` = read-only, `member`/`admin` = read + write. Unknown tools fail
  closed as writes.
- Removed `isNetworkTierAllowed()` — dead code with no tier data anywhere in
  this package.

**Behaviour change:** an existing `--http --host 0.0.0.0` deployment will now
refuse to start until `MCP_AUTH_SECRET` is set, and only accepts localhost-class
`Host` headers unless `MCP_ALLOWED_HOSTS` names the address clients use. This is
deliberate: that deployment was previously reachable by anyone with forgeable
admin credentials.

## 5.0.0 (2026-08-11)

### Breaking: Node.js 20+ required

The server now requires Node.js >= 20 (previously >= 18; Node 18 has been EOL
since April 2025). No tool behavior changes.

### Changed: migrated to MCP TypeScript SDK v2 + 2026-07-28 protocol support

Replaced the discontinued `@modelcontextprotocol/sdk` v1 with the v2 packages
(`@modelcontextprotocol/server` + `@modelcontextprotocol/node`) and moved both
transports onto the v2 serving entries:

- **stdio** now goes through `serveStdio`, which negotiates the protocol era
  per connection: 2025-era hosts (Claude Code, Claude Desktop, Cursor — the
  classic `initialize` handshake) are served exactly as before, and hosts
  speaking the new stateless 2026-07-28 revision (`server/discover`) are now
  supported on the same endpoint.
- **--http** now goes through `createMcpHandler` + `toNodeHandler`: stateless
  serving for both eras from one factory — the previous per-request transport
  wiring is gone. Health endpoint, localhost binding, Origin-header DNS
  rebinding protection, and the optional JWT auth gate are unchanged.

Existing clients need no changes — protocol version negotiation is untouched
for 2025-era hosts and verified against a v1-SDK client.

### Fixed: startup log reported a stale version

The stdio startup line hardcoded "v4.6.3" regardless of the installed version;
it now derives from package.json like `--version` and `serverInfo`.

## 4.6.2 (2026-06-17)

### Fixed: vulnerability_scan returned empty severity, fix versions, and summaries

The scanner enumerated advisories via OSV's `/v1/querybatch` endpoint, which returns
only `{ id, modified }` per vulnerability — but it then mapped that index-only object
as if it were the full record. Every rich field collapsed to a default: `summary`
became the bare advisory ID, `fixed_version` was always `null` (so every
recommendation read "no fix version published"), `references` was always empty,
`published` actually carried the *modified* timestamp, and severity was hardcoded
(`medium` for any GHSA, `unknown` for anything else) rather than derived.

The scanner now hydrates each matched advisory via `/v1/vulns/{id}` (cache-first,
24h TTL, bounded concurrency over the vulnerable subset only) and derives severity
honestly: a CVSS base score — computed from the vector string when OSV provides one
(new `cvss.ts`, CVSS v3.0/3.1) — wins, then the GitHub-advisory severity label, then
`unknown`. No more fabricated default buckets. Real fix versions, CVE aliases,
summaries, and reference links now flow through.

## 4.6.1 (2026-06-11)

### Improved: prescriptive tool descriptions

All 14 tool descriptions now state WHEN to call the tool, not just what it does
(e.g. "Call when the user asks about security, vulnerabilities, or CVEs"). Both the
slim tool list and the full schemas carry explicit triggers, so calling models select
the right tool more reliably. A regression test enforces this going forward.

### Changed: license

Relicensed to Apache-2.0 (from MIT). The 4DA desktop app remains under
FSL-1.1-Apache-2.0; this MCP connector is intentionally permissive to maximize adoption.

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
