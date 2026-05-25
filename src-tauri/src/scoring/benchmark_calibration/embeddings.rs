// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Embedding generation for benchmark calibration scenarios.

use std::collections::HashMap;
use tracing::info;

use super::types::pad_and_normalize;
use super::Scenario;

/// Embed all scenario texts and profile topic names using fastembed (snowflake-arctic-embed-m).
///
/// Returns (item_embeddings, topic_embeddings) where:
/// - item_embeddings: scenario_id -> embedding vector
/// - topic_embeddings: topic_name -> embedding vector
pub(super) fn generate_all_embeddings(
    scenarios: &[Scenario],
) -> crate::error::Result<(HashMap<String, Vec<f32>>, HashMap<String, Vec<f32>>)> {
    // Collect unique item texts: "{title}. {content}"
    let mut item_texts: Vec<String> = Vec::with_capacity(scenarios.len());
    let mut item_ids: Vec<String> = Vec::with_capacity(scenarios.len());
    for s in scenarios {
        item_texts.push(format!("{}. {}", s.item.title, s.item.content));
        item_ids.push(s.id.clone());
    }

    info!(
        "Embedding {} scenario texts via fastembed...",
        item_texts.len()
    );
    let item_vectors: Vec<Vec<f32>> = crate::fastembed_sync(&item_texts)?
        .into_iter()
        .map(pad_and_normalize)
        .collect();

    let mut item_embeddings = HashMap::with_capacity(scenarios.len());
    for (id, vec) in item_ids.into_iter().zip(item_vectors) {
        item_embeddings.insert(id, vec);
    }

    // Collect ALL unique topic names across all profiles — interest names, ACE
    // active_topics, and detected_tech. The semantic boost function looks up by
    // lowercase key so we embed every variant and store under lowercase.
    let all_profile_topics: &[&[&str]] = &[
        // rust_developer: interests + ACE topics + detected tech + deps
        &[
            "Rust",
            "systems programming",
            "Tauri",
            "rust",
            "tauri",
            "sqlite",
            "tokio",
            "serde",
            "hyper",
        ],
        // fullstack_js: interests + ACE topics + detected tech
        &[
            "TypeScript",
            "React",
            "Node.js",
            "typescript",
            "react",
            "nodejs",
            "next",
            "express",
        ],
        // python_data_scientist: interests + ACE topics + detected tech
        &[
            "Machine Learning",
            "Python",
            "Data Science",
            "python",
            "pytorch",
            "ml",
            "torch",
            "transformers",
        ],
    ];

    let mut unique_topics: Vec<String> = Vec::new();
    for group in all_profile_topics {
        for &t in *group {
            let ts = t.to_string();
            if !unique_topics.contains(&ts) {
                unique_topics.push(ts);
            }
        }
    }

    info!(
        "Embedding {} topic names via fastembed...",
        unique_topics.len()
    );
    let topic_vectors: Vec<Vec<f32>> = crate::fastembed_sync(&unique_topics)?
        .into_iter()
        .map(pad_and_normalize)
        .collect();

    let mut topic_embeddings = HashMap::with_capacity(unique_topics.len() * 2);
    for (name, vec) in unique_topics.into_iter().zip(topic_vectors) {
        let lower = name.to_lowercase();
        if lower != name {
            topic_embeddings.insert(lower, vec.clone());
        }
        topic_embeddings.insert(name, vec);
    }

    Ok((item_embeddings, topic_embeddings))
}
