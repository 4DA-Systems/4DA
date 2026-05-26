// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Sidecar lifecycle manager for llama-server (llama.cpp).
//!
//! Manages spawning, health-checking, and stopping the bundled llama-server
//! binary which exposes an OpenAI-compatible API (`/v1/chat/completions`,
//! `/v1/embeddings`). The existing `complete_openai()` in `llm.rs` speaks
//! this protocol — this module only manages the process lifecycle.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::error::{FourDaError, Result, ResultExt};

/// Runtime state for the llama-server sidecar process.
pub(crate) struct SidecarState {
    child: Option<Child>,
    port: u16,
    model_path: PathBuf,
    status: SidecarStatus,
}

/// Lifecycle status of the sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SidecarStatus {
    Stopped,
    Starting,
    Ready,
    Error,
}

static SIDECAR: OnceLock<Arc<Mutex<SidecarState>>> = OnceLock::new();

fn state() -> &'static Arc<Mutex<SidecarState>> {
    SIDECAR.get_or_init(|| {
        Arc::new(Mutex::new(SidecarState {
            child: None,
            port: 0,
            model_path: PathBuf::new(),
            status: SidecarStatus::Stopped,
        }))
    })
}

fn lock_state() -> std::result::Result<std::sync::MutexGuard<'static, SidecarState>, FourDaError> {
    state()
        .lock()
        .map_err(|_| FourDaError::Internal("Sidecar lock poisoned".into()))
}

/// Bind to port 0 and let the OS assign a free ephemeral port.
fn find_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind ephemeral port")?;
    let port = listener
        .local_addr()
        .context("Failed to read local address")?
        .port();
    drop(listener);
    Ok(port)
}

/// Resolve the llama-server binary path.
/// Priority: `LLAMA_SERVER_PATH` env var, then Tauri `externalBin` convention.
fn sidecar_binary_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LLAMA_SERVER_PATH") {
        let path = PathBuf::from(&p);
        if path.exists() {
            debug!(path = %path.display(), "Using LLAMA_SERVER_PATH override");
            return Some(path);
        }
        warn!(path = %p, "LLAMA_SERVER_PATH set but file does not exist");
    }
    let name = platform_binary_name();
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(5) {
            let candidate = ancestor.join("binaries").join(name);
            if candidate.exists() {
                debug!(path = %candidate.display(), "Found bundled llama-server");
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn platform_binary_name() -> &'static str {
    "llama-server-x86_64-pc-windows-msvc.exe"
}
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn platform_binary_name() -> &'static str {
    "llama-server-x86_64-apple-darwin"
}
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn platform_binary_name() -> &'static str {
    "llama-server-aarch64-apple-darwin"
}
#[cfg(target_os = "linux")]
fn platform_binary_name() -> &'static str {
    "llama-server-x86_64-unknown-linux-gnu"
}

/// Start the llama-server sidecar with the given GGUF model. Returns the port.
/// If already running with the same model, returns immediately. Restarts on model change.
pub(crate) async fn start_sidecar(model_path: &Path) -> Result<u16> {
    let binary = sidecar_binary_path()
        .ok_or_else(|| FourDaError::Llm("llama-server binary not found".into()))?;
    if !model_path.exists() {
        return Err(FourDaError::Llm(format!(
            "Model file not found: {}",
            model_path.display()
        )));
    }

    // Fast path: already running with the same model
    {
        let g = lock_state()?;
        if g.status == SidecarStatus::Ready && g.model_path == model_path {
            debug!(
                port = g.port,
                "Sidecar already running with requested model"
            );
            return Ok(g.port);
        }
    }

    stop_sidecar();

    let port = find_free_port()?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(4);

    info!(port, threads, model = %model_path.display(), "Starting llama-server sidecar");

    {
        let mut g = lock_state()?;
        g.status = SidecarStatus::Starting;
        g.port = port;
        g.model_path = model_path.to_path_buf();
    }

    let mut cmd = Command::new(&binary);
    cmd.args([
        "--model",
        &model_path.to_string_lossy(),
        "--port",
        &port.to_string(),
        "--host",
        "127.0.0.1",
        "--ctx-size",
        "16384",
        "--threads",
        &threads.to_string(),
        "--log-disable",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd.spawn().context("Failed to spawn llama-server")?;
    {
        lock_state()?.child = Some(child);
    }

    // Poll /health — 14B model loads in ~25s on fast hardware, allow 120s for slower machines
    let ready = wait_for_health(port, 120).await;

    let mut g = lock_state()?;
    if ready {
        g.status = SidecarStatus::Ready;
        info!(port, "llama-server sidecar is ready");
        Ok(port)
    } else {
        error!("llama-server failed to become healthy within 120s");
        if let Some(ref mut c) = g.child {
            let _ = c.kill();
            let _ = c.wait();
        }
        g.child = None;
        g.status = SidecarStatus::Error;
        Err(FourDaError::Llm(
            "llama-server did not become healthy within 120 seconds".into(),
        ))
    }
}

/// Poll `/health` until it returns 200 or the timeout expires.
async fn wait_for_health(port: u16, timeout_secs: u64) -> bool {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        if health_check(port).await {
            return true;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

/// Ping the sidecar's `/health` endpoint. Returns true on 200 OK.
pub(crate) async fn health_check(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/health");
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    matches!(client.get(&url).send().await, Ok(r) if r.status().is_success())
}

/// Stop the running sidecar process gracefully.
pub(crate) fn stop_sidecar() {
    let Ok(mut guard) = state().lock() else {
        warn!("Cannot acquire sidecar lock for shutdown");
        return;
    };
    if let Some(ref mut child) = guard.child {
        info!("Stopping llama-server sidecar");
        kill_child(child);
    }
    guard.child = None;
    guard.status = SidecarStatus::Stopped;
}

#[cfg(unix)]
fn kill_child(child: &mut Child) {
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Get the current sidecar lifecycle status.
pub(crate) fn sidecar_status() -> SidecarStatus {
    state()
        .lock()
        .map(|g| g.status)
        .unwrap_or(SidecarStatus::Error)
}

/// Get the port the sidecar is listening on, if it is running.
pub(crate) fn sidecar_port() -> Option<u16> {
    let g = state().lock().ok()?;
    (g.status == SidecarStatus::Ready).then_some(g.port)
}

/// Base URL for the sidecar's OpenAI-compatible API (`http://127.0.0.1:<port>/v1`).
pub(crate) fn sidecar_base_url() -> Option<String> {
    sidecar_port().map(|p| format!("http://127.0.0.1:{p}/v1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_free_port_returns_valid_port() {
        let port = find_free_port().expect("should find a free port");
        assert!(port > 0);
    }

    #[test]
    fn sidecar_status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&SidecarStatus::Ready).unwrap(),
            r#""ready""#
        );
        assert_eq!(
            serde_json::to_string(&SidecarStatus::Stopped).unwrap(),
            r#""stopped""#
        );
        assert_eq!(
            serde_json::to_string(&SidecarStatus::Starting).unwrap(),
            r#""starting""#
        );
        assert_eq!(
            serde_json::to_string(&SidecarStatus::Error).unwrap(),
            r#""error""#
        );
    }

    #[test]
    fn sidecar_binary_path_does_not_panic() {
        // Can't assert None — the binary might exist in the build tree.
        // Just verify no panic.
        std::env::remove_var("LLAMA_SERVER_PATH");
        let _ = sidecar_binary_path();
    }

    #[test]
    fn state_initializes_to_stopped() {
        let g = state().lock().unwrap();
        assert_eq!(g.status, SidecarStatus::Stopped);
        assert!(g.child.is_none());
    }

    #[test]
    fn sidecar_base_url_none_when_stopped() {
        if sidecar_status() == SidecarStatus::Stopped {
            assert!(sidecar_base_url().is_none());
        }
    }
}
