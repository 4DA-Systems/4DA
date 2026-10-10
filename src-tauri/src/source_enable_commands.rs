// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Turning interests on and off (AD-054 decision 3).
//!
//! `get_source_settings` lists every source with its class and whether it is
//! on; `set_source_enabled` writes `sources.enabled`. Stack sources
//! (registries and advisories) are always on and cannot be turned off.
//!
//! Turning an interest off takes effect at once: its items leave the
//! in-memory display set, and every reader of source items skips it from
//! then on (`source_class::enabled_source_sql`). Nothing is deleted — turning
//! it back on restores its items and verdicts, and the next analysis cycle
//! brings them back into view.

use serde::Serialize;
use tracing::info;

use crate::error::{FourDaError, Result};
use crate::sources::source_class;

/// One row of the Settings source list.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SourceSetting {
    pub source_type: String,
    pub name: String,
    /// `"stack"` (always on) or `"interest"` (opt-in).
    pub class: &'static str,
    pub enabled: bool,
    pub last_fetch: Option<String>,
}

/// The adapters this build fetches, as `(source_type, name)`.
fn adapters() -> Vec<(&'static str, &'static str)> {
    crate::sources::build_all_sources()
        .iter()
        .map(|s| (s.source_type(), s.name()))
        .collect()
}

/// Every source with its class and on/off state, stack sources first.
#[tauri::command]
pub(crate) async fn get_source_settings() -> Result<Vec<SourceSetting>> {
    let db = crate::get_database()?;
    let stored: std::collections::HashMap<String, (bool, Option<String>)> = db
        .get_all_sources()?
        .into_iter()
        .map(|(st, _name, enabled, last_fetch)| (st, (enabled, last_fetch)))
        .collect();
    let mut rows: Vec<SourceSetting> = adapters()
        .into_iter()
        .map(|(source_type, name)| {
            let (enabled, last_fetch) = stored
                .get(source_type)
                .cloned()
                .unwrap_or((source_class::default_enabled(source_type), None));
            SourceSetting {
                source_type: source_type.to_string(),
                name: name.to_string(),
                class: source_class::class_name(source_type),
                // A stack source is on whatever the row says (AD-054).
                enabled: enabled || source_class::is_stack_source(source_type),
                last_fetch,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        source_class::is_interest(&a.source_type)
            .cmp(&source_class::is_interest(&b.source_type))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(rows)
}

/// Turn an interest on or off. Refuses unknown sources and turning a stack
/// source off.
#[tauri::command]
pub(crate) async fn set_source_enabled(
    source_type: String,
    enabled: bool,
) -> Result<SourceSetting> {
    let Some((st, name)) = adapters().into_iter().find(|(st, _)| *st == source_type) else {
        return Err(FourDaError::Validation(format!(
            "Unknown source: {source_type}"
        )));
    };
    if !enabled && source_class::is_stack_source(st) {
        return Err(FourDaError::Validation(format!(
            "{name} is a stack source: 4DA needs it to check your dependencies, so it stays on"
        )));
    }
    let db = crate::get_database()?;
    db.set_source_enabled(st, name, enabled)?;
    if !enabled {
        let removed = drop_from_display_set(st);
        info!(target: "4da::sources", source = st, removed, "Interest turned off");
    } else {
        info!(target: "4da::sources", source = st, "Interest turned on");
    }
    Ok(SourceSetting {
        source_type: st.to_string(),
        name: name.to_string(),
        class: source_class::class_name(st),
        enabled,
        last_fetch: db.get_source_last_fetch(st).ok().flatten(),
    })
}

/// Remove a turned-off source's rows from the in-memory analysis results,
/// so the Brief and Signal stop showing them before the next cycle runs.
fn drop_from_display_set(source_type: &str) -> usize {
    let mut state = crate::get_analysis_state().lock();
    let mut removed = 0;
    if let Some(results) = state.results.as_mut() {
        let before = results.len();
        results.retain(|r| r.source_type != source_type);
        removed = before - results.len();
    }
    if let Some(near) = state.near_misses.as_mut() {
        near.retain(|r| r.source_type != source_type);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_adapter_is_listed_once() {
        let types: Vec<&str> = adapters().iter().map(|(st, _)| *st).collect();
        let unique: std::collections::HashSet<&str> = types.iter().copied().collect();
        assert_eq!(types.len(), unique.len());
        assert!(types.contains(&"osv") && types.contains(&"hackernews"));
    }
}
