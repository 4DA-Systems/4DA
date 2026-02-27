//! Suns -- Self-sustaining background systems for STREETS modules.
//!
//! Each "Sun" is a background job that maintains data freshness for a STREETS module.
//! Suns run on intervals, store results, and generate alerts.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

pub mod api_cost_monitor;
pub mod automation_auditor;
pub mod edge_detector;
pub mod execution_tracker;
pub mod hardware_monitor;
pub mod market_tracker;
pub mod price_tracker;
pub mod stream_monitor;
pub mod tech_moat_scanner;
pub mod uptime_monitor;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunStatus {
    pub id: String,
    pub name: String,
    pub module_id: String,
    pub enabled: bool,
    pub interval_secs: u64,
    pub last_run: Option<String>,
    pub next_run_in_secs: Option<u64>,
    pub last_result: Option<String>,
    pub run_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunAlert {
    pub id: i64,
    pub sun_id: String,
    pub alert_type: String,
    pub message: String,
    pub acknowledged: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleHealth {
    pub module_id: String,
    pub module_name: String,
    pub score: f32, // 0.0 - 1.0
    pub sun_count: usize,
    pub success_rate: f32, // sun success rate over last 7 days
    pub lessons_completed: usize,
    pub total_lessons: usize,
    pub last_activity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreetHealthScore {
    pub overall: f32, // 0.0 - 1.0
    pub module_scores: Vec<ModuleHealth>,
    pub trend: String,      // "improving" | "stable" | "declining"
    pub top_action: String, // Most impactful next action
}

// ============================================================================
// Sun Registry
// ============================================================================

/// Registry of all suns with interval tracking and enable/disable state.
pub struct SunRegistry {
    suns: Vec<SunDef>,
    last_runs: HashMap<String, Arc<AtomicU64>>,
    enabled: HashMap<String, bool>,
    run_counts: HashMap<String, Arc<AtomicU64>>,
    last_messages: HashMap<String, String>,
}

struct SunDef {
    id: String,
    name: String,
    module_id: String,
    interval_secs: u64,
    execute: fn() -> SunResult,
}

impl SunRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            suns: Vec::new(),
            last_runs: HashMap::new(),
            enabled: HashMap::new(),
            run_counts: HashMap::new(),
            last_messages: HashMap::new(),
        };

        // Register all suns
        // S = Sovereignty module, R = Revenue module
        registry.register(
            "hardware_monitor",
            "Hardware Monitor",
            "S",
            86400,
            hardware_monitor::execute,
        ); // 24h
        registry.register(
            "price_tracker",
            "Price Tracker",
            "S",
            604800,
            price_tracker::execute,
        ); // 7 days
        registry.register(
            "uptime_monitor",
            "Uptime Monitor",
            "S",
            300,
            uptime_monitor::execute,
        ); // 5 min
        registry.register(
            "market_tracker",
            "Market Tracker",
            "R",
            86400,
            market_tracker::execute,
        ); // 24h
        registry.register(
            "api_cost_monitor",
            "API Cost Monitor",
            "R",
            3600,
            api_cost_monitor::execute,
        ); // 1h
           // T = Technical Moats module
        registry.register(
            "tech_moat_scanner",
            "Tech Moat Scanner",
            "T",
            86400,
            tech_moat_scanner::execute,
        ); // 24h
           // E1 = Execution Playbook module
        registry.register(
            "execution_tracker",
            "Execution Tracker",
            "E1",
            43200,
            execution_tracker::execute,
        ); // 12h
           // E2 = Evolving Edge module
        registry.register(
            "edge_detector",
            "Edge Detector",
            "E2",
            86400,
            edge_detector::execute,
        ); // 24h
           // T2 = Tactical Automation module
        registry.register(
            "automation_auditor",
            "Automation Auditor",
            "T2",
            86400,
            automation_auditor::execute,
        ); // 24h
           // S2 = Stacking Streams module
        registry.register(
            "stream_monitor",
            "Stream Monitor",
            "S2",
            21600,
            stream_monitor::execute,
        ); // 6h

        registry
    }

    fn register(
        &mut self,
        id: &str,
        name: &str,
        module_id: &str,
        interval_secs: u64,
        execute: fn() -> SunResult,
    ) {
        self.suns.push(SunDef {
            id: id.to_string(),
            name: name.to_string(),
            module_id: module_id.to_string(),
            interval_secs,
            execute,
        });
        self.last_runs
            .insert(id.to_string(), Arc::new(AtomicU64::new(0)));
        self.enabled.insert(id.to_string(), true);
        self.run_counts
            .insert(id.to_string(), Arc::new(AtomicU64::new(0)));
    }

    /// Return status for each registered sun.
    pub fn get_statuses(&self) -> Vec<SunStatus> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.suns
            .iter()
            .map(|sun| {
                let last_run_ts = self
                    .last_runs
                    .get(&sun.id)
                    .map(|a| a.load(Ordering::Relaxed))
                    .unwrap_or(0);
                let enabled = self.enabled.get(&sun.id).copied().unwrap_or(true);
                let run_count = self
                    .run_counts
                    .get(&sun.id)
                    .map(|a| a.load(Ordering::Relaxed))
                    .unwrap_or(0);

                let last_run = if last_run_ts > 0 {
                    // Convert unix timestamp to ISO string
                    chrono::DateTime::from_timestamp(last_run_ts as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                } else {
                    None
                };

                let next_run_in_secs = if enabled && last_run_ts > 0 {
                    let elapsed = now.saturating_sub(last_run_ts);
                    if elapsed >= sun.interval_secs {
                        Some(0)
                    } else {
                        Some(sun.interval_secs - elapsed)
                    }
                } else if enabled {
                    Some(0) // Never run, due immediately
                } else {
                    None // Disabled
                };

                let last_result = self.last_messages.get(&sun.id).cloned();

                SunStatus {
                    id: sun.id.clone(),
                    name: sun.name.clone(),
                    module_id: sun.module_id.clone(),
                    enabled,
                    interval_secs: sun.interval_secs,
                    last_run,
                    next_run_in_secs,
                    last_result,
                    run_count,
                }
            })
            .collect()
    }

    /// Get sun definitions grouped by module ID.
    pub fn get_module_sun_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for sun in &self.suns {
            *counts.entry(sun.module_id.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Enable or disable a sun by ID.
    pub fn set_enabled(&mut self, sun_id: &str, enabled: bool) {
        self.enabled.insert(sun_id.to_string(), enabled);
        info!(target: "4da::suns", sun = sun_id, enabled, "Sun toggled");
    }

    /// Tick all suns -- called from the monitoring scheduler.
    /// Returns a list of (sun_id, result) for suns that executed this tick.
    pub fn tick(&mut self) -> Vec<(String, SunResult)> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut results = Vec::new();

        for sun in &self.suns {
            if !self.enabled.get(&sun.id).copied().unwrap_or(true) {
                continue;
            }

            let last_run = self
                .last_runs
                .get(&sun.id)
                .map(|a| a.load(Ordering::Relaxed))
                .unwrap_or(0);

            if now - last_run < sun.interval_secs {
                continue;
            }

            // Update last run timestamp
            if let Some(tracker) = self.last_runs.get(&sun.id) {
                tracker.store(now, Ordering::Relaxed);
            }

            debug!(target: "4da::suns", sun = %sun.id, "Executing sun");
            let start = std::time::Instant::now();
            let result = (sun.execute)();
            let duration_ms = start.elapsed().as_millis() as i64;

            // Update run count
            if let Some(counter) = self.run_counts.get(&sun.id) {
                counter.fetch_add(1, Ordering::Relaxed);
            }

            // Store last message
            self.last_messages
                .insert(sun.id.clone(), result.message.clone());

            // Store result in DB (non-fatal)
            store_sun_run(&sun.id, &sun.module_id, &result, duration_ms);

            // Generate alert on failure
            if !result.success {
                store_sun_alert(&sun.id, "failure", &result.message);
            }

            results.push((sun.id.clone(), result));
        }

        results
    }

    /// Execute a specific sun by ID, bypassing the interval check.
    pub fn execute_one(&mut self, sun_id: &str) -> Option<SunResult> {
        let sun = self.suns.iter().find(|s| s.id == sun_id)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(tracker) = self.last_runs.get(&sun.id) {
            tracker.store(now, Ordering::Relaxed);
        }

        let start = std::time::Instant::now();
        let result = (sun.execute)();
        let duration_ms = start.elapsed().as_millis() as i64;

        if let Some(counter) = self.run_counts.get(&sun.id) {
            counter.fetch_add(1, Ordering::Relaxed);
        }

        self.last_messages
            .insert(sun.id.clone(), result.message.clone());
        let module_id = sun.module_id.clone();
        store_sun_run(sun_id, &module_id, &result, duration_ms);

        if !result.success {
            store_sun_alert(sun_id, "failure", &result.message);
        }

        Some(result)
    }
}

// ============================================================================
// DB Helpers
// ============================================================================

fn store_sun_run(sun_id: &str, module_id: &str, result: &SunResult, duration_ms: i64) {
    if let Ok(conn) = crate::open_db_connection() {
        let _ = conn.execute(
            "INSERT INTO sun_runs (sun_id, module_id, success, result_message, data_json, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                sun_id,
                module_id,
                result.success as i32,
                result.message,
                result.data.as_ref().map(|d| d.to_string()),
                duration_ms,
            ],
        );
    }
}

pub(crate) fn store_sun_alert(sun_id: &str, alert_type: &str, message: &str) {
    if let Ok(conn) = crate::open_db_connection() {
        let _ = conn.execute(
            "INSERT INTO sun_alerts (sun_id, alert_type, message) VALUES (?1, ?2, ?3)",
            params![sun_id, alert_type, message],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! sun_smoke_test {
        ($name:ident, $module:ident) => {
            #[test]
            fn $name() {
                let result = $module::execute();
                assert!(
                    !result.message.is_empty(),
                    "Sun {} must produce a non-empty message",
                    stringify!($module)
                );
            }
        };
    }

    sun_smoke_test!(smoke_uptime, uptime_monitor);
    sun_smoke_test!(smoke_hardware, hardware_monitor);
    sun_smoke_test!(smoke_price_tracker, price_tracker);
    sun_smoke_test!(smoke_market_tracker, market_tracker);
    sun_smoke_test!(smoke_api_cost_monitor, api_cost_monitor);
    sun_smoke_test!(smoke_tech_moat_scanner, tech_moat_scanner);
    sun_smoke_test!(smoke_execution_tracker, execution_tracker);
    sun_smoke_test!(smoke_edge_detector, edge_detector);
    sun_smoke_test!(smoke_automation_auditor, automation_auditor);
    sun_smoke_test!(smoke_stream_monitor, stream_monitor);

    // Test registry construction and behavior
    #[test]
    fn test_registry_has_10_suns() {
        let registry = SunRegistry::new();
        let statuses = registry.get_statuses();
        assert_eq!(statuses.len(), 10, "Expected 10 registered suns");
    }

    #[test]
    fn test_registry_module_counts() {
        let registry = SunRegistry::new();
        let counts = registry.get_module_sun_counts();
        // Should have entries for S, R, T, E1, E2, T2, S2
        assert!(
            counts.len() >= 7,
            "Expected at least 7 module categories, got {}",
            counts.len()
        );
    }

    #[test]
    fn test_set_enabled_disables_sun() {
        let mut registry = SunRegistry::new();
        registry.set_enabled("uptime_monitor", false);
        let statuses = registry.get_statuses();
        let uptime = statuses.iter().find(|s| s.id == "uptime_monitor").unwrap();
        assert!(!uptime.enabled);
        assert!(
            uptime.next_run_in_secs.is_none(),
            "Disabled sun should have no next_run"
        );
    }
}
