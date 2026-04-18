// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! L1/L2 Template Processor — synchronous text transformation engine.
//!
//! Processes personalization template syntax in markdown files:
//! - L1 `{= path | fallback("default") =}` — value interpolation
//! - L2 `{? if condition ?}...{? elif condition ?}...{? else ?}...{? endif ?}` — conditionals
//! - L3 `{@ insight block_id engine=N @}` — injection markers (passed through for frontend)
//!
//! Operates ONLY on text outside code blocks (triple-backtick fences).

use super::context::PersonalizationContext;
use super::template_conditionals::process_l2;
use super::template_resolver::resolve_path;

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
            if let Some(value) = resolve_path(path, ctx) {
                output.push_str(&value);
                resolved += 1;
            } else {
                output.push_str(fallback.unwrap_or(path));
                fallbacks += 1;
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
