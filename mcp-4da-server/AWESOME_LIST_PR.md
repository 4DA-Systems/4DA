# Awesome MCP Servers — PR Submission Draft

## PR Title

```
Add @4da/mcp-server — developer intelligence with codebase-aware content scoring
```

## Line to Add

Insert this line in the **Developer Tools** section, alphabetically between
`rsdouglas/janee` and `ryan0204/github-repo-mcp`:

```markdown
- [runyourempire/4DA](https://github.com/runyourempire/4DA/tree/main/mcp-4da-server) 📇 🏠 🍎 🪟 🐧 - 14 MCP tools (9 standalone): live vulnerability scanning (OSV.dev) across npm/Rust/Python/Go, dependency health and upgrade planning, codebase-aware content scoring, Hacker News ecosystem pulse, decision memory, knowledge-gap detection, and cross-agent persistent memory. Privacy-first - everything stays local. `npx @4da/mcp-server`
```

## PR Description / Body

```markdown
## What

Adds [@4da/mcp-server](https://www.npmjs.com/package/@4da/mcp-server) to the Developer Tools section.

## About the server

**@4da/mcp-server** provides 14 MCP tools (9 work standalone; 5 surface richer data when the 4DA desktop app is installed) that connect AI coding assistants (Claude Code, Cursor, Windsurf, Copilot) to a developer's local codebase - live vulnerability scanning via OSV.dev, content scoring against their tech stack, and persistent project memory. Zero config: on first run it creates a local database and scans the project.

**Key capabilities:**
- **Dependency security** - Live CVE scanning via OSV.dev, dependency health (version freshness, deprecation, CVE counts), and ranked upgrade planning across npm, Rust, Python, and Go
- **Content scoring** - Ranks articles and releases against the user's actual dependencies and tech stack; Hacker News ecosystem pulse standalone, full multi-source feed with the 4DA desktop app
- **Developer context** - Auto-discovers project identity, interests, and technology affinities from the local codebase
- **Intelligence** - Pre-task briefings, actionable-signal classification, and knowledge-gap detection (blind-spot dependencies you never read about)
- **Decision memory** - Records architectural decisions and checks proposed changes against them across sessions
- **Agent integration** - Cross-agent persistent memory and a Developer DNA profile shared across editors

**Privacy-first:** Local SQLite reads; the only outbound call is vulnerability_scan (package names + versions to OSV.dev). No code leaves your machine. No accounts, no telemetry. Works with local Ollama models.

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
