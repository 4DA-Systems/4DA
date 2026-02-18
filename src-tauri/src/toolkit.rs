//! Toolkit — Backend commands for the Developer Toolkit.
//!
//! Provides port scanning, environment snapshots, and HTTP probing.
//! All commands are Tauri-invocable from the frontend Toolkit view.

use crate::error::{FourDaError, Result};
use crate::state::get_database;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Instant;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListeningPort {
    pub port: u16,
    pub protocol: String,
    pub pid: u32,
    pub process_name: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSnapshot {
    pub os: String,
    pub os_version: String,
    pub hostname: String,
    pub git_branch: Option<String>,
    pub git_status: Option<String>,
    pub git_recent_commits: Vec<String>,
    pub node_version: Option<String>,
    pub pnpm_version: Option<String>,
    pub npm_version: Option<String>,
    pub rust_version: Option<String>,
    pub python_version: Option<String>,
    pub ports: Vec<ListeningPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHistoryEntry {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub created_at: String,
}

// ============================================================================
// Port Scanner
// ============================================================================

#[tauri::command]
pub async fn toolkit_list_ports() -> Result<Vec<ListeningPort>> {
    tokio::task::spawn_blocking(|| {
        let mut ports = Vec::new();

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("cmd")
                .args(["/C", "netstat -ano -p TCP"])
                .output()
                .map_err(|e| FourDaError::Internal(format!("netstat failed: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(4) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[3] == "LISTENING" {
                    if let Some(addr) = parts.get(1) {
                        if let Some(port_str) = addr.rsplit(':').next() {
                            if let (Ok(port), Ok(pid)) =
                                (port_str.parse::<u16>(), parts[4].parse::<u32>())
                            {
                                let process_name = get_process_name_win(pid);
                                ports.push(ListeningPort {
                                    port,
                                    protocol: "TCP".into(),
                                    pid,
                                    process_name,
                                    address: addr.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new("ss")
                .args(["-tlnp"])
                .output()
                .map_err(|e| FourDaError::Internal(format!("ss failed: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    if let Some(addr) = parts.get(3) {
                        if let Some(port_str) = addr.rsplit(':').next() {
                            if let Ok(port) = port_str.parse::<u16>() {
                                let pid_info = parts.get(5).unwrap_or(&"");
                                let pid = extract_pid_unix(pid_info);
                                ports.push(ListeningPort {
                                    port,
                                    protocol: "TCP".into(),
                                    pid,
                                    process_name: pid_info.to_string(),
                                    address: addr.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort by port, deduplicate
        ports.sort_by_key(|p| p.port);
        ports.dedup_by_key(|p| p.port);

        debug!(target: "4da::toolkit", count = ports.len(), "Listed listening ports");
        Ok(ports)
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {e}")))?
}

#[cfg(target_os = "windows")]
fn get_process_name_win(pid: u32) -> String {
    if pid == 0 {
        return "System Idle".into();
    }
    let output = Command::new("cmd")
        .args(["/C", &format!("tasklist /FI \"PID eq {pid}\" /NH /FO CSV")])
        .output();

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .find(|l| !l.starts_with("INFO:") && l.contains(','))
                .and_then(|line| line.split(',').next())
                .map(|name| name.trim_matches('"').to_string())
                .unwrap_or_else(|| format!("PID {pid}"))
        }
        Err(_) => format!("PID {pid}"),
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_pid_unix(info: &str) -> u32 {
    // Format: "users:((\"node\",pid=12345,fd=3))"
    info.split("pid=")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn toolkit_kill_process(pid: u32) -> Result<String> {
    if pid == 0 || pid == 4 {
        return Err(FourDaError::Config(
            "Cannot kill system processes".to_string(),
        ));
    }

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let result = Command::new("cmd")
            .args(["/C", &format!("taskkill /F /PID {pid}")])
            .output();

        #[cfg(not(target_os = "windows"))]
        let result = Command::new("kill").args(["-9", &pid.to_string()]).output();

        match result {
            Ok(out) if out.status.success() => {
                debug!(target: "4da::toolkit", pid, "Process killed");
                Ok(format!("Process {pid} terminated"))
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(FourDaError::Internal(format!(
                    "Failed to kill PID {pid}: {stderr}"
                )))
            }
            Err(e) => Err(FourDaError::Internal(format!("Kill command failed: {e}"))),
        }
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {e}")))?
}

// ============================================================================
// Environment Snapshot
// ============================================================================

#[tauri::command]
pub async fn toolkit_env_snapshot(working_dir: Option<String>) -> Result<EnvSnapshot> {
    let work_dir = working_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    tokio::task::spawn_blocking(move || {
        let run = |prog: &str, args: &[&str]| -> Option<String> {
            Command::new(prog)
                .args(args)
                .current_dir(&work_dir)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };

        #[cfg(target_os = "windows")]
        let run_shell = |cmd: &str| -> Option<String> {
            Command::new("cmd")
                .args(["/C", cmd])
                .current_dir(&work_dir)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };

        #[cfg(not(target_os = "windows"))]
        let run_shell = |cmd: &str| -> Option<String> {
            Command::new("sh")
                .args(["-c", cmd])
                .current_dir(&work_dir)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };

        let git_branch = run("git", &["rev-parse", "--abbrev-ref", "HEAD"]);
        let git_status = run("git", &["status", "--short"]);
        let git_log = run_shell("git log --oneline -5");
        let git_recent_commits = git_log
            .map(|s| s.lines().map(String::from).collect())
            .unwrap_or_default();

        let node_version = run("node", &["--version"]);
        let pnpm_version = run("pnpm", &["--version"]);
        let npm_version = run("npm", &["--version"]);
        let rust_version = run_shell("rustc --version");
        let python_version =
            run("python", &["--version"]).or_else(|| run("python3", &["--version"]));

        let hostname = run_shell(if cfg!(target_os = "windows") {
            "hostname"
        } else {
            "hostname -s"
        })
        .unwrap_or_else(|| "unknown".into());

        let os = std::env::consts::OS.to_string();
        let os_version = run_shell(if cfg!(target_os = "windows") {
            "ver"
        } else {
            "uname -r"
        })
        .unwrap_or_else(|| "unknown".into());

        // Quick port scan (reuse logic but inline for perf)
        let ports = Vec::new(); // Frontend calls toolkit_list_ports separately

        debug!(target: "4da::toolkit", "Environment snapshot captured");

        Ok(EnvSnapshot {
            os,
            os_version,
            hostname,
            git_branch,
            git_status,
            git_recent_commits,
            node_version,
            pnpm_version,
            npm_version,
            rust_version,
            python_version,
            ports,
        })
    })
    .await
    .map_err(|e| FourDaError::Internal(format!("Task join error: {e}")))?
}

// ============================================================================
// HTTP Probe
// ============================================================================

#[tauri::command]
pub async fn toolkit_http_request(request: HttpProbeRequest) -> Result<HttpProbeResponse> {
    // Validate URL
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err(FourDaError::Config(
            "URL must start with http:// or https://".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| FourDaError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let method = request
        .method
        .parse::<reqwest::Method>()
        .map_err(|e| FourDaError::Config(format!("Invalid HTTP method: {e}")))?;

    let mut req = client.request(method, &request.url);

    for (key, value) in &request.headers {
        req = req.header(key.as_str(), value.as_str());
    }

    if let Some(body) = &request.body {
        req = req.body(body.clone());
    }

    let start = Instant::now();
    let response = req
        .send()
        .await
        .map_err(|e| FourDaError::Internal(format!("HTTP request failed: {e}")))?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();

    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<binary or unreadable>".into());
    let size_bytes = body.len();

    // Truncate very large responses
    let body = if body.len() > 500_000 {
        format!("{}...\n\n(truncated at 500KB)", &body[..500_000])
    } else {
        body
    };

    // Save to history (non-fatal)
    if let Err(e) = save_http_history(&request.method, &request.url, status, duration_ms) {
        warn!(target: "4da::toolkit", error = %e, "Failed to save HTTP history");
    }

    debug!(target: "4da::toolkit", url = %request.url, status, duration_ms, "HTTP probe complete");

    Ok(HttpProbeResponse {
        status,
        status_text,
        headers,
        body,
        duration_ms,
        size_bytes,
    })
}

#[tauri::command]
pub async fn toolkit_get_http_history(limit: Option<u32>) -> Result<Vec<HttpHistoryEntry>> {
    let limit = limit.unwrap_or(50).min(200);
    let db = get_database()?;

    let rows = db.get_http_history(limit).map_err(FourDaError::Db)?;

    Ok(rows
        .into_iter()
        .map(|r| HttpHistoryEntry {
            id: r.id,
            method: r.method,
            url: r.url,
            status: r.status,
            duration_ms: r.duration_ms,
            created_at: r.created_at,
        })
        .collect())
}

/// Persist an HTTP request in the history table.
fn save_http_history(method: &str, url: &str, status: u16, duration_ms: u64) -> Result<()> {
    let db = get_database()?;
    db.save_http_history(method, url, status, duration_ms)
        .map_err(FourDaError::Db)?;
    Ok(())
}
