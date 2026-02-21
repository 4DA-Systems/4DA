//! Suns -- Self-sustaining background systems for STREETS modules.
//!
//! Each "Sun" is a background job that maintains data freshness for a STREETS module.
//! Suns run on intervals, store results, and generate alerts.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub mod api_cost_monitor;
pub mod hardware_monitor;
pub mod market_tracker;
pub mod price_tracker;
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
