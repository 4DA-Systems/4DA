//! Health Monitoring and Anomaly Detection
//!
//! Implements nuke-proof reliability for ACE:
//! - Component health tracking
//! - Anomaly detection for context drift
//! - Graceful degradation with fallback chain
//! - Audit trail for all decisions
//! - Accuracy metrics tracking

#![allow(dead_code)]

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

// ============================================================================
// Health Monitoring
// ============================================================================

/// System health monitor
pub struct HealthMonitor {
    conn: Arc<Mutex<Connection>>,
    /// Recent health checks for trend analysis
    health_history: VecDeque<HealthSnapshot>,
    /// Configuration
    config: HealthConfig,
}

#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Maximum health history entries
    pub max_history: usize,
    /// Error threshold before degraded status
    pub error_threshold: u32,
    /// Hours before stale warning
    pub stale_threshold_hours: i64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            max_history: 100,
            error_threshold: 3,
            stale_threshold_hours: 24,
        }
    }
}

/// Snapshot of system health at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub timestamp: String,
    pub components: Vec<ComponentStatus>,
    pub overall_status: HealthStatus,
    pub context_quality: ContextQuality,
    pub active_alerts: Vec<HealthAlert>,
}

/// Status of a single component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub name: String,
    pub status: HealthStatus,
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub error_count: u32,
    pub metrics: ComponentMetrics,
}

/// Metrics for a component
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentMetrics {
    pub items_processed: u64,
    pub avg_latency_ms: f32,
    pub success_rate: f32,
}

/// Health status levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
    Disabled,
}

impl HealthStatus {
    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }
}

/// Context quality levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContextQuality {
    Excellent,  // 95%+ accuracy expected
    Good,       // 85%+ accuracy expected
    Acceptable, // 75%+ accuracy expected
    Degraded,   // 60%+ accuracy expected
    Minimal,    // 50%+ accuracy expected
    Emergency,  // Best effort
}

impl ContextQuality {
    pub fn expected_accuracy(&self) -> f32 {
        match self {
            ContextQuality::Excellent => 0.95,
            ContextQuality::Good => 0.85,
            ContextQuality::Acceptable => 0.75,
            ContextQuality::Degraded => 0.60,
            ContextQuality::Minimal => 0.50,
            ContextQuality::Emergency => 0.30,
        }
    }
}

/// Health alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub alert_type: AlertType,
    pub component: String,
    pub message: String,
    pub severity: AlertSeverity,
    pub timestamp: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    ComponentFailure,
    HighErrorRate,
    StaleData,
    AccuracyDrop,
    AnomalyDetected,
    ResourceExhausted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl HealthMonitor {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            health_history: VecDeque::new(),
            config: HealthConfig::default(),
        }
    }

    /// Perform a complete health check
    pub fn check_health(&mut self) -> HealthSnapshot {
        let mut components = Vec::new();
        let mut alerts = Vec::new();

        // Check each component
        components.push(self.check_project_scanner());
        components.push(self.check_file_watcher());
        components.push(self.check_git_analyzer());
        components.push(self.check_behavior_learner());
        components.push(self.check_database());

        // Generate alerts for unhealthy components
        for comp in &components {
            if comp.status == HealthStatus::Failed {
                alerts.push(HealthAlert {
                    alert_type: AlertType::ComponentFailure,
                    component: comp.name.clone(),
                    message: format!("{} has failed", comp.name),
                    severity: AlertSeverity::Error,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    recommended_action: format!(
                        "Check {} logs and restart if necessary",
                        comp.name
                    ),
                });
            } else if comp.error_count >= self.config.error_threshold {
                alerts.push(HealthAlert {
                    alert_type: AlertType::HighErrorRate,
                    component: comp.name.clone(),
                    message: format!("{} has {} recent errors", comp.name, comp.error_count),
                    severity: AlertSeverity::Warning,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    recommended_action: "Monitor closely and investigate error logs".to_string(),
                });
            }
        }

        // Check for stale data
        if let Some(alert) = self.check_stale_data() {
            alerts.push(alert);
        }

        // Compute overall status
        let overall_status = self.compute_overall_status(&components);
        let context_quality = self.compute_context_quality(&components);

        let snapshot = HealthSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            components,
            overall_status,
            context_quality,
            active_alerts: alerts,
        };

        // Store in history
        self.health_history.push_back(snapshot.clone());
        if self.health_history.len() > self.config.max_history {
            self.health_history.pop_front();
        }

        // Persist to database
        let _ = self.store_health_check(&snapshot);

        snapshot
    }

    fn check_project_scanner(&self) -> ComponentStatus {
        let conn = self.conn.lock();

        // Check recent project detections
        let (count, last_scan): (u32, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(updated_at) FROM detected_projects",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, None));

        let status = if count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        };

        ComponentStatus {
            name: "project_scanner".to_string(),
            status,
            last_success: last_scan,
            last_error: None,
            error_count: 0,
            metrics: ComponentMetrics {
                items_processed: count as u64,
                avg_latency_ms: 0.0,
                success_rate: 1.0,
            },
        }
    }

    fn check_file_watcher(&self) -> ComponentStatus {
        let conn = self.conn.lock();

        // Check recent file signals
        let result: Result<(u32, Option<String>), _> = conn.query_row(
            "SELECT COUNT(*), MAX(timestamp) FROM file_signals
             WHERE timestamp > datetime('now', '-1 hour')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        let (count, last_signal) = result.unwrap_or((0, None));
        let status = if count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Disabled // No recent signals means watcher likely not running
        };

        ComponentStatus {
            name: "file_watcher".to_string(),
            status,
            last_success: last_signal,
            last_error: None,
            error_count: 0,
            metrics: ComponentMetrics {
                items_processed: count as u64,
                avg_latency_ms: 0.0,
                success_rate: 1.0,
            },
        }
    }

    fn check_git_analyzer(&self) -> ComponentStatus {
        let conn = self.conn.lock();

        // Check recent git signals
        let result: Result<(u32, Option<String>), _> = conn.query_row(
            "SELECT COUNT(*), MAX(timestamp) FROM git_signals",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        let (count, last_signal) = result.unwrap_or((0, None));
        let status = if count > 0 {
            HealthStatus::Healthy
        } else {
            HealthStatus::Disabled
        };

        ComponentStatus {
            name: "git_analyzer".to_string(),
            status,
            last_success: last_signal,
            last_error: None,
            error_count: 0,
            metrics: ComponentMetrics {
                items_processed: count as u64,
                avg_latency_ms: 0.0,
                success_rate: 1.0,
            },
        }
    }

    fn check_behavior_learner(&self) -> ComponentStatus {
        let conn = self.conn.lock();

        // Check interactions count
        let result: Result<(u32, Option<String>), _> = conn.query_row(
            "SELECT COUNT(*), MAX(timestamp) FROM interactions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

        let (count, last_interaction) = result.unwrap_or((0, None));
        let status = HealthStatus::Healthy; // Always healthy, just may have no data

        ComponentStatus {
            name: "behavior_learner".to_string(),
            status,
            last_success: last_interaction,
            last_error: None,
            error_count: 0,
            metrics: ComponentMetrics {
                items_processed: count as u64,
                avg_latency_ms: 0.0,
                success_rate: 1.0,
            },
        }
    }

    fn check_database(&self) -> ComponentStatus {
        let conn = self.conn.lock();

        // Simple health check - try a query
        let result = conn.query_row("SELECT 1", [], |_| Ok(()));
        let (status, last_error, error_count) = match &result {
            Ok(_) => (HealthStatus::Healthy, None, 0),
            Err(e) => (HealthStatus::Failed, Some(e.to_string()), 1),
        };

        ComponentStatus {
            name: "database".to_string(),
            status,
            last_success: Some(chrono::Utc::now().to_rfc3339()),
            last_error,
            error_count,
            metrics: ComponentMetrics::default(),
        }
    }

    fn check_stale_data(&self) -> Option<HealthAlert> {
        let conn = self.conn.lock();

        // Check if active topics are stale
        let result: Result<Option<String>, _> =
            conn.query_row("SELECT MAX(last_seen) FROM active_topics", [], |row| {
                row.get(0)
            });

        if let Ok(Some(last_seen)) = result {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&last_seen) {
                let hours_since = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_hours();
                if hours_since > self.config.stale_threshold_hours {
                    return Some(HealthAlert {
                        alert_type: AlertType::StaleData,
                        component: "active_topics".to_string(),
                        message: format!("No context updates for {} hours", hours_since),
                        severity: AlertSeverity::Warning,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        recommended_action: "Run a full context scan".to_string(),
                    });
                }
            }
        }

        None
    }

    fn compute_overall_status(&self, components: &[ComponentStatus]) -> HealthStatus {
        let healthy_count = components
            .iter()
            .filter(|c| c.status == HealthStatus::Healthy)
            .count();
        let failed_count = components
            .iter()
            .filter(|c| c.status == HealthStatus::Failed)
            .count();

        if failed_count > 0 {
            HealthStatus::Failed
        } else if healthy_count == components.len() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded
        }
    }

    fn compute_context_quality(&self, components: &[ComponentStatus]) -> ContextQuality {
        let operational_count = components
            .iter()
            .filter(|c| c.status.is_operational())
            .count();

        match operational_count {
            5 => ContextQuality::Excellent,
            4 => ContextQuality::Good,
            3 => ContextQuality::Acceptable,
            2 => ContextQuality::Degraded,
            1 => ContextQuality::Minimal,
            _ => ContextQuality::Emergency,
        }
    }

    fn store_health_check(&self, snapshot: &HealthSnapshot) -> Result<(), String> {
        let conn = self.conn.lock();

        for comp in &snapshot.components {
            let status_str = match comp.status {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Degraded => "degraded",
                HealthStatus::Failed => "failed",
                HealthStatus::Disabled => "disabled",
            };

            conn.execute(
                "INSERT INTO system_health (component, status, last_success, error_count, last_error)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(component) DO UPDATE SET
                    status = excluded.status,
                    last_success = COALESCE(excluded.last_success, system_health.last_success),
                    error_count = excluded.error_count,
                    last_error = excluded.last_error,
                    checked_at = datetime('now')",
                rusqlite::params![
                    comp.name,
                    status_str,
                    comp.last_success,
                    comp.error_count,
                    comp.last_error
                ],
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Get health history
    pub fn get_history(&self) -> Vec<HealthSnapshot> {
        self.health_history.iter().cloned().collect()
    }

    /// Get recent alerts
    pub fn get_recent_alerts(&self) -> Vec<HealthAlert> {
        self.health_history
            .iter()
            .rev()
            .take(10)
            .flat_map(|s| s.active_alerts.clone())
            .collect()
    }
}

// ============================================================================
// Graceful Degradation
// ============================================================================

/// Fallback chain for graceful degradation
pub struct FallbackChain {
    current_level: FallbackLevel,
    component_status: Vec<(String, bool)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackLevel {
    Full,        // All systems operational
    NoActivity,  // Activity tracker failed/disabled
    NoGit,       // Git analyzer unavailable
    NoFileWatch, // File watcher failed
    ManualOnly,  // Only explicit user input works
    Emergency,   // Everything failed, use defaults
}

impl FallbackLevel {
    pub fn context_quality(&self) -> ContextQuality {
        match self {
            FallbackLevel::Full => ContextQuality::Excellent,
            FallbackLevel::NoActivity => ContextQuality::Good,
            FallbackLevel::NoGit => ContextQuality::Acceptable,
            FallbackLevel::NoFileWatch => ContextQuality::Degraded,
            FallbackLevel::ManualOnly => ContextQuality::Minimal,
            FallbackLevel::Emergency => ContextQuality::Emergency,
        }
    }
}

impl FallbackChain {
    pub fn new() -> Self {
        Self {
            current_level: FallbackLevel::Full,
            component_status: Vec::new(),
        }
    }

    /// Update fallback level based on component status
    pub fn update_from_health(&mut self, snapshot: &HealthSnapshot) {
        self.component_status = snapshot
            .components
            .iter()
            .map(|c| (c.name.clone(), c.status.is_operational()))
            .collect();

        // Determine fallback level
        let project_scanner = self.is_operational("project_scanner");
        let file_watcher = self.is_operational("file_watcher");
        let git_analyzer = self.is_operational("git_analyzer");
        let behavior_learner = self.is_operational("behavior_learner");

        self.current_level = if project_scanner && file_watcher && git_analyzer && behavior_learner
        {
            FallbackLevel::Full
        } else if project_scanner && file_watcher && git_analyzer {
            FallbackLevel::NoActivity
        } else if project_scanner && file_watcher {
            FallbackLevel::NoGit
        } else if project_scanner {
            FallbackLevel::NoFileWatch
        } else if behavior_learner {
            FallbackLevel::ManualOnly
        } else {
            FallbackLevel::Emergency
        };
    }

    fn is_operational(&self, name: &str) -> bool {
        self.component_status
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, op)| *op)
            .unwrap_or(false)
    }

    /// Get current fallback level
    pub fn current_level(&self) -> FallbackLevel {
        self.current_level
    }

    /// Get context quality for current level
    pub fn context_quality(&self) -> ContextQuality {
        self.current_level.context_quality()
    }

    /// Get list of available features at current level
    pub fn available_features(&self) -> Vec<String> {
        let mut features = Vec::new();

        match self.current_level {
            FallbackLevel::Full => {
                features.push("Project manifest scanning".to_string());
                features.push("Real-time file watching".to_string());
                features.push("Git history analysis".to_string());
                features.push("Behavior learning".to_string());
                features.push("Activity tracking".to_string());
            }
            FallbackLevel::NoActivity => {
                features.push("Project manifest scanning".to_string());
                features.push("Real-time file watching".to_string());
                features.push("Git history analysis".to_string());
                features.push("Behavior learning".to_string());
            }
            FallbackLevel::NoGit => {
                features.push("Project manifest scanning".to_string());
                features.push("Real-time file watching".to_string());
                features.push("Behavior learning".to_string());
            }
            FallbackLevel::NoFileWatch => {
                features.push("Project manifest scanning".to_string());
                features.push("Behavior learning".to_string());
            }
            FallbackLevel::ManualOnly => {
                features.push("Behavior learning".to_string());
                features.push("Manual context input".to_string());
            }
            FallbackLevel::Emergency => {
                features.push("Default context only".to_string());
            }
        }

        features
    }
}

impl Default for FallbackChain {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Audit Trail
// ============================================================================

/// Audit logger for tracking all ACE decisions
pub struct AuditLogger {
    conn: Arc<Mutex<Connection>>,
    /// In-memory buffer for recent entries
    buffer: VecDeque<AuditEntry>,
    max_buffer: usize,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Option<i64>,
    pub timestamp: String,
    pub entry_type: AuditEntryType,
    pub action: String,
    pub reason: String,
    pub contributing_factors: Vec<String>,
    pub before_state: Option<String>,
    pub after_state: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEntryType {
    ContextUpdate,
    RelevanceDecision,
    ExclusionApplied,
    FeedbackReceived,
    AnomalyDetected,
    FallbackActivated,
    HealthCheck,
    ConfigChange,
}

impl AuditLogger {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            buffer: VecDeque::new(),
            max_buffer: 1000,
        }
    }

    /// Log an audit entry
    pub fn log(&mut self, entry: AuditEntry) -> Result<i64, String> {
        let conn = self.conn.lock();

        let entry_type_str = match entry.entry_type {
            AuditEntryType::ContextUpdate => "context_update",
            AuditEntryType::RelevanceDecision => "relevance_decision",
            AuditEntryType::ExclusionApplied => "exclusion_applied",
            AuditEntryType::FeedbackReceived => "feedback_received",
            AuditEntryType::AnomalyDetected => "anomaly_detected",
            AuditEntryType::FallbackActivated => "fallback_activated",
            AuditEntryType::HealthCheck => "health_check",
            AuditEntryType::ConfigChange => "config_change",
        };

        let factors_json = serde_json::to_string(&entry.contributing_factors).unwrap_or_default();

        conn.execute(
            "INSERT INTO audit_log (entry_type, action, reason, contributing_factors, before_state, after_state, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                entry_type_str,
                entry.action,
                entry.reason,
                factors_json,
                entry.before_state,
                entry.after_state,
                entry.confidence
            ],
        ).map_err(|e| format!("Failed to log audit entry: {}", e))?;

        let id = conn.last_insert_rowid();

        // Add to buffer
        let mut entry_with_id = entry;
        entry_with_id.id = Some(id);
        self.buffer.push_back(entry_with_id);
        if self.buffer.len() > self.max_buffer {
            self.buffer.pop_front();
        }

        Ok(id)
    }

    /// Log a context update
    pub fn log_context_update(
        &mut self,
        action: &str,
        topics: &[String],
        confidence: f32,
    ) -> Result<i64, String> {
        self.log(AuditEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            entry_type: AuditEntryType::ContextUpdate,
            action: action.to_string(),
            reason: "Context detected from signals".to_string(),
            contributing_factors: topics.to_vec(),
            before_state: None,
            after_state: Some(format!("{} topics", topics.len())),
            confidence,
        })
    }

    /// Log a relevance decision
    pub fn log_relevance_decision(
        &mut self,
        item_id: i64,
        score: f32,
        matches: &[String],
        confidence: f32,
    ) -> Result<i64, String> {
        self.log(AuditEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            entry_type: AuditEntryType::RelevanceDecision,
            action: format!("Scored item {} = {:.2}", item_id, score),
            reason: if score > 0.7 {
                "High relevance match".to_string()
            } else if score > 0.4 {
                "Moderate relevance".to_string()
            } else {
                "Low relevance".to_string()
            },
            contributing_factors: matches.to_vec(),
            before_state: None,
            after_state: Some(format!("score={:.2}", score)),
            confidence,
        })
    }

    /// Log an exclusion application
    pub fn log_exclusion(
        &mut self,
        topic: &str,
        strength: &str,
        item_id: Option<i64>,
    ) -> Result<i64, String> {
        self.log(AuditEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            entry_type: AuditEntryType::ExclusionApplied,
            action: format!("Applied {} exclusion for '{}'", strength, topic),
            reason: format!(
                "Item {} matched exclusion",
                item_id.map(|i| i.to_string()).unwrap_or_default()
            ),
            contributing_factors: vec![topic.to_string()],
            before_state: None,
            after_state: Some(format!("excluded={}", strength)),
            confidence: 1.0,
        })
    }

    /// Log an anomaly detection
    pub fn log_anomaly(
        &mut self,
        anomaly_type: &str,
        topic: &str,
        confidence: f32,
    ) -> Result<i64, String> {
        self.log(AuditEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            entry_type: AuditEntryType::AnomalyDetected,
            action: format!("Detected {} anomaly for '{}'", anomaly_type, topic),
            reason: "Signal validation detected unexpected pattern".to_string(),
            contributing_factors: vec![topic.to_string(), anomaly_type.to_string()],
            before_state: None,
            after_state: None,
            confidence,
        })
    }

    /// Log a fallback activation
    pub fn log_fallback(&mut self, level: FallbackLevel, reason: &str) -> Result<i64, String> {
        let level_str = match level {
            FallbackLevel::Full => "full",
            FallbackLevel::NoActivity => "no_activity",
            FallbackLevel::NoGit => "no_git",
            FallbackLevel::NoFileWatch => "no_file_watch",
            FallbackLevel::ManualOnly => "manual_only",
            FallbackLevel::Emergency => "emergency",
        };

        self.log(AuditEntry {
            id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            entry_type: AuditEntryType::FallbackActivated,
            action: format!("Activated {} fallback mode", level_str),
            reason: reason.to_string(),
            contributing_factors: vec![],
            before_state: None,
            after_state: Some(level_str.to_string()),
            confidence: 1.0,
        })
    }

    /// Get recent audit entries
    pub fn get_recent(&self, limit: usize) -> Vec<AuditEntry> {
        self.buffer.iter().rev().take(limit).cloned().collect()
    }

    /// Query audit log from database
    pub fn query(
        &self,
        entry_type: Option<AuditEntryType>,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, String> {
        let conn = self.conn.lock();

        let query = if let Some(et) = entry_type {
            let type_str = match et {
                AuditEntryType::ContextUpdate => "context_update",
                AuditEntryType::RelevanceDecision => "relevance_decision",
                AuditEntryType::ExclusionApplied => "exclusion_applied",
                AuditEntryType::FeedbackReceived => "feedback_received",
                AuditEntryType::AnomalyDetected => "anomaly_detected",
                AuditEntryType::FallbackActivated => "fallback_activated",
                AuditEntryType::HealthCheck => "health_check",
                AuditEntryType::ConfigChange => "config_change",
            };
            format!(
                "SELECT id, timestamp, entry_type, action, reason, contributing_factors, before_state, after_state, confidence
                 FROM audit_log WHERE entry_type = '{}' ORDER BY timestamp DESC LIMIT {}",
                type_str, limit
            )
        } else {
            format!(
                "SELECT id, timestamp, entry_type, action, reason, contributing_factors, before_state, after_state, confidence
                 FROM audit_log ORDER BY timestamp DESC LIMIT {}",
                limit
            )
        };

        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

        let entries = stmt
            .query_map([], |row| {
                let type_str: String = row.get(2)?;
                let factors_json: String = row.get(5)?;

                Ok(AuditEntry {
                    id: Some(row.get(0)?),
                    timestamp: row.get(1)?,
                    entry_type: match type_str.as_str() {
                        "context_update" => AuditEntryType::ContextUpdate,
                        "relevance_decision" => AuditEntryType::RelevanceDecision,
                        "exclusion_applied" => AuditEntryType::ExclusionApplied,
                        "feedback_received" => AuditEntryType::FeedbackReceived,
                        "anomaly_detected" => AuditEntryType::AnomalyDetected,
                        "fallback_activated" => AuditEntryType::FallbackActivated,
                        "health_check" => AuditEntryType::HealthCheck,
                        _ => AuditEntryType::ConfigChange,
                    },
                    action: row.get(3)?,
                    reason: row.get(4)?,
                    contributing_factors: serde_json::from_str(&factors_json).unwrap_or_default(),
                    before_state: row.get(6)?,
                    after_state: row.get(7)?,
                    confidence: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;

        entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Explain a relevance decision for an item
    pub fn explain_decision(&self, item_id: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock();

        let result: Result<(String, String, String, f32), _> = conn.query_row(
            "SELECT action, reason, contributing_factors, confidence FROM audit_log
             WHERE entry_type = 'relevance_decision' AND action LIKE ?
             ORDER BY timestamp DESC LIMIT 1",
            [format!("%item {}%", item_id)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        );

        match result {
            Ok((action, reason, factors, confidence)) => {
                let factors_vec: Vec<String> = serde_json::from_str(&factors).unwrap_or_default();
                Ok(Some(format!(
                    "Decision: {}\nReason: {}\nFactors: {}\nConfidence: {:.0}%",
                    action,
                    reason,
                    factors_vec.join(", "),
                    confidence * 100.0
                )))
            }
            Err(_) => Ok(None),
        }
    }
}

// ============================================================================
// Accuracy Metrics
// ============================================================================

/// Accuracy metrics tracker
pub struct AccuracyTracker {
    conn: Arc<Mutex<Connection>>,
    /// Recent results for sliding window
    recent_results: VecDeque<FeedbackResult>,
    window_size: usize,
    /// Cumulative counters
    total_shown: u64,
    total_clicked: u64,
    total_positive: u64,
    total_negative: u64,
}

/// Result of user feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResult {
    pub item_id: i64,
    pub predicted_score: f32,
    pub feedback: FeedbackType,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    Click,
    Save,
    Share,
    ThumbsUp,
    ThumbsDown,
    Dismiss,
    Ignore,
}

impl FeedbackType {
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            FeedbackType::Click | FeedbackType::Save | FeedbackType::Share | FeedbackType::ThumbsUp
        )
    }

    pub fn is_negative(&self) -> bool {
        matches!(self, FeedbackType::ThumbsDown | FeedbackType::Dismiss)
    }
}

/// Computed accuracy metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyMetrics {
    pub precision: f32,
    pub engagement_rate: f32,
    pub positive_ratio: f32,
    pub calibration_error: f32,
    pub window_size: usize,
    pub total_shown: u64,
    pub total_clicked: u64,
    pub meets_targets: bool,
}

/// Target accuracy thresholds
pub const ACE_TARGETS: AccuracyTargets = AccuracyTargets {
    min_precision: 0.85,
    min_engagement: 0.30,
    max_calibration_error: 0.10,
};

pub struct AccuracyTargets {
    pub min_precision: f32,
    pub min_engagement: f32,
    pub max_calibration_error: f32,
}

impl AccuracyTracker {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            recent_results: VecDeque::new(),
            window_size: 100,
            total_shown: 0,
            total_clicked: 0,
            total_positive: 0,
            total_negative: 0,
        }
    }

    /// Record feedback for an item
    pub fn record_feedback(&mut self, result: FeedbackResult) {
        // Update counters
        self.total_shown += 1;
        if result.feedback == FeedbackType::Click {
            self.total_clicked += 1;
        }
        if result.feedback.is_positive() {
            self.total_positive += 1;
        }
        if result.feedback.is_negative() {
            self.total_negative += 1;
        }

        // Add to sliding window
        self.recent_results.push_back(result);
        if self.recent_results.len() > self.window_size {
            self.recent_results.pop_front();
        }
    }

    /// Compute current accuracy metrics
    pub fn compute_metrics(&self) -> AccuracyMetrics {
        let window: Vec<_> = self.recent_results.iter().collect();

        // Precision: positive feedback / total shown
        let precision = if window.is_empty() {
            0.0
        } else {
            window.iter().filter(|r| r.feedback.is_positive()).count() as f32 / window.len() as f32
        };

        // Engagement rate: clicks / shown
        let engagement_rate = if self.total_shown > 0 {
            self.total_clicked as f32 / self.total_shown as f32
        } else {
            0.0
        };

        // Positive ratio: positive / (positive + negative)
        let positive_ratio = if self.total_positive + self.total_negative > 0 {
            self.total_positive as f32 / (self.total_positive + self.total_negative) as f32
        } else {
            0.5
        };

        // Calibration error: average difference between predicted score and actual outcome
        let calibration_error = if window.is_empty() {
            0.0
        } else {
            let sum: f32 = window
                .iter()
                .map(|r| {
                    let actual = if r.feedback.is_positive() { 1.0 } else { 0.0 };
                    (r.predicted_score - actual).abs()
                })
                .sum();
            sum / window.len() as f32
        };

        let meets_targets = precision >= ACE_TARGETS.min_precision
            && engagement_rate >= ACE_TARGETS.min_engagement
            && calibration_error <= ACE_TARGETS.max_calibration_error;

        AccuracyMetrics {
            precision,
            engagement_rate,
            positive_ratio,
            calibration_error,
            window_size: window.len(),
            total_shown: self.total_shown,
            total_clicked: self.total_clicked,
            meets_targets,
        }
    }

    /// Check for accuracy alerts
    pub fn check_alerts(&self) -> Vec<HealthAlert> {
        let metrics = self.compute_metrics();
        let mut alerts = Vec::new();

        if metrics.precision < ACE_TARGETS.min_precision && self.total_shown >= 20 {
            alerts.push(HealthAlert {
                alert_type: AlertType::AccuracyDrop,
                component: "accuracy".to_string(),
                message: format!(
                    "Precision {:.0}% below target {:.0}%",
                    metrics.precision * 100.0,
                    ACE_TARGETS.min_precision * 100.0
                ),
                severity: AlertSeverity::Warning,
                timestamp: chrono::Utc::now().to_rfc3339(),
                recommended_action: "Review recent context and consider retraining".to_string(),
            });
        }

        if metrics.engagement_rate < ACE_TARGETS.min_engagement && self.total_shown >= 20 {
            alerts.push(HealthAlert {
                alert_type: AlertType::AccuracyDrop,
                component: "engagement".to_string(),
                message: format!(
                    "Engagement {:.0}% below target {:.0}%",
                    metrics.engagement_rate * 100.0,
                    ACE_TARGETS.min_engagement * 100.0
                ),
                severity: AlertSeverity::Warning,
                timestamp: chrono::Utc::now().to_rfc3339(),
                recommended_action: "Adjust relevance thresholds or improve context".to_string(),
            });
        }

        alerts
    }

    /// Persist metrics to database
    pub fn persist_metrics(&self) -> Result<(), String> {
        let metrics = self.compute_metrics();
        let conn = self.conn.lock();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        conn.execute(
            "INSERT INTO accuracy_metrics (metric_date, precision_score, engagement_rate, items_shown, items_clicked, positive_feedback, negative_feedback)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(metric_date) DO UPDATE SET
                precision_score = excluded.precision_score,
                engagement_rate = excluded.engagement_rate,
                items_shown = excluded.items_shown,
                items_clicked = excluded.items_clicked,
                positive_feedback = excluded.positive_feedback,
                negative_feedback = excluded.negative_feedback",
            rusqlite::params![
                today,
                metrics.precision,
                metrics.engagement_rate,
                self.total_shown,
                self.total_clicked,
                self.total_positive,
                self.total_negative
            ],
        ).map_err(|e| format!("Failed to persist metrics: {}", e))?;

        Ok(())
    }

    /// Load historical metrics
    pub fn get_history(&self, days: u32) -> Result<Vec<DailyMetrics>, String> {
        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare(
                "SELECT metric_date, precision_score, engagement_rate, items_shown, items_clicked
             FROM accuracy_metrics
             WHERE metric_date >= date('now', ?1)
             ORDER BY metric_date DESC",
            )
            .map_err(|e| e.to_string())?;

        let days_str = format!("-{} days", days);
        let rows = stmt
            .query_map([days_str], |row| {
                Ok(DailyMetrics {
                    date: row.get(0)?,
                    precision: row.get(1)?,
                    engagement: row.get(2)?,
                    items_shown: row.get(3)?,
                    items_clicked: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

/// Daily metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMetrics {
    pub date: String,
    pub precision: f32,
    pub engagement: f32,
    pub items_shown: u64,
    pub items_clicked: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE detected_projects (id INTEGER PRIMARY KEY, updated_at TEXT);
            CREATE TABLE file_signals (id INTEGER PRIMARY KEY, timestamp TEXT);
            CREATE TABLE git_signals (id INTEGER PRIMARY KEY, timestamp TEXT);
            CREATE TABLE interactions (id INTEGER PRIMARY KEY, timestamp TEXT);
            CREATE TABLE active_topics (id INTEGER PRIMARY KEY, last_seen TEXT);
            CREATE TABLE system_health (
                component TEXT PRIMARY KEY,
                status TEXT,
                last_success TEXT,
                error_count INTEGER,
                last_error TEXT,
                checked_at TEXT
            );
            CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT,
                entry_type TEXT,
                action TEXT,
                reason TEXT,
                contributing_factors TEXT,
                before_state TEXT,
                after_state TEXT,
                confidence REAL
            );
            CREATE TABLE accuracy_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric_date TEXT UNIQUE,
                precision_score REAL,
                engagement_rate REAL,
                items_shown INTEGER,
                items_clicked INTEGER,
                positive_feedback INTEGER,
                negative_feedback INTEGER
            );
        "#,
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_health_check() {
        let conn = setup_test_db();
        let mut monitor = HealthMonitor::new(conn);

        let snapshot = monitor.check_health();
        assert!(snapshot.components.len() >= 4);
    }

    #[test]
    fn test_fallback_chain() {
        let mut chain = FallbackChain::new();
        assert_eq!(chain.current_level(), FallbackLevel::Full);

        // Simulate degraded state
        let snapshot = HealthSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            components: vec![
                ComponentStatus {
                    name: "project_scanner".to_string(),
                    status: HealthStatus::Healthy,
                    last_success: None,
                    last_error: None,
                    error_count: 0,
                    metrics: ComponentMetrics::default(),
                },
                ComponentStatus {
                    name: "file_watcher".to_string(),
                    status: HealthStatus::Failed,
                    last_success: None,
                    last_error: Some("Error".to_string()),
                    error_count: 5,
                    metrics: ComponentMetrics::default(),
                },
            ],
            overall_status: HealthStatus::Degraded,
            context_quality: ContextQuality::Degraded,
            active_alerts: vec![],
        };

        chain.update_from_health(&snapshot);
        assert_eq!(chain.current_level(), FallbackLevel::NoFileWatch);
    }

    #[test]
    fn test_audit_logger() {
        let conn = setup_test_db();
        let mut logger = AuditLogger::new(conn);

        let id = logger.log_context_update("Detected Rust project", &["rust".to_string()], 0.9);
        assert!(id.is_ok());

        let recent = logger.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].entry_type, AuditEntryType::ContextUpdate);
    }

    #[test]
    fn test_accuracy_tracker() {
        let conn = setup_test_db();
        let mut tracker = AccuracyTracker::new(conn);

        // Record some feedback
        for i in 0..10 {
            tracker.record_feedback(FeedbackResult {
                item_id: i,
                predicted_score: 0.8,
                feedback: if i % 2 == 0 {
                    FeedbackType::Click
                } else {
                    FeedbackType::Ignore
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        let metrics = tracker.compute_metrics();
        assert_eq!(metrics.window_size, 10);
        assert!(metrics.precision > 0.0);
    }

    #[test]
    fn test_context_quality_accuracy() {
        assert_eq!(ContextQuality::Excellent.expected_accuracy(), 0.95);
        assert_eq!(ContextQuality::Good.expected_accuracy(), 0.85);
        assert_eq!(ContextQuality::Emergency.expected_accuracy(), 0.30);
    }
}
