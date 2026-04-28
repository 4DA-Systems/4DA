# STREETS x 4DA Product Integration Spec

**Author:** Lead Dev
**Date:** 2026-02-18
**Revised:** 2026-04-28
**Status:** Design Draft
**Purpose:** Define how STREETS connects to 4DA at the product level — not just marketing, but actual features.

---

## 1. Strategic Rationale

STREETS is not a separate product bolted onto 4DA. It's the **education layer** that makes 4DA's intelligence actionable for developers who want to earn independently.

STREETS is free. All 7 modules, no email, no account, no paywall. It exists because this knowledge shouldn't cost anything.

**The logic chain:**
1. Developer discovers STREETS (standalone page at 4da.ai/streets or inside 4DA)
2. STREETS teaches the developer income mindset and the 8 revenue engines
3. Developer uses 4DA to surface signals: trending frameworks, breaking changes, new techniques
4. Developer recognizes these signals as income opportunities (STREETS mindset)
5. Developer upgrades to Signal for AI briefings, deeper intelligence
6. Intelligence helps the developer build products, content, services
7. Income justifies Signal subscription many times over
8. Developer shares STREETS with other developers who need it (organic growth)

**Revenue impact:** STREETS transforms the value narrative of 4DA Signal from "nice to have intelligence" to "income-generating radar." The $12/mo Signal subscription pays for itself if it surfaces ONE opportunity per month.

**Monetization model:** STREETS is free. Revenue comes from 4DA Signal subscriptions, not from STREETS itself. STREETS drives 4DA adoption by giving developers the framework to act on what 4DA surfaces.

---

## 2. In-App Integration Points

### 2.1 Opportunity Signal Classification

**What:** Extend 4DA's existing signal classification to tag items that represent income opportunities.

**Where:** `src-tauri/src/signals.rs` (or new `opportunity.rs` module)

**New signal subtypes:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    /// Trending framework/tool with no starter kit or template available
    ProductGap,
    /// Breaking change or migration with no guide published yet
    MigrationGuide,
    /// New technique/paper with no implementation tutorial
    EducationGap,
    /// Growing niche with low competition (based on search volume vs content volume)
    NicheOpening,
    /// Dependency vulnerability with no automated fix tool
    SecurityTool,
    /// API or service with no developer-friendly wrapper
    ApiWrapper,
    /// Ecosystem need identified from community discussions (HN, Reddit)
    CommunityDemand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunitySignal {
    pub opportunity_type: OpportunityType,
    pub confidence: f32,           // 0.0-1.0
    pub time_sensitivity: TimeSensitivity,  // Hours, Days, Weeks
    pub estimated_revenue: RevenueEstimate, // Low/Medium/High
    pub relevant_engine: StreetsEngine,      // Which STREETS revenue engine applies
    pub reasoning: String,         // LLM-generated explanation of why this is an opportunity
}
```

**Detection rules (heuristic + LLM):**

| Signal Pattern | Opportunity Type | Example |
|---|---|---|
| Trending framework + no "starter kit" in ecosystem | ProductGap | "Tauri 3.0 announced" + no production templates exist |
| CVE published + no migration tool | SecurityTool | "Log4j-style vuln in popular package" |
| HN/Reddit thread asking "how do I do X?" with 50+ upvotes | CommunityDemand | "How to run LLMs privately for enterprise?" |
| arXiv paper with 100+ citations in 7 days + no implementation | EducationGap | New attention mechanism with no PyTorch impl |
| Breaking change in package with 10K+ dependents | MigrationGuide | Major version bump in a core dependency |
| New API launched with poor DX | ApiWrapper | Raw API with no SDK in your language |

**Implementation approach:**
- Phase 1: Rule-based detection from existing signal data (no LLM cost)
- Phase 2: LLM-enhanced classification for Signal users (uses their BYOK key)

### 2.2 STREETS Dashboard Panel (Signal Feature)

**What:** A new panel in the 4DA UI showing opportunity signals in a dedicated view.

**Where:** New component `src/components/StreetsPanel.tsx`

**Layout:**

```
+--------------------------------------------------+
| STREETS — Opportunity Radar            [Signal]   |
+--------------------------------------------------+
| This Week: 3 opportunities detected              |
|                                                   |
| [!] ProductGap — HIGH                    2h ago   |
|     "Astro 5.0 released, no production            |
|      starter with auth + DB exists yet"           |
|     Engine: Digital Products                      |
|     Time window: ~2 weeks before saturation       |
|     [Explore] [Dismiss] [Save]                    |
|                                                   |
| [i] EducationGap — MEDIUM               1d ago   |
|     "New Rust async trait RFC merged,              |
|      no migration guide for existing code"        |
|     Engine: Content Monetization                  |
|     Time window: ~1 month                         |
|     [Explore] [Dismiss] [Save]                    |
|                                                   |
| [i] CommunityDemand — MEDIUM            3d ago   |
|     "r/selfhosted: 200+ upvotes asking for        |
|      local-first alternative to Notion"           |
|     Engine: Micro-SaaS                            |
|     Time window: ~3 months                        |
|     [Explore] [Dismiss] [Save]                    |
+--------------------------------------------------+
```

**Interactions:**
- **Explore:** Opens detail view with full context, related signals, and suggested first steps
- **Dismiss:** Removes from view, trains the model on what's NOT relevant
- **Save:** Bookmarks to a "My Opportunities" list for later action

**Design tokens:**
- Uses existing 4DA dark theme
- Opportunity type badges use accent colors: HIGH = #EF4444, MEDIUM = #F97316, LOW = #22C55E
- Revenue engine tag uses gold accent (#D4AF37)

### 2.3 Opportunity in Daily Briefing (Signal Feature)

**What:** Add an "Opportunities" section to the existing AI daily briefing.

**Where:** Extend `src-tauri/src/agent_brief.rs`

**Briefing addition:**

```markdown
## Opportunities

Based on today's signals and your Developer DNA:

**Product Gap Detected:** Tauri 3.0 was announced yesterday. There are currently
no production-ready starter templates that include auth, database, and deployment.
Your DNA shows Tauri as a primary stack technology. You could build and sell a
"Tauri 3.0 Production Starter" on Gumroad within 1-2 weeks.

→ Estimated market: 5,000+ Tauri developers in first month
→ Suggested price: $49-79
→ Time investment: ~20 hours
→ Revenue engine: Digital Products
```

**Rules:**
- Maximum 1-2 opportunities per briefing (don't overwhelm)
- Only show HIGH confidence opportunities
- Must match user's Developer DNA (don't suggest React templates to a Rust-only dev)
- Include time sensitivity ("act within X days/weeks")

### 2.4 Developer DNA — Income Readiness Score

**What:** Add an "Income Readiness" dimension to Developer DNA.

**Where:** Extend `src-tauri/src/developer_dna.rs`

**New fields:**

```rust
pub struct IncomeReadiness {
    /// Skills that are currently in-demand and monetizable
    pub marketable_skills: Vec<MarketableSkill>,
    /// Gaps between what you know and what the market pays for
    pub skill_gaps: Vec<SkillGap>,
    /// Your niche uniqueness score (how rare is your skill combination?)
    pub niche_score: f32,
    /// Suggested revenue engines based on your DNA
    pub suggested_engines: Vec<StreetsEngine>,
    /// Estimated income potential range (monthly)
    pub income_potential: IncomeRange,
}

pub struct MarketableSkill {
    pub skill: String,
    pub market_demand: Demand,     // Low/Medium/High/Critical
    pub competition: Competition,   // Saturated/Competitive/Moderate/Low/BlueOcean
    pub typical_rate: String,       // "$150-250/hr consulting" or "$29/mo SaaS"
}
```

**Display:** Add a "STREETS" tab to the Developer DNA view:

```
+--------------------------------------------------+
| Developer DNA — Income Readiness        [Signal]  |
+--------------------------------------------------+
|                                                   |
| Niche Score: 87/100                               |
| "Your combination of Rust + Tauri + AI is rare.   |
|  Only ~2% of developers share this profile."      |
|                                                   |
| Top Marketable Skills:                            |
|   Rust systems programming    — High demand, Low competition  |
|   Tauri desktop apps          — Medium demand, Very low competition |
|   Local AI deployment         — Critical demand, Low competition |
|                                                   |
| Suggested Revenue Engines:                        |
|   1. Consulting ($200-350/hr for Rust + AI)       |
|   2. Digital Products (Tauri templates)            |
|   3. Education (Local AI deployment course)        |
|                                                   |
| Estimated Monthly Potential: $3,000-8,000         |
| (Based on 10 hrs/week at your skill level)        |
|                                                   |
| Skill Gaps to Unlock More:                        |
|   Marketing/copywriting — would 2x product sales  |
|   Video production — YouTube is underserved for    |
|   Rust content                                     |
+--------------------------------------------------+
```

### 2.5 MCP Tools Extension

**What:** Add STREETS-related tools to the 4DA MCP server.

**New tools:**

```typescript
// Get current income opportunities based on recent signals
get_opportunities(options?: {
  min_confidence?: number,   // 0.0-1.0, default 0.5
  engine?: StreetsEngine,    // Filter by revenue engine
  max_results?: number       // default 5
}): OpportunitySignal[]

// Get income readiness analysis from Developer DNA
get_income_readiness(): IncomeReadiness

// Get opportunity history (what you've explored, dismissed, saved)
get_opportunity_history(options?: {
  status?: 'explored' | 'dismissed' | 'saved',
  days?: number  // lookback period
}): OpportunitySignal[]
```

**Use case:** A developer in Claude Code asks "What should I build next?" and the MCP tool provides data-driven suggestions based on actual market signals.

---

## 3. Cross-Promotion

### 3.1 In-App STREETS Promotion (Non-Intrusive)

**Where:** Settings panel or a "Learn" section in the sidebar.

**Rules:**
- NEVER interrupt the core 4DA experience
- NEVER show popups or modals about STREETS
- Only surface in dedicated "Learn" or "Grow" sections
- Free tier: small banner in the Opportunity panel — "Learn to act on these signals → STREETS"
- Signal tier: contextual tip when viewing an opportunity — "STREETS Module R teaches 8 revenue engines for signals like this"

### 3.2 Pricing Model

```typescript
interface StreetsPricingConfig {
  // STREETS Playbook — all 7 modules, completely free
  playbook: {
    price: 0,
    access: "free forever — no email, no account, no paywall",
    includes: "all 7 modules, 42 lessons, 8 revenue engines, income stacking framework",
  },

  // Revenue comes from 4DA, not STREETS
  // STREETS drives 4DA adoption; 4DA Signal is the paid product
  revenueModel: "STREETS is free. 4DA Signal ($12/mo or $99/yr) is the upgrade path.",
}
```

### 3.3 Growth Loop

```
Developer discovers STREETS (search, word of mouth, 4da.ai/streets)
    → Learns the 8 revenue engines and income stacking framework
    → "How do I find these opportunities in real-time?"
    → Discovers 4DA — the intelligence layer that surfaces signals
    → Downloads 4DA (free)
    → Sees opportunity signals mapped to STREETS engines
    → Upgrades to Signal for AI briefings and deeper intelligence
    → Shares STREETS with developers who need it (organic)

4DA User sees Opportunity Signal
    → "Learn the full playbook for acting on signals like this"
    → Link to STREETS with UTM tracking
    → Attribution: 4da_opportunity_panel
```

---

## 4. Implementation Phases

### Phase 1: Content-Only (No Code Changes)
**Timeline:** Immediate
**What:**
- STREETS playbook live at 4da.ai/streets (standalone page)
- All 7 modules free, embedded in 4DA app
- Cross-promote on 4DA landing page and in README
- Add "STREETS" link to 4DA app's About/Help section
- No code changes to 4DA itself

### Phase 2: Opportunity Signals (Backend)
**Timeline:** 4-6 weeks
**What:**
- Add `OpportunityType` classification to signal pipeline
- Rule-based detection (no LLM cost)
- Store opportunity signals in existing SQLite database
- Expose via existing MCP server

### Phase 3: STREETS Panel (Frontend)
**Timeline:** 6-8 weeks
**What:**
- Build STREETS panel component
- Add opportunity section to daily briefing
- Income readiness score in Developer DNA
- In-app cross-promotion (non-intrusive)

### Phase 4: Intelligence Loop (LLM-Enhanced)
**Timeline:** 8-12 weeks
**What:**
- LLM-powered opportunity detection (Signal only, uses BYOK key)
- Personalized opportunity scoring based on Developer DNA
- New MCP tools

---

## 5. Success Metrics

| Metric | Target (Month 1) | Target (Month 3) |
|--------|-------------------|-------------------|
| STREETS playbook opens from 4DA | 500 | 2,000 |
| 4DA downloads from STREETS landing page | 20 | 100 |
| Signal upgrades attributed to STREETS | 5 | 30 |
| Opportunity signals generated/day | 2-5 | 5-15 |
| Opportunity explore rate | 30%+ | 40%+ |

### Revenue Attribution

```
4DA Signal:            $12/mo per user (or $99/yr)
STREETS Playbook:      $0 (free — drives 4DA adoption)

At 200 Signal users (30 attributed to STREETS funnel):
  4DA Signal MRR:      $2,400/mo
  STREETS-attributed:  $360/mo (15% of Signal MRR)

STREETS ROI is measured by Signal conversion lift, not direct revenue.
```

---

## 6. Resolved Design Questions

1. **Should STREETS be a separate domain or a section of 4da.ai?**
   **Decision:** `4da.ai/streets` — keeps brand connection, dedicated SEO, shared authority. STREETS is a gift within 4DA, also accessible standalone.

2. **Should opportunity signals be free or Signal-only?**
   **Decision:** Show 1 opportunity/week on free, unlimited on Signal.

3. **Should STREETS content live in the 4DA repo or separate?**
   **Decision:** Same repo. Content is in `docs/streets/`, landing page in `site/`, in-app integration in main codebase. Single deploy, single source of truth.

---

*This spec is a living document. Update as STREETS and 4DA evolve together.*
