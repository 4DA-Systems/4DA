//! Monitoring job reactions -- background tasks triggered by the scheduler.
//!
//! Contains: auto-briefing on critical signals, digest scheduler,
//! anomaly-to-notification bridge, and smart notification batching.

use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

// ============================================================================
// Auto-Briefing on Critical Signals (Fix 1)
// ============================================================================

/// When critical signals are detected, auto-generate an AI briefing.
/// Called from the scheduler after anomaly detection finds medium+ severity.
pub async fn maybe_auto_briefing<R: Runtime>(app: &AppHandle<R>) {
    let enabled = {
        let settings = crate::get_settings_manager().lock();
        settings
            .get()
            .monitoring
            .auto_briefing_on_critical
            .unwrap_or(true)
    };
    if !enabled {
        info!(target: "4da::jobs", "Auto-briefing disabled, skipping");
        return;
    }

    // Check if LLM is configured
    let has_llm = {
        let settings = crate::get_settings_manager().lock();
        let llm = &settings.get().llm;
        llm.provider == "ollama" || !llm.api_key.is_empty()
    };
    if !has_llm {
        info!(target: "4da::jobs", "No LLM configured, skipping auto-briefing");
        return;
    }

    // Gather anomaly context for injection
    let anomalies = {
        if let Ok(ace) = crate::get_ace_engine() {
            let conn = ace.get_conn().lock();
            crate::anomaly::get_unresolved(&conn).ok().map(|list| {
                list.iter()
                    .map(|a| a.description.clone())
                    .collect::<Vec<_>>()
            })
        } else {
            None
        }
    };

    match crate::digest_commands::generate_briefing_internal(true, anomalies).await {
        Ok(result) => {
            if result
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                info!(target: "4da::jobs", "Auto-briefing generated successfully");
                if let Err(e) = app.emit("briefing-auto-generated", &result) {
                    tracing::warn!("Failed to emit 'briefing-auto-generated': {e}");
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::jobs", error = %e, "Auto-briefing generation failed");
        }
    }
}

// ============================================================================
// Digest Scheduler (Fix 2)
// ============================================================================

/// Check if a digest is due and generate it if so.
/// Called from the scheduler on each tick (every minute).
pub async fn maybe_generate_digest<R: Runtime>(app: &AppHandle<R>) {
    let (enabled, frequency, last_sent) = {
        let settings = crate::get_settings_manager().lock();
        let digest = &settings.get().digest;
        (digest.enabled, digest.frequency.clone(), digest.last_sent)
    };

    if !enabled {
        return;
    }

    let now = chrono::Utc::now();
    let is_due = match last_sent {
        None => true, // Never sent -- generate now
        Some(last) => {
            let elapsed = now - last;
            match frequency.as_str() {
                "daily" => elapsed.num_hours() >= 24,
                "weekly" => elapsed.num_days() >= 7,
                _ => false, // "realtime" handled by direct triggers
            }
        }
    };

    if !is_due {
        return;
    }

    info!(target: "4da::jobs", frequency = %frequency, "Digest is due, generating");

    // For weekly frequency on Pro, try the full weekly digest with signal chains + knowledge gaps
    if frequency == "weekly" && crate::settings::is_pro() {
        if let Ok(conn) = crate::open_db_connection() {
            if crate::weekly_digest::should_generate_digest(&conn) {
                match crate::weekly_digest::generate_weekly_digest().await {
                    Ok(digest) => {
                        // Update last_sent timestamp
                        {
                            let mut settings = crate::get_settings_manager().lock();
                            settings.get_mut().digest.last_sent = Some(now);
                            if let Err(e) = settings.save() {
                                tracing::warn!("Failed to save: {e}");
                            }
                        }
                        info!(target: "4da::jobs", "Weekly intelligence digest generated and emitted");
                        if let Err(e) = app.emit("digest-ready", &digest) {
                            tracing::warn!("Failed to emit 'digest-ready': {e}");
                        }

                        // Send system tray notification
                        if let Err(e) = app
                            .notification()
                            .builder()
                            .title("4DA - Weekly Digest")
                            .body("Your weekly intelligence digest is ready")
                            .show()
                        {
                            warn!(target: "4da::jobs", error = %e, "Failed to send weekly digest notification");
                        } else {
                            info!(target: "4da::jobs", "Sent weekly digest notification");
                        }

                        return;
                    }
                    Err(e) => {
                        warn!(target: "4da::jobs", error = %e, "Full weekly digest failed, falling back to simple digest");
                    }
                }
            }
        }
    }

    // Generate the simple digest using the existing digest module
    if let Ok(db) = crate::get_database() {
        let period_start = match frequency.as_str() {
            "weekly" => now - chrono::Duration::days(7),
            _ => now - chrono::Duration::hours(24),
        };

        match db.get_relevant_items_since(period_start, 0.3, 20) {
            Ok(items) if !items.is_empty() => {
                // Update last_sent timestamp
                {
                    let mut settings = crate::get_settings_manager().lock();
                    settings.get_mut().digest.last_sent = Some(now);
                    if let Err(e) = settings.save() {
                        tracing::warn!("Failed to save: {e}");
                    }
                }

                let item_count = items.len();
                info!(target: "4da::jobs", items = item_count, "Digest generated");
                let _ = app.emit(
                    "digest-generated",
                    serde_json::json!({
                        "item_count": item_count,
                        "frequency": frequency,
                        "generated_at": now.to_rfc3339(),
                    }),
                );

                // Send system tray notification for the digest
                let notif_body = if frequency == "weekly" {
                    "Your weekly intelligence digest is ready".to_string()
                } else {
                    format!("{} new items in your {} digest", item_count, frequency)
                };
                if let Err(e) = app
                    .notification()
                    .builder()
                    .title("4DA - Digest Ready")
                    .body(&notif_body)
                    .show()
                {
                    warn!(target: "4da::jobs", error = %e, "Failed to send digest notification");
                } else {
                    info!(target: "4da::jobs", "Sent digest notification");
                }
            }
            Ok(_) => {
                info!(target: "4da::jobs", "No items for digest, skipping");
            }
            Err(e) => {
                warn!(target: "4da::jobs", error = %e, "Failed to fetch digest items");
            }
        }
    }
}

// ============================================================================
// Anomaly Bridge (Fix 5)
// ============================================================================

/// Process anomaly detection results: emit events for medium+ severity,
/// auto-remediate StaleData anomalies, and trigger auto-briefing on critical.
pub async fn process_anomalies<R: Runtime>(
    app: &AppHandle<R>,
    anomalies: &[crate::anomaly::Anomaly],
) {
    let medium_plus: Vec<&crate::anomaly::Anomaly> = anomalies
        .iter()
        .filter(|a| a.severity >= crate::anomaly::AnomalySeverity::Medium)
        .collect();

    if medium_plus.is_empty() {
        return;
    }

    info!(target: "4da::jobs", count = medium_plus.len(), "Processing medium+ anomalies");

    // Emit anomaly events to frontend
    for anomaly in &medium_plus {
        let _ = app.emit(
            "anomaly-detected",
            serde_json::json!({
                "type": format!("{:?}", anomaly.anomaly_type),
                "severity": format!("{:?}", anomaly.severity),
                "description": anomaly.description,
            }),
        );
    }

    // Auto-remediate StaleData: trigger a context rescan
    let has_stale = medium_plus
        .iter()
        .any(|a| a.anomaly_type == crate::anomaly::AnomalyType::StaleData);
    if has_stale {
        info!(target: "4da::jobs", "StaleData detected -- triggering context rescan");
        if let Err(e) = app.emit("scheduled-analysis", ()) {
            tracing::warn!("Failed to emit 'scheduled-analysis': {e}");
        }
    }

    // If any critical anomalies, trigger auto-briefing
    let has_critical = medium_plus
        .iter()
        .any(|a| a.severity >= crate::anomaly::AnomalySeverity::High);
    if has_critical {
        maybe_auto_briefing(app).await;
    }
}

// ============================================================================
// Smart Notification Batching (Improvement E)
// ============================================================================

/// When enough items accumulate in the batch, save a mini-digest locally.
pub fn maybe_save_mini_digest(state: &crate::monitoring::MonitoringState) {
    let batched_count = state.batched_items.lock().len();
    if batched_count < 5 {
        return;
    }

    info!(target: "4da::jobs", batched = batched_count, "Smart batching: saving mini-digest");

    let items = crate::monitoring::drain_batched_notifications(state);
    if items.is_empty() {
        return;
    }

    // Save to a local mini-digest file in the data directory
    let digest_content = items
        .iter()
        .map(|b| {
            format!(
                "- [{}] {} (score: {:.0}%)",
                b.source_type,
                b.title,
                b.score * 100.0
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("mini_digest_{}.md", timestamp);

    if let Ok(data_dir) = std::env::current_dir() {
        let digest_dir = data_dir.join("data").join("digests");
        let _ = std::fs::create_dir_all(&digest_dir);
        let path = digest_dir.join(filename);
        let content = format!(
            "# 4DA Mini-Digest\n\nGenerated: {}\nItems: {}\n\n{}\n",
            chrono::Utc::now().to_rfc3339(),
            items.len(),
            digest_content,
        );
        if let Err(e) = std::fs::write(&path, content) {
            warn!(target: "4da::jobs", error = %e, "Failed to save mini-digest");
        } else {
            info!(target: "4da::jobs", path = %path.display(), "Mini-digest saved");
        }
    }
}
