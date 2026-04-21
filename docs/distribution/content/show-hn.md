# Show HN Post — @4da/mcp-server

## Title

Show HN: MCP server that finds CVEs in your actual dependencies (zero config, privacy-first)

## Post Body

I built an MCP server that scans your project dependencies for known vulnerabilities via OSV.dev. It reads your lock files (package-lock.json, Cargo.lock, go.sum, poetry.lock), resolves exact versions, and batch-queries the OSV vulnerability database. Your AI assistant gets real CVE data with severity, CVSS scores, and fix versions — in context, while you're coding.

    claude mcp add 4da -- npx @4da/mcp-server

Then ask: "Scan my project for vulnerabilities"

What gets sent: package names + versions (same public manifest data). No source code, no file paths, no accounts, no telemetry. Set FOURDA_OFFLINE=true to disable all network calls.

Ecosystems: npm, crates.io (Rust), PyPI, Go. On first run it auto-detects your stack from manifest files — no configuration.

Beyond vuln scanning, there are 35 more tools: tech stack detection, knowledge gap analysis (deps you use but never read about), persistent decision memory across sessions, developer identity profiling, and agent memory that survives context resets. The companion desktop app adds content intelligence from HN, arXiv, GitHub, etc.

TypeScript, MIT licensed, ~2500 lines, 71 tests.

GitHub: https://github.com/runyourempire/4DA/tree/main/mcp-4da-server
npm: https://www.npmjs.com/package/@4da/mcp-server

---

## Strategy Notes

**Timing:** Post Tuesday or Wednesday, 8-9 AM US Eastern. Avoid Mondays and Fridays.

**Link target:** GitHub repo URL, not the landing page. HN penalizes marketing sites.

**Critical window:** First 2 hours determine ranking. Respond to every comment immediately.

**Engagement rules:**
- Be technical and direct. HN values substance over polish.
- If someone finds a bug, acknowledge and fix it live if possible
- Do not argue with critics — acknowledge valid points briefly
- Link to code, not marketing, when answering

**Before posting — checklist:**
- [ ] Clean install test on a fresh machine (Windows, macOS, Linux)
- [ ] Run vuln scanner against 10 popular OSS projects, document results
- [ ] Have real CVE findings ready as prepared replies
- [ ] Verify `npx @4da/mcp-server --doctor` works on clean install
- [ ] All 71 tests passing

---

## Prepared Responses

### Q: "Why not just use npm audit / cargo audit?"

Those give you a list in your terminal. This gives your AI the data in context. When Claude recommends a dependency, it already knows if there's an active CVE. When you ask "any issues with my deps?" it answers with specifics: which packages, what severity, exact upgrade paths. The difference is the same as "go read the docs" vs. having the docs in the conversation.

### Q: "Does it phone home?"

The only network call is vulnerability_scan: package names + versions to OSV.dev. Same data that's public in your manifest files. Everything else is local SQLite reads. No accounts, no telemetry, no analytics. Set FOURDA_OFFLINE=true to go fully offline. The code is MIT — read it yourself.

### Q: "How does it know my stack?"

On startup it reads manifest files in your working directory: package.json, Cargo.toml, pyproject.toml, go.mod. Then it resolves exact versions from lock files (package-lock.json, pnpm-lock.yaml, yarn.lock, Cargo.lock, poetry.lock, go.sum). No configuration needed — if your project has a manifest, it works.

### Q: "Why 36 tools?"

The vuln scanner is the instant-value hook. The other tools solve different problems: knowledge gaps (which deps you use daily but never read about), decision memory (enforce architectural choices across sessions), developer DNA (your tech identity for agent handoff), agent memory (persistent across context resets). Your AI picks the relevant tools per question. You don't need to learn them — just ask natural questions.

### Q: "How is this different from Snyk/Dependabot?"

Snyk requires an account and sends your dependency graph to their servers. Dependabot only works on GitHub. This runs entirely on your machine, works with any MCP-compatible AI tool (Claude, Cursor, Copilot, Windsurf), and the data never leaves your laptop. Also: it's MIT licensed and free. No vendor lock-in.

### Q: "What's the business model?"

The MCP server is MIT and stays free. The companion desktop app (4DA) has paid tiers for advanced content intelligence that compounds over time. Not venture-funded, not optimizing for growth metrics.

### Q: "Why not open source the desktop app?"

It's source-available under FSL-1.1-Apache-2.0. You can read, modify, and run it. The only restriction is you can't build a competing product. Converts to full Apache 2.0 after two years. The MCP server has no such restriction — plain MIT.

### Q: "Isn't MCP a fad?"

MCP is the interface layer. The vulnerability scanner queries OSV.dev regardless. Whether you call it via MCP, CLI, or API doesn't change what it does. MCP just means your AI assistant can call it without you copy-pasting command output.

---

## Real CVE Findings (Prepare Before Posting)

**TODO:** Run `vulnerability_scan` against these popular OSS project starters and document real findings:

1. create-next-app (latest)
2. create-react-app
3. Remix starter
4. Astro starter
5. SvelteKit starter
6. create-t3-app
7. Vite vanilla template
8. Tauri starter
9. FastAPI template
10. gin-gonic example

For each: record the CVEs found, packages affected, severity levels. These become your proof points in HN comments.
