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
                .and_then(serde_json::Value::as_bool)
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

    // For weekly frequency, try the full weekly digest with signal chains + knowledge gaps
    if frequency == "weekly" && crate::settings::is_signal() {
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

                        // Send digest email (if configured — opt-in only)
                        maybe_send_digest_email(app).await;

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

        let user_lang = crate::i18n::get_user_language();
        match db.get_relevant_items_since(period_start, 0.3, 20, &user_lang) {
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

                // Send digest email (if configured — opt-in only)
                maybe_send_digest_email(app).await;

                // Send system tray notification for the digest
                let notif_body = if frequency == "weekly" {
                    "Your weekly intelligence digest is ready".to_string()
                } else {
                    format!("{item_count} new items in your {frequency} digest")
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

// ============================================================================
// Digest Email Delivery (opt-in only)
// ============================================================================

/// Send digest email if the user has configured email delivery.
/// Does nothing if email is not set or SMTP is not configured.
/// Failures are logged but never block the digest pipeline.
async fn maybe_send_digest_email<R: Runtime>(_app: &AppHandle<R>) {
    let (email, smtp) = {
        let settings = crate::get_settings_manager().lock();
        let digest = &settings.get().digest;
        (digest.email.clone(), digest.smtp.clone())
    };

    let email = match email {
        Some(e) if !e.is_empty() => e,
        _ => return, // No email configured — silent return
    };
    let smtp = match smtp {
        Some(s) => s,
        None => return, // No SMTP configured — silent return
    };

    // Build a digest from recent items for the email body
    if let Ok(db) = crate::get_database() {
        let now = chrono::Utc::now();
        let (frequency, _) = {
            let settings = crate::get_settings_manager().lock();
            let d = &settings.get().digest;
            (d.frequency.clone(), d.min_score)
        };

        let period_start = match frequency.as_str() {
            "weekly" => now - chrono::Duration::days(7),
            _ => now - chrono::Duration::hours(24),
        };

        let user_lang = crate::i18n::get_user_language();
        match db.get_relevant_items_since(period_start, 0.3, 20, &user_lang) {
            Ok(items) if !items.is_empty() => {
                let digest_items: Vec<crate::digest::DigestItem> = items
                    .iter()
                    .map(|item| crate::digest::DigestItem {
                        id: item.id,
                        title: item.title.clone(),
                        url: item.url.clone(),
                        source: item.source_type.clone(),
                        relevance_score: item.relevance_score.unwrap_or(0.0),
                        matched_topics: item.topics.clone(),
                        discovered_at: item.created_at,
                        summary: None,
                        signal_type: None,
                        signal_priority: None,
                        signal_action: None,
                    })
                    .collect();

                let digest = crate::digest::Digest::new(digest_items, period_start, now);

                match crate::digest_email::send_digest_email(&email, &smtp, &digest).await {
                    Ok(()) => {
                        info!(target: "4da::jobs", to = %email, "Digest email delivered");
                    }
                    Err(e) => {
                        warn!(target: "4da::jobs", error = %e, "Digest email delivery failed");
                    }
                }
            }
            _ => {
                info!(target: "4da::jobs", "No items for email digest, skipping email");
            }
        }
    }
}

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
    let filename = format!("mini_digest_{timestamp}.md");

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

// ============================================================================
// Proactive Chain Prediction Notifications (Phase B)
// ============================================================================

/// Check signal chains for escalating/peak phases and send OS notifications.
/// Called hourly alongside anomaly detection and decision window checks.
pub fn maybe_notify_escalating_chains<R: Runtime>(app: &AppHandle<R>) {
    // Only for Signal users (signal chains are a Signal feature)
    if !crate::settings::is_signal() {
        return;
    }

    // Check notification threshold — respect user's preference
    let threshold = {
        let settings = crate::get_settings_manager().lock();
        settings.get().monitoring.notification_threshold.clone()
    };

    // critical_only threshold means don't send chain predictions unless critical
    let require_critical = threshold == "critical_only";

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return,
    };

    let chains = match crate::signal_chains::detect_chains(&conn) {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::jobs", error = %e, "Chain detection failed for notifications");
            return;
        }
    };

    let mut notified = 0;
    for chain in &chains {
        let prediction = crate::signal_chains::predict_chain_lifecycle(chain);

        let dominated = matches!(
            prediction.phase,
            crate::signal_chains::ChainPhase::Escalating | crate::signal_chains::ChainPhase::Peak
        );

        if !dominated || prediction.confidence < 0.3 || chain.confidence < 0.7 {
            continue;
        }

        // For critical_only, only send if chain has security signals
        if require_critical {
            let has_security = chain
                .links
                .iter()
                .any(|l| l.signal_type == "security_alert");
            if !has_security {
                continue;
            }
        }

        let phase_str = match prediction.phase {
            crate::signal_chains::ChainPhase::Escalating => "escalating",
            crate::signal_chains::ChainPhase::Peak => "peak",
            _ => continue,
        };

        crate::monitoring_notifications::send_chain_prediction_notification(
            app,
            &chain.chain_name,
            phase_str,
            &prediction.forecast,
        );
        notified += 1;

        // Cap notifications per check to avoid flooding
        if notified >= 3 {
            break;
        }
    }

    if notified > 0 {
        info!(target: "4da::jobs", notified, "Chain prediction notifications sent");
    }
}

// ============================================================================
// CVE Scan — Developer Immune System
// ============================================================================

/// Background CVE scan: fetch advisories, cross-reference against user deps,
/// store alerts, emit notifications for Critical/High matches.
/// Runs every 30 minutes from the monitoring scheduler.
pub async fn run_cve_scan<R: Runtime>(app: &AppHandle<R>) {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::jobs", "CVE scan: database unavailable: {e}");
            return;
        }
    };

    // 1. Load user dependencies
    let user_deps = match db.get_all_user_dependencies() {
        Ok(deps) => deps,
        Err(e) => {
            warn!(target: "4da::jobs", "CVE scan: failed to load dependencies: {e}");
            return;
        }
    };

    if !user_deps.is_empty() {
        // 2. Fetch recent advisories from GitHub Advisory Database
        let advisories = match crate::sources::cve::fetch_github_advisories(None).await {
            Ok(a) => a,
            Err(e) => {
                warn!(target: "4da::jobs", "CVE scan: failed to fetch advisories: {e}");
                Vec::new()
            }
        };

        if !advisories.is_empty() {
            // 3. Cross-reference with semver version matching
            let dep_tuples: Vec<(String, String, Option<String>)> = user_deps
                .iter()
                .map(|d| {
                    (
                        d.package_name.clone(),
                        d.ecosystem.clone(),
                        d.version.clone(),
                    )
                })
                .collect();

            let matches = crate::sources::cve::cross_reference_advisories(&advisories, &dep_tuples);

            if matches.is_empty() {
                info!(target: "4da::jobs", advisories = advisories.len(), deps = user_deps.len(), "CVE scan complete: no matches");
            } else {
                // 4. Store alerts + emit notifications (cap at 5 notifications per scan)
                let mut alerts_created = 0u32;
                let mut notifications_sent = 0u32;
                const MAX_NOTIFICATIONS: u32 = 5;

                for (advisory, matched_pkgs) in &matches {
                    for pkg in matched_pkgs {
                        let alert = crate::db::DependencyAlert {
                            id: 0,
                            package_name: pkg.name.clone(),
                            ecosystem: pkg.ecosystem.clone(),
                            alert_type: "cve".to_string(),
                            severity: advisory.severity.clone(),
                            title: format!("{}: {}", advisory.cve_id, advisory.title),
                            description: Some(advisory.description.clone()),
                            affected_versions: Some(pkg.affected_versions.clone()),
                            source_url: Some(advisory.source_url.clone()),
                            source_item_id: None,
                            detected_at: chrono::Utc::now().to_rfc3339(),
                            resolved_at: None,
                        };

                        match db.store_dependency_alert(&alert) {
                            Ok(id) if id > 0 => {
                                alerts_created += 1;

                                // Emit notification for Critical/High (capped)
                                let sev = advisory.severity.to_lowercase();
                                if (sev == "critical" || sev == "high")
                                    && notifications_sent < MAX_NOTIFICATIONS
                                {
                                    let _ = app.emit(
                                        "security-alert",
                                        serde_json::json!({
                                            "package": pkg.name,
                                            "cve_id": advisory.cve_id,
                                            "severity": advisory.severity,
                                            "title": advisory.title,
                                        }),
                                    );
                                    notifications_sent += 1;
                                }
                            }
                            _ => {} // Duplicate or error — skip
                        }
                    }
                }

                if alerts_created > 0 {
                    info!(target: "4da::jobs", alerts = alerts_created, notifications = notifications_sent, "CVE scan: new alerts created");

                    // Summary notification if we hit the cap
                    if notifications_sent >= MAX_NOTIFICATIONS && alerts_created > MAX_NOTIFICATIONS
                    {
                        let remaining = alerts_created - MAX_NOTIFICATIONS;
                        let _ = app.emit(
                            "security-alert-summary",
                            serde_json::json!({
                                "additional_alerts": remaining,
                                "total_alerts": alerts_created,
                            }),
                        );
                    }
                }
            }
        }
    }

    // 5. Run local audit tools (npm audit, cargo audit) if available
    let local_findings = crate::local_audit::run_local_audits().await;
    for finding in local_findings {
        let alert = crate::db::DependencyAlert {
            id: 0,
            package_name: finding.package_name,
            ecosystem: finding.ecosystem,
            alert_type: "audit".to_string(),
            severity: finding.severity,
            title: finding.title,
            description: finding.description,
            affected_versions: finding.affected_versions,
            source_url: finding.source_url,
            source_item_id: None,
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
        };
        let _ = db.store_dependency_alert(&alert);
    }
}
