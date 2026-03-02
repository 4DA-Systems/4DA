//! File Watcher - Real-time context updates
//!
//! Watches configured directories for file changes and triggers context updates.
//! Uses debouncing to prevent excessive processing during rapid saves.
//! Includes state persistence for restart recovery.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// File watcher configuration
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce duration (ms) - wait this long after last change before processing
    pub debounce_ms: u64,
    /// Batch size - process this many files at once
    pub batch_size: usize,
    /// File extensions to watch
    pub watched_extensions: HashSet<String>,
    /// Directories to skip
    pub skip_dirs: HashSet<String>,
    /// Maximum file size to process (bytes)
    pub max_file_size: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        let mut watched_extensions = HashSet::new();
        // Code files
        for ext in [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h", "hpp",
        ] {
            watched_extensions.insert(ext.to_string());
        }
        // Config/docs
        for ext in ["md", "txt", "json", "toml", "yaml", "yml"] {
            watched_extensions.insert(ext.to_string());
        }
        // Documents (Phase 1 extractors)
        for ext in ["pdf", "docx", "xlsx"] {
            watched_extensions.insert(ext.to_string());
        }
        // Archives (Phase 1 extractors)
        for ext in ["zip", "tar", "gz", "tgz"] {
            watched_extensions.insert(ext.to_string());
        }

        let mut skip_dirs = HashSet::new();
        for dir in [
            "node_modules",
            "target",
            ".git",
            "dist",
            "build",
            ".next",
            "__pycache__",
            ".venv",
            "venv",
            "vendor",
            ".cargo",
            "pkg",
            ".idea",
            ".vscode",
            "coverage",
            ".nyc_output",
        ] {
            skip_dirs.insert(dir.to_string());
        }

        Self {
            debounce_ms: 500,
            batch_size: 50,
            watched_extensions,
            skip_dirs,
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// A file change event (debounced)
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

/// Callback type for file change notifications
pub type ChangeCallback = Box<dyn Fn(Vec<FileChange>) + Send + Sync>;

/// The file watcher manager
pub struct FileWatcher {
    config: WatcherConfig,
    watcher: Option<RecommendedWatcher>,
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    pending_changes: Arc<Mutex<HashMap<PathBuf, FileChange>>>,
    last_batch_time: Arc<Mutex<Instant>>,
    callback: Arc<Mutex<Option<ChangeCallback>>>,
    running: Arc<Mutex<bool>>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(config: WatcherConfig) -> Self {
        Self {
            config,
            watcher: None,
            watched_paths: Arc::new(Mutex::new(HashSet::new())),
            pending_changes: Arc::new(Mutex::new(HashMap::new())),
            last_batch_time: Arc::new(Mutex::new(Instant::now())),
            callback: Arc::new(Mutex::new(None)),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Set the callback for file changes
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<FileChange>) + Send + Sync + 'static,
    {
        *self.callback.lock() = Some(Box::new(callback));
    }

    /// Start watching a directory
    pub fn watch(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }

        // Create watcher if not exists
        if self.watcher.is_none() {
            self.start_watcher()?;
        }

        if let Some(ref mut watcher) = self.watcher {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| format!("Failed to watch {}: {}", path.display(), e))?;

            self.watched_paths.lock().insert(path.to_path_buf());
            info!(target: "ace::watcher", path = %path.display(), "Watching");
        }

        Ok(())
    }

    /// Stop watching a directory
    pub fn unwatch(&mut self, path: &Path) -> Result<(), String> {
        if let Some(ref mut watcher) = self.watcher {
            watcher
                .unwatch(path)
                .map_err(|e| format!("Failed to unwatch {}: {}", path.display(), e))?;

            self.watched_paths.lock().remove(path);
            info!(target: "ace::watcher", path = %path.display(), "Stopped watching");
        }

        Ok(())
    }

    /// Start the internal watcher
    fn start_watcher(&mut self) -> Result<(), String> {
        let pending_changes = self.pending_changes.clone();
        let config = self.config.clone();
        let callback = self.callback.clone();
        let last_batch_time = self.last_batch_time.clone();
        let running = self.running.clone();

        // Create channel for events
        let (tx, rx): (Sender<Result<Event, notify::Error>>, Receiver<_>) = channel();

        // Create the watcher
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .map_err(|e| format!("Failed to create watcher: {}", e))?;

        self.watcher = Some(watcher);
        *running.lock() = true;

        // Spawn event processing thread
        std::thread::spawn(move || {
            let debounce_duration = Duration::from_millis(config.debounce_ms);

            loop {
                if !*running.lock() {
                    break;
                }

                // Check for new events with timeout
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(event)) => {
                        Self::process_event(&event, &pending_changes, &config);
                    }
                    Ok(Err(e)) => {
                        warn!(target: "ace::watcher", error = ?e, "Watcher error");
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Check if we should flush pending changes
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }

                // Check if we should flush pending changes
                let should_flush = {
                    let pending = pending_changes.lock();
                    let last_time = *last_batch_time.lock();

                    !pending.is_empty() && last_time.elapsed() >= debounce_duration
                };

                if should_flush {
                    let changes: Vec<FileChange> = {
                        let mut pending = pending_changes.lock();
                        let changes: Vec<_> = pending.values().cloned().collect();
                        pending.clear();
                        changes
                    };

                    *last_batch_time.lock() = Instant::now();

                    if !changes.is_empty() {
                        if let Some(ref cb) = *callback.lock() {
                            cb(changes);
                        }
                    }
                }
            }

            debug!(target: "ace::watcher", "Event processing thread stopped");
        });

        info!(target: "ace::watcher", "Started");
        Ok(())
    }

    /// Process a single event
    fn process_event(
        event: &Event,
        pending_changes: &Arc<Mutex<HashMap<PathBuf, FileChange>>>,
        config: &WatcherConfig,
    ) {
        let change_type = match event.kind {
            EventKind::Create(_) => FileChangeType::Created,
            EventKind::Modify(_) => FileChangeType::Modified,
            EventKind::Remove(_) => FileChangeType::Deleted,
            _ => return, // Ignore other events
        };

        for path in &event.paths {
            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Check extension
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !config.watched_extensions.contains(ext) {
                    continue;
                }
            } else {
                continue;
            }

            // Check if in skip directory
            let path_str = path.to_string_lossy();
            if config.skip_dirs.iter().any(|skip| path_str.contains(skip)) {
                continue;
            }

            // Check file size for non-delete events
            if change_type != FileChangeType::Deleted {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if metadata.len() > config.max_file_size {
                        continue;
                    }
                }
            }

            // Add to pending changes (overwrites previous for same path)
            let change = FileChange {
                path: path.clone(),
                change_type,
                timestamp: Instant::now(),
            };

            let mut pending = pending_changes.lock();
            // Cap pending changes to prevent memory exhaustion from mass file events
            if pending.len() >= 10_000 && !pending.contains_key(path) {
                // At capacity with new path — skip to prevent unbounded growth
                continue;
            }
            pending.insert(path.clone(), change);
        }
    }

    /// Stop the watcher
    pub fn stop(&mut self) {
        *self.running.lock() = false;
        self.watcher = None;
        self.watched_paths.lock().clear();
        info!(target: "ace::watcher", "Stopped");
    }

    /// Get list of watched paths
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.lock().iter().cloned().collect()
    }

    /// Check if currently watching any paths
    pub fn is_watching(&self) -> bool {
        !self.watched_paths.lock().is_empty()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// Topic Extraction from File Content
// ============================================================================

/// Extract topics from a code file's content
pub fn extract_topics_from_file(path: &Path) -> Result<Vec<String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    Ok(extract_topics_from_content(&content, ext))
}

/// Extract topics from content based on file type
pub fn extract_topics_from_content(content: &str, file_ext: &str) -> Vec<String> {
    let mut topics = HashSet::new();

    match file_ext {
        "rs" => extract_rust_topics(content, &mut topics),
        "ts" | "tsx" | "js" | "jsx" => extract_js_topics(content, &mut topics),
        "py" => extract_python_topics(content, &mut topics),
        "go" => extract_go_topics(content, &mut topics),
        _ => extract_generic_topics(content, &mut topics),
    }

    topics.into_iter().collect()
}

fn extract_rust_topics(content: &str, topics: &mut HashSet<String>) {
    // Extract use statements
    for line in content.lines() {
        let trimmed = line.trim();

        // use statements
        if trimmed.starts_with("use ") {
            if let Some(crate_name) = trimmed
                .strip_prefix("use ")
                .and_then(|s| s.split("::").next())
                .map(|s| s.trim_end_matches(';'))
            {
                if !["std", "core", "alloc", "self", "super", "crate"].contains(&crate_name) {
                    topics.insert(crate_name.to_string());
                }
            }
        }

        // extern crate
        if trimmed.starts_with("extern crate ") {
            if let Some(crate_name) = trimmed
                .strip_prefix("extern crate ")
                .and_then(|s| s.split(&[';', ' '][..]).next())
            {
                topics.insert(crate_name.to_string());
            }
        }
    }

    // Common Rust patterns
    if content.contains("async fn") || content.contains("async move") {
        topics.insert("async".to_string());
    }
    if content.contains("#[tokio::") {
        topics.insert("tokio".to_string());
    }
    if content.contains("#[derive(") && content.contains("Serialize") {
        topics.insert("serde".to_string());
    }
}

fn extract_js_topics(content: &str, topics: &mut HashSet<String>) {
    // Extract imports
    for line in content.lines() {
        let trimmed = line.trim();

        // ES6 imports
        if trimmed.starts_with("import ") {
            if let Some(from_idx) = trimmed.find(" from ") {
                let module = &trimmed[from_idx + 7..];
                let module = module.trim_matches(&['"', '\'', ';'][..]);
                if !module.starts_with('.') && !module.starts_with('/') {
                    // External module
                    let base_module = module.split('/').next().unwrap_or(module);
                    let base_module = base_module.trim_start_matches('@');
                    topics.insert(base_module.to_string());
                }
            }
        }

        // require()
        if trimmed.contains("require(") {
            if let Some(start) = trimmed.find("require(") {
                let rest = &trimmed[start + 8..];
                if let Some(end) = rest.find(')') {
                    let module = rest[..end].trim_matches(&['"', '\''][..]);
                    if !module.starts_with('.') && !module.starts_with('/') {
                        let base_module = module.split('/').next().unwrap_or(module);
                        topics.insert(base_module.to_string());
                    }
                }
            }
        }
    }

    // React detection
    if content.contains("React") || content.contains("useState") || content.contains("useEffect") {
        topics.insert("react".to_string());
    }
    if content.contains("Vue") || content.contains("defineComponent") {
        topics.insert("vue".to_string());
    }
}

fn extract_python_topics(content: &str, topics: &mut HashSet<String>) {
    for line in content.lines() {
        let trimmed = line.trim();

        // import statements
        if trimmed.starts_with("import ") {
            let module = trimmed
                .strip_prefix("import ")
                .unwrap_or("")
                .split(&[' ', ',', '.'][..])
                .next()
                .unwrap_or("");
            if !module.is_empty() {
                topics.insert(module.to_string());
            }
        }

        // from X import Y
        if trimmed.starts_with("from ") {
            if let Some(module) = trimmed
                .strip_prefix("from ")
                .and_then(|s| s.split(' ').next())
                .map(|s| s.split('.').next().unwrap_or(s))
            {
                if !module.is_empty() && module != "." {
                    topics.insert(module.to_string());
                }
            }
        }
    }
}

fn extract_go_topics(content: &str, topics: &mut HashSet<String>) {
    let mut in_import = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "import (" {
            in_import = true;
            continue;
        } else if trimmed == ")" && in_import {
            in_import = false;
            continue;
        }

        if in_import || trimmed.starts_with("import ") {
            let import_path = trimmed
                .trim_start_matches("import ")
                .trim_matches(&['"', '`', ' '][..]);

            if !import_path.is_empty() {
                // Extract the last part of the path or known frameworks
                let parts: Vec<&str> = import_path.split('/').collect();
                if let Some(last) = parts.last() {
                    topics.insert(last.to_string());
                }

                // Detect common frameworks
                if import_path.contains("gin-gonic") {
                    topics.insert("gin".to_string());
                }
                if import_path.contains("gorilla") {
                    topics.insert("gorilla".to_string());
                }
            }
        }
    }
}

fn extract_generic_topics(content: &str, topics: &mut HashSet<String>) {
    // Look for common tech keywords
    let keywords = [
        "docker",
        "kubernetes",
        "k8s",
        "aws",
        "gcp",
        "azure",
        "terraform",
        "graphql",
        "rest",
        "grpc",
        "websocket",
        "redis",
        "postgres",
        "mysql",
        "mongodb",
        "elasticsearch",
        "kafka",
        "rabbitmq",
    ];

    let content_lower = content.to_lowercase();
    for keyword in keywords {
        if content_lower.contains(keyword) {
            topics.insert(keyword.to_string());
        }
    }
}

// ============================================================================
// State Persistence
// ============================================================================

/// Persisted watcher state for restart recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherState {
    pub watched_paths: Vec<PathBuf>,
    pub last_active: String,
    pub config: WatcherConfigSerializable,
}

/// Serializable version of WatcherConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfigSerializable {
    pub debounce_ms: u64,
    pub batch_size: usize,
    pub watched_extensions: Vec<String>,
    pub skip_dirs: Vec<String>,
    pub max_file_size: u64,
}

impl From<&WatcherConfig> for WatcherConfigSerializable {
    fn from(config: &WatcherConfig) -> Self {
        Self {
            debounce_ms: config.debounce_ms,
            batch_size: config.batch_size,
            watched_extensions: config.watched_extensions.iter().cloned().collect(),
            skip_dirs: config.skip_dirs.iter().cloned().collect(),
            max_file_size: config.max_file_size,
        }
    }
}

impl From<WatcherConfigSerializable> for WatcherConfig {
    fn from(config: WatcherConfigSerializable) -> Self {
        Self {
            debounce_ms: config.debounce_ms,
            batch_size: config.batch_size,
            watched_extensions: config.watched_extensions.into_iter().collect(),
            skip_dirs: config.skip_dirs.into_iter().collect(),
            max_file_size: config.max_file_size,
        }
    }
}

/// State persistence manager
pub struct WatcherStatePersistence {
    conn: Arc<Mutex<Connection>>,
}

impl WatcherStatePersistence {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self, String> {
        // Initialize state table
        let c = conn.lock();
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS watcher_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                state_json TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )
        .map_err(|e| format!("Failed to create watcher_state table: {}", e))?;
        drop(c);

        Ok(Self { conn })
    }

    /// Save watcher state
    pub fn save(&self, watcher: &FileWatcher) -> Result<(), String> {
        let state = WatcherState {
            watched_paths: watcher.watched_paths(),
            last_active: chrono::Utc::now().to_rfc3339(),
            config: WatcherConfigSerializable::from(&watcher.config),
        };

        let json = serde_json::to_string(&state)
            .map_err(|e| format!("Failed to serialize watcher state: {}", e))?;

        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO watcher_state (id, state_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET state_json = excluded.state_json, updated_at = datetime('now')",
            [&json],
        ).map_err(|e| format!("Failed to save watcher state: {}", e))?;

        Ok(())
    }

    /// Load watcher state
    pub fn load(&self) -> Result<Option<WatcherState>, String> {
        let conn = self.conn.lock();
        let result: Result<String, _> = conn.query_row(
            "SELECT state_json FROM watcher_state WHERE id = 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(json) => {
                let state: WatcherState = serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to deserialize watcher state: {}", e))?;
                Ok(Some(state))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to load watcher state: {}", e)),
        }
    }

    /// Restore watcher from saved state
    pub fn restore(&self, watcher: &mut FileWatcher) -> Result<usize, String> {
        let state = self.load()?;

        if let Some(state) = state {
            let mut restored = 0;
            for path in state.watched_paths {
                if path.exists() && watcher.watch(&path).is_ok() {
                    restored += 1;
                }
            }
            info!(
                target: "ace::watcher",
                restored = restored,
                "Restored watched paths from saved state"
            );
            Ok(restored)
        } else {
            Ok(0)
        }
    }

    /// Clear saved state
    pub fn clear(&self) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM watcher_state WHERE id = 1", [])
            .map_err(|e| format!("Failed to clear watcher state: {}", e))?;
        Ok(())
    }
}

// ============================================================================
// Rate Limiter
// ============================================================================

/// Rate limiter for controlling request frequency
pub struct RateLimiter {
    /// Maximum requests per window
    max_requests: u32,
    /// Window size in seconds
    window_seconds: u64,
    /// Request timestamps
    requests: Mutex<Vec<Instant>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Check if a request is allowed (returns true if allowed)
    pub fn check(&self) -> bool {
        let mut requests = self.requests.lock();
        let now = Instant::now();
        let window = Duration::from_secs(self.window_seconds);

        // Remove old requests outside the window
        requests.retain(|t| now.duration_since(*t) < window);

        // Check if we're within limit
        if requests.len() < self.max_requests as usize {
            requests.push(now);
            true
        } else {
            false
        }
    }

    /// Record a request without checking (for manual tracking)
    pub fn record(&self) {
        let mut requests = self.requests.lock();
        requests.push(Instant::now());
    }

    /// Get remaining requests in current window
    pub fn remaining(&self) -> u32 {
        let requests = self.requests.lock();
        let now = Instant::now();
        let window = Duration::from_secs(self.window_seconds);

        let active_requests = requests
            .iter()
            .filter(|t| now.duration_since(**t) < window)
            .count() as u32;

        self.max_requests.saturating_sub(active_requests)
    }

    /// Get time until next request is allowed (0 if allowed now)
    pub fn time_until_available(&self) -> Duration {
        if self.check() {
            Duration::ZERO
        } else {
            let requests = self.requests.lock();
            if let Some(oldest) = requests.first() {
                let window = Duration::from_secs(self.window_seconds);
                let elapsed = Instant::now().duration_since(*oldest);
                if elapsed < window {
                    window - elapsed
                } else {
                    Duration::ZERO
                }
            } else {
                Duration::ZERO
            }
        }
    }

    /// Reset the rate limiter
    pub fn reset(&self) {
        self.requests.lock().clear();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        // Default: 100 requests per minute
        Self::new(100, 60)
    }
}

/// Rate limiter specifically for ACE interactions
pub struct InteractionRateLimiter {
    /// Per-source rate limiters
    source_limiters: Mutex<HashMap<String, RateLimiter>>,
    /// Global rate limiter
    global: RateLimiter,
    /// Default per-source limit
    default_source_limit: u32,
    /// Window size in seconds
    window_seconds: u64,
}

impl InteractionRateLimiter {
    pub fn new(global_limit: u32, source_limit: u32, window_seconds: u64) -> Self {
        Self {
            source_limiters: Mutex::new(HashMap::new()),
            global: RateLimiter::new(global_limit, window_seconds),
            default_source_limit: source_limit,
            window_seconds,
        }
    }

    /// Check if an interaction is allowed
    pub fn check(&self, source: &str) -> bool {
        // Check global limit first
        if !self.global.check() {
            return false;
        }

        // Check per-source limit
        let mut limiters = self.source_limiters.lock();
        let limiter = limiters
            .entry(source.to_string())
            .or_insert_with(|| RateLimiter::new(self.default_source_limit, self.window_seconds));

        limiter.check()
    }

    /// Get rate limit status
    pub fn status(&self, source: &str) -> RateLimitStatus {
        let global_remaining = self.global.remaining();

        let source_remaining = {
            let limiters = self.source_limiters.lock();
            limiters
                .get(source)
                .map(|l| l.remaining())
                .unwrap_or(self.default_source_limit)
        };

        RateLimitStatus {
            global_remaining,
            source_remaining,
            is_limited: global_remaining == 0 || source_remaining == 0,
        }
    }
}

impl Default for InteractionRateLimiter {
    fn default() -> Self {
        // Default: 1000 global, 100 per source, per minute
        Self::new(1000, 100, 60)
    }
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub global_remaining: u32,
    pub source_remaining: u32,
    pub is_limited: bool,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_topics() {
        let content = r#"
use tokio::runtime::Runtime;
use serde::{Serialize, Deserialize};
use reqwest::Client;

async fn main() {
    println!("Hello");
}
"#;
        let topics = extract_topics_from_content(content, "rs");
        assert!(topics.contains(&"tokio".to_string()));
        assert!(topics.contains(&"serde".to_string()));
        assert!(topics.contains(&"reqwest".to_string()));
        assert!(topics.contains(&"async".to_string()));
    }

    #[test]
    fn test_extract_js_topics() {
        let content = r#"
import React, { useState } from 'react';
import axios from 'axios';
const lodash = require('lodash');
"#;
        let topics = extract_topics_from_content(content, "ts");
        assert!(topics.contains(&"react".to_string()));
        assert!(topics.contains(&"axios".to_string()));
        assert!(topics.contains(&"lodash".to_string()));
    }

    #[test]
    fn test_extract_python_topics() {
        let content = r#"
import pandas as pd
import numpy as np
from flask import Flask, request
from sqlalchemy.orm import Session
"#;
        let topics = extract_topics_from_content(content, "py");
        assert!(topics.contains(&"pandas".to_string()));
        assert!(topics.contains(&"numpy".to_string()));
        assert!(topics.contains(&"flask".to_string()));
        assert!(topics.contains(&"sqlalchemy".to_string()));
    }

    #[test]
    fn test_watcher_config_defaults() {
        let config = WatcherConfig::default();
        assert!(config.watched_extensions.contains("rs"));
        assert!(config.watched_extensions.contains("ts"));
        assert!(config.skip_dirs.contains("node_modules"));
        assert!(config.skip_dirs.contains("target"));
    }
}
