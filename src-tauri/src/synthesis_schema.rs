// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Structured output schema for morning briefing synthesis.
//!
//! When JSON mode is enabled, the LLM returns this schema instead of
//! free-text prose. The pipeline validates the output and falls back
//! to the legacy free-text path if parsing fails.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SynthesisOutput {
    pub clusters: Vec<SynthesisCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SynthesisCluster {
    pub insight: String,
    pub evidence_ids: Vec<usize>,
    pub action: String,
    pub confidence: f64,
}

impl SynthesisOutput {
    pub fn validate(&self) -> Result<(), String> {
        if self.clusters.is_empty() {
            return Err("No clusters in synthesis output".into());
        }
        if self.clusters.len() > 3 {
            return Err(format!(
                "Too many clusters: {} (max 3)",
                self.clusters.len()
            ));
        }
        for (i, cluster) in self.clusters.iter().enumerate() {
            if cluster.insight.is_empty() {
                return Err(format!("Cluster {i}: empty insight"));
            }
            if cluster.action.is_empty() {
                return Err(format!("Cluster {i}: empty action"));
            }
            if !(0.0..=1.0).contains(&cluster.confidence) {
                return Err(format!(
                    "Cluster {i}: confidence {} out of [0.0, 1.0]",
                    cluster.confidence
                ));
            }
            if cluster.evidence_ids.is_empty() {
                return Err(format!("Cluster {i}: no evidence_ids cited"));
            }
        }
        Ok(())
    }

    pub fn validate_evidence_ids(&self, max_signal_index: usize) -> Vec<String> {
        let mut warnings = Vec::new();
        for (i, cluster) in self.clusters.iter().enumerate() {
            for &id in &cluster.evidence_ids {
                if id == 0 || id > max_signal_index {
                    warnings.push(format!(
                        "Cluster {i}: evidence_id {id} out of range (1..={max_signal_index})"
                    ));
                }
            }
        }
        warnings
    }

    pub fn to_prose(&self) -> String {
        self.clusters
            .iter()
            .map(|c| format!("{}\n\n{}", c.insight, c.action))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub(crate) const SYNTHESIS_JSON_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "clusters": {
      "type": "array",
      "minItems": 1,
      "maxItems": 3,
      "items": {
        "type": "object",
        "properties": {
          "insight": {
            "type": "string",
            "description": "1-3 sentence synthesis connecting signals. Must add value beyond titles."
          },
          "evidence_ids": {
            "type": "array",
            "items": { "type": "integer", "minimum": 1 },
            "minItems": 1,
            "description": "1-indexed signal numbers this insight is grounded in."
          },
          "action": {
            "type": "string",
            "description": "Concrete action for a senior developer. No filler."
          },
          "confidence": {
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0,
            "description": "How confident you are in this cluster (0.0-1.0)."
          }
        },
        "required": ["insight", "evidence_ids", "action", "confidence"],
        "additionalProperties": false
      }
    }
  },
  "required": ["clusters"],
  "additionalProperties": false
}"#;

/// GBNF grammar for llama-server grammar-constrained decoding.
/// Matches the JSON schema above — forces the model to output valid JSON.
pub(crate) const SYNTHESIS_GBNF_GRAMMAR: &str = r#"root ::= "{" ws "\"clusters\"" ws ":" ws clusters ws "}"
clusters ::= "[" ws cluster (ws "," ws cluster)* ws "]"
cluster ::= "{" ws insight ws "," ws evidence ws "," ws action ws "," ws confidence ws "}"
insight ::= "\"insight\"" ws ":" ws string
evidence ::= "\"evidence_ids\"" ws ":" ws "[" ws number (ws "," ws number)* ws "]"
action ::= "\"action\"" ws ":" ws string
confidence ::= "\"confidence\"" ws ":" ws decimal
string ::= "\"" chars "\""
chars ::= char*
char ::= [^"\\] | "\\" escape
escape ::= ["\\nrt/]
number ::= [1-9] [0-9]*
decimal ::= "0" "." [0-9]+ | "1" "." "0"+  | "1" | "0"
ws ::= [ \t\n\r]*
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_output_passes_validation() {
        let output = SynthesisOutput {
            clusters: vec![SynthesisCluster {
                insight: "Tokio 1.38.6 patches a confirmed RCE".into(),
                evidence_ids: vec![1, 3],
                action: "Upgrade tokio to 1.38.6 today".into(),
                confidence: 0.95,
            }],
        };
        assert!(output.validate().is_ok());
    }

    #[test]
    fn empty_clusters_fails() {
        let output = SynthesisOutput { clusters: vec![] };
        assert!(output.validate().is_err());
    }

    #[test]
    fn too_many_clusters_fails() {
        let clusters: Vec<SynthesisCluster> = (0..4)
            .map(|i| SynthesisCluster {
                insight: format!("Insight {i}"),
                evidence_ids: vec![1],
                action: format!("Action {i}"),
                confidence: 0.5,
            })
            .collect();
        let output = SynthesisOutput { clusters };
        assert!(output.validate().is_err());
    }

    #[test]
    fn confidence_out_of_range_fails() {
        let output = SynthesisOutput {
            clusters: vec![SynthesisCluster {
                insight: "Test".into(),
                evidence_ids: vec![1],
                action: "Test".into(),
                confidence: 1.5,
            }],
        };
        assert!(output.validate().is_err());
    }

    #[test]
    fn evidence_id_validation_catches_out_of_range() {
        let output = SynthesisOutput {
            clusters: vec![SynthesisCluster {
                insight: "Test".into(),
                evidence_ids: vec![1, 5, 99],
                action: "Test".into(),
                confidence: 0.8,
            }],
        };
        let warnings = output.validate_evidence_ids(10);
        assert_eq!(warnings.len(), 1); // 99 is out of range, 1 and 5 are fine
    }

    #[test]
    fn to_prose_formats_correctly() {
        let output = SynthesisOutput {
            clusters: vec![
                SynthesisCluster {
                    insight: "First insight.".into(),
                    evidence_ids: vec![1],
                    action: "First action.".into(),
                    confidence: 0.9,
                },
                SynthesisCluster {
                    insight: "Second insight.".into(),
                    evidence_ids: vec![2, 3],
                    action: "Second action.".into(),
                    confidence: 0.7,
                },
            ],
        };
        let prose = output.to_prose();
        assert!(prose.contains("First insight."));
        assert!(prose.contains("Second action."));
    }

    #[test]
    fn schema_is_valid_json() {
        let parsed: serde_json::Value = serde_json::from_str(SYNTHESIS_JSON_SCHEMA).unwrap();
        assert_eq!(parsed["type"], "object");
    }

    #[test]
    fn round_trip_serialization() {
        let output = SynthesisOutput {
            clusters: vec![SynthesisCluster {
                insight: "Test insight".into(),
                evidence_ids: vec![1, 2],
                action: "Test action".into(),
                confidence: 0.85,
            }],
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: SynthesisOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.clusters.len(), 1);
        assert_eq!(parsed.clusters[0].evidence_ids, vec![1, 2]);
    }
}
