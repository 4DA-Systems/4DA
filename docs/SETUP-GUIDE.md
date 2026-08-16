# 4DA Setup & Troubleshooting Guide

Everything you need to configure 4DA, fix common issues, and get the most out of your intelligence system.

---

## Table of Contents

1. [First Launch](#first-launch)
2. [AI Provider Setup](#ai-provider-setup)
3. [Language & Translation](#language--translation)
4. [Your Profile & Tech Stack](#your-profile--tech-stack)
5. [Content Sources](#content-sources)
6. [Context Discovery (ACE)](#context-discovery-ace)
7. [Intelligence System](#intelligence-system)
8. [License Activation](#license-activation)
9. [STREETS Playbook (on the web)](#streets-playbook-on-the-web)
10. [Keyboard Shortcuts](#keyboard-shortcuts)
11. [Troubleshooting](#troubleshooting)

---

## First Launch

When you first open 4DA, you'll see a splash screen while the system initializes (database, embedding models, sources). Once ready, you'll land on the **Brief** view.

### Navigation

**Main Views** (tab bar, above the content area):

| Tab | Purpose |
|-----|---------|
| **Brief** | Your intelligence at a glance — AI-generated summary, top picks, attention cards |
| **Preemption** | What matters before it hurts — forward-looking dependency and ecosystem risk |
| **Blind Spots** | What you're not watching — gaps in your coverage |
| **Signal** | Your curated intelligence feed — every item scored against your stack. Toggle between **List** and **Graph** views (top right of the panel) |

**Settings** (gear icon, top right) has these tabs:

| Tab | What's Inside |
|-----|--------------|
| **General** | Language, background monitoring and refresh interval, data retention, deep clean |
| **Intelligence** | AI provider, API keys, model selection, blind-spot auto-assess, Your Stack (which projects count toward relevance), license |
| **Sources** | Enable/disable content sources, configure RSS feeds |
| **Projects** | ACE scan directories and auto-discovery, indexed documents, personalization (role, tech stack, interests, exclusions), learned preferences |
| **About** | Version, attribution, keyboard shortcuts |

A **Team** tab appears additionally on Team and Enterprise tiers.

---

## AI Provider Setup

4DA works with multiple AI providers. You need at least one configured for AI features (briefings, search, re-ranking).

### Option 1: Built-in Local (Default, Free)

No configuration needed. 4DA includes a built-in local embedding model for basic scoring. This works offline with zero API cost but doesn't support AI briefings or LLM re-ranking. To use AI briefings (free), add an LLM provider below.

**Best for:** Privacy-first users, offline use, trying 4DA without API keys.

### Option 2: Ollama (Free, Local, Recommended)

Full AI capabilities running entirely on your machine.

1. Install Ollama from [ollama.com](https://ollama.com)
2. Open a terminal and pull a model:
   ```
   ollama pull llama3.2
   ```
3. In 4DA: **Settings > Intelligence > AI Provider** > select **Ollama**
4. The app auto-detects Ollama. If not, click **Recheck**
5. Select your model from the dropdown

**Best for:** Full AI features with complete privacy. Requires 8GB+ RAM.

### Option 3: Anthropic (Claude)

1. Get an API key from [console.anthropic.com](https://console.anthropic.com)
2. In 4DA: **Settings > Intelligence > AI Provider** > select **Anthropic**
3. Paste your API key
4. Select a model (claude-3-haiku is cheapest, claude-3-opus is best)

**Best for:** Highest quality briefings and analysis.

### Option 4: OpenAI

1. Get an API key from [platform.openai.com](https://platform.openai.com)
2. In 4DA: **Settings > Intelligence > AI Provider** > select **OpenAI**
3. Paste your API key
4. Select a model (gpt-4o-mini is cheapest)

**Best for:** Users already in the OpenAI ecosystem.

### Re-Ranking (Optional)

Re-ranking uses AI to improve the order of your results beyond basic scoring.

- **Settings > Intelligence > Re-Ranking** > Enable
- Set **Max Items per Batch** (default: 15)
- Set **Min Score** threshold (default: 0.25)
- Set daily token and cost limits to control spending

---

## Language & Translation

4DA supports 13 languages natively. Your system language is auto-detected during first launch.

**Change language:** Settings > General > Locale > Language

**Set up content translation:** For faster, higher-quality translation of feed content, configure a dedicated translation API. Azure Translator gives you 2M characters/month free:

1. Create a free Azure Translator resource at [portal.azure.com](https://portal.azure.com)
2. Copy the API key
3. In 4DA: Settings > General > Locale > Content Translation > Azure Translator > paste key

Without a dedicated API, translations use your local Ollama model (free, private, slightly slower).

For the complete multilingual guide including all provider setup instructions, platform notes, and troubleshooting, see **[Multilingual Guide](MULTILINGUAL.md)**.

---

## Your Profile & Tech Stack

Your profile tells 4DA what you work on so it can surface relevant content.

### Setting Your Role

**Settings > Projects > Personalization > Your Role**

Enter your job title or role (e.g., "Senior Rust Developer", "Full-Stack Engineer", "ML Researcher"). This shapes how content is prioritized.

### Managing Your Tech Stack

**Settings > Projects > Personalization > Tech Stack**

Your tech stack is the most important personalization signal. It affects:
- Which content scores higher
- What appears in your Developer DNA

**To add technologies:** Type a technology name and press Enter or click Add.

**To remove incorrect entries:** Click the **x** button on any tag.

> **Important:** 4DA's ACE engine auto-detects technologies from your local projects. If it scans a project you don't actively work on, incorrect technologies may appear. See [Fixing Incorrect Tech Detection](#fixing-incorrect-tech-detection) below.

### Setting Interests

**Settings > Projects > Personalization > Interests**

Add topics you want to see more of. These boost relevance scores for matching content.

**Examples:** `distributed systems`, `machine learning`, `systems programming`, `developer tools`

### Setting Exclusions

**Settings > Projects > Personalization > Exclusions**

Add topics you never want to see. These apply a penalty to matching content.

**Examples:** `cryptocurrency`, `web3`, `nft`, `dropshipping`

---

## Content Sources

**Settings > Sources**

4DA fetches content from multiple sources and scores everything against your profile.

| Source | Content Type | Default Interval |
|--------|-------------|-----------------|
| **Hacker News** | Tech news, discussions | 5 minutes |
| **Reddit** | Subreddit posts | 10 minutes |
| **arXiv** | Academic papers | 1 hour |
| **GitHub** | Trending repos, releases | 15 minutes |
| **RSS Feeds** | Any RSS/Atom feed you add | Configurable |

### Adding RSS Feeds

1. **Settings > Sources** > scroll to RSS section
2. Enter the feed URL
3. Click Add
4. The feed will be fetched on the next analysis cycle

### Running an Analysis

Click the **Analyze** button (or press **R**) to fetch fresh content from all enabled sources and score it against your profile.

---

## Context Discovery (ACE)

ACE (Autonomous Context Engine) scans your local projects to understand what you work on. It detects:

- Programming languages and frameworks
- Dependencies from manifest files (package.json, Cargo.toml, etc.)
- Active topics from file contents
- Git commit patterns

### Configuring Scan Directories

**Settings > Projects**

1. Click **Auto-Discover** to let ACE find common project directories
2. Or manually add directories using the input field
3. Click **Full Scan** to run a comprehensive scan

**Default locations checked:**
- `~/projects`, `~/code`, `~/dev`, `~/src`
- `~/Documents/GitHub`, `~/repos`
- `~/workspace`, `~/work`

### What ACE Detects

ACE scans up to 5 levels deep in each directory, looking for:

| Manifest | Languages/Frameworks |
|----------|---------------------|
| `package.json` | JavaScript, TypeScript, React, Vue, Angular, Svelte, Next.js, Vite, Tailwind, etc. |
| `Cargo.toml` | Rust, Tokio, Actix, Serde, etc. |
| `pyproject.toml` / `requirements.txt` | Python, Django, Flask, FastAPI, etc. |
| `go.mod` | Go |
| `composer.json` | PHP, Laravel |
| `Gemfile` | Ruby, Rails |
| `pom.xml` / `build.gradle` | Java, Spring |
| `pubspec.yaml` | Dart, Flutter |

### Fixing Incorrect Tech Detection

ACE scans all projects in your configured directories. If it picks up technology from a project you don't actively work on (for example, scanning a tutorial repo that uses Drizzle when you don't use Drizzle), you have two ways to fix it:

**Method 1: Remove the tag (Quick)**

1. Open **Settings > Projects > Personalization > Tech Stack**
2. Find the incorrect technology tag
3. Click the **x** button to remove it

Also check the **Interests** list in the same panel — ACE may have auto-seeded a matching interest. Remove it the same way.

**Method 2: Stop the source project from counting (Thorough)**

If the wrong technology keeps coming back, the project it came from is still being scanned:

1. Open **Settings > Intelligence > Your Stack**
2. Find the project the technology came from (each row shows its path and dependency count)
3. Toggle it **off** so its dependencies stop counting toward relevance

To stop scanning the directory entirely, remove it under **Settings > Projects > scan directories**.

After removal, scoring and Developer DNA reflect the corrected stack from the next analysis cycle.

---

## Intelligence System

4DA records your interactions to build a preference profile you can inspect and control. Relevance scoring stays grounded in your codebase; your actions shape what the Brief shows you.

### How Your Actions Are Used

Every time you interact with a result, 4DA records a signal:

| Action | Signal | Effect |
|--------|--------|--------|
| **Save** | Strong positive | Bookmarks the item; builds your preference profile |
| **Click / Read** | Mild positive | Recorded in your activity view |
| **Dismiss** | Mild negative | Tells the Brief to stop showing similar items |
| **Mark Irrelevant** | Strong negative | Strong Brief suppression; builds your filter |

### Learned Preferences

**Settings > Projects > Learned Preferences**

Shows what 4DA has learned about your interests from your interactions. Each preference can be:

- **Pinned** — always show content matching this preference
- **Forgotten** — drop it from the profile

You can also reset the whole learned profile from this panel. If nothing is listed yet, keep saving, dismissing, and rating items — the profile builds from those signals.

### Engagement Pulse

On the **Brief** view you'll see the **Engagement Pulse** — a compact activity sparkline and your current engagement streak.

### What You Would Have Missed

On the **Signal** view (List mode, after an analysis completes), 4DA shows how much noise it rejected for you: the number of items scanned, the number rejected, your own rejection rate as a percentage, and the single highest-value item you would otherwise have missed.

This panel only appears once the rejection rate is high enough to tell a meaningful story.

---

## License Activation

4DA has three tiers:

| Tier | Price | Features |
|------|-------|----------|
| **Free** | $0 | All sources, scoring, learning, the OSV security floor, plus AI briefings, natural language search, Developer DNA (BYOK), Score Autopsy, signal chains, and channels |
| **Signal** | Paid | Everything in Free + Blind Spots, Knowledge Gaps, Standing Queries, Semantic Shifts, Project Health, Attention Report, Decision Health, cross-project intelligence, Trust Ledger analytics |
| **Team** | Paid | Everything in Signal + team features |

### Activating a License Key

1. Open **Settings > Intelligence** > scroll to the **License** section
2. Paste your license key
3. Click **Activate**
4. You should see a gold **SIGNAL** badge appear in the header bar

Your license persists across restarts. You should never need to re-enter it.

### Verifying Your Tier

Your current tier is shown:
- In the header bar (gold tier badge — **SIGNAL** when licensed, **SIGNAL TRIAL** during the 14-day trial, gray **FREE** otherwise)
- In **Settings > Intelligence > License** section

---

## STREETS Playbook (on the web)

The STREETS Playbook — 7 modules on turning developer skills into independent income — is published free on the open web. It is not a tab inside the app; 4DA stays focused on intelligence (Brief, Preemption, Blind Spots, Signal).

| Module | Name | Read it |
|--------|------|---------|
| **S** | Sovereign Setup | [4da.ai/streets/sovereign-setup](https://4da.ai/streets/sovereign-setup/) |
| **T** | Technical Moats | [4da.ai/streets/technical-moats](https://4da.ai/streets/technical-moats/) |
| **R** | Revenue Engines | [4da.ai/streets/revenue-engines](https://4da.ai/streets/revenue-engines/) |
| **E** | Execution Playbook | [4da.ai/streets/execution-playbook](https://4da.ai/streets/execution-playbook/) |
| **E** | Evolving Edge | [4da.ai/streets/evolving-edge](https://4da.ai/streets/evolving-edge/) |
| **T** | Tactical Automation | [4da.ai/streets/tactical-automation](https://4da.ai/streets/tactical-automation/) |
| **S** | Stacking Streams | [4da.ai/streets/stacking-streams](https://4da.ai/streets/stacking-streams/) |

Bonus: [The 2026 Developer Income Map](https://4da.ai/streets/income-map/).

Every module is published in 13 languages (English, Arabic, German, Spanish, French, Hindi, Italian, Japanese, Korean, Portuguese (BR), Russian, Turkish, Chinese) — use the language switcher on any module page.

---

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| **R** | Run analysis |
| **F** | Toggle source filter |
| **B** | Open briefing |
| **,** | Open settings |
| **?** | Show help |
| **Esc** | Close panel |
| **Ctrl+`** | Toggle command deck |
| **S** | Save item |
| **J / K** | Navigate items |

---

## Troubleshooting

### App Won't Start / Stuck on Splash Screen

The splash screen shows initialization progress. If it gets stuck:

1. Click the **Refresh** button (top-right corner of splash screen)
2. If that doesn't work, check that no other instance of 4DA is running
3. Check the log file for errors:
   - Windows: `%APPDATA%\4da\logs\`
   - macOS: `~/Library/Logs/4da/`
   - Linux: `~/.local/share/4da/logs/`

### "No API Key Configured"

You need at least one AI provider configured for AI briefings and re-ranking. AI briefings are free — just add a provider. See [AI Provider Setup](#ai-provider-setup).

For basic scoring without AI, select **Built-in (Local)** as your provider — no API key needed.

### Ollama Not Detected

1. Make sure Ollama is running:
   ```
   ollama serve
   ```
2. Check a model is downloaded:
   ```
   ollama list
   ```
3. In 4DA Settings, verify the base URL is `http://localhost:11434`
4. Click **Recheck** in the Ollama status area

### Analysis Returns No Results

- Ensure at least one source is enabled (**Settings > Sources**)
- Check your internet connection (sources need to fetch from the web)
- Try broadening your interests or reducing exclusions
- Wait for sources to fetch — the first analysis may take 30-60 seconds

### Wrong Technology in My Profile

ACE auto-detects technologies from your local projects. If it detects something incorrect:

1. **Quick fix**: **Settings > Projects > Personalization > Tech Stack** > click **x** on the wrong tag (and check the **Interests** list in the same panel)
2. **Thorough fix**: **Settings > Intelligence > Your Stack** > toggle off the project the technology came from

Scoring and Developer DNA reflect the corrected stack from the next analysis cycle.

See [Fixing Incorrect Tech Detection](#fixing-incorrect-tech-detection) for full details.

### No Learned Preferences Yet

If **Settings > Projects > Learned Preferences** is empty, the system hasn't detected any interaction patterns yet. To build your profile:

1. Run an analysis (**R** key)
2. **Save** articles you find relevant (boosts those topics)
3. **Dismiss** articles you don't care about (deprioritizes those topics)
4. After 3+ interactions per topic, affinities will appear

### License Key Not Persisting After Restart

Your license should persist across restarts. If it reverts to "Free":

1. Re-enter your license key in **Settings > Intelligence > License**
2. Click **Activate**
3. Verify the gold "Signal" badge appears
4. Restart the app to confirm it persists

If the issue continues, check that 4DA has write access to its data directory.

### Briefing Says "Configure Ollama for AI Synthesis"

This means no LLM provider is configured for AI briefings:

1. Set up Ollama (free, local) — see [AI Provider Setup](#ai-provider-setup)
2. Or configure Anthropic/OpenAI with an API key
3. The free briefing (non-AI) still works and shows your top scored items

### Sources Show Errors or "Circuit Open"

If a source shows errors in the briefing header:

- **Circuit open**: The source failed multiple times and is temporarily paused. It will retry automatically.
- **Timeout**: The source took too long to respond. Check your internet connection.
- **Rate limited**: You're fetching too frequently. Increase the fetch interval in Settings.

### High Token Usage

If your API costs are higher than expected:

1. **Settings > Intelligence > Re-Ranking** > reduce **Max Items per Batch**
2. Set a **Daily Token Limit** (e.g., 100,000)
3. Set a **Daily Cost Limit** (e.g., $0.50)
4. Switch to a cheaper model (claude-3-haiku or gpt-4o-mini)
5. Or switch to Ollama for zero API cost

### Database Issues

4DA stores its data in a local SQLite database. If you suspect corruption:

1. Close 4DA
2. Back up the database file:
   - Windows: `%APPDATA%\4da\data\4da.db`
   - macOS: `~/Library/Application Support/4da/data/4da.db`
3. Delete the database file
4. Restart 4DA — it will create a fresh database
5. Your settings are preserved (stored separately in `settings.json`)

> **Note:** Deleting the database resets your learned preferences, decisions, and indexed documents. Your settings and license key are not affected.

---

## Getting Help

- **Privacy Policy**: [4da.ai/privacy](https://4da.ai/privacy)
- **Terms of Service**: [4da.ai/terms](https://4da.ai/terms)
- **Support**: support@4da.ai

---

*4DA v1.0.0 — All signal. No feed.*
*Built by 4DA Systems. Engineered with Claude.*
