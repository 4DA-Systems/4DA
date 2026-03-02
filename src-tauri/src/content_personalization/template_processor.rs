//! L1/L2 Template Processor — synchronous text transformation engine.
//!
//! Processes personalization template syntax in markdown files:
//! - L1 `{= path | fallback("default") =}` — value interpolation
//! - L2 `{? if condition ?}...{? elif condition ?}...{? else ?}...{? endif ?}` — conditionals
//! - L3 `{@ insight block_id engine=N @}` — injection markers (passed through for frontend)
//!
//! Operates ONLY on text outside code blocks (triple-backtick fences).

use super::context::PersonalizationContext;
use super::template_resolver::resolve_path;

use tracing::debug;

// ============================================================================
// Public API
// ============================================================================

/// Result of template processing — processed content + depth stats.
pub struct ProcessResult {
    pub content: String,
    pub l1_resolved: u32,
    pub l1_fallbacks: u32,
    pub l2_evaluated: u32,
    pub injection_markers: Vec<String>,
}

/// Process a raw markdown template with L1 interpolation and L2 conditionals.
/// Code blocks (triple-backtick fences) are passed through untouched.
pub fn process_template(raw: &str, ctx: &PersonalizationContext) -> ProcessResult {
    let mut result = ProcessResult {
        content: String::with_capacity(raw.len()),
        l1_resolved: 0,
        l1_fallbacks: 0,
        l2_evaluated: 0,
        injection_markers: Vec::new(),
    };

    // Split into code-block-safe segments
    let segments = split_code_blocks(raw);

    for segment in segments {
        match segment {
            Segment::Code(code) => {
                // Pass code blocks through untouched
                result.content.push_str(&code);
            }
            Segment::Text(text) => {
                // Process L2 conditionals first (they may contain L1 tokens)
                let (after_l2, l2_count) = process_l2(&text, ctx);
                result.l2_evaluated += l2_count;

                // Process L1 interpolation
                let (after_l1, resolved, fallbacks) = process_l1(&after_l2, ctx);
                result.l1_resolved += resolved;
                result.l1_fallbacks += fallbacks;

                // Collect L3 injection markers
                collect_injection_markers(&after_l1, &mut result.injection_markers);

                result.content.push_str(&after_l1);
            }
        }
    }

    result
}

// ============================================================================
// Code Block Splitting
// ============================================================================

enum Segment {
    Text(String),
    Code(String),
}

/// Split markdown into alternating text and code block segments.
/// Code blocks are identified by lines starting with ``` (triple backtick).
fn split_code_blocks(input: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_code_block = false;

    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code_block {
                // End of code block — include closing fence
                current.push_str(line);
                current.push('\n');
                segments.push(Segment::Code(current));
                current = String::new();
                in_code_block = false;
            } else {
                // Start of code block — flush text segment first
                if !current.is_empty() {
                    segments.push(Segment::Text(current));
                    current = String::new();
                }
                current.push_str(line);
                current.push('\n');
                in_code_block = true;
            }
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    // Flush remaining content
    if !current.is_empty() {
        if in_code_block {
            segments.push(Segment::Code(current));
        } else {
            segments.push(Segment::Text(current));
        }
    }

    segments
}

// ============================================================================
// L1: Value Interpolation
// ============================================================================

/// Process `{= path | fallback("default") =}` tokens.
/// Returns (processed_text, resolved_count, fallback_count).
fn process_l1(input: &str, ctx: &PersonalizationContext) -> (String, u32, u32) {
    let mut output = String::with_capacity(input.len());
    let mut resolved = 0u32;
    let mut fallbacks = 0u32;
    let mut remaining = input;

    while let Some(start) = remaining.find("{=") {
        // Output text before the token
        output.push_str(&remaining[..start]);

        if let Some(end) = remaining[start..].find("=}") {
            let token_end = start + end + 2;
            let token_body = &remaining[start + 2..start + end].trim();

            // Parse path and optional fallback
            let (path, fallback) = parse_l1_token(token_body);

            // Resolve the path against the context
            match resolve_path(path, ctx) {
                Some(value) => {
                    output.push_str(&value);
                    resolved += 1;
                }
                None => {
                    output.push_str(fallback.unwrap_or(path));
                    fallbacks += 1;
                }
            }

            remaining = &remaining[token_end..];
        } else {
            // Malformed token — pass through
            output.push_str("{=");
            remaining = &remaining[start + 2..];
        }
    }

    output.push_str(remaining);
    (output, resolved, fallbacks)
}

/// Parse an L1 token body like `profile.cpu.model | fallback("your CPU")`.
/// Returns (path, optional_fallback).
fn parse_l1_token(body: &str) -> (&str, Option<&str>) {
    if let Some(pipe_pos) = body.find('|') {
        let path = body[..pipe_pos].trim();
        let rest = body[pipe_pos + 1..].trim();

        // Parse fallback("...") syntax
        if let Some(inner) = extract_fallback_value(rest) {
            return (path, Some(inner));
        }

        (path, None)
    } else {
        (body.trim(), None)
    }
}

/// Extract value from `fallback("...")` or `fallback('...')`.
fn extract_fallback_value(s: &str) -> Option<&str> {
    let s = s.trim();
    let inner = s.strip_prefix("fallback(")?;
    let inner = inner.strip_suffix(')')?;
    let inner = inner.trim();

    // Remove surrounding quotes
    if (inner.starts_with('"') && inner.ends_with('"'))
        || (inner.starts_with('\'') && inner.ends_with('\''))
    {
        Some(&inner[1..inner.len() - 1])
    } else {
        Some(inner)
    }
}

// ============================================================================
// L2: Conditional Blocks
// ============================================================================

/// Process `{? if/elif/else/endif ?}` conditional blocks.
/// Returns (processed_text, conditions_evaluated_count).
fn process_l2(input: &str, ctx: &PersonalizationContext) -> (String, u32) {
    let mut output = String::with_capacity(input.len());
    let mut evaluated = 0u32;
    let mut remaining = input;

    while let Some(if_start) = remaining.find("{? if ") {
        // Output text before the conditional
        output.push_str(&remaining[..if_start]);

        // Find the matching endif
        match find_conditional_block(&remaining[if_start..]) {
            Some((block, after_block_len)) => {
                let (result, count) = evaluate_conditional_block(&block, ctx);
                output.push_str(&result);
                evaluated += count;
                remaining = &remaining[if_start + after_block_len..];
            }
            None => {
                // Malformed block — pass through and move past the tag
                debug!(target: "4da::personalize", "Malformed L2 conditional block");
                output.push_str("{? if ");
                remaining = &remaining[if_start + 6..];
            }
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
    resolve_path(cond, ctx)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

// ============================================================================
// L3: Injection Marker Collection
// ============================================================================

/// Collect `{@ insight block_id ... @}` markers from processed text.
/// These are left in the output for the frontend to hydrate with React components.
fn collect_injection_markers(input: &str, markers: &mut Vec<String>) {
    let mut remaining = input;
    while let Some(start) = remaining.find("{@") {
        if let Some(end) = remaining[start..].find("@}") {
            let body = remaining[start + 2..start + end].trim();
            // Extract block_id (first word after "insight")
            if let Some(rest) = body.strip_prefix("insight") {
                let block_id = rest.split_whitespace().next().unwrap_or("unknown");
                markers.push(block_id.to_string());
            }
            remaining = &remaining[start + end + 2..];
        } else {
            break;
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_personalization::context::*;

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
    fn test_l1_basic_interpolation() {
        let ctx = test_ctx();
        let input = "Your CPU is {= profile.cpu.model =}.";
        let result = process_template(input, &ctx);
        assert_eq!(result.content, "Your CPU is AMD Ryzen 9 7950X.\n");
        assert_eq!(result.l1_resolved, 1);
        assert_eq!(result.l1_fallbacks, 0);
    }

    #[test]
    fn test_l1_fallback() {
        let ctx = test_ctx();
        let input = "Your tablet: {= profile.tablet.model | fallback(\"not detected\") =}.";
        let result = process_template(input, &ctx);
        assert_eq!(result.content, "Your tablet: not detected.\n");
        assert_eq!(result.l1_fallbacks, 1);
    }

    #[test]
    fn test_l1_multiple_tokens() {
        let ctx = test_ctx();
        let input = "CPU: {= profile.cpu.model =}, GPU: {= profile.gpu.name =}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("AMD Ryzen 9 7950X"));
        assert!(result.content.contains("NVIDIA RTX 4090"));
        assert_eq!(result.l1_resolved, 2);
    }

    #[test]
    fn test_l1_code_block_preserved() {
        let ctx = test_ctx();
        let input = "Normal: {= profile.cpu.model =}\n```bash\necho {= this.should.not.resolve =}\n```\nAfter.";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("AMD Ryzen 9 7950X"));
        assert!(result.content.contains("{= this.should.not.resolve =}"));
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

    #[test]
    fn test_l3_injection_markers_collected() {
        let ctx = test_ctx();
        let input = "Before\n{@ insight hardware_benchmark engine=1 @}\nAfter";
        let result = process_template(input, &ctx);
        assert_eq!(result.injection_markers, vec!["hardware_benchmark"]);
        // Markers are preserved in content for frontend
        assert!(result
            .content
            .contains("{@ insight hardware_benchmark engine=1 @}"));
    }

    #[test]
    fn test_combined_l1_l2() {
        let ctx = test_ctx();
        let input = "{? if profile.gpu.exists ?}Your GPU: {= profile.gpu.name =}{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("Your GPU: NVIDIA RTX 4090"));
    }

    #[test]
    fn test_empty_context_graceful() {
        let ctx = PersonalizationContext {
            profile: ProfileData::default(),
            stack: StackData::default(),
            radar: RadarData::default(),
            regional: RegionalData::default(),
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData::default(),
            dna: DnaData::default(),
            computed: ComputedFields::default(),
        };
        let input = "CPU: {= profile.cpu.model | fallback(\"unknown\") =}. {? if profile.gpu.exists ?}GPU found.{? else ?}No GPU.{? endif ?}";
        let result = process_template(input, &ctx);
        assert!(result.content.contains("CPU: unknown."));
        assert!(result.content.contains("No GPU."));
    }
}
