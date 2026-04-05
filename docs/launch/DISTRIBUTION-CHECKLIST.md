# 4DA Distribution Checklist

## Pre-Launch Infrastructure

### Package Managers
- [ ] **Homebrew Cask** — Template ready at `docs/launch/homebrew-cask.rb`
  - After first release: calculate SHA256, submit PR to homebrew-cask
  - Users install: `brew install --cask 4da`
- [ ] **Winget** — Template ready at `docs/launch/winget-manifest.yaml`
  - After first release: calculate SHA256, submit via `wingetcreate`
  - Users install: `winget install 4DA.4DAHome`
- [ ] **AUR (Arch Linux)** — Create PKGBUILD after first release
  - Users install: `yay -S 4da-bin`
- [ ] **Flathub** — Create Flatpak manifest after launch
  - Users install: `flatpak install 4da`

### MCP Ecosystem
- [x] **npm package published** — `@4da/mcp-server` v4.0.1 (33 tools)
- [ ] Submit to MCP server registries (mcpservers.org, mcp.run)
- [ ] Submit to "Top MCP Servers" listicles on DEV.to
- [ ] Write MCP tutorial blog post (Post #5)

### Website (4da.ai)
- [x] Landing page live with accurate copy
- [x] Structured data (JSON-LD SoftwareApplication)
- [x] Open Graph + Twitter cards configured
- [x] Signal tier page (/signal)
- [x] Privacy policy + Terms of Service
- [ ] Blog section with 5 SEO posts (drafts in `docs/launch/blog-posts.md`)
- [ ] Download page with platform detection

### SmartScreen Reputation
- [ ] Seed 50+ trusted Windows downloads before public launch
- [ ] Document SmartScreen bypass in getting started guide
- [ ] SSL.com EV cert — awaiting org validation

## Launch Day Assets

### Content
- [ ] **Show HN post** — Draft in `docs/launch/show-hn-draft.md`
- [ ] **Product Hunt listing** — Draft in `docs/launch/product-hunt-draft.md`
- [ ] **Newsletter pitches** — Templates in `docs/launch/newsletter-pitch.md`
- [ ] **60-second demo video** — Screen recording of first-run → results
- [ ] **5 screenshots** for Product Hunt gallery
- [ ] **"Why I Built 4DA" blog post** — Personal story, technical details

### Social Proof
- [ ] 5 real testimonials from seeded users (need 7+ days of usage)
- [ ] Screenshot of real scoring results with real content

## 30-Day Launch Calendar

### Week 1 (Seeding)
- [ ] Day 1-3: Seed 15 trusted developers
- [ ] Day 1-3: Write "Why I Built 4DA" blog post
- [ ] Day 4-5: Submit Homebrew + Winget after first release
- [ ] Day 5-7: Send newsletter pitches

### Week 2 (Launch)
- [ ] Day 8-10: Record demo video
- [ ] Day 10-12: Publish first 2 SEO blog posts
- [ ] Day 12-14: Register Product Hunt Coming Soon
- [ ] Day 14: **LAUNCH — Show HN + Reddit + DEV.to**
- [ ] Day 14-16: Respond to every comment for 48 hours

### Week 3 (Proof)
- [ ] Day 17-20: Collect testimonials from 7-day users
- [ ] Day 20-22: Write remaining blog posts
- [ ] Day 22-25: **Product Hunt launch** with social proof

### Week 4 (Compound)
- [ ] Day 25-28: Submit MCP server to registries
- [ ] Day 28-30: Publish case study with real user data
- [ ] Day 30: First monthly review — conversion rates, retention, feedback

## Target Newsletters

| Newsletter | Audience | Pitch Angle | Status |
|-----------|----------|-------------|--------|
| This Week in Rust | Rust devs | Built with Rust + Tauri, native performance | [ ] |
| Console.dev | Dev tool enthusiasts | New category: codebase-aware content scoring | [ ] |
| TLDR | 1M+ developers | Privacy-first alternative to daily.dev | [ ] |
| Changelog | Open source | FSL license, MCP integration, 20 sources | [ ] |
| Hacker Newsletter | HN curators | Tool that scores HN itself | [ ] |
| Pointer.io | Senior devs | Compound intelligence, Developer DNA | [ ] |
| DevOps Weekly | DevOps | CVE detection, dependency monitoring | [ ] |
| Rust Weekly | Rust ecosystem | Community-built with Tauri 2.0 | [ ] |

## SEO Blog Posts

| # | Title | Target Keyword | Status |
|---|-------|---------------|--------|
| 1 | Why I Stopped Using daily.dev | daily.dev alternative privacy | [ ] Draft |
| 2 | How 5-Axis Scoring Kills 99% of Noise | developer news noise filter | [ ] Draft |
| 3 | The Case for Local-First Dev Tools | local first developer tools 2026 | [ ] Draft |
| 4 | Privacy-First Alternative to Feedly | Feedly alternative developer privacy | [ ] Draft |
| 5 | Give Claude Code Your Tech Stack | Claude Code MCP server tech stack | [ ] Draft |

## Success Metrics (30-Day Targets)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Total installs | 1,500+ | GitHub release downloads |
| Trial activations | 200+ | Keygen API |
| Signal conversions | 30-40 | Keygen API |
| MRR | $360-480 | Stripe/Keygen |
| Show HN upvotes | 100+ | Hacker News |
| Product Hunt rank | Top 5 | Product Hunt |
| Newsletter mentions | 3+ | Manual tracking |
| Homebrew installs | 100+ | brew analytics |
