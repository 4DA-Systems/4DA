# 4DA Source Plugin Development Guide

Plugins add custom content sources to 4DA. A plugin is a standalone executable that 4DA
runs on a schedule — it receives configuration on stdin and returns content items on stdout.

## Quick Start

1. Create a directory under `data/plugins/` named after your plugin
2. Add a `manifest.json` describing the plugin
3. Add your executable (Node.js script, Python script, or native binary)
4. Restart 4DA — it discovers plugins automatically

```
data/plugins/
  my-plugin/
    manifest.json
    plugin.js        # or plugin.py, or a native binary
```

## Protocol

The plugin protocol is JSON-over-stdio:

1. 4DA spawns your binary and sends a JSON `PluginConfig` object on **stdin**
2. Your plugin reads stdin to EOF, processes the config, fetches content
3. Your plugin writes a JSON array of `PluginItem` objects to **stdout**
4. Your plugin exits with code 0 (success) or non-zero (error)

Errors go to **stderr** — 4DA captures stderr for logging but does not parse it.

## Manifest Format

`manifest.json` — every plugin must have one.

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Plugin identifier. **Must match** the directory name exactly. |
| `version` | string | yes | — | Semantic version (e.g., `"1.0.0"`). |
| `description` | string | yes | — | Short description of what the plugin provides. |
| `author` | string | no | `null` | Plugin author name. |
| `binary` | string | yes | — | Filename of the executable, relative to the plugin directory. No path separators allowed. |
| `poll_interval_secs` | number | no | `600` | How often 4DA runs the plugin (in seconds). |
| `max_items` | number | no | `50` | Maximum items to return per execution. |

**Validation rules:**
- `name` must be a simple identifier — no `/`, `\`, `..`, or null bytes
- `binary` must be a plain filename — no path separators or `..`
- `name` must match the containing directory name exactly

## Config Format (stdin)

4DA sends this JSON object on stdin:

```json
{
  "tech_stack": ["rust", "typescript", "react"],
  "interests": ["systems programming", "web performance"],
  "max_items": 20,
  "custom": {}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tech_stack` | string[] | Technologies detected from the user's local projects. |
| `interests` | string[] | User's declared interest topics. |
| `max_items` | number | Maximum items 4DA wants back (respects manifest `max_items`). |
| `custom` | object | Plugin-specific config from user settings. Currently `null`/`{}`. |

Your plugin can use `tech_stack` and `interests` to filter or personalize results, or
ignore them entirely.

## Item Format (stdout)

Your plugin must output a JSON array of items:

```json
[
  {
    "title": "Article Title",
    "url": "https://example.com/article",
    "content": "Article summary or full text for relevance scoring.",
    "source_type": "my_source",
    "author": "Author Name",
    "published_at": "2026-03-21T12:00:00Z"
  }
]
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | yes | Item title. |
| `url` | string | no | URL to the original content. |
| `content` | string | yes | Main text — used for embedding and relevance scoring. More text = better scoring. |
| `source_type` | string | yes | Identifier for the source (e.g., `"mastodon"`, `"internal_wiki"`, `"rss_custom"`). |
| `author` | string | no | Author name. |
| `published_at` | string | no | ISO 8601 timestamp (e.g., `"2026-03-21T12:00:00Z"`). |

Returning an empty array `[]` is valid — it means no new content.

## Security Model

Plugins run in an isolated environment with these constraints:

- **30-second timeout** — plugins that exceed this are killed
- **10 MB output limit** — stdout larger than this is discarded
- **Cleared environment** — only `PATH`, `TEMP`/`TMP`, and platform essentials (`HOME`/`USERPROFILE`, `SystemRoot`) are passed through
- **Working directory** — set to the plugin's own directory
- **No network restrictions** — plugins can make HTTP requests (they need to fetch content)
- **Crash isolation** — one plugin failure never affects other plugins or 4DA itself

If a plugin fails (non-zero exit, timeout, invalid output), 4DA logs the error and
continues with other sources. Your plugin should handle its own errors gracefully —
write diagnostics to stderr and exit non-zero.

## Supported Runtimes

The loader detects the binary's file extension and prepends the right interpreter:

| Extension | Interpreter | Notes |
|-----------|------------|-------|
| `.js`, `.mjs` | `node` | Node.js must be on PATH |
| `.py` | `python` | Python must be on PATH |
| *(other)* | *(direct execution)* | Native binary or script with shebang |

For Node.js plugins, use ESM imports (`import` not `require`). For Python plugins,
use only stdlib or document dependencies clearly.

## Example Plugin

See `data/plugins/example-hackernews-best/` for a complete working example that:

1. Reads config from stdin
2. Fetches Hacker News best story IDs
3. Fetches details for the top N stories in parallel
4. Outputs `PluginItem[]` to stdout

The example is under 60 lines and covers the full protocol.

## Testing Your Plugin

Test locally by piping config JSON into your plugin:

```bash
echo '{"tech_stack":[],"interests":[],"max_items":5,"custom":null}' | node data/plugins/example-hackernews-best/plugin.js
```

Verify:
- Output is valid JSON (pipe through `| jq .` or `| python -m json.tool`)
- Every item has `title`, `content`, and `source_type`
- Plugin exits with code 0
- Plugin completes within 30 seconds
- Output is under 10 MB

To test error handling, send invalid input:

```bash
echo 'not json' | node data/plugins/my-plugin/plugin.js
```

Your plugin should either output `[]` and exit 0, or write an error to stderr and exit 1.

## Tips

- **Keep plugins fast.** The 30-second timeout is a hard kill. Fetch in parallel where possible.
- **Provide good `content` text.** 4DA scores relevance by embedding the content field. More meaningful text = better matching to the user's context.
- **Use `source_type` consistently.** It appears in the UI and helps users identify where items came from.
- **Degrade gracefully.** If an API is down, return `[]` rather than crashing. 4DA will retry on the next poll cycle.
- **Log to stderr.** Diagnostic output on stderr is captured in 4DA's logs. Never write non-JSON to stdout.
