//! Intelligence Mesh — Layer 2 (Advisory Mesh): the `IntelligenceCore` trait.
//!
//! The deterministic pipeline stays sovereign (scoring::pipeline_v2).
//! Every LLM the pipeline asks for an opinion — judge, rerank, summarize,
//! translate — speaks to the rest of 4DA through this trait, never through
//! a concrete type. The trait is the seam that makes Phases 5, 6, and 7
//! possible:
//!
//!   - Phase 5 (Calibration): a `CalibratedCore` wrapper takes any
//!     `impl IntelligenceCore`, runs its outputs through a per-model
//!     Brier/ECE manifold, and returns normalized scores. Calibration
//!     wraps a trait object; it does not care which model is inside.
//!
//!   - Phase 6 (Shadow Arena): two `dyn IntelligenceCore` objects run
//!     the same `JudgeRequest` in parallel, their `Validated<...>`
//!     outputs feed a disagreement analyzer. No model-specific code.
//!
//!   - Phase 7 (Receipts UI): the UI renders `Validated<JudgeResponse>`
//!     — identity, prompt_version, calibration_id — as a "why this
//!     score?" drawer. The user sees which model judged each item.
//!
//! Today there is ONE concrete impl (`LlmJudgeCore`) wrapping the existing
//! `RelevanceJudge`. That is intentionally minimal: the trait exists so we
//! can introduce the second and third impls without re-plumbing the rerank
//! loop each time. The shape of `Validated<T>` already carries the provenance
//! fields Phase 3 wrote into the DB; the trait is the runtime counterpart
//! of that persisted record.
//!
//! See `docs/strategy/INTELLIGENCE-MESH.md` §2 Layer 2 for the full design.

use crate::error::{Result, ResultExt};
use crate::llm::{RelevanceJudge, RelevanceJudgment};
use crate::provenance::{ModelIdentity, PRE_MESH_CALIBRATION_ID};
use crate::settings::LLMProvider;
use async_trait::async_trait;

/// Request to the `judge` task: rate a batch of items for relevance
/// against the user's declared context.
///
/// The tuple `(id, title, content_snippet)` mirrors the existing
/// `RelevanceJudge::judge_batch` signature so the adapter is a thin
/// wrapper rather than a reshuffle.
#[derive(Debug, Clone)]
pub struct JudgeRequest {
    pub context_summary: String,
    pub items: Vec<(String, String, String)>,
}

/// Response from the `judge` task.
///
/// Token counts are carried on the response (not on `Validated<_>`) because
/// they describe the runtime cost of this specific call, not an invariant
/// of the model's identity.
#[derive(Debug, Clone)]
pub struct JudgeResponse {
    pub judgments: Vec<RelevanceJudgment>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// A value produced by an `IntelligenceCore`, stamped with the provenance
/// needed for receipts, audit, and calibration cohort queries.
///
/// `raw_response_hash` is reserved for Phase 6 (Shadow Arena) where two
/// peers' raw responses need stable IDs for comparison. Until then it's
/// `None`; callers should not rely on its presence.
///
/// The metadata fields (`identity`, `prompt_version`, `calibration_id`,
/// `raw_response_hash`) are populated today but not yet read by the
/// rerank loop — it captures identity/prompt_version once up front and
/// trusts them to be stable for the call. Phase 7 receipts UI will read
/// them per-response to render "Why this score?", at which point the
/// `allow(dead_code)` here becomes removable.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Metadata fields consumed by Phase 7 receipts UI.
pub struct Validated<T> {
    pub value: T,
    pub identity: ModelIdentity,
    pub prompt_version: String,
    pub calibration_id: Option<String>,
    pub raw_response_hash: Option<String>,
}

/// The Advisory Mesh trait. Every LLM-backed advisor in 4DA implements this.
///
/// Methods are scoped to the tasks actually consumed today. `summarize`,
/// `embed`, and `chat` will be added when their first call site migrates —
/// adding them now before a consumer exists is premature abstraction.
///
/// The trait is `Send + Sync` because reranking can happen on a background
/// task and the core may be shared across requests.
#[async_trait]
pub trait IntelligenceCore: Send + Sync {
    /// Stable identity of the underlying model. Used as the join key on
    /// provenance rows, calibration curves, and shadow-arena comparisons.
    fn identity(&self) -> ModelIdentity;

    /// Stable version identifier for the prompt template this core uses
    /// for the `judge` task. Bumping invalidates prior calibration cohorts.
    fn prompt_version(&self) -> &'static str;

    /// Current calibration curve ID for the `judge` task, or the pre-mesh
    /// sentinel until Phase 5 generates real curves.
    fn calibration_id(&self) -> Option<String> {
        Some(PRE_MESH_CALIBRATION_ID.to_string())
    }

    /// Judge a batch of items for relevance. The pipeline remains
    /// authoritative over the final rank — this is advisory input to the
    /// reconciler (Layer 3).
    async fn judge(&self, req: JudgeRequest) -> Result<Validated<JudgeResponse>>;

    /// Estimate cost in cents for the tokens reported by a prior `judge`
    /// response. Daily cost caps are enforced by the caller, not here.
    fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64;
}

/// Concrete core: wraps the existing `RelevanceJudge` so the rerank loop
/// can call the trait instead of the concrete type.
///
/// Identity is computed once at construction and cached — the provider
/// settings cannot change within a single rerank pass.
pub struct LlmJudgeCore {
    judge: RelevanceJudge,
    identity: ModelIdentity,
}

impl LlmJudgeCore {
    pub fn new(provider: LLMProvider) -> Self {
        let mut identity = ModelIdentity::new(&provider.provider, &provider.model);
        if let Some(base_url) = provider.base_url.as_deref().filter(|s| !s.is_empty()) {
            identity = identity.with_base_url(base_url);
        }
        Self {
            judge: RelevanceJudge::new(provider),
            identity,
        }
    }
}

#[async_trait]
impl IntelligenceCore for LlmJudgeCore {
    fn identity(&self) -> ModelIdentity {
        self.identity.clone()
    }

    fn prompt_version(&self) -> &'static str {
        crate::llm_judge::PROMPT_VERSION
    }

    async fn judge(&self, req: JudgeRequest) -> Result<Validated<JudgeResponse>> {
        let (judgments, input_tokens, output_tokens) = self
            .judge
            .judge_batch(&req.context_summary, req.items)
            .await
            .context("IntelligenceCore::judge underlying LLM call failed")?;

        Ok(Validated {
            value: JudgeResponse {
                judgments,
                input_tokens,
                output_tokens,
            },
            identity: self.identity.clone(),
            prompt_version: crate::llm_judge::PROMPT_VERSION.to_string(),
            calibration_id: Some(PRE_MESH_CALIBRATION_ID.to_string()),
            raw_response_hash: None, // Phase 6
        })
    }

    fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        self.judge.estimate_cost_cents(input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock core that returns caller-specified judgments without hitting any
    /// LLM. This is the tool Phase 6 will need to test shadow-arena disagreement
    /// analyzers, and Phase 5 will need to test calibration wrappers —
    /// landing it here means those phases build on tested infrastructure.
    struct MockJudgeCore {
        identity: ModelIdentity,
        canned: Vec<RelevanceJudgment>,
        input_tokens: u64,
        output_tokens: u64,
    }

    impl MockJudgeCore {
        fn new(provider: &str, model: &str, canned: Vec<RelevanceJudgment>) -> Self {
            Self {
                identity: ModelIdentity::new(provider, model),
                canned,
                input_tokens: 100,
                output_tokens: 50,
            }
        }
    }

    #[async_trait]
    impl IntelligenceCore for MockJudgeCore {
        fn identity(&self) -> ModelIdentity {
            self.identity.clone()
        }
        fn prompt_version(&self) -> &'static str {
            "mock-v0"
        }
        async fn judge(&self, _req: JudgeRequest) -> Result<Validated<JudgeResponse>> {
            Ok(Validated {
                value: JudgeResponse {
                    judgments: self.canned.clone(),
                    input_tokens: self.input_tokens,
                    output_tokens: self.output_tokens,
                },
                identity: self.identity.clone(),
                prompt_version: "mock-v0".to_string(),
                calibration_id: None,
                raw_response_hash: None,
            })
        }
        fn estimate_cost_cents(&self, _i: u64, _o: u64) -> u64 {
            0
        }
    }

    fn sample_judgment(id: &str, confidence: f32, relevant: bool) -> RelevanceJudgment {
        RelevanceJudgment {
            item_id: id.to_string(),
            relevant,
            confidence,
            reasoning: format!("mock reason for {id}"),
            key_connections: Vec::new(),
        }
    }

    #[tokio::test]
    async fn mock_core_round_trip_preserves_judgments() {
        let canned = vec![
            sample_judgment("a", 0.9, true),
            sample_judgment("b", 0.2, false),
        ];
        let core = MockJudgeCore::new("mock-provider", "mock-model", canned.clone());

        let req = JudgeRequest {
            context_summary: "rust async test".to_string(),
            items: vec![
                (
                    "a".to_string(),
                    "title a".to_string(),
                    "content a".to_string(),
                ),
                (
                    "b".to_string(),
                    "title b".to_string(),
                    "content b".to_string(),
                ),
            ],
        };
        let validated = core.judge(req).await.expect("mock judge should succeed");

        assert_eq!(validated.value.judgments.len(), 2);
        assert_eq!(validated.value.judgments[0].item_id, "a");
        assert!(validated.value.judgments[0].relevant);
        assert!((validated.value.judgments[0].confidence - 0.9).abs() < 1e-5);
        assert_eq!(validated.value.judgments[1].item_id, "b");
        assert!(!validated.value.judgments[1].relevant);
    }

    #[tokio::test]
    async fn mock_core_stamps_identity_and_prompt_version() {
        let core = MockJudgeCore::new("ollama", "llama3.2", vec![]);
        let req = JudgeRequest {
            context_summary: "x".to_string(),
            items: vec![],
        };
        let validated = core.judge(req).await.unwrap();

        assert_eq!(validated.identity.provider, "ollama");
        assert_eq!(validated.identity.model, "llama3.2");
        assert_eq!(validated.prompt_version, "mock-v0");
        assert!(validated.calibration_id.is_none());
        assert!(validated.raw_response_hash.is_none());
    }

    #[tokio::test]
    async fn trait_object_allows_runtime_model_swap() {
        // This is the load-bearing test: a consumer holds `Arc<dyn
        // IntelligenceCore>` and can be handed ANY implementation.
        // Phase 6's shadow arena depends on this being true.
        use std::sync::Arc;

        let core_a: Arc<dyn IntelligenceCore> = Arc::new(MockJudgeCore::new(
            "ollama",
            "llama3.2",
            vec![sample_judgment("x", 0.8, true)],
        ));
        let core_b: Arc<dyn IntelligenceCore> = Arc::new(MockJudgeCore::new(
            "anthropic",
            "claude-sonnet-4-6",
            vec![sample_judgment("x", 0.3, false)],
        ));

        let req = JudgeRequest {
            context_summary: "shadow arena sanity".to_string(),
            items: vec![("x".to_string(), "t".to_string(), "c".to_string())],
        };

        let v_a = core_a.judge(req.clone()).await.unwrap();
        let v_b = core_b.judge(req).await.unwrap();

        // Identities distinct — this is what the shadow arena will join on.
        assert_ne!(v_a.identity.hash(), v_b.identity.hash());
        // Different judgments — peers disagreed, which is the whole point.
        assert_ne!(
            v_a.value.judgments[0].confidence,
            v_b.value.judgments[0].confidence
        );
    }

    #[test]
    fn validated_exposes_provenance_fields_expected_by_receipts_ui() {
        // Phase 7 reads these four fields to render "Why this score?".
        // Guard: if any is renamed, a huge surface renames silently —
        // tests pin the field names.
        let v = Validated {
            value: JudgeResponse {
                judgments: vec![],
                input_tokens: 0,
                output_tokens: 0,
            },
            identity: ModelIdentity::new("p", "m"),
            prompt_version: "judge-v1-2026-04-15".to_string(),
            calibration_id: Some("pre-mesh".to_string()),
            raw_response_hash: None,
        };
        let _ = (
            &v.value,
            &v.identity,
            &v.prompt_version,
            &v.calibration_id,
            &v.raw_response_hash,
        );
    }
}
