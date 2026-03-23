//! ACE embedding-related commands: similar topics search and embedding status.

use crate::error::Result;
use crate::get_ace_engine;

/// Find similar topics using embeddings
#[tauri::command]
pub async fn ace_find_similar_topics(
    query: String,
    top_k: Option<usize>,
) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let top_k = top_k.unwrap_or(5);
    let results = ace.find_similar_topics(&query, top_k)?;

    Ok(serde_json::json!({
        "query": query,
        "results": results.iter().map(|(topic, score)| {
            serde_json::json!({
                "topic": topic,
                "similarity": score
            })
        }).collect::<Vec<_>>()
    }))
}

/// Check if embedding service is operational
#[tauri::command]
pub async fn ace_embedding_status() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let operational = ace.is_embedding_operational();

    Ok(serde_json::json!({
        "operational": operational
    }))
}
