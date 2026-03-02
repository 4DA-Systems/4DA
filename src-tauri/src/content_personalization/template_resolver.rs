//! L1 path resolution — resolves dotted paths against PersonalizationContext.
//!
//! Extracted from template_processor.rs to keep file sizes within limits.
//! The main entry point is `resolve_path()`, called by L1 interpolation
//! and L2 conditional evaluation.

use super::context::PersonalizationContext;

/// Resolve a dotted path like `profile.cpu.model` against the PersonalizationContext.
pub(super) fn resolve_path(path: &str, ctx: &PersonalizationContext) -> Option<String> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "profile" => resolve_profile_path(&parts[1..], ctx),
        "stack" => resolve_stack_path(&parts[1..], ctx),
        "radar" => resolve_radar_path(&parts[1..], ctx),
        "regional" => resolve_regional_path(&parts[1..], ctx),
        "progress" => resolve_progress_path(&parts[1..], ctx),
        "settings" => resolve_settings_path(&parts[1..], ctx),
        "dna" => resolve_dna_path(&parts[1..], ctx),
        "computed" => resolve_computed_path(&parts[1..], ctx),
        _ => None,
    }
}

fn resolve_profile_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    if parts.is_empty() {
        return None;
    }
    let category_map = match parts[0] {
        "cpu" => &ctx.profile.cpu,
        "ram" => &ctx.profile.ram,
        "gpu" => &ctx.profile.gpu,
        "storage" => &ctx.profile.storage,
        "network" => &ctx.profile.network,
        "os" => &ctx.profile.os,
        "llm" => &ctx.profile.llm,
        "legal" => &ctx.profile.legal,
        "budget" => &ctx.profile.budget,
        _ => return None,
    };

    if parts.len() == 1 {
        // Return all values as comma-separated string
        if category_map.is_empty() {
            return None;
        }
        let mut pairs: Vec<_> = category_map.iter().collect();
        pairs.sort_by_key(|(k, _)| (*k).clone());
        return Some(
            pairs
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    // Direct key lookup: profile.cpu.model
    category_map
        .get(parts[1])
        .cloned()
        .filter(|v| !v.is_empty())
}

fn resolve_stack_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "primary" => non_empty_join(&ctx.stack.primary),
        "adjacent" => non_empty_join(&ctx.stack.adjacent),
        "interests" => non_empty_join(&ctx.stack.interests),
        _ => None,
    }
}

fn resolve_radar_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "adopt" => non_empty_join(&ctx.radar.adopt),
        "trial" => non_empty_join(&ctx.radar.trial),
        "assess" => non_empty_join(&ctx.radar.assess),
        "hold" => non_empty_join(&ctx.radar.hold),
        _ => None,
    }
}

fn resolve_regional_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    let val = match *parts.first()? {
        "country" => &ctx.regional.country,
        "currency" => &ctx.regional.currency,
        "currency_symbol" => &ctx.regional.currency_symbol,
        "electricity_kwh" => return Some(format!("{:.3}", ctx.regional.electricity_kwh)),
        "internet_monthly" => return Some(format!("{:.0}", ctx.regional.internet_monthly)),
        "business_registration_cost" => {
            return Some(format!("{:.0}", ctx.regional.business_registration_cost))
        }
        "business_entity_type" => &ctx.regional.business_entity_type,
        "tax_note" => &ctx.regional.tax_note,
        "payment_processors" => return non_empty_join(&ctx.regional.payment_processors),
        _ => return None,
    };
    if val.is_empty() {
        None
    } else {
        Some(val.clone())
    }
}

fn resolve_progress_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "completed_count" => Some(ctx.progress.completed_lesson_count.to_string()),
        "total_count" => Some(ctx.progress.total_lesson_count.to_string()),
        "completed_modules" => non_empty_join(&ctx.progress.completed_modules),
        _ => None,
    }
}

fn resolve_settings_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "llm_provider" => non_empty_str(&ctx.settings.llm_provider),
        "llm_model" => non_empty_str(&ctx.settings.llm_model),
        _ => None,
    }
}

fn resolve_dna_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "primary_stack" => non_empty_join(&ctx.dna.primary_stack),
        "interests" => non_empty_join(&ctx.dna.interests),
        "identity_summary" => non_empty_str(&ctx.dna.identity_summary),
        "blind_spots" => non_empty_join(&ctx.dna.blind_spots),
        "top_engaged_topics" => non_empty_join(&ctx.dna.top_engaged_topics),
        _ => None,
    }
}

fn resolve_computed_path(parts: &[&str], ctx: &PersonalizationContext) -> Option<String> {
    match *parts.first()? {
        "llm_tier" => Some(ctx.computed.llm_tier.clone()),
        "gpu_tier" => Some(ctx.computed.gpu_tier.clone()),
        "os_family" => Some(ctx.computed.os_family.clone()),
        "profile_completeness" => Some(format!("{:.0}", ctx.computed.profile_completeness)),
        "monthly_electricity_estimate" => {
            Some(format!("{:.1}", ctx.computed.monthly_electricity_estimate))
        }
        _ => None,
    }
}

/// Helper: join a Vec<String> with ", " or return None if empty.
fn non_empty_join(v: &[String]) -> Option<String> {
    if v.is_empty() {
        None
    } else {
        Some(v.join(", "))
    }
}

/// Helper: return Some if non-empty string, None otherwise.
fn non_empty_str(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
