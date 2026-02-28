// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Pre-built markdown templates for STREETS Community and Cohort members.
//!
//! Provides actionable templates for launch planning, revenue tracking,
//! automation blueprints, competitive analysis, and pricing strategy.

use serde::{Deserialize, Serialize};
use tracing::debug;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub content: String,
}

// ============================================================================
// Template Content
// ============================================================================

const TEMPLATE_30DAY_LAUNCH: &str = "\
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

const TEMPLATE_REVENUE_TRACKER: &str = "\
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

const TEMPLATE_AUTOMATION_BLUEPRINT: &str = "\
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

const TEMPLATE_COMPETITIVE_ANALYSIS: &str = "\
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

const TEMPLATE_PRICING_CALCULATOR: &str = "\
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

// ============================================================================
// Template Registry
// ============================================================================

fn get_all_templates() -> Vec<CoachTemplate> {
    vec![
        CoachTemplate {
            id: "30-day-launch".into(),
            title: "30-Day Engine Launch Plan".into(),
            description: "Step-by-step plan to launch your first revenue engine in 30 days".into(),
            category: "launch".into(),
            content: TEMPLATE_30DAY_LAUNCH.to_string(),
        },
        CoachTemplate {
            id: "revenue-tracker".into(),
            title: "Revenue Tracking Spreadsheet".into(),
            description: "Track revenue, costs, and key metrics across your engines".into(),
            category: "tracking".into(),
            content: TEMPLATE_REVENUE_TRACKER.to_string(),
        },
        CoachTemplate {
            id: "automation-blueprint".into(),
            title: "Automation Blueprint".into(),
            description: "Audit manual processes and build a 4-week automation plan".into(),
            category: "automation".into(),
            content: TEMPLATE_AUTOMATION_BLUEPRINT.to_string(),
        },
        CoachTemplate {
            id: "competitive-analysis".into(),
            title: "Competitive Analysis Framework".into(),
            description: "Analyze competitors, find gaps, and define your moat".into(),
            category: "analysis".into(),
            content: TEMPLATE_COMPETITIVE_ANALYSIS.to_string(),
        },
        CoachTemplate {
            id: "pricing-calculator".into(),
            title: "Pricing Calculator Worksheet".into(),
            description: "Calculate costs, model pricing tiers, and plan revenue milestones".into(),
            category: "pricing".into(),
            content: TEMPLATE_PRICING_CALCULATOR.to_string(),
        },
    ]
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all available coach templates.
/// Gate: requires streets_community membership.
#[tauri::command]
pub fn get_templates() -> Result<Vec<CoachTemplate>, String> {
    crate::settings::require_streets_feature("streets_community")?;

    let templates = get_all_templates();
    debug!(
        target: "4da::coach",
        count = templates.len(),
        "Returning coach templates"
    );

    Ok(templates)
}

/// Get a specific template by ID.
/// Gate: requires streets_community membership.
#[tauri::command]
pub fn get_template_content(template_id: String) -> Result<CoachTemplate, String> {
    crate::settings::require_streets_feature("streets_community")?;

    let templates = get_all_templates();
    let template = templates
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| format!("Template not found: {}", template_id))?;

    debug!(
        target: "4da::coach",
        template_id = %template_id,
        "Returning template content"
    );

    Ok(template)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- get_all_templates registry -------------------------------------------

    #[test]
    fn test_get_all_templates_returns_five() {
        let templates = get_all_templates();
        assert_eq!(
            templates.len(),
            5,
            "registry should contain exactly 5 templates"
        );
    }

    #[test]
    fn test_template_ids_are_unique() {
        let templates = get_all_templates();
        let mut ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 5, "all template IDs must be unique");
    }

    #[test]
    fn test_expected_template_ids_present() {
        let templates = get_all_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        let expected = [
            "30-day-launch",
            "revenue-tracker",
            "automation-blueprint",
            "competitive-analysis",
            "pricing-calculator",
        ];
        for id in &expected {
            assert!(ids.contains(id), "expected template id '{}' missing", id);
        }
    }

    #[test]
    fn test_template_categories_are_distinct() {
        let templates = get_all_templates();
        let mut cats: Vec<&str> = templates.iter().map(|t| t.category.as_str()).collect();
        cats.sort();
        cats.dedup();
        assert_eq!(cats.len(), 5, "each template should have a unique category");
    }

    #[test]
    fn test_no_template_has_empty_fields() {
        for t in get_all_templates() {
            assert!(!t.id.is_empty(), "template id must not be empty");
            assert!(!t.title.is_empty(), "template title must not be empty");
            assert!(
                !t.description.is_empty(),
                "template '{}' description must not be empty",
                t.id
            );
            assert!(
                !t.category.is_empty(),
                "template '{}' category must not be empty",
                t.id
            );
            assert!(
                !t.content.is_empty(),
                "template '{}' content must not be empty",
                t.id
            );
        }
    }

    // -- Template content validation ------------------------------------------

    #[test]
    fn test_template_content_starts_with_markdown_heading() {
        for t in get_all_templates() {
            assert!(
                t.content.starts_with("# "),
                "template '{}' content should start with a markdown heading, got: {:?}",
                t.id,
                &t.content[..t.content.len().min(40)]
            );
        }
    }

    #[test]
    fn test_template_content_contains_checkboxes() {
        for t in get_all_templates() {
            assert!(
                t.content.contains("- [ ]"),
                "template '{}' should contain at least one markdown checkbox",
                t.id
            );
        }
    }

    #[test]
    fn test_launch_template_has_four_weeks() {
        let templates = get_all_templates();
        let launch = templates.iter().find(|t| t.id == "30-day-launch").unwrap();
        assert!(launch.content.contains("## Week 1"));
        assert!(launch.content.contains("## Week 2"));
        assert!(launch.content.contains("## Week 3"));
        assert!(launch.content.contains("## Week 4"));
    }

    #[test]
    fn test_revenue_tracker_has_expense_table() {
        let templates = get_all_templates();
        let tracker = templates
            .iter()
            .find(|t| t.id == "revenue-tracker")
            .unwrap();
        assert!(
            tracker.content.contains("## Expense Categories"),
            "revenue tracker should include expense categories section"
        );
        assert!(
            tracker.content.contains("| Category"),
            "revenue tracker should include expense table header"
        );
    }

    // -- Lookup by ID (mirrors get_template_content logic) --------------------

    #[test]
    fn test_find_template_by_valid_id() {
        let templates = get_all_templates();
        let found = templates.into_iter().find(|t| t.id == "pricing-calculator");
        assert!(found.is_some(), "should find pricing-calculator template");
        let tmpl = found.unwrap();
        assert_eq!(tmpl.category, "pricing");
        assert!(tmpl.content.contains("Pricing Calculator Worksheet"));
    }

    #[test]
    fn test_find_template_by_invalid_id_returns_none() {
        let templates = get_all_templates();
        let found = templates
            .into_iter()
            .find(|t| t.id == "nonexistent-template");
        assert!(
            found.is_none(),
            "should return None for a nonexistent template ID"
        );
    }

    // -- Serde roundtrip ------------------------------------------------------

    #[test]
    fn test_coach_template_serde_roundtrip() {
        let original = CoachTemplate {
            id: "test-id".into(),
            title: "Test Title".into(),
            description: "Test description".into(),
            category: "testing".into(),
            content: "# Test\n\n- [ ] Item".into(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: CoachTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.title, original.title);
        assert_eq!(deserialized.description, original.description);
        assert_eq!(deserialized.category, original.category);
        assert_eq!(deserialized.content, original.content);
    }

    #[test]
    fn test_coach_template_json_field_names() {
        let tmpl = CoachTemplate {
            id: "abc".into(),
            title: "T".into(),
            description: "D".into(),
            category: "C".into(),
            content: "X".into(),
        };
        let val = serde_json::to_value(&tmpl).expect("to_value");
        assert!(val.get("id").is_some(), "JSON must include 'id'");
        assert!(val.get("title").is_some(), "JSON must include 'title'");
        assert!(
            val.get("description").is_some(),
            "JSON must include 'description'"
        );
        assert!(
            val.get("category").is_some(),
            "JSON must include 'category'"
        );
        assert!(val.get("content").is_some(), "JSON must include 'content'");
        // Ensure no extra fields
        let obj = val.as_object().unwrap();
        assert_eq!(
            obj.len(),
            5,
            "CoachTemplate should serialize to exactly 5 fields"
        );
    }

    #[test]
    fn test_all_templates_serialize_successfully() {
        for t in get_all_templates() {
            let result = serde_json::to_value(&t);
            assert!(
                result.is_ok(),
                "template '{}' should serialize to JSON without error",
                t.id
            );
        }
    }
}
