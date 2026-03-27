//! Relevance Judge - LLM-powered relevance scoring
//!
//! Extracted from llm.rs to keep files under 1000-line limit.

use crate::error::{Result, ResultExt};
use crate::llm::{LLMClient, Message, RelevanceJudgment};
use crate::settings::LLMProvider;
use tracing::debug;

/// The relevance judge uses an LLM to determine true relevance
pub struct RelevanceJudge {
    client: LLMClient,
}

impl RelevanceJudge {
    pub fn new(provider: LLMProvider) -> Self {
        Self {
            client: LLMClient::new(provider),
        }
    }

    /// Judge relevance of multiple items against user context.
    /// Uses a 1-5 scoring rubric and sends real article content.
    pub async fn judge_batch(
        &self,
        context_summary: &str,
        items: Vec<(String, String, String)>, // (id, title, content_snippet)
    ) -> Result<(Vec<RelevanceJudgment>, u64, u64)> {
        if items.is_empty() {
            return Ok((vec![], 0, 0));
        }

        let system_prompt = r#"You are a relevance judge for a developer intelligence tool. Rate each article's genuine usefulness to THIS specific developer — not whether it mentions their tech, but whether they'd actually benefit from reading it.

## Scoring Rubric (be strict — most items should score 1-2)
5 = MUST-READ: Security alert for their dependency, breaking change they must act on, directly solves a problem they're currently working on
4 = HIGH VALUE: Advanced technique for their core tech, important release for a dependency they use daily, architectural pattern directly applicable to their project
3 = WORTH KNOWING: Relevant ecosystem news, useful tool that fits their exact stack, technical deep-dive in their specific domain
2 = MARGINAL: Mentions their tech but isn't actionable, generic advice, tangentially related
1 = NOISE: Wrong domain, competing tech focused, beginner content for tech they know well, self-promotional "I built X", career/hiring, academic papers outside their domain

## Critical Rules
- "Mentions Rust" does NOT mean relevant. A Supabase SDK in Rust is irrelevant if they don't use Supabase. Judge the TOPIC, not the language.
- "I built X" and "Show HN" posts are almost always score 1-2 unless X is directly applicable to their specific project.
- Content about competing/alternative technologies they've chosen against = score 1.
- Tutorials for technologies they already use expertly = score 1-2.
- Score >= 3 should mean: "This developer would thank me for showing them this."

Output JSON array (one per article):
[{"id": N, "score": N, "reason": "one sentence"}]"#;

        let items_text = items
            .iter()
            .enumerate()
            .map(|(i, (id, title, content))| {
                let snippet = if content.len() > 2000 {
                    let truncated: String = content.chars().take(2000).collect();
                    truncated
                } else {
                    content.clone()
                };
                format!("{}. [ID: {}] \"{}\"\n   {}", i + 1, id, title, snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let user_message = format!(
            "## Developer Context\n{context_summary}\n\n## Articles to Judge\n{items_text}\n\nRate each article 1-5 per the rubric. Output JSON array:"
        );

        let response = self
            .client
            .complete(
                system_prompt,
                vec![Message {
                    role: "user".to_string(),
                    content: user_message,
                }],
            )
            .await
            .context("LLM relevance judging failed")?;

        // Parse the score-based JSON response
        let judgments = self
            .parse_judgments(&response.content, &items)
            .context("Failed to parse relevance judgments")?;

        Ok((judgments, response.input_tokens, response.output_tokens))
    }

    fn parse_judgments(
        &self,
        response: &str,
        items: &[(String, String, String)],
    ) -> Result<Vec<RelevanceJudgment>> {
        // Try to extract JSON from the response
        let json_str = if let Some(start) = response.find('[') {
            if let Some(end) = response.rfind(']') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str).map_err(|e| {
            format!("Failed to parse LLM response as JSON: {e}. Response: {response}")
        })?;

        let mut judgments = Vec::new();

        for value in parsed {
            // Handle ID as string or number
            let id = value["id"]
                .as_str()
                .map(std::string::ToString::to_string)
                .or_else(|| value["id"].as_u64().map(|n| n.to_string()))
                .or_else(|| value["id"].as_i64().map(|n| n.to_string()))
                .unwrap_or_default();

            // New: parse score (1-5) instead of relevant boolean
            let score = value["score"]
                .as_f64()
                .or_else(|| value["score"].as_i64().map(|n| n as f64))
                .or_else(|| value["score"].as_str().and_then(|s| s.parse::<f64>().ok()))
                .unwrap_or(1.0)
                .clamp(1.0, 5.0) as f32;

            // Map score to relevant/confidence
            let relevant = score >= 3.0;
            let confidence = score / 5.0;

            // Support both "reason" and "reasoning" keys
            let reasoning = value["reason"]
                .as_str()
                .or_else(|| value["reasoning"].as_str())
                .unwrap_or("")
                .to_string();

            // Legacy support: if "relevant" field exists and "score" doesn't, use old format
            let (relevant, confidence) = if value.get("score").is_none() {
                if let Some(rel) = value["relevant"].as_bool() {
                    let conf = value["confidence"]
                        .as_f64()
                        .unwrap_or(if rel { 0.6 } else { 0.2 })
                        as f32;
                    (rel, conf)
                } else {
                    (relevant, confidence)
                }
            } else {
                (relevant, confidence)
            };

            let key_connections: Vec<String> = value["key_connections"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            // Debug log first few judgments
            if judgments.len() < 3 {
                debug!(
                    target: "4da::llm",
                    id = %id,
                    score = score,
                    relevant = %relevant,
                    confidence = confidence,
                    reason = %&reasoning[..reasoning.len().min(50)],
                    "Parsed judgment"
                );
            }

            judgments.push(RelevanceJudgment {
                item_id: id,
                relevant,
                confidence,
                reasoning,
                key_connections,
            });
        }

        // Ensure we have judgments for all items (in case LLM missed some)
        for (id, _, _) in items {
            if !judgments.iter().any(|j| j.item_id == *id) {
                judgments.push(RelevanceJudgment {
                    item_id: id.clone(),
                    relevant: false,
                    confidence: 0.0,
                    reasoning: "No judgment provided by LLM".to_string(),
                    key_connections: vec![],
                });
            }
        }

        Ok(judgments)
    }

    /// Estimate cost for judging items
    pub fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        self.client.estimate_cost_cents(input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // parse_judgments — malformed / invalid API responses

    #[test]
    fn test_parse_judgments_valid_response() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title 1".to_string(),
            "Content 1".to_string(),
        )];

        let response = r#"[{"id": "item1", "score": 4, "reason": "Highly relevant"}]"#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        assert_eq!(judgments.len(), 1);
        assert_eq!(judgments[0].item_id, "item1");
        assert!(judgments[0].relevant); // score 4 >= 3 -> relevant
        assert!((judgments[0].confidence - 0.8).abs() < f32::EPSILON); // 4/5
    }

    #[test]
    fn test_parse_judgments_invalid_json() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title 1".to_string(),
            "Content 1".to_string(),
        )];

        let response = "This is not valid JSON at all";
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse LLM response as JSON"));
    }

    #[test]
    fn test_parse_judgments_empty_array() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title 1".to_string(),
            "Content 1".to_string(),
        )];

        let response = "[]";
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        // Missing item should get a default "no judgment" entry
        assert_eq!(judgments.len(), 1);
        assert_eq!(judgments[0].item_id, "item1");
        assert!(!judgments[0].relevant);
        assert!((judgments[0].confidence - 0.0).abs() < f32::EPSILON);
        assert_eq!(judgments[0].reasoning, "No judgment provided by LLM");
    }

    #[test]
    fn test_parse_judgments_json_with_surrounding_text() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title".to_string(),
            "Content".to_string(),
        )];

        // LLM sometimes wraps response in text before/after the JSON array
        let response = r#"Here are the judgments:
[{"id": "item1", "score": 2, "reason": "Marginal relevance"}]
That's it."#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        assert_eq!(judgments[0].item_id, "item1");
        assert!(!judgments[0].relevant); // score 2 < 3 -> not relevant
    }

    #[test]
    fn test_parse_judgments_missing_fields_use_defaults() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title".to_string(),
            "Content".to_string(),
        )];

        // Response with missing score, reason, etc.
        let response = r#"[{"id": "item1"}]"#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        assert_eq!(judgments[0].item_id, "item1");
        // Default score is 1.0, so not relevant, confidence = 1/5 = 0.2
        assert!(!judgments[0].relevant);
        assert!((judgments[0].confidence - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_judgments_score_clamped_out_of_range() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![
            (
                "item1".to_string(),
                "Title".to_string(),
                "Content".to_string(),
            ),
            (
                "item2".to_string(),
                "Title 2".to_string(),
                "Content 2".to_string(),
            ),
        ];

        // Score 10 should be clamped to 5, score -3 should be clamped to 1
        let response = r#"[
            {"id": "item1", "score": 10, "reason": "Over max"},
            {"id": "item2", "score": -3, "reason": "Under min"}
        ]"#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        // Score 10 clamped to 5 -> confidence = 5/5 = 1.0
        assert!(judgments[0].relevant);
        assert!((judgments[0].confidence - 1.0).abs() < f32::EPSILON);
        // Score -3 clamped to 1 -> confidence = 1/5 = 0.2
        assert!(!judgments[1].relevant);
        assert!((judgments[1].confidence - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_judgments_legacy_boolean_format() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![(
            "item1".to_string(),
            "Title".to_string(),
            "Content".to_string(),
        )];

        // Legacy format: "relevant" boolean instead of "score"
        let response = r#"[{"id": "item1", "relevant": true, "confidence": 0.85, "reasoning": "Very useful"}]"#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        assert!(judgments[0].relevant);
        assert!((judgments[0].confidence - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_judgments_numeric_id() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);
        let items = vec![("42".to_string(), "Title".to_string(), "Content".to_string())];

        // LLM returns id as number instead of string
        let response = r#"[{"id": 42, "score": 3, "reason": "Worth knowing"}]"#;
        let result = judge.parse_judgments(response, &items);
        assert!(result.is_ok());
        let judgments = result.unwrap();
        assert_eq!(judgments[0].item_id, "42");
    }

    // judge_batch — empty items returns immediately

    #[tokio::test]
    async fn test_judge_batch_empty_items() {
        let provider = LLMProvider::default();
        let judge = RelevanceJudge::new(provider);

        let result = judge.judge_batch("test context", vec![]).await;
        assert!(result.is_ok());
        let (judgments, input_tokens, output_tokens) = result.unwrap();
        assert!(judgments.is_empty());
        assert_eq!(input_tokens, 0);
        assert_eq!(output_tokens, 0);
    }
}
