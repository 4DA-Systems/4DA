# 4DA — Developer Intelligence in Your Editor

> All signal. No feed. Privacy-first developer intelligence directly in VS Code.

4DA surfaces the developer signals that matter to *you* -- security advisories, breaking changes, trending tools, and tech shifts -- scored against your actual tech stack and interests. No algorithmic feeds, no social noise. Just signal.

![4DA Status Bar](https://4da.ai/vscode/status-bar.png)

## Features

### Status Bar -- Signal Count

4DA shows the number of relevant signals in your status bar. Click to open the signal panel.

- Color-coded by severity: white = normal, yellow = security warning
- Updates automatically every 5 minutes (configurable)
- Shows a shield icon when security alerts are active

### Signal Panel

View your top developer signals without leaving the editor. Open the 4DA sidebar to see your personalized intelligence feed.

- Signals scored against YOUR tech stack and interests using PASIFA scoring
- Color-coded badges: `SEC` (security), `BRK` (breaking change), `NEW` (tool discovery), `TRD` (tech trend), `LRN` (learning)
- Click any signal to open the full article in your browser
- Matte black design matching the 4DA desktop aesthetic

### Import Hover Intelligence

Hover over any import statement to see dependency intelligence inline.

- Current version installed in your project
- Active security advisories with CVE identifiers
- Breaking change alerts from your signal feed
- Works with **TypeScript**, **JavaScript**, **Python**, **Rust**, and **Go** imports

### Inline Diagnostics

Security and dependency alerts appear as squiggles directly on your import statements.

- **Red squiggle**: Critical CVE or security vulnerability in your dependency
- **Yellow squiggle**: High/medium severity advisory or breaking change
- **Blue info**: Low severity or informational alert
- Diagnostics update automatically when manifest files change (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, etc.)

## Requirements

**Recommended**: Install [4DA Desktop](https://4da.ai) for full intelligence with 12+ content sources and PASIFA relevance scoring.

**Standalone**: The extension works without 4DA Desktop installed. If the MCP server is not available, it runs in degraded mode -- no errors, just no signal data until connected.

### Supported Languages

| Language | Import Detection | Ecosystem |
|----------|-----------------|-----------|
| TypeScript / JavaScript | `import`, `require()` | npm |
| Python | `import`, `from ... import` | pip |
| Rust | `use crate::` | cargo |
| Go | `import "..."` | go |
| Ruby | File watching only | gem |

## Getting Started

1. Install the extension from the VS Code Marketplace
2. Open a project containing `package.json`, `Cargo.toml`, `pyproject.toml`, or `go.mod`
3. The status bar shows your signal count automatically
4. Hover over import statements for dependency intelligence
5. Open the 4DA sidebar panel for your personalized signal feed

## Extension Settings

| Setting | Description | Default |
|---------|-------------|---------|
| `4da.showStatusBar` | Show signal count in the status bar | `true` |
| `4da.refreshInterval` | Signal refresh interval in seconds | `300` |
| `4da.mcpServerPath` | Custom path to the MCP server (user settings only) | Auto-detected |

## Commands

| Command | Description |
|---------|-------------|
| `4DA: Show Signals` | Focus the signal panel sidebar |
| `4DA: Refresh Signals` | Manually refresh all signals, diagnostics, and the panel |
| `4DA: Open Desktop App` | Open the 4DA desktop application |

## How It Works

The extension communicates with 4DA's MCP server via JSON-RPC over stdio. The MCP server runs locally as a subprocess -- no cloud services, no external API calls from the extension itself.

**Server discovery** (checked in order):
1. User-configured path in VS Code user settings
2. `FOURDA_MCP_SERVER` environment variable
3. Global npm installation (`@4da/mcp-server`)
4. 4DA app data directory
5. Development path (relative to extension)

## Privacy

4DA is local-first. Your code, dependencies, and project data never leave your machine. The extension communicates exclusively with the 4DA MCP server running on localhost -- no cloud services, no telemetry, no data collection.

Your API keys (if using 4DA Desktop) are stored locally and never transmitted to any third party.

## Links

- [4DA Desktop](https://4da.ai) -- Full developer intelligence platform
- [MCP Server](https://www.npmjs.com/package/@4da/mcp-server) -- Use with Claude Code, Cursor, Windsurf
- [GitHub](https://github.com/4da-systems/4da) -- Source code

## License

Functional Source License 1.1, Apache 2.0 Future License (FSL-1.1-Apache-2.0)

Copyright 2026 4DA Systems Pty Ltd
