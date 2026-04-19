# Awesome MCP Servers — PR Submission Draft

## PR Title

```
Add @4da/mcp-server — developer intelligence with codebase-aware content scoring
```

## Line to Add

Insert this line in the **Developer Tools** section, alphabetically between
`rsdouglas/janee` and `ryan0204/github-repo-mcp`:

```markdown
- [runyourempire/4DA](https://github.com/runyourempire/4DA/tree/main/mcp-4da-server) 📇 🏠 🍎 🪟 🐧 - 35 MCP tools that score content from Hacker News, arXiv, Reddit, and GitHub against your actual codebase. Includes developer context profiling, decision memory, knowledge gap detection, tech radar, and agent session handoff. Privacy-first — everything stays local. `npx @4da/mcp-server`
```

## PR Description / Body

```markdown
## What

Adds [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) to the Developer Tools section.

## About the server

**@4da/mcp-server** provides 35 MCP tools that connect AI coding assistants (Claude Code, Cursor, Windsurf, Copilot) to a developer's local codebase and score internet content against their tech stack.

**Key capabilities:**
- **Content scoring** — Scores articles from Hacker News, arXiv, Reddit, and GitHub against the user's actual project dependencies and tech stack
- **Developer context** — Auto-discovers project identity, interests, and technology affinities from local codebases
- **Intelligence analysis** — Topic connections, signal chains, semantic shifts, trend analysis, and knowledge gap detection
- **Decision memory** — Records, checks, and replays architectural decisions across sessions
- **Agent integration** — Session briefs, context packets, delegation scoring, and cross-agent persistent memory

**Privacy-first:** All data stays on the user's machine. No telemetry, no cloud dependencies. Works with local Ollama models.

**Install:** `npx @4da/mcp-server`

- **npm:** https://www.npmjs.com/package/@4da/mcp-server
- **GitHub:** https://github.com/runyourempire/4DA/tree/main/mcp-4da-server
- **License:** MIT
- **Language:** TypeScript
- **Platforms:** macOS, Windows, Linux (local service)
```

## Checklist (from CONTRIBUTING.md)

- [x] Server name linked to its repository
- [x] Brief description of functionality
- [x] Categorized under relevant section (Developer Tools)
- [x] Alphabetical order maintained (between `rsdouglas/janee` and `ryan0204/github-repo-mcp`)
- [x] One server per line
- [x] Follows existing format: `- [org/repo](url) <badges> - Description`
- [x] Badges used: 📇 (TypeScript), 🏠 (Local Service), 🍎 (macOS), 🪟 (Windows), 🐧 (Linux)

## How to Submit

1. Fork https://github.com/punkpeye/awesome-mcp-servers
2. Create branch: `git checkout -b add-4da-mcp-server`
3. Edit `README.md` — add the line above in the Developer Tools section
4. Commit: `git commit -m "Add @4da/mcp-server — developer intelligence with codebase-aware content scoring"`
5. Push and open PR with the title and body above
