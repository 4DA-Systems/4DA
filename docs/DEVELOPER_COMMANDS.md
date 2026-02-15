# 4DA Developer Commands

Claude Code commands for developing, debugging, and maintaining 4DA. Run any command by typing it in a Claude Code session.

## Quick Reference

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `/commit-organize` | Split messy working tree into atomic commits | Uncommitted changes piling up |
| `/validate-config` | Check all config files for errors | After changing settings, before shipping |
| `/optimize-context` | Improve relevance by tuning watched dirs | Relevance feels off, new project added |
| `/why-relevant` | Explain why an item scored the way it did | Debugging unexpected scores |
| `/synthesize` | Executive briefing from recent discoveries | Weekly review, catching up |
| `/trends` | Statistical trend analysis over time | Spotting patterns, anomaly investigation |

---

## /commit-organize

**What it does:** Analyzes your uncommitted changes, groups them into logical atomic commits by semantic intent, and executes them in dependency order after you approve.

**When to use it:**
- You've been coding for a while and have 5+ files modified
- The git hygiene hook warned you about accumulating changes
- Before a PR, to clean up your commit history

**Usage:**
```
/commit-organize              # Full flow: test → analyze → propose → execute
/commit-organize --dry-run    # Show the plan but don't commit
/commit-organize --skip-tests # Skip cargo test / pnpm test preflight
/commit-organize --auto       # Execute without asking (use with caution)
```

**How it groups changes:**

The command categorizes every file by what actually changed (reads diffs, not just filenames):

| Priority | Category | Example |
|----------|----------|---------|
| 1 | `structure` | lib.rs split into modules |
| 2 | `infra` | Cargo.toml dep changes |
| 3 | `refactor` | Dead code removal, import reorg |
| 4 | `feature` | New command, new component |
| 5 | `fix` | SQL bug fix, panic fix |
| 6 | `test` | Test files (bundled with source) |
| 7 | `config` | settings.json changes |
| 8 | `docs` | Architecture docs |
| 9 | `legal` | LICENSE, CLA |
| 10 | `style` | Formatting only |

Files that depend on each other are always grouped together. The commit order ensures foundations land before features.

**Interactive controls:** After seeing the plan, you can say:
- `go` — execute the plan
- `merge 1+2` — combine two commit groups
- `split 3` — break a group apart
- `reorder` — change commit sequence
- `edit message 2` — change a draft commit message

**Safety:** Runs tests first. Never uses `git add .`. Never pushes. Scans for secrets. Never amends existing commits.

---

## /validate-config

**What it does:** Validates all 4DA configuration files (JSON syntax, required fields, path existence, API key security, cross-file consistency).

**When to use it:**
- After editing `data/settings.json` or `.mcp.json`
- Before shipping a new release
- When something seems misconfigured

**Usage:**
```
/validate-config               # Full validation
/validate-config --security    # Security audit only
/validate-config --fix         # Include fix commands for each issue
```

**Files checked:**
- `data/settings.json` — main app settings
- `.mcp.json` — MCP server config
- `src-tauri/tauri.conf.json` — Tauri build/runtime config
- `.claude/settings.json` — Claude Code project settings

**What it catches:**
- Invalid JSON syntax
- Missing required fields
- Watched directories that don't exist
- API keys stored in files (should use env vars)
- Mismatched embedding dimensions across configs
- Unreasonable daily cost limits

---

## /optimize-context

**What it does:** Analyzes your watched directories and scoring configuration to find ways to improve relevance quality.

**When to use it:**
- Relevance scores feel off (too many irrelevant items, or missing things you care about)
- You added a new project directory
- Periodic tuning (monthly)

**Usage:**
```
/optimize-context              # Full analysis with recommendations
/optimize-context --quick      # Quick recommendations only
/optimize-context --directory=/path  # Analyze a specific directory
```

**What it analyzes:**
- File types and sizes in watched directories
- Technology stack detection from code
- Noise sources (build artifacts, generated files)
- Coverage gaps (projects not being watched)
- Interest inference from code patterns and git activity

**Output includes:**
- Context quality score (0-100)
- Specific recommendations ranked by impact
- Copy-paste fix commands for each recommendation
- Noise detection with size estimates

---

## /why-relevant

**What it does:** Performs a complete score autopsy on a specific item, breaking down every scoring component and explaining in plain language why it scored the way it did.

**When to use it:**
- An item scored surprisingly high or low
- You want to understand the scoring system by example
- Debugging after changing scoring config

**Usage:**
```
/why-relevant [item_id]        # Autopsy a specific item
/why-relevant hn_12345         # By source + ID
/why-relevant                  # Most recent high-scoring item
```

**Score components explained:**
- **Context match** — does it relate to your watched codebases?
- **Interest match** — does it match your declared/learned interests?
- **ACE signals** — autonomous context engine discoveries
- **Learned behavior** — patterns from your click/save/dismiss history
- **Dependency match** — does it relate to your project dependencies?

Each component is shown with its raw value and contribution to the final score, plus a plain-English explanation.

---

## /synthesize

**What it does:** Generates an executive briefing from 4DA's recent high-scoring discoveries, grouping them into themes with actionable recommendations.

**When to use it:**
- Start of the week review
- Catching up after time away
- Preparing a summary for your team

**Usage:**
```
/synthesize                    # Last 7 days, all sources
/synthesize --period=30d       # Last 30 days
/synthesize --topic=rust       # Focus on specific topic
/synthesize --format=brief     # Short summary only
```

**Output includes:**
- TL;DR with your single top action item
- Key themes with item counts and average scores
- Notable individual items with recommended actions
- Prioritized action list (high/medium/low)

---

## /trends

**What it does:** Applies statistical analysis to your content history — topic frequency, anomaly detection, correlations, and predictions.

**When to use it:**
- Spotting what's gaining or losing traction
- Investigating a volume spike or drop
- Understanding which topics co-occur

**Usage:**
```
/trends                        # 30-day trend analysis
/trends --topic=rust           # Specific topic trend
/trends --compare              # Week-over-week comparison
/trends --anomalies            # Focus on unusual patterns
```

**Output includes:**
- Rising and falling topics with percent change
- Volume sparkline visualization
- Anomaly detection (z-score based, flags 2+ sigma events)
- Topic correlations (which topics appear together)
- Predictions based on trajectory

---

## Git Hygiene Hook

Not a command — runs automatically on every prompt you submit. Checks your working tree and warns when uncommitted changes accumulate.

**Thresholds:**

| Files Changed | Warning Level | Message |
|---------------|---------------|---------|
| 8+ files & 300+ lines | Note | Gentle reminder |
| 12+ files | Notice | Suggests checkpointing |
| 20+ files | Warning | Strongly recommends `/commit-organize` |

The hook excludes `.claude/` directory changes from the count (those are tooling, not code).

**Configuration:** Defined in `.claude/settings.local.json` under the `hooks.UserPromptSubmit` key. To disable, remove the hooks section.

---

## Running Tests

These aren't slash commands, but they're the most common developer operations:

```bash
# Rust tests (run from src-tauri/)
cd src-tauri && cargo test 2>&1 | grep "test result"

# Rust tests including gated features
cd src-tauri && cargo test --features void-universe

# Frontend tests
pnpm run test

# Full validation (types + lint + format + tests)
pnpm run validate:all

# MCP server build
cd mcp-4da-server && pnpm run build
```

---

## Workflow Patterns

### Daily development
1. Code your changes
2. When the hygiene hook fires, run `/commit-organize`
3. Review the proposed groups, approve or adjust
4. Continue coding

### Weekly review
1. `/synthesize` — get your briefing
2. `/trends` — spot what's moving
3. `/optimize-context` — tune if needed

### Before shipping
1. `/validate-config` — check all configs
2. `pnpm run validate:all` — full test suite
3. `/commit-organize` — clean up any remaining changes

### Debugging relevance
1. `/why-relevant [item]` — understand a specific score
2. `/optimize-context` — check if your context is well-configured
3. Adjust scoring config in `pipeline.scoring` if systematic issues found
