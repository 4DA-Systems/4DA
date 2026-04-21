// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

use crate::error::{Result, ResultExt};

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
    #[allow(dead_code)] // Reason: field populated by watcher events but not yet read
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
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()).into());
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
    #[allow(dead_code)] // Reason: FileWatcher public API, not yet called from ACE orchestrator
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
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
    fn start_watcher(&mut self) -> Result<()> {
        let pending_changes = self.pending_changes.clone();
        let config = self.config.clone();
        let callback = self.callback.clone();
        let last_batch_time = self.last_batch_time.clone();
        let running = self.running.clone();

        // Create channel for events
        let (tx, rx): (
            Sender<std::result::Result<Event, notify::Error>>,
            Receiver<_>,
        ) = channel();

        // Create the watcher
        let watcher = RecommendedWatcher::new(
            move |res| {
                if let Err(e) = tx.send(res) {
                    tracing::warn!("Channel send failed: {e}");
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .context("Failed to create watcher")?;

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
                // Single lock acquisition for check + drain to avoid TOCTOU race
                let changes_to_flush: Option<Vec<FileChange>> = {
                    let mut pending = pending_changes.lock();
                    let last_time = *last_batch_time.lock();
                    if !pending.is_empty() && last_time.elapsed() >= debounce_duration {
                        let changes: Vec<_> = pending.values().cloned().collect();
                        pending.clear();
                        Some(changes)
                    } else {
                        None
                    }
                };
                // Locks released BEFORE callback

                if let Some(changes) = changes_to_flush {
                    *last_batch_time.lock() = Instant::now();
                    if let Some(ref cb) = *callback.lock() {
                        cb(changes);
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
    #[allow(dead_code)] // Reason: FileWatcher public API, called by ACE::is_watching but ACE method itself is unused
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
pub fn extract_topics_from_file(path: &Path) -> Result<Vec<String>> {
    let metadata =
        std::fs::metadata(path).map_err(|e| format!("Failed to stat {}: {}", path.display(), e))?;
    if metadata.len() > 10_000_000 {
        return Ok(Vec::new());
    }
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
// Rich Topic Extraction — deeper semantic analysis of file content
// ============================================================================

/// Extract richer semantic topics from file content beyond basic imports.
/// Returns (topic, confidence) pairs. Supplements the basic extract_topics_from_content().
pub fn extract_rich_topics(content: &str, file_ext: &str) -> Vec<(String, f32)> {
    let mut topics: Vec<(String, f32)> = Vec::new();

    match file_ext {
        "rs" => extract_rich_rust(content, &mut topics),
        "ts" | "tsx" | "js" | "jsx" => extract_rich_js(content, &mut topics),
        "py" => extract_rich_python(content, &mut topics),
        _ => {}
    }

    // Dedup by topic name, keep highest confidence
    topics.sort_by(|a, b| a.0.cmp(&b.0));
    topics.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1 = b.1.max(a.1);
            true
        } else {
            false
        }
    });

    topics
}

fn extract_rich_rust(content: &str, topics: &mut Vec<(String, f32)>) {
    for line in content.lines() {
        let trimmed = line.trim();

        // fn signatures → extract function purpose from name
        if (trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn "))
            && !trimmed.starts_with("fn main")
        {
            let cleaned = trimmed.replace("pub ", "").replace("async ", "");
            let name = cleaned
                .strip_prefix("fn ")
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.len() >= 4 {
                // Split snake_case into semantic tokens
                for part in name.split('_') {
                    if part.len() >= 3
                        && !["get", "set", "new", "the", "for", "mut", "ref"].contains(&part)
                    {
                        topics.push((part.to_string(), 0.5));
                    }
                }
            }
        }

        // impl blocks → trait/type being implemented
        if trimmed.starts_with("impl ") {
            let rest = trimmed.strip_prefix("impl ").unwrap_or("");
            let type_name = if rest.contains(" for ") {
                rest.split(" for ")
                    .nth(1)
                    .and_then(|s| s.split_whitespace().next())
            } else {
                rest.split_whitespace().next()
            };
            if let Some(name) = type_name {
                let clean = name.trim_matches(&['<', '>', '{'][..]);
                if clean.len() >= 3 {
                    topics.push((clean.to_lowercase(), 0.65));
                }
            }
        }

        // Error handling patterns
        if trimmed.contains("anyhow::") || trimmed.contains("thiserror") {
            topics.push(("error_handling".to_string(), 0.5));
        }
        if trimmed.contains("MutexGuard")
            || trimmed.contains("RwLock")
            || trimmed.contains("Arc<Mutex")
        {
            topics.push(("concurrency".to_string(), 0.6));
        }
        if trimmed.contains(".await") || trimmed.contains("tokio::spawn") {
            topics.push(("async_runtime".to_string(), 0.55));
        }
    }

    // Detect broader patterns from content
    if content.contains("rusqlite") || content.contains("diesel") || content.contains("sqlx") {
        topics.push(("database".to_string(), 0.7));
    }
    if content.contains("reqwest") || content.contains("hyper") || content.contains("axum") {
        topics.push(("http".to_string(), 0.65));
    }
}

fn extract_rich_js(content: &str, topics: &mut Vec<(String, f32)>) {
    for line in content.lines() {
        let trimmed = line.trim();

        // Export function/const names → API surface
        if trimmed.starts_with("export function ")
            || trimmed.starts_with("export const ")
            || trimmed.starts_with("export async function ")
        {
            let cleaned = trimmed
                .replace("export ", "")
                .replace("async ", "")
                .replace("function ", "")
                .replace("const ", "");
            let name = cleaned
                .split(&['(', ':', ' ', '='][..])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.len() >= 4 {
                // Split camelCase
                let mut parts = Vec::new();
                let mut current = String::new();
                for ch in name.chars() {
                    if ch.is_uppercase() && !current.is_empty() {
                        parts.push(current.to_lowercase());
                        current = String::new();
                    }
                    current.push(ch);
                }
                if !current.is_empty() {
                    parts.push(current.to_lowercase());
                }
                for part in parts {
                    if part.len() >= 3
                        && !["get", "set", "use", "the", "for"].contains(&part.as_str())
                    {
                        topics.push((part, 0.5));
                    }
                }
            }
        }

        // React hooks
        if trimmed.contains("useState") {
            topics.push(("state_management".to_string(), 0.5));
        }
        if trimmed.contains("useEffect") {
            topics.push(("side_effects".to_string(), 0.5));
        }
        if trimmed.contains("useMemo") || trimmed.contains("useCallback") {
            topics.push(("performance_optimization".to_string(), 0.5));
        }
    }

    // Broader patterns
    if content.contains("fetch(") || content.contains("axios") {
        topics.push(("api_calls".to_string(), 0.6));
    }
    if content.contains("tailwind") || content.contains("className=") {
        topics.push(("styling".to_string(), 0.45));
    }
}

fn extract_rich_python(content: &str, topics: &mut Vec<(String, f32)>) {
    for line in content.lines() {
        let trimmed = line.trim();

        // class/def definitions
        if trimmed.starts_with("class ") {
            let name = trimmed
                .strip_prefix("class ")
                .unwrap_or("")
                .split(&['(', ':'][..])
                .next()
                .unwrap_or("")
                .trim();
            if name.len() >= 3 {
                topics.push((name.to_lowercase(), 0.6));
            }
        }
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let name = trimmed
                .replace("async ", "")
                .strip_prefix("def ")
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.len() >= 4 && !name.starts_with('_') {
                for part in name.split('_') {
                    if part.len() >= 3 {
                        topics.push((part.to_string(), 0.5));
                    }
                }
            }
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
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        // Initialize state table
        let c = conn.lock();
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS watcher_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                state_json TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )?;
        drop(c);

        Ok(Self { conn })
    }

    /// Save watcher state
    pub fn save(&self, watcher: &FileWatcher) -> Result<()> {
        let state = WatcherState {
            watched_paths: watcher.watched_paths(),
            last_active: chrono::Utc::now().to_rfc3339(),
            config: WatcherConfigSerializable::from(&watcher.config),
        };

        let json = serde_json::to_string(&state)?;

        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO watcher_state (id, state_json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET state_json = excluded.state_json, updated_at = datetime('now')",
            [&json],
        )?;

        Ok(())
    }

    /// Load watcher state
    #[allow(dead_code)] // Reason: WatcherPersistence API, not yet called from main app startup
    pub fn load(&self) -> Result<Option<WatcherState>> {
        let conn = self.conn.lock();
        let result: std::result::Result<String, _> = conn.query_row(
            "SELECT state_json FROM watcher_state WHERE id = 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(json) => {
                let state: WatcherState = serde_json::from_str(&json)?;
                Ok(Some(state))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to load watcher state: {e}").into()),
        }
    }

    /// Restore watcher from saved state
    #[allow(dead_code)] // Reason: WatcherPersistence API, not yet called from main app startup
    pub fn restore(&self, watcher: &mut FileWatcher) -> Result<usize> {
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
    #[allow(dead_code)] // Reason: WatcherPersistence API, not yet called from main app
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM watcher_state WHERE id = 1", [])?;
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
    #[allow(dead_code)] // Reason: RateLimiter public API, not yet called from main app
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
    #[allow(dead_code)] // Reason: RateLimiter public API, not yet called from main app
    pub fn time_until_available(&self) -> Duration {
        if self.check() {
            Duration::ZERO
        } else {
            let requests = self.requests.lock();
            if let Some(oldest) = requests.first() {
                let window = Duration::from_secs(self.window_seconds);
                let elapsed = Instant::now().duration_since(*oldest);
                if elapsed < window {
                    window.saturating_sub(elapsed)
                } else {
                    Duration::ZERO
                }
            } else {
                Duration::ZERO
            }
        }
    }

    /// Reset the rate limiter
    #[allow(dead_code)] // Reason: RateLimiter public API, not yet called from main app
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
                .map_or(self.default_source_limit, RateLimiter::remaining)
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
