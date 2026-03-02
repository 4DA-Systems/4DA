// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Template string constants for STREETS coach templates.
//!
//! Extracted from `coach_templates.rs` to keep file sizes under the 600-line limit.

pub(crate) const TEMPLATE_30DAY_LAUNCH: &str = "\
# 30-Day Engine Launch Plan

## Week 1: Validation (Days 1-7)
- [ ] Define your engine type (Digital Product / Micro-SaaS / Content / API / etc.)
- [ ] Identify target audience (who has the pain you solve?)
- [ ] Research 3 competitors — note pricing, positioning, gaps
- [ ] Define MVP scope (max 3 features, ruthlessly cut everything else)
- [ ] Write a one-sentence value proposition
- [ ] Validate with 5 real conversations (not surveys)
- [ ] Decision gate: proceed, pivot, or kill

## Week 2: Build (Days 8-14)
- [ ] Set up project repository with CI/CD
- [ ] Build core feature #1 (the one users cannot live without)
- [ ] Build core feature #2 (the differentiator)
- [ ] Deploy to staging environment
- [ ] Write 3 test scenarios and verify manually
- [ ] Set up error tracking (Sentry, LogRocket, etc.)
- [ ] Create a README with setup instructions

## Week 3: Launch Prep (Days 15-21)
- [ ] Set up payment processing (Stripe / Lemon Squeezy / Gumroad)
- [ ] Create landing page with clear CTA
- [ ] Write launch announcement (3 paragraphs max)
- [ ] Prepare 3 social posts for launch day
- [ ] Set up analytics (Plausible / PostHog / Simple Analytics)
- [ ] Soft launch to 10 trusted people for feedback
- [ ] Fix showstopper bugs from soft launch

## Week 4: Launch & Iterate (Days 22-30)
- [ ] Public launch — share in 3 relevant communities
- [ ] Collect feedback systematically (spreadsheet or Notion)
- [ ] Fix top 3 issues reported by users
- [ ] Set up basic monitoring and uptime alerts
- [ ] Write a retrospective: what worked, what didn't
- [ ] Plan next 30-day iteration cycle
- [ ] Celebrate shipping something real
";

pub(crate) const TEMPLATE_REVENUE_TRACKER: &str = "\
# Revenue Tracking Spreadsheet

## Monthly Revenue Dashboard

| Month | Engine | Revenue | Costs | Profit | Customers | MRR |
|-------|--------|---------|-------|--------|-----------|-----|
| Jan   |        | $0      | $0    | $0     | 0         | $0  |
| Feb   |        | $0      | $0    | $0     | 0         | $0  |
| Mar   |        | $0      | $0    | $0     | 0         | $0  |

## Revenue Engine Breakdown

### Engine 1: _________________
- [ ] Type: (Digital Product / SaaS / Content / etc.)
- [ ] Monthly target: $______
- [ ] Current MRR: $______
- [ ] Customer count: ______
- [ ] Churn rate: ______%
- [ ] CAC (Customer Acquisition Cost): $______
- [ ] LTV (Lifetime Value): $______

### Engine 2: _________________
- [ ] Type: (Digital Product / SaaS / Content / etc.)
- [ ] Monthly target: $______
- [ ] Current MRR: $______
- [ ] Customer count: ______
- [ ] Churn rate: ______%
- [ ] CAC: $______
- [ ] LTV: $______

## Key Metrics

### Monthly Targets
- [ ] Total revenue target: $______/month
- [ ] Total profit target: $______/month
- [ ] Runway remaining: ______ months

### Growth Tracking
- [ ] Week-over-week growth: ______%
- [ ] Month-over-month growth: ______%
- [ ] Revenue per customer: $______
- [ ] Time to first dollar: ______ days

## Expense Categories

| Category        | Monthly Cost | Annual Cost | Notes |
|-----------------|-------------|-------------|-------|
| Hosting         | $0          | $0          |       |
| Domain/DNS      | $0          | $0          |       |
| Tools/SaaS      | $0          | $0          |       |
| Marketing       | $0          | $0          |       |
| Payment fees    | $0          | $0          |       |
| **Total**       | **$0**      | **$0**      |       |
";

pub(crate) const TEMPLATE_AUTOMATION_BLUEPRINT: &str = "\
# Automation Blueprint

## Current Manual Processes

List every repetitive task you do weekly. Be honest — automation only works on real workflows.

| Task | Frequency | Time Spent | Automatable? | Priority |
|------|-----------|------------|-------------|----------|
|      | daily     | 15 min     | Yes/No      | High     |
|      | weekly    | 30 min     | Yes/No      | Medium   |
|      | monthly   | 1 hour     | Yes/No      | Low      |

## Automation Stack

### Tier 1: Zero-Code (start here)
- [ ] Email filters and auto-responses
- [ ] Calendar scheduling (Calendly / Cal.com)
- [ ] Social media scheduling (Buffer / native schedulers)
- [ ] Invoice generation (Stripe auto-billing)
- [ ] File backup (automated cloud sync)

### Tier 2: Low-Code
- [ ] Zapier / Make.com workflows for cross-app automation
- [ ] GitHub Actions for CI/CD and scheduled tasks
- [ ] Cron jobs for data collection and reporting
- [ ] Auto-deploy pipelines (push to main = live)
- [ ] Monitoring alerts (uptime, error rate, revenue threshold)

### Tier 3: Custom Code
- [ ] Custom CLI tools for your specific workflow
- [ ] API integrations between your tools
- [ ] Automated testing and quality checks
- [ ] Content generation pipelines
- [ ] Customer onboarding automation

## Implementation Plan

### Week 1: Audit
- [ ] Log all manual tasks for one full week
- [ ] Categorize by automation tier (zero-code / low-code / custom)
- [ ] Calculate time saved per automation
- [ ] Prioritize by (time saved * frequency)

### Week 2: Quick Wins
- [ ] Implement top 3 zero-code automations
- [ ] Set up monitoring for automated processes
- [ ] Document each automation (trigger, action, fallback)

### Week 3: Core Automations
- [ ] Build the one custom automation that saves the most time
- [ ] Connect your revenue tools (payment -> CRM -> email)
- [ ] Set up error notifications for all automations

### Week 4: Harden
- [ ] Test failure modes for each automation
- [ ] Add fallback procedures for when automations break
- [ ] Create a dashboard to monitor all automations
- [ ] Write runbook for manual overrides
";

pub(crate) const TEMPLATE_COMPETITIVE_ANALYSIS: &str = "\
# Competitive Analysis Framework

## Your Product
- **Name:** _________________
- **One-line description:** _________________
- **Target audience:** _________________
- **Price point:** $______

## Competitor Matrix

### Competitor 1: _________________
- [ ] Website: _________________
- [ ] Pricing: $______ (free tier? trial?)
- [ ] Monthly traffic estimate: ______
- [ ] Key features (top 3):
  1. _________________
  2. _________________
  3. _________________
- [ ] Strengths: _________________
- [ ] Weaknesses: _________________
- [ ] What users complain about (check reviews, Twitter, Reddit):
  - _________________
  - _________________

### Competitor 2: _________________
- [ ] Website: _________________
- [ ] Pricing: $______
- [ ] Monthly traffic estimate: ______
- [ ] Key features (top 3):
  1. _________________
  2. _________________
  3. _________________
- [ ] Strengths: _________________
- [ ] Weaknesses: _________________
- [ ] User complaints:
  - _________________

### Competitor 3: _________________
- [ ] Website: _________________
- [ ] Pricing: $______
- [ ] Monthly traffic estimate: ______
- [ ] Key features (top 3):
  1. _________________
  2. _________________
  3. _________________
- [ ] Strengths: _________________
- [ ] Weaknesses: _________________
- [ ] User complaints:
  - _________________

## Feature Comparison

| Feature          | You | Comp 1 | Comp 2 | Comp 3 |
|-----------------|-----|--------|--------|--------|
| Feature A       |     |        |        |        |
| Feature B       |     |        |        |        |
| Feature C       |     |        |        |        |
| Free tier       |     |        |        |        |
| API access      |     |        |        |        |
| Self-hosted     |     |        |        |        |

## Your Moat

### What makes you different? (pick ONE)
- [ ] **Speed:** You ship faster than anyone
- [ ] **Depth:** You go deeper on a niche problem
- [ ] **Price:** You undercut the market sustainably
- [ ] **Experience:** Your UX is 10x better
- [ ] **Integration:** You work where others don't
- [ ] **Trust:** You're privacy-first / open-source / transparent

### Gap Analysis
- Gaps competitors leave open: _________________
- Features users want but nobody builds: _________________
- Your unfair advantage: _________________
";

pub(crate) const TEMPLATE_PRICING_CALCULATOR: &str = "\
# Pricing Calculator Worksheet

## Cost Structure

### Fixed Costs (monthly)
| Item                | Cost    |
|---------------------|---------|
| Hosting / infra     | $______ |
| Domain / DNS        | $______ |
| Email service       | $______ |
| Analytics           | $______ |
| Error tracking      | $______ |
| Other tools         | $______ |
| **Total fixed**     | $______ |

### Variable Costs (per customer)
| Item                | Cost    |
|---------------------|---------|
| Payment processing  | $______ |
| Support time value  | $______ |
| Bandwidth / compute | $______ |
| **Total variable**  | $______ |

## Pricing Models

### Option A: One-Time Purchase
- [ ] Price: $______
- [ ] Break-even customers: ______ (fixed costs / (price - variable cost))
- [ ] Target monthly sales: ______
- [ ] Projected monthly revenue: $______
- [ ] Pros: simple, no churn
- [ ] Cons: no recurring revenue

### Option B: Subscription
- [ ] Monthly price: $______/mo
- [ ] Annual price: $______/yr (______% discount)
- [ ] Break-even subscribers: ______
- [ ] Target MRR: $______
- [ ] Expected churn rate: ______%
- [ ] Projected LTV: $______
- [ ] Pros: predictable revenue, compounding
- [ ] Cons: must retain customers

### Option C: Tiered Pricing
| Tier   | Price    | Features                  | Target % |
|--------|----------|---------------------------|----------|
| Free   | $0       | Basic features            | 80%      |
| Pro    | $______  | + advanced features       | 15%      |
| Team   | $______  | + collaboration, priority | 5%       |

## Revenue Targets

### The $1K/Month Milestone
- [ ] At price $______, need ______ customers
- [ ] Realistic timeline: ______ months
- [ ] Primary acquisition channel: _________________
- [ ] Conversion rate assumption: ______%
- [ ] Traffic needed: ______ visitors/month

### The $10K/Month Milestone
- [ ] At price $______, need ______ customers
- [ ] Or: ______ customers at higher tier $______
- [ ] Churn budget: lose max ______ customers/month
- [ ] Growth rate needed: ______%/month
- [ ] Additional channels to explore: _________________

## Pricing Psychology Checklist
- [ ] Price ends in 7 or 9 ($29, $47, $97)
- [ ] Annual plan offers meaningful savings (2+ months free)
- [ ] Free tier exists for lead generation (if SaaS)
- [ ] Price anchoring: show most expensive plan first
- [ ] Social proof near pricing (testimonials, customer count)
- [ ] Money-back guarantee reduces purchase friction
- [ ] Compare to cost of NOT solving the problem
";
