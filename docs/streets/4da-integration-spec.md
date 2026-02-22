# STREETS x 4DA Product Integration Spec

**Author:** Lead Dev
**Date:** 2026-02-18
**Status:** Design Draft
**Purpose:** Define how STREETS connects to 4DA at the product level — not just marketing, but actual features.

---

## 1. Strategic Rationale

STREETS is not a separate product bolted onto 4DA. It's the **monetization thesis** that makes 4DA indispensable for a specific, high-value user segment: developers who generate income from their skills.

**The logic chain:**
1. Developer uses 4DA to stay current (free tier)
2. 4DA surfaces signals: trending frameworks, breaking changes, new techniques
3. Developer realizes these signals = income opportunities (STREETS mindset)
4. Developer upgrades to Pro for AI briefings, deeper intelligence
5. Developer uses intelligence to build products, content, services
6. Income justifies Pro subscription many times over
7. Developer tells other developers (organic growth)

**Revenue impact:** STREETS transforms the value narrative of 4DA Pro from "nice to have intelligence" to "income-generating radar." The $12/mo Pro subscription pays for itself if it surfaces ONE opportunity per month.

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
- Phase 2: LLM-enhanced classification for Pro users (uses their BYOK key)
- Phase 3: Community-validated opportunities (users confirm/dismiss)

### 2.2 STREETS Dashboard Panel (Pro Feature)

**What:** A new panel in the 4DA UI showing opportunity signals in a dedicated view.

**Where:** New component `src/components/StreetsPanel.tsx`

**Layout:**

```
+--------------------------------------------------+
| STREETS — Opportunity Radar            [Pro]      |
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

### 2.3 Opportunity in Daily Briefing (Pro Feature)

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
| Developer DNA — Income Readiness        [Pro]     |
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

## 3. Cross-Sell Integration

### 3.1 In-App STREETS Promotion (Non-Intrusive)

**Where:** Settings panel or a "Learn" section in the sidebar.

**Rules:**
- NEVER interrupt the core 4DA experience
- NEVER show popups or modals about STREETS
- Only surface in dedicated "Learn" or "Grow" sections
- Free tier: small banner in the Opportunity panel — "Learn to act on these signals → STREETS"
- Pro tier: contextual tip when viewing an opportunity — "STREETS Module R teaches 8 revenue engines for signals like this"

### 3.2 Pricing Model

```typescript
interface StreetsPricingConfig {
  // STREETS Playbook — all 7 modules free inside 4DA
  playbook: {
    price: 0,
    access: "free forever — no email, no account, no paywall",
  },

  // Community — accountability, networking, ongoing value
  community: {
    monthlyPrice: 29,
    annualPrice: 249,  // save $99/year
    includes: "Discord, office hours, case studies, templates",
  },

  // Cohort — structured live program
  cohort: {
    price: 499,
    includes: "8-week live program, 1-on-1 strategy session, accountability group",
  },
}
```

### 3.3 Referral Loop

```
STREETS Student completes Module T (Technical Moats)
    → "Identify opportunities in your niche using 4DA"
    → Link to 4DA download with UTM tracking
    → Attribution: streets_module_t

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
- Launch STREETS course as standalone content product
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
- In-app cross-sell (non-intrusive)

### Phase 4: Intelligence Loop (LLM-Enhanced)
**Timeline:** 8-12 weeks
**What:**
- LLM-powered opportunity detection (Pro only, uses BYOK key)
- Personalized opportunity scoring based on Developer DNA
- Community-validated opportunities
- New MCP tools

---

## 5. Success Metrics

| Metric | Target (Month 1) | Target (Month 3) |
|--------|-------------------|-------------------|
| STREETS playbook opens from 4DA | 500 | 2,000 |
| Community signups from 4DA users | 10 | 50 |
| 4DA downloads from STREETS landing page | 20 | 100 |
| Cohort enrollments | 5 | 24 (full cohort) |
| Opportunity signals generated/day | 2-5 | 5-15 |
| Opportunity explore rate | 30%+ | 40%+ |

### Revenue Attribution

```
4DA Pro only:              $12/mo per user
STREETS Playbook:          $0 (free inside 4DA — drives adoption)
STREETS Community:         $29/mo per user (or $249/year)
STREETS Cohort:            $499 per participant

At 200 Pro users + 150 Community members + 24 Cohort participants:
  4DA MRR:         $2,400/mo
  Community MRR:   $4,350/mo
  Cohort Rev:      $11,976/quarter (24 seats x $499)

  Combined MRR:        ~$6,750/mo
  Combined Annual:     ~$92,976
```

---

## 6. Open Questions

1. **Should STREETS be a separate domain or a section of 4da.ai?**
   - Separate (streets.4da.ai): cleaner brand, dedicated SEO
   - Section (4da.ai/streets): simpler infrastructure, shared authority
   - **Recommendation:** Subdomain `streets.4da.ai` — keeps brand connection while allowing independent growth

2. **Should opportunity signals be free or Pro-only?**
   - Free: drives upgrades by showing value ("you're seeing opportunities, imagine what Pro briefings add")
   - Pro: cleaner tier separation, more upgrade incentive
   - **Recommendation:** Show 1 opportunity/week on free, unlimited on Pro

3. **Should STREETS content live in the 4DA repo or separate?**
   - Same repo: easier to manage, single deploy
   - Separate: cleaner separation, different release cadence
   - **Recommendation:** Separate repo, linked via submodule or just URLs

4. **Annual update model — who writes the 2027 edition?**
   - Solo dev writes it (current model)
   - Community contributes case studies, solo dev curates
   - **Recommendation:** Hybrid — solo dev writes framework, community case studies fill it out

---

*This spec is a living document. Update as STREETS and 4DA evolve together.*
