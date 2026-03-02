//! Digest configuration, briefing cache, and decision context builder.
//!
//! Extracted from digest_commands.rs to keep both files under the 600-line limit.

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tracing::info;

use crate::error::Result;
use crate::get_settings_manager;

/// Cached latest briefing text for TTS and handoff features
pub(crate) static LATEST_BRIEFING: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Get the latest generated briefing text (used by TTS and handoff)
pub(crate) fn get_latest_briefing_text() -> Option<String> {
    LATEST_BRIEFING.lock().clone()
}

// ============================================================================
// Digest Commands
// ============================================================================

/// Get digest configuration
#[tauri::command]
pub async fn get_digest_config() -> Result<serde_json::Value> {
    // Clone data out of lock immediately to avoid holding across async boundary
    let json = {
        let settings_guard = get_settings_manager().lock();
        let digest = &settings_guard.get().digest;
        serde_json::json!({
            "enabled": digest.enabled,
            "frequency": digest.frequency,
            "email": digest.email,
            "save_local": digest.save_local,
            "min_score": digest.min_score,
            "max_items": digest.max_items,
            "last_sent": digest.last_sent,
            "generate_summaries": digest.generate_summaries
        })
    };
    Ok(json)
}

/// Update digest configuration
#[tauri::command]
pub async fn set_digest_config(
    enabled: Option<bool>,
    frequency: Option<String>,
    email: Option<String>,
    save_local: Option<bool>,
    min_score: Option<f64>,
    max_items: Option<usize>,
) -> Result<serde_json::Value> {
    // Mutate and save within scoped lock, then release before returning
    let json = {
        let mut settings_guard = get_settings_manager().lock();
        let digest = &mut settings_guard.get_mut().digest;

        if let Some(e) = enabled {
            digest.enabled = e;
        }
        if let Some(f) = frequency {
            digest.frequency = f;
        }
        if let Some(e) = email {
            digest.email = Some(e);
        }
        if let Some(s) = save_local {
            digest.save_local = s;
        }
        if let Some(s) = min_score {
            digest.min_score = s;
        }
        if let Some(m) = max_items {
            digest.max_items = m;
        }

        settings_guard.save()?;

        let digest = &settings_guard.get().digest;
        info!(
            target: "4da::digest",
            enabled = digest.enabled,
            frequency = %digest.frequency,
            "Digest config updated"
        );

        serde_json::json!({
            "success": true,
            "config": {
                "enabled": digest.enabled,
                "frequency": digest.frequency,
                "email": digest.email,
                "save_local": digest.save_local,
                "min_score": digest.min_score,
                "max_items": digest.max_items
            }
        })
    };
    Ok(json)
}

// ============================================================================
// Decision Context for Briefing (Step 1.4)
// ============================================================================

/// Build a Decision Context section to inject into the AI briefing prompt.
/// Includes active decisions, radar movement summary, and review prompts.
pub(crate) fn build_decision_context_for_briefing() -> String {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    // Get active decisions (limit 10 most recent)
    let decisions = match crate::decisions::list_decisions(
        &conn,
        None,
        Some(&crate::decisions::DecisionStatus::Active),
        10,
    ) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    if decisions.is_empty() {
        return String::new();
    }

    let mut sections = Vec::new();

    // Active decisions summary
    let decision_lines: Vec<String> = decisions
        .iter()
        .take(5)
        .map(|d| {
            let alts = if d.alternatives_rejected.is_empty() {
                String::new()
            } else {
                format!(" (rejected: {})", d.alternatives_rejected.join(", "))
            };
            format!("  - {}: {}{}", d.subject, d.decision, alts)
        })
        .collect();
    sections.push(format!(
        "- My active decisions:\n{}",
        decision_lines.join("\n")
    ));

    // Tech radar movement summary (if tech_radar module available)
    if let Ok(radar) = crate::tech_radar::compute_radar(&conn) {
        let moving_up: Vec<&str> = radar
            .entries
            .iter()
            .filter(|e| e.movement == crate::tech_radar::RadarMovement::Up)
            .map(|e| e.name.as_str())
            .take(3)
            .collect();
        let on_hold: Vec<&str> = radar
            .entries
            .iter()
            .filter(|e| e.ring == crate::tech_radar::RadarRing::Hold)
            .map(|e| e.name.as_str())
            .take(3)
            .collect();
        if !moving_up.is_empty() || !on_hold.is_empty() {
            let mut radar_text = String::from("- Radar movement:");
            if !moving_up.is_empty() {
                radar_text.push_str(&format!(" rising={}", moving_up.join(",")));
            }
            if !on_hold.is_empty() {
                radar_text.push_str(&format!(" on-hold={}", on_hold.join(",")));
            }
            sections.push(radar_text);
        }
    }

    // Decision review prompts: tech in "reconsidering" status
    let reconsidering = decisions
        .iter()
        .filter(|d| d.status == crate::decisions::DecisionStatus::Reconsidering)
        .take(2)
        .collect::<Vec<_>>();
    if !reconsidering.is_empty() {
        let review_lines: Vec<String> = reconsidering
            .iter()
            .map(|d| format!("  - Reconsidering: {} ({})", d.subject, d.decision))
            .collect();
        sections.push(format!(
            "- Decisions under review:\n{}",
            review_lines.join("\n")
        ));
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!("\n{}", sections.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // LATEST_BRIEFING static tests
    // ========================================================================

    /// Tests for the LATEST_BRIEFING static are combined into a single test
    /// to avoid data races (tests run in parallel and share the global static).
    #[test]
    fn latest_briefing_lifecycle() {
        // 1. Write a value and read it back
        {
            *LATEST_BRIEFING.lock() = Some("First briefing".to_string());
        }
        let result = get_latest_briefing_text();
        assert_eq!(result, Some("First briefing".to_string()));

        // 2. Overwrite with a new value
        {
            *LATEST_BRIEFING.lock() = Some("Second briefing".to_string());
        }
        let result = get_latest_briefing_text();
        assert_eq!(result, Some("Second briefing".to_string()));

        // 3. Clear back to None
        {
            *LATEST_BRIEFING.lock() = None;
        }
        let result = get_latest_briefing_text();
        assert!(result.is_none());

        // 4. get_latest_briefing_text returns a clone, not a reference
        {
            *LATEST_BRIEFING.lock() = Some("Clone test".to_string());
        }
        let cloned = get_latest_briefing_text();
        // Mutating the static should not affect the already-cloned value
        {
            *LATEST_BRIEFING.lock() = Some("Changed after clone".to_string());
        }
        assert_eq!(cloned, Some("Clone test".to_string()));

        // Clean up
        *LATEST_BRIEFING.lock() = None;
    }

    // ========================================================================
    // DigestConfig default values (used by get_digest_config / set_digest_config)
    // ========================================================================

    #[test]
    fn digest_config_default_values() {
        let config = crate::digest::DigestConfig::default();
        assert!(config.enabled);
        assert_eq!(config.frequency, "daily");
        assert!(config.email.is_none());
        assert!(config.save_local);
        assert_eq!(config.min_score, 0.35);
        assert_eq!(config.max_items, 20);
        assert!(config.last_sent.is_none());
        assert!(!config.generate_summaries);
    }

    #[test]
    fn digest_config_serializes_to_valid_json() {
        let config = crate::digest::DigestConfig::default();
        let json = serde_json::to_value(&config);
        assert!(json.is_ok());
        let val = json.unwrap();
        assert_eq!(val["enabled"], true);
        assert_eq!(val["frequency"], "daily");
        assert_eq!(val["min_score"], 0.35);
        assert_eq!(val["max_items"], 20);
    }

    #[test]
    fn digest_config_deserializes_from_json() {
        let json_str = r#"{
            "enabled": false,
            "frequency": "weekly",
            "email": "user@example.com",
            "smtp": null,
            "save_local": false,
            "output_dir": null,
            "min_score": 0.5,
            "max_items": 10,
            "last_sent": null,
            "generate_summaries": true
        }"#;
        let config: std::result::Result<crate::digest::DigestConfig, _> =
            serde_json::from_str(json_str);
        assert!(config.is_ok());
        let c = config.unwrap();
        assert!(!c.enabled);
        assert_eq!(c.frequency, "weekly");
        assert_eq!(c.email, Some("user@example.com".to_string()));
        assert!(!c.save_local);
        assert_eq!(c.min_score, 0.5);
        assert_eq!(c.max_items, 10);
        assert!(c.generate_summaries);
    }
}
