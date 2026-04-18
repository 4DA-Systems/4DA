// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! L2 Conditional Block Processing for the template engine.
//!
//! Handles `{? if/elif/else/endif ?}` conditional blocks by parsing them
//! into a `ConditionalBlock` with `Branch` entries, then evaluating each
//! branch's condition against the `PersonalizationContext`.

use super::context::PersonalizationContext;
use super::template_resolver::resolve_path;

use tracing::debug;

// ============================================================================
// L2: Conditional Blocks
// ============================================================================

/// Process `{? if/elif/else/endif ?}` conditional blocks.
/// Returns (processed_text, conditions_evaluated_count).
pub(crate) fn process_l2(input: &str, ctx: &PersonalizationContext) -> (String, u32) {
    let mut output = String::with_capacity(input.len());
    let mut evaluated = 0u32;
    let mut remaining = input;

    while let Some(if_start) = remaining.find("{? if ") {
        // Output text before the conditional
        output.push_str(&remaining[..if_start]);

        // Find the matching endif
        if let Some((block, after_block_len)) = find_conditional_block(&remaining[if_start..]) {
            let (result, count) = evaluate_conditional_block(&block, ctx);
            output.push_str(&result);
            evaluated += count;
            remaining = &remaining[if_start + after_block_len..];
        } else {
            // Malformed block — pass through and move past the tag
            debug!(target: "4da::personalize", "Malformed L2 conditional block");
            output.push_str("{? if ");
            remaining = &remaining[if_start + 6..];
        }
    }

    output.push_str(remaining);
    (output, evaluated)
}

/// A parsed conditional block with branches.
struct ConditionalBlock<'a> {
    branches: Vec<Branch<'a>>,
}

struct Branch<'a> {
    condition: Option<&'a str>, // None for else
    content: &'a str,
}

/// Find a complete `{? if ... ?}...{? endif ?}` block and return the parsed
/// structure plus the total length consumed from the input.
fn find_conditional_block(input: &str) -> Option<(ConditionalBlock<'_>, usize)> {
    let mut branches = Vec::new();
    let mut depth = 0;
    let mut current_start = 0;
    let mut current_condition: Option<&str> = None;
    let mut pos = 0;
    let mut found_if = false;

    while pos < input.len() {
        if let Some(tag_start) = input[pos..].find("{?") {
            let abs_start = pos + tag_start;
            if let Some(tag_end) = input[abs_start..].find("?}") {
                let abs_end = abs_start + tag_end + 2;
                let tag_body = input[abs_start + 2..abs_start + tag_end].trim();

                if tag_body.starts_with("if ") {
                    if depth == 0 && !found_if {
                        // Opening if
                        let condition = tag_body.strip_prefix("if ").unwrap_or(tag_body).trim();
                        current_condition = Some(condition);
                        current_start = abs_end;
                        found_if = true;
                    }
                    depth += 1;
                    pos = abs_end;
                } else if tag_body.starts_with("elif ") && depth == 1 {
                    // Save previous branch
                    branches.push(Branch {
                        condition: current_condition,
                        content: &input[current_start..abs_start],
                    });
                    let condition = tag_body.strip_prefix("elif ").unwrap_or(tag_body).trim();
                    current_condition = Some(condition);
                    current_start = abs_end;
                    pos = abs_end;
                } else if tag_body == "else" && depth == 1 {
                    branches.push(Branch {
                        condition: current_condition,
                        content: &input[current_start..abs_start],
                    });
                    current_condition = None;
                    current_start = abs_end;
                    pos = abs_end;
                } else if tag_body == "endif" {
                    if depth == 1 {
                        // Closing endif — complete the block
                        branches.push(Branch {
                            condition: current_condition,
                            content: &input[current_start..abs_start],
                        });
                        return Some((ConditionalBlock { branches }, abs_end));
                    }
                    depth -= 1;
                    pos = abs_end;
                } else {
                    pos = abs_end;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    None
}

/// Evaluate a parsed conditional block — return the content of the first
/// branch whose condition is true, or the else branch, or empty string.
fn evaluate_conditional_block(
    block: &ConditionalBlock<'_>,
    ctx: &PersonalizationContext,
) -> (String, u32) {
    let mut count = 0u32;
    for branch in &block.branches {
        count += 1;
        match branch.condition {
            Some(condition) => {
                if evaluate_condition(condition, ctx) {
                    return (branch.content.to_string(), count);
                }
            }
            None => {
                // else branch — always matches
                return (branch.content.to_string(), count);
            }
        }
    }
    // No branch matched
    (String::new(), count)
}

/// Evaluate a single condition expression against the context.
///
/// Supported conditions:
/// - `profile.gpu.exists` — checks if GPU category has data
/// - `profile.gpu.has_nvidia` — checks for NVIDIA GPU
/// - `radar.has("rust", "adopt")` — checks radar ring membership
/// - `stack.contains("python")` — checks stack inclusion
/// - `progress.completed("S")` — checks module completion
/// - `settings.has_llm` — checks LLM availability
/// - `dna.is_full` — checks if full DNA is available
/// - `computed.os_family == "windows"` — equality check
fn evaluate_condition(condition: &str, ctx: &PersonalizationContext) -> bool {
    let cond = condition.trim();

    // Boolean properties
    match cond {
        "profile.gpu.exists" => return !ctx.profile.gpu.is_empty(),
        "profile.gpu.has_nvidia" | "computed.has_nvidia" => return ctx.computed.has_nvidia,
        "settings.has_llm" => return ctx.settings.has_llm,
        "dna.is_full" => return ctx.dna.is_full,
        _ => {}
    }

    // Function-style conditions: radar.has("tech", "ring")
    if let Some(args) = cond.strip_prefix("radar.has(") {
        if let Some(args) = args.strip_suffix(')') {
            let parts: Vec<&str> = args
                .split(',')
                .map(|s| s.trim().trim_matches('"').trim_matches('\''))
                .collect();
            if parts.len() == 2 {
                let tech = parts[0].to_lowercase();
                let ring = parts[1].to_lowercase();
                return match ring.as_str() {
                    "adopt" => ctx.radar.adopt.iter().any(|t| t.to_lowercase() == tech),
                    "trial" => ctx.radar.trial.iter().any(|t| t.to_lowercase() == tech),
                    "assess" => ctx.radar.assess.iter().any(|t| t.to_lowercase() == tech),
                    "hold" => ctx.radar.hold.iter().any(|t| t.to_lowercase() == tech),
                    _ => false,
                };
            }
        }
    }

    // stack.contains("tech")
    if let Some(args) = cond.strip_prefix("stack.contains(") {
        if let Some(args) = args.strip_suffix(')') {
            let tech = args
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_lowercase();
            return ctx.stack.primary.iter().any(|t| t.to_lowercase() == tech)
                || ctx.stack.adjacent.iter().any(|t| t.to_lowercase() == tech);
        }
    }

    // progress.completed("MODULE_ID")
    if let Some(args) = cond.strip_prefix("progress.completed(") {
        if let Some(args) = args.strip_suffix(')') {
            let module_id = args.trim().trim_matches('"').trim_matches('\'');
            return ctx
                .progress
                .completed_modules
                .contains(&module_id.to_string());
        }
    }

    // Equality checks: computed.os_family == "windows"
    if let Some(eq_pos) = cond.find("==") {
        let lhs = cond[..eq_pos].trim();
        let rhs = cond[eq_pos + 2..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if let Some(value) = resolve_path(lhs, ctx) {
            return value.to_lowercase() == rhs.to_lowercase();
        }
        return false;
    }

    // Not-equal checks: computed.os_family != "windows"
    if let Some(ne_pos) = cond.find("!=") {
        let lhs = cond[..ne_pos].trim();
        let rhs = cond[ne_pos + 2..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if let Some(value) = resolve_path(lhs, ctx) {
            return value.to_lowercase() != rhs.to_lowercase();
        }
        return true;
    }

    // Truthy path check: any dotted path that resolves to a non-empty value
    resolve_path(cond, ctx).is_some_and(|v| !v.is_empty())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::content_personalization::context::*;
    use crate::content_personalization::template_processor::process_template;

    fn test_ctx() -> PersonalizationContext {
        let mut cpu = std::collections::HashMap::new();
        cpu.insert("model".into(), "AMD Ryzen 9 7950X".into());
        cpu.insert("cores".into(), "16".into());

        let mut gpu = std::collections::HashMap::new();
        gpu.insert("name".into(), "NVIDIA RTX 4090".into());
        gpu.insert("memory_total".into(), "24 GB".into());

        PersonalizationContext {
            profile: ProfileData {
                cpu,
                gpu,
                ..Default::default()
            },
            stack: StackData {
                primary: vec!["rust".into(), "typescript".into()],
                adjacent: vec!["wasm".into(), "tauri".into()],
                ..Default::default()
            },
            radar: RadarData {
                adopt: vec!["rust".into(), "typescript".into()],
                trial: vec!["zig".into()],
                ..Default::default()
            },
            regional: RegionalData {
                country: "US".into(),
                currency: "USD".into(),
                currency_symbol: "$".into(),
                electricity_kwh: 0.16,
                business_entity_type: "LLC".into(),
                ..Default::default()
            },
            decisions: Vec::new(),
            progress: ProgressData {
                completed_modules: vec!["S".into()],
                completed_lesson_count: 5,
                total_lesson_count: 35,
                ..Default::default()
            },
            settings: SettingsData {
                has_llm: true,
                llm_provider: "ollama".into(),
                llm_model: "llama3".into(),
            },
            dna: DnaData {
                is_full: true,
                primary_stack: vec!["rust".into()],
                identity_summary: "Rust/TS systems developer".into(),
                ..Default::default()
            },
            computed: ComputedFields {
                llm_tier: "local".into(),
                gpu_tier: "workstation".into(),
                has_nvidia: true,
                os_family: "windows".into(),
                profile_completeness: 55.0,
                monthly_electricity_estimate: 48.0,
            },
        }
    }

    #[test]
    fn test_l2_basic_if() {
        let ctx = test_ctx();
        let input = "{? if profile.gpu.exists ?}You have a GPU.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("You have a GPU."));
        assert!(result.l2_evaluated > 0);
    }

    #[test]
    fn test_l2_if_else() {
        let ctx = test_ctx();
        let input =
            "{? if profile.gpu.has_nvidia ?}NVIDIA detected.{? else ?}No NVIDIA.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("NVIDIA detected."));
        assert!(!result.content.contains("No NVIDIA."));
    }

    #[test]
    fn test_l2_false_condition() {
        let mut ctx = test_ctx();
        ctx.settings.has_llm = false;
        let input = "{? if settings.has_llm ?}LLM ready.{? else ?}No LLM configured.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("No LLM configured."));
        assert!(!result.content.contains("LLM ready."));
    }

    #[test]
    fn test_l2_radar_has() {
        let ctx = test_ctx();
        let input = "{? if radar.has(\"rust\", \"adopt\") ?}Rust in Adopt ring.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("Rust in Adopt ring."));
    }

    #[test]
    fn test_l2_stack_contains() {
        let ctx = test_ctx();
        let input =
            "{? if stack.contains(\"python\") ?}Python user.{? else ?}No Python.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("No Python."));
    }

    #[test]
    fn test_l2_progress_completed() {
        let ctx = test_ctx();
        let input = "{? if progress.completed(\"S\") ?}Module S done!{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("Module S done!"));
    }

    #[test]
    fn test_l2_equality() {
        let ctx = test_ctx();
        let input = "{? if computed.os_family == \"windows\" ?}Windows user.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("Windows user."));
    }
}
