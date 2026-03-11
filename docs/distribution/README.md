# 4DA Distribution — Shadow Drop Playbook

Everything needed to make @4da/mcp-server appear across the MCP ecosystem.
No personal branding. No "I built this." The product appears everywhere, silently.

## Status

- [ ] **BLOCKED: Legal/banking** — No desktop app distribution until resolved
- [x] MCP server already public on npm (v4.0.0)
- [ ] MCP server v4.0.1 published

## Folder Structure

```
docs/distribution/
  README.md                              # This file — master checklist
  directories/
    submissions.md                       # Ready-to-paste copy for every directory
    official-registry.md                 # MCP Registry setup instructions
  content/
    devto-mcp-architecture.md            # Article: "Building a 30-Tool MCP Server"
    devto-codebase-awareness.md          # Article: "Why Your AI Doesn't Know What You Build"
    show-hn.md                           # Show HN post + strategy + prepared responses
  community/
    discord-messages.md                  # Tauri, MCP Community, MCP Contributors
    reddit-responses.md                  # Template replies for relevant threads
  assets/
    logo-requirements.md                 # Logo specs for Cline Marketplace etc.
```

## Execution Checklist

### Phase 0: Prep (Do First)
- [ ] Publish @4da/mcp-server@4.0.1 to npm
- [ ] Create 400x400 PNG logo (needed for Cline Marketplace)
- [ ] Verify Smithery listing is live and accurate
- [ ] Add `mcpName` to mcp-4da-server/package.json for Official Registry

### Phase 1: Official Registry
- [ ] Install mcp-publisher CLI
- [ ] Run `mcp-publisher init` to generate server.json
- [ ] `mcp-publisher login github`
- [ ] `mcp-publisher publish`
- [ ] Verify listing at registry.modelcontextprotocol.io

### Phase 2: High-Impact Directories
- [ ] punkpeye/awesome-mcp-servers — Submit PR (draft ready in AWESOME_LIST_PR.md)
- [ ] Cline Marketplace — GitHub issue with logo
- [ ] mcp.so — Submit via site
- [ ] PulseMCP — pulsemcp.com/submit
- [ ] Smithery — Verify/complete listing

### Phase 3: Wide Coverage (Batch)
- [ ] mcpservers.org/submit
- [ ] mcpmarket.com/submit
- [ ] lobehub.com/mcp — Submit MCP
- [ ] mcpserverfinder.com — Submit
- [ ] mcp-server-directory.com/submit
- [ ] mcpserverdirectory.org/submit
- [ ] mcp-servers-hub.net/submit
- [ ] devhunt.org — Submit dev tool

### Phase 4: Awesome Lists (PRs)
- [ ] appcypher/awesome-mcp-servers — PR
- [ ] tauri-apps/awesome-tauri — PR
- [ ] jamesmurdza/awesome-ai-devtools — PR

### Phase 5: Community Showcases
- [ ] Tauri Discord #showcase — See community/discord-messages.md
- [ ] MCP Community Discord — See community/discord-messages.md
- [ ] MCP Contributors Discord — See community/discord-messages.md
- [ ] modelcontextprotocol GitHub Discussions
- [ ] tauri-apps GitHub Discussions (Show and Tell)

### Phase 6: Newsletters (Submit Links)
- [ ] Console.dev — Submit at console.dev
- [ ] This Week in Rust — PR to github.com/rust-lang/this-week-in-rust
- [ ] Rust Bytes — Substack submission
- [ ] TLDR — Submit link (AI, Web Dev, DevOps editions)
- [ ] Changelog News — Submit link

### Phase 7: Content Publishing (When Ready)
- [ ] Dev.to Article 1: MCP Architecture — See content/devto-mcp-architecture.md
- [ ] Dev.to Article 2: Codebase Awareness — See content/devto-codebase-awareness.md

### Phase 8: Desktop App Launch (BLOCKED — Requires Legal/Banking)
- [ ] Legal documents finalized
- [ ] Banking/payment set up
- [ ] Build release binaries (pnpm run tauri build)
- [ ] Host builds (S3 presigned URLs or 4da.ai password-protected page)
- [ ] Show HN — See content/show-hn.md
- [ ] Newsletter submissions that link to downloads
- [ ] YouTube outreach (Cole Medin, etc.)

## Discord Invite Links

- MCP Community: https://discord.com/invite/model-context-protocol-1312302100125843476
- MCP Contributors: https://discord.com/invite/6CSzBmMkjX
- Tauri: https://discord.com/invite/tauri
- Ollama: https://discord.com/invite/ollama

## Key URLs

- npm: https://www.npmjs.com/package/@4da/mcp-server
- Website: https://4da.ai
- Shop: https://shop.4da.ai
- Smithery: https://smithery.ai/server/@4da/mcp-server

## Rules

1. Never say "I built" or reference a person — the product is the author
2. Never link to the private GitHub repo for the desktop app
3. The MCP server npm link and 4da.ai are the only public-facing URLs
4. Desktop app builds are BLOCKED until legal/banking clears
5. No beta testers, no free Signal subscriptions, no discounts
6. `npx @4da/mcp-server --setup` is the primary call-to-action everywhere
