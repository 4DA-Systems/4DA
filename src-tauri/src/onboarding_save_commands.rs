// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! "Enter 4DA" — save the Quick Setup context in ONE command.
//!
//! Quick Setup used to fan out one IPC call per interest (`add_interest`), one
//! per detected technology (`add_tech_stack`) and one for the stacks: 21
//! concurrent calls on the fresh-profile E2E (2026-10-09). Each `add_interest`
//! took the settings lock and hydrated the keychain for its own embedding
//! request, upserted through the shared context engine and then invalidated
//! it, so the next caller re-opened a connection and re-ran the engine's
//! migration; every `add_tech_stack` ran its SQLite writes on an async runtime
//! worker. Seven interests were saved at 02:04:56Z; the eighth only sent its
//! embedding request at 02:05:25.97Z, 21 ms after the background scoring pass
//! reached its first `.await` (Ollama's server log has that lone request),
//! and "Enter 4DA" sat frozen in between.
//!
//! This command embeds every new interest in one request, does all the writes
//! through one context-engine handle on the blocking pool, and invalidates the
//! engine once. Each stage's time is logged, and a slow save says which stage
//! was slow.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::context_engine::InterestSource;
use crate::error::Result;

/// Longest topic / technology name accepted (same bound as `add_interest`).
const MAX_ITEM_LEN: usize = 200;
/// More than this many items in one save is not a Quick Setup.
const MAX_ITEMS: usize = 100;
/// A save slower than this is logged as a warning, with its stage times.
const SLOW_SAVE: Duration = Duration::from_secs(5);

/// What Quick Setup's "Enter 4DA" writes.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingContextSave {
    /// Interests the user chose that are not stored yet (saved as explicit).
    #[serde(default)]
    pub add_interests: Vec<String>,
    /// Taste-test likes the user took out in Quick Setup.
    #[serde(default)]
    pub remove_interests: Vec<String>,
    /// Detected technologies the user kept.
    #[serde(default)]
    pub technologies: Vec<String>,
    /// Stack profiles to select; `None` leaves the selection untouched.
    #[serde(default)]
    pub stack_profile_ids: Option<Vec<String>>,
}

/// Stage timings of one save, returned so the UI and logs can say where time
/// went.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingSaveReport {
    pub interests_added: usize,
    pub interests_removed: usize,
    pub technologies_added: usize,
    pub embed_ms: u64,
    pub write_ms: u64,
}

fn validate(save: &OnboardingContextSave) -> Result<()> {
    let items = save.add_interests.len() + save.remove_interests.len() + save.technologies.len();
    if items > MAX_ITEMS {
        return Err(format!("Too many items in one save ({items}, max {MAX_ITEMS})").into());
    }
    for value in save
        .add_interests
        .iter()
        .chain(&save.remove_interests)
        .chain(&save.technologies)
    {
        crate::settings_commands::validate_input_length(
            value,
            "Interest or technology",
            MAX_ITEM_LEN,
        )?;
    }
    for id in save.stack_profile_ids.iter().flatten() {
        if crate::stacks::get_profile(id).is_none() {
            return Err(format!("Unknown stack profile: {id}").into());
        }
    }
    Ok(())
}

/// All of the save's database writes, through one context-engine handle.
fn write_all(save: &OnboardingContextSave, embeddings: &[Vec<f32>]) -> Result<()> {
    let engine = crate::get_context_engine()?;
    for (i, topic) in save.add_interests.iter().enumerate() {
        let emb = embeddings.get(i).map(Vec::as_slice);
        engine
            .add_interest(topic, 1.0, emb, InterestSource::Explicit)
            .map_err(|e| format!("Failed to add interest '{topic}': {e}"))?;
    }
    for topic in &save.remove_interests {
        engine
            .remove_interest(topic)
            .map_err(|e| format!("Failed to remove interest '{topic}': {e}"))?;
    }
    for tech in &save.technologies {
        engine
            .add_technology(tech)
            .map_err(|e| format!("Failed to add technology '{tech}': {e}"))?;
    }
    if let Some(ids) = &save.stack_profile_ids {
        let conn = crate::open_db_connection()?;
        crate::stacks::save_selected_stacks(&conn, ids)
            .map_err(|e| format!("Failed to save stacks: {e}"))?;
    }
    Ok(())
}

/// Save Quick Setup's interests, technologies and stacks in one call.
#[tauri::command]
pub async fn save_onboarding_context(save: OnboardingContextSave) -> Result<OnboardingSaveReport> {
    validate(&save)?;
    let started = Instant::now();

    // One embedding request for every new interest (one settings lock, one
    // keychain hydration, one HTTP call) instead of one per interest.
    let embeddings = if save.add_interests.is_empty() {
        Vec::new()
    } else {
        crate::embed_texts(&save.add_interests).await?
    };
    let embed_ms = started.elapsed().as_millis() as u64;

    let write_started = Instant::now();
    let save = std::sync::Arc::new(save);
    let writes = std::sync::Arc::clone(&save);
    crate::ipc_blocking::off_ui_thread("save_onboarding_context", move || {
        write_all(&writes, &embeddings)
    })
    .await?;
    let write_ms = write_started.elapsed().as_millis() as u64;
    crate::invalidate_context_engine();

    let report = OnboardingSaveReport {
        interests_added: save.add_interests.len(),
        interests_removed: save.remove_interests.len(),
        technologies_added: save.technologies.len(),
        embed_ms,
        write_ms,
    };
    if started.elapsed() > SLOW_SAVE {
        warn!(target: "4da::context", ?report, "Onboarding context save was slow");
    } else {
        info!(target: "4da::context", ?report, "Onboarding context saved");
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_stack_and_oversized_items_before_writing() {
        let unknown = OnboardingContextSave {
            stack_profile_ids: Some(vec!["not_a_stack".into()]),
            ..Default::default()
        };
        assert!(validate(&unknown).is_err());

        let long = OnboardingContextSave {
            add_interests: vec!["x".repeat(MAX_ITEM_LEN + 1)],
            ..Default::default()
        };
        assert!(validate(&long).is_err());

        let many = OnboardingContextSave {
            technologies: vec!["t".into(); MAX_ITEMS + 1],
            ..Default::default()
        };
        assert!(validate(&many).is_err());

        let ok = OnboardingContextSave {
            add_interests: vec!["Rust".into()],
            stack_profile_ids: Some(vec!["rust_systems".into()]),
            ..Default::default()
        };
        assert!(validate(&ok).is_ok());
    }

    #[test]
    fn payload_is_camel_case() {
        let save: OnboardingContextSave = serde_json::from_value(serde_json::json!({
            "addInterests": ["Rust"],
            "removeInterests": ["Go"],
            "technologies": ["tokio"],
            "stackProfileIds": ["rust_systems"],
        }))
        .unwrap();
        assert_eq!(save.add_interests, vec!["Rust"]);
        assert_eq!(save.remove_interests, vec!["Go"]);
        assert_eq!(
            save.stack_profile_ids,
            Some(vec!["rust_systems".to_string()])
        );
    }

    /// One save writes interests (explicit, with their embeddings), removes
    /// taken-out likes and adds technologies — through the real context engine
    /// on this test process's isolated database.
    #[test]
    fn write_all_applies_every_part_of_the_save() {
        let engine = crate::get_context_engine().unwrap();
        engine
            .add_interest("onb-liked-removed", 0.8, None, InterestSource::Explicit)
            .unwrap();
        let save = OnboardingContextSave {
            add_interests: vec!["onb-kept-guess".into()],
            remove_interests: vec!["onb-liked-removed".into()],
            technologies: vec!["onb-tech".into()],
            stack_profile_ids: None,
        };
        write_all(&save, &[vec![0.5; 4]]).unwrap();

        let interests = engine.get_interests().unwrap();
        let kept = interests
            .iter()
            .find(|i| i.topic == "onb-kept-guess")
            .expect("added");
        assert_eq!(kept.source, InterestSource::Explicit);
        assert!(kept.embedding.is_some());
        assert!(!interests.iter().any(|i| i.topic == "onb-liked-removed"));
        assert!(engine
            .get_tech_stack()
            .unwrap()
            .contains(&"onb-tech".to_string()));
    }
}
