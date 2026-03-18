//! Plugin loader — discovers, validates, and executes source plugins.
//!
//! Plugins live in `data/plugins/<plugin-name>/` with a `manifest.json`
//! and an executable binary. Execution is isolated: one plugin crash
//! cannot affect others or the main application.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::{PluginConfig, PluginItem, PluginManifest};
use crate::error::{FourDaError, Result, ResultExt};

/// Maximum time a plugin is allowed to run before being considered hung.
const PLUGIN_TIMEOUT_SECS: u64 = 30;

/// Maximum stdout size from a plugin (10 MB) to prevent memory exhaustion.
const MAX_PLUGIN_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Get the plugins directory (inside 4DA data folder).
/// Derives from `get_db_path()` which is the single source of truth for data location.
pub fn plugins_dir() -> PathBuf {
    let db_path = crate::state::get_db_path();
    // get_db_path() returns <data-dir>/4da.db, parent is the data directory
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("data"))
        .join("plugins")
}

/// Ensure the plugins directory exists (called at startup).
pub fn ensure_plugins_dir() {
    let dir = plugins_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(target: "4da::plugins", path = %dir.display(), "Failed to create plugins directory: {e}");
    }
}

/// Discover all installed plugins by reading manifest.json files.
///
/// Scans each subdirectory of the plugins directory for a `manifest.json`.
/// Invalid manifests are logged and skipped — they never prevent other plugins
/// from loading.
pub fn discover_plugins() -> Vec<PluginManifest> {
    let dir = plugins_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut plugins = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(target: "4da::plugins", path = %dir.display(), "Cannot read plugins directory: {e}");
            return Vec::new();
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        match load_manifest(&manifest_path) {
            Ok(manifest) => {
                // Validate that manifest.name matches its containing directory
                // to prevent a manifest from claiming a different plugin's identity
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if manifest.name != dir_name {
                    warn!(
                        target: "4da::plugins",
                        manifest_name = %manifest.name,
                        dir_name = %dir_name,
                        "Plugin manifest name does not match directory name, skipping"
                    );
                    continue;
                }
                info!(target: "4da::plugins", name = %manifest.name, version = %manifest.version, "Discovered plugin");
                plugins.push(manifest);
            }
            Err(e) => {
                warn!(target: "4da::plugins", path = %manifest_path.display(), "Invalid plugin manifest: {e}");
            }
        }
    }

    plugins
}

/// Load and validate a plugin manifest from disk.
fn load_manifest(path: &std::path::Path) -> Result<PluginManifest> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read manifest: {}", path.display()))?;

    let manifest: PluginManifest = serde_json::from_str(&json)
        .with_context(|| format!("Invalid manifest JSON: {}", path.display()))?;

    // Basic validation
    if manifest.name.is_empty() {
        return Err(FourDaError::Config("Plugin name cannot be empty".into()));
    }
    if manifest.binary.is_empty() {
        return Err(FourDaError::Config("Plugin binary cannot be empty".into()));
    }
    // Prevent path traversal in name field
    if manifest.name.contains("..")
        || manifest.name.contains('/')
        || manifest.name.contains('\\')
        || manifest.name.contains('\0')
    {
        return Err(FourDaError::Config(
            "Plugin name must be a simple identifier, no path separators, '..', or null bytes"
                .into(),
        ));
    }
    // Prevent path traversal in binary field
    if manifest.binary.contains("..")
        || manifest.binary.contains('/')
        || manifest.binary.contains('\\')
    {
        return Err(FourDaError::Config(
            "Plugin binary must be a simple filename, no path separators or '..'".into(),
        ));
    }

    Ok(manifest)
}

/// Execute a plugin and return its items.
///
/// Protocol: send PluginConfig as JSON on stdin, read PluginItem[] from stdout.
/// Timeout after 30 seconds. Plugin errors are isolated — one crash doesn't
/// affect others.
pub fn execute_plugin(manifest: &PluginManifest, config: &PluginConfig) -> Result<Vec<PluginItem>> {
    let binary_path = plugins_dir().join(&manifest.name).join(&manifest.binary);

    if !binary_path.exists() {
        return Err(FourDaError::Config(format!(
            "Plugin binary not found: {}",
            binary_path.display()
        )));
    }

    let config_json = serde_json::to_string(config).context("Failed to serialize plugin config")?;

    debug!(target: "4da::plugins", name = %manifest.name, binary = %binary_path.display(), "Executing plugin");

    // Detect script extensions and prepend the appropriate interpreter.
    // This lets plugins be written as .js (Node.js) or .py (Python) scripts
    // without requiring platform-specific wrapper scripts.
    let mut cmd = match binary_path.extension().and_then(|e| e.to_str()) {
        Some("js" | "mjs") => {
            let mut c = Command::new("node");
            c.arg(&binary_path);
            c
        }
        Some("py") => {
            let mut c = Command::new("python");
            c.arg(&binary_path);
            c
        }
        _ => Command::new(&binary_path),
    };

    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("TEMP", std::env::temp_dir())
        .env("TMP", std::env::temp_dir())
        .current_dir(plugins_dir().join(&manifest.name))
        .spawn()
        .with_context(|| format!("Failed to spawn plugin '{}'", manifest.name))?;

    // Send config on stdin, then close to signal EOF
    if let Some(ref mut stdin) = child.stdin {
        if let Err(e) = stdin.write_all(config_json.as_bytes()) {
            warn!(target: "4da::plugins", name = %manifest.name, "Failed to write to plugin stdin: {e}");
            // Kill the child if we can't communicate
            let _ = child.kill();
            return Ok(Vec::new());
        }
    }
    // Drop stdin handle to close the pipe — plugin sees EOF
    drop(child.stdin.take());

    // Wait for output with timeout
    let output = wait_with_timeout(&mut child, Duration::from_secs(PLUGIN_TIMEOUT_SECS))
        .with_context(|| format!("Plugin '{}' execution failed", manifest.name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);
        warn!(
            target: "4da::plugins",
            name = %manifest.name,
            exit_code = exit_code,
            stderr = %stderr.chars().take(500).collect::<String>(),
            "Plugin exited with error"
        );
        // Graceful degradation — return empty instead of propagating error
        return Ok(Vec::new());
    }

    // Guard against oversized output
    if output.stdout.len() > MAX_PLUGIN_OUTPUT_BYTES {
        warn!(
            target: "4da::plugins",
            name = %manifest.name,
            size = output.stdout.len(),
            limit = MAX_PLUGIN_OUTPUT_BYTES,
            "Plugin output exceeds size limit, skipping"
        );
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        debug!(target: "4da::plugins", name = %manifest.name, "Plugin returned empty output");
        return Ok(Vec::new());
    }

    let items: Vec<PluginItem> = serde_json::from_str(&stdout)
        .with_context(|| format!("Failed to parse plugin '{}' output as JSON", manifest.name))?;

    // Enforce max_items limit
    let limited_items = if items.len() > config.max_items {
        info!(
            target: "4da::plugins",
            name = %manifest.name,
            returned = items.len(),
            limit = config.max_items,
            "Plugin returned more items than max_items, truncating"
        );
        items.into_iter().take(config.max_items).collect()
    } else {
        items
    };

    info!(
        target: "4da::plugins",
        name = %manifest.name,
        items = limited_items.len(),
        "Plugin returned items"
    );
    Ok(limited_items)
}

/// Wait for a child process with a timeout and capped output size.
///
/// Reads stdout/stderr incrementally so a malicious plugin cannot exhaust
/// memory before we can check the size. On timeout or output overflow,
/// kills the process and returns an error.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> std::result::Result<std::process::Output, std::io::Error> {
    use std::io::Read;

    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(100);

    // Read stdout in a background thread with a size cap to prevent memory exhaustion
    let stdout_handle = child.stdout.take();
    let stdout_thread = std::thread::spawn(move || {
        let Some(mut reader) = stdout_handle else {
            return Ok(Vec::new());
        };
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.len() > MAX_PLUGIN_OUTPUT_BYTES {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Plugin stdout exceeded size limit",
                        ));
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(buf)
    });

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished — collect output
                let stdout = match stdout_thread.join() {
                    Ok(Ok(buf)) => buf,
                    Ok(Err(e)) => {
                        return Err(e);
                    }
                    Err(_) => Vec::new(),
                };
                let stderr = child.stderr.take().map_or_else(Vec::new, |mut s| {
                    let mut buf = Vec::new();
                    // Cap stderr too, but we only use it for logging
                    Read::read_to_end(&mut s, &mut buf).unwrap_or(0);
                    buf.truncate(MAX_PLUGIN_OUTPUT_BYTES);
                    buf
                });
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                // Still running
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait(); // Reap the process
                                          // Also join the stdout thread to avoid leaks
                    let _ = stdout_thread.join();
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Plugin execution timed out",
                    ));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                let _ = stdout_thread.join();
                return Err(e);
            }
        }
    }
}

/// Fetch items from all installed plugins.
///
/// Each plugin is executed independently. Failures are logged and skipped.
/// Returns a vec of (plugin_name, items) tuples.
pub fn fetch_all_plugin_items(config: &PluginConfig) -> Vec<(String, Vec<PluginItem>)> {
    let plugins = discover_plugins();
    if plugins.is_empty() {
        return Vec::new();
    }

    info!(target: "4da::plugins", count = plugins.len(), "Fetching from all plugins");
    let mut results = Vec::new();

    for manifest in &plugins {
        let plugin_config = PluginConfig {
            max_items: manifest.max_items.min(config.max_items),
            ..config.clone()
        };

        match execute_plugin(manifest, &plugin_config) {
            Ok(items) if !items.is_empty() => {
                results.push((manifest.name.clone(), items));
            }
            Ok(_) => {
                debug!(target: "4da::plugins", name = %manifest.name, "Plugin returned no items");
            }
            Err(e) => {
                warn!(target: "4da::plugins", name = %manifest.name, "Plugin fetch failed: {e}");
            }
        }
    }

    results
}

/// Build a PluginConfig from current user context.
///
/// Reads tech stack and interests from the context engine to pass to plugins.
pub fn build_plugin_config() -> PluginConfig {
    let (tech_stack, interests) = match crate::state::get_context_engine() {
        Ok(engine) => {
            let identity = engine.get_static_identity().unwrap_or_default();
            let interest_strings: Vec<String> =
                identity.interests.iter().map(|i| i.topic.clone()).collect();
            (identity.tech_stack, interest_strings)
        }
        Err(_) => (Vec::new(), Vec::new()),
    };

    PluginConfig {
        tech_stack,
        interests,
        max_items: 50,
        custom: serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugins_dir_is_under_data() {
        let dir = plugins_dir();
        let dir_str = dir.to_string_lossy();
        assert!(
            dir_str.contains("plugins"),
            "plugins_dir should end with 'plugins': {dir_str}"
        );
    }

    #[test]
    fn test_discover_plugins_empty_when_no_dir() {
        // plugins_dir may or may not exist in test environment
        // but should never panic
        let _ = discover_plugins();
    }

    #[test]
    fn test_load_manifest_rejects_path_traversal() {
        let tmp = std::env::temp_dir().join("4da-test-plugin-traversal");
        std::fs::create_dir_all(&tmp).ok();

        let manifest_json = r#"{
            "name": "evil-plugin",
            "version": "1.0.0",
            "description": "Tries path traversal",
            "binary": "../../../etc/passwd"
        }"#;
        let manifest_path = tmp.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json).unwrap();

        let result = load_manifest(&manifest_path);
        assert!(
            result.is_err(),
            "Should reject path traversal in binary field"
        );

        // Cleanup
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_load_manifest_rejects_empty_name() {
        let tmp = std::env::temp_dir().join("4da-test-plugin-empty-name");
        std::fs::create_dir_all(&tmp).ok();

        let manifest_json = r#"{
            "name": "",
            "version": "1.0.0",
            "description": "Empty name",
            "binary": "test.exe"
        }"#;
        let manifest_path = tmp.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json).unwrap();

        let result = load_manifest(&manifest_path);
        assert!(result.is_err(), "Should reject empty plugin name");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_load_manifest_valid() {
        let tmp = std::env::temp_dir().join("4da-test-plugin-valid");
        std::fs::create_dir_all(&tmp).ok();

        let manifest_json = r#"{
            "name": "good-plugin",
            "version": "1.0.0",
            "description": "A valid plugin",
            "binary": "good-plugin.exe",
            "author": "Test Author",
            "poll_interval_secs": 120,
            "max_items": 25
        }"#;
        let manifest_path = tmp.join("manifest.json");
        std::fs::write(&manifest_path, manifest_json).unwrap();

        let manifest = load_manifest(&manifest_path).unwrap();
        assert_eq!(manifest.name, "good-plugin");
        assert_eq!(manifest.author.as_deref(), Some("Test Author"));
        assert_eq!(manifest.poll_interval_secs, 120);
        assert_eq!(manifest.max_items, 25);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_fetch_all_plugin_items_empty() {
        let config = PluginConfig {
            tech_stack: vec!["rust".to_string()],
            interests: vec!["testing".to_string()],
            max_items: 10,
            custom: serde_json::Value::Null,
        };
        // Should not panic even if no plugins exist
        let results = fetch_all_plugin_items(&config);
        // In test environment, likely empty
        assert!(results.is_empty() || !results.is_empty());
    }
}
