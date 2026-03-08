# 4DA Setup & Troubleshooting Guide

Everything you need to configure 4DA, fix common issues, and get the most out of your intelligence system.

---

## Table of Contents

1. [First Launch](#first-launch)
2. [AI Provider Setup](#ai-provider-setup)
3. [Your Profile & Tech Stack](#your-profile--tech-stack)
4. [Content Sources](#content-sources)
5. [Context Discovery (ACE)](#context-discovery-ace)
6. [Intelligence System](#intelligence-system)
7. [License Activation](#license-activation)
8. [STREETS Playbook](#streets-playbook)
9. [Keyboard Shortcuts](#keyboard-shortcuts)
10. [Troubleshooting](#troubleshooting)

---

## First Launch

When you first open 4DA, you'll see a splash screen while the system initializes (database, embedding models, sources). Once ready, you'll land on the **Briefing** view.

### Navigation

**Main Views** (bottom tab bar):

| Tab | Purpose |
|-----|---------|
| **Briefing** | AI-generated intelligence summary, top picks, decision windows |
| **Channels** | Custom topic channels you create and monitor |
| **Results** | All scored content from every source |
| **Profile** | Your sovereign developer profile and identity |
| **Insights** | Trends, decisions, system health |
| **Saved** | Bookmarked items |
| **Toolkit** | Developer tools and utilities |
| **Playbook** | STREETS independence modules |
| **Calibrate** | Tune scoring accuracy |

**Settings** (gear icon, top right) has these tabs:

| Tab | What's Inside |
|-----|--------------|
| **General** | AI provider, API keys, re-ranking, usage stats, language, license |
| **Sources** | Enable/disable content sources, configure RSS feeds |
| **Profile** | Your role, tech stack, interests, exclusions |
| **Discovery** | ACE scan directories, auto-discovery |
| **Health** | System diagnostics, anomaly detection, learned behavior |
| **About** | Version, attribution, keyboard shortcuts |

---

## AI Provider Setup

4DA works with multiple AI providers. You need at least one configured for AI features (briefings, search, re-ranking).

### Option 1: Built-in Local (Default, Free)

No configuration needed. 4DA includes a built-in local embedding model for basic scoring. This works offline with zero API cost but doesn't support AI briefings or re-ranking.

**Best for:** Privacy-first users, offline use, trying 4DA without API keys.

### Option 2: Ollama (Free, Local, Recommended)

Full AI capabilities running entirely on your machine.

1. Install Ollama from [ollama.com](https://ollama.com)
2. Open a terminal and pull a model:
   ```
   ollama pull llama3.2
   ```
3. In 4DA: **Settings > General > AI Provider** > select **Ollama**
4. The app auto-detects Ollama. If not, click **Recheck**
5. Select your model from the dropdown

**Best for:** Full AI features with complete privacy. Requires 8GB+ RAM.

### Option 3: Anthropic (Claude)

1. Get an API key from [console.anthropic.com](https://console.anthropic.com)
2. In 4DA: **Settings > General > AI Provider** > select **Anthropic**
3. Paste your API key
4. Select a model (claude-3-haiku is cheapest, claude-3-opus is best)

**Best for:** Highest quality briefings and analysis.

### Option 4: OpenAI

1. Get an API key from [platform.openai.com](https://platform.openai.com)
2. In 4DA: **Settings > General > AI Provider** > select **OpenAI**
3. Paste your API key
4. Select a model (gpt-4o-mini is cheapest)

**Best for:** Users already in the OpenAI ecosystem.

### Re-Ranking (Optional)

Re-ranking uses AI to improve the order of your results beyond basic scoring.

- **Settings > General > Re-Ranking** > Enable
- Set **Max Items per Batch** (default: 15)
- Set **Min Score** threshold (default: 0.25)
- Set daily token and cost limits to control spending

---

## Your Profile & Tech Stack

Your profile tells 4DA what you work on so it can surface relevant content.

### Setting Your Role

**Settings > Profile > Your Role**

Enter your job title or role (e.g., "Senior Rust Developer", "Full-Stack Engineer", "ML Researcher"). This shapes how content is prioritized.

### Managing Your Tech Stack

**Settings > Profile > Tech Stack**

Your tech stack is the most important personalization signal. It affects:
- Which content scores higher
- How the STREETS Playbook is personalized
- What appears in your Developer DNA
- Decision Windows and tech radar entries

**To add technologies:** Type a technology name and press Enter or click Add.

**To remove incorrect entries:** Click the **x** button on any tag.

> **Important:** 4DA's ACE engine auto-detects technologies from your local projects. If it scans a project you don't actively work on, incorrect technologies may appear. See [Fixing Incorrect Tech Detection](#fixing-incorrect-tech-detection) below.

### Setting Interests

**Settings > Profile > Interests**

Add topics you want to see more of. These boost relevance scores for matching content.

**Examples:** `distributed systems`, `machine learning`, `systems programming`, `developer tools`

### Setting Exclusions

**Settings > Profile > Exclusions**

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

**Settings > Discovery**

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

**Method 1: Remove from Settings (Quick)**

1. Open **Settings > Profile > Tech Stack**
2. Find the incorrect technology tag
3. Click the **x** button to remove it

**Method 2: Remove from Decision Memory (Thorough)**

This method also removes the technology from your interests and decision history:

1. Go to the **Insights** tab in the main view
2. Scroll to **Decision Windows**
3. Find the incorrect tech entry (e.g., "drizzle")
4. Click to expand it
5. Click the red **Remove** button

This cleans the technology from three places:
- **Tech stack** (primary storage, affects scoring and playbook)
- **Interests** (may have been auto-seeded by ACE)
- **Decisions** (supersedes the tech_choice decision record)

After removal, the STREETS Playbook will regenerate without the incorrect technology on your next visit.

> **Tip:** Auto-detected tech decisions show an amber banner: *"Some tech choices were auto-detected from your local projects."* Use the Remove button on any that don't belong.

---

## Intelligence System

4DA learns from your interactions to improve over time.

### How Learning Works

Every time you interact with a result, 4DA records a signal:

| Action | Signal | Effect |
|--------|--------|--------|
| **Save** | Strong positive | Boosts similar content |
| **Click / Read** | Mild positive | Slightly boosts topic |
| **Dismiss** | Mild negative | Slightly reduces topic |
| **Mark Irrelevant** | Strong negative | Reduces topic, may create anti-topic |

### Learning Indicator

The **"Learning: N preferences"** bar below the action bar shows how many topic preferences the system has learned. Click it to expand and see:

- **Green pills** (+): Topics you engage with (positive affinity)
- **Red pills** (-): Topics you've rejected (anti-topics)

### Intelligence Profile

On the **Briefing** view (scroll down), you'll see **"Your Intelligence Profile"** with:

- **Top Affinities / Strongest Signals**: Your most significant learned preferences
- **Learning Velocity**: Total topics the system has learned about
- **System Activity**: Items analyzed, items you've engaged with, learning cycles completed

### Intelligence Metrics

Click **"Intelligence Metrics"** on the Briefing view to expand detailed analytics:

- **Engagement Pulse**: Your interaction patterns
- **Intelligence Pulse**: Calibration accuracy, source quality, anti-patterns
- **Scoring Delta**: How scores are shifting over time
- **Compound Advantage**: Your overall intelligence advantage score

### Calibration

Go to the **Calibrate** tab to review and tune scoring accuracy. Rate items as relevant or irrelevant to train the system.

---

## License Activation

4DA has three tiers:

| Tier | Price | Features |
|------|-------|----------|
| **Free** | $0 | All sources, scoring, learning, basic briefing, Module S (Sovereign Setup) |
| **Pro** | Paid | AI briefings, Developer DNA, full STREETS Playbook, tech radar, decision windows |
| **Team** | Paid | Everything in Pro + team features |

### Activating a License Key

1. Open **Settings > General** > scroll to **License** section
2. Paste your license key (format: `XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX-V3`)
3. Click **Activate**
4. You should see a green "Pro" badge appear

Your license persists across restarts. You should never need to re-enter it.

### Verifying Your Tier

Your current tier is shown:
- In the header bar (green **PRO** badge)
- In **Settings > General > License** section
- In **Settings > General > STREETS Membership**

---

## STREETS Playbook

The STREETS Playbook is 4DA's built-in independence curriculum. It's personalized to your actual tech stack, hardware, and profile.

### Modules

| Module | Name | Focus |
|--------|------|-------|
| **S** | Sovereign Setup | Configure your rig as a business asset (FREE) |
| **T** | Technical Moats | Build what competitors can't easily copy |
| **R** | Revenue Engines | Eight ways to turn skills into income |
| **E1** | Execution Playbook | Ship your first revenue engine |
| **E2** | Evolving Edge | Stay ahead as markets shift |
| **T2** | Tactical Automation | Automate your income streams |
| **S2** | Stacking Streams | Combine engines for resilience |

### Personalization Levels

The Playbook adapts to you through 5 levels of personalization:

1. **L1 — Interpolation**: Your tech stack, hardware specs, and profile data inserted into lessons
2. **L2 — Conditionals**: Content branches based on your GPU tier, OS, LLM setup
3. **L3 — Insight Cards**: Computed cards showing hardware benchmarks, stack fit, cost projections
4. **L4 — Mirror Blocks**: Connections between your profile and lesson concepts
5. **L5 — Temporal Blocks**: Content that changes based on what's new since your last visit

### If Playbook Shows Incorrect Data

The Playbook pulls your tech stack from the same profile data as everything else. If it references a technology you don't use:

1. **Fix the source**: Remove the incorrect tech from **Settings > Profile > Tech Stack** or via **Insights > Decision Windows > Remove**
2. **Revisit the lesson**: The Playbook regenerates personalized content on each visit using your current profile

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

You need at least one AI provider for briefings and re-ranking. See [AI Provider Setup](#ai-provider-setup).

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

### Wrong Technology in My Profile / Playbook

ACE auto-detects technologies from your local projects. If it detects something incorrect:

1. **Quick fix**: **Settings > Profile > Tech Stack** > click **x** on the wrong tag
2. **Thorough fix**: **Insights tab > Decision Windows** > expand the entry > click **Remove**

The Remove button cleans the technology from your tech stack, interests, and decision history. The Playbook regenerates automatically.

See [Fixing Incorrect Tech Detection](#fixing-incorrect-tech-detection) for full details.

### "0 Topics Learned" in Intelligence Profile

This means the system hasn't detected any interaction patterns yet. To build your profile:

1. Run an analysis (**R** key)
2. **Save** articles you find relevant (boosts those topics)
3. **Dismiss** articles you don't care about (deprioritizes those topics)
4. After 3+ interactions per topic, affinities will appear

### Learning Preferences Dropdown Won't Expand

If the "Learning: N preferences" bar doesn't expand when clicked:

1. Make sure you're clicking the bar itself (not the area around it)
2. Try scrolling down — the expanded content appears below the bar
3. If the issue persists, refresh the page (Ctrl+R)

### License Key Not Persisting After Restart

Your license should persist across restarts. If it reverts to "Free":

1. Re-enter your license key in **Settings > General > License**
2. Click **Activate**
3. Verify the green "Pro" badge appears
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

1. **Settings > General > Re-Ranking** > reduce **Max Items per Batch**
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
