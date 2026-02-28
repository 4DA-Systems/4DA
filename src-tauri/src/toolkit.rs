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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Struct construction & serde roundtrip ---------------------------------

    #[test]
    fn listening_port_serde_roundtrip() {
        let port = ListeningPort {
            port: 4444,
            protocol: "TCP".into(),
            pid: 1234,
            process_name: "node".into(),
            address: "127.0.0.1:4444".into(),
        };
        let json = serde_json::to_string(&port).unwrap();
        let restored: ListeningPort = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.port, 4444);
        assert_eq!(restored.protocol, "TCP");
        assert_eq!(restored.pid, 1234);
        assert_eq!(restored.process_name, "node");
        assert_eq!(restored.address, "127.0.0.1:4444");
    }

    #[test]
    fn http_probe_request_serde_roundtrip() {
        let req = HttpProbeRequest {
            method: "POST".into(),
            url: "https://example.com/api".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some("{\"key\":\"value\"}".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: HttpProbeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.method, "POST");
        assert_eq!(restored.url, "https://example.com/api");
        assert_eq!(restored.headers.len(), 1);
        assert_eq!(restored.body.as_deref(), Some("{\"key\":\"value\"}"));
    }

    #[test]
    fn http_probe_response_serde_roundtrip() {
        let resp = HttpProbeResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "text/html".into())],
            body: "<html></html>".into(),
            duration_ms: 42,
            size_bytes: 13,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: HttpProbeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.status, 200);
        assert_eq!(restored.status_text, "OK");
        assert_eq!(restored.duration_ms, 42);
        assert_eq!(restored.size_bytes, 13);
    }

    #[test]
    fn http_history_entry_serde_roundtrip() {
        let entry = HttpHistoryEntry {
            id: 1,
            method: "GET".into(),
            url: "https://example.com".into(),
            status: 404,
            duration_ms: 150,
            created_at: "2025-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: HttpHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, 1);
        assert_eq!(restored.status, 404);
        assert_eq!(restored.created_at, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn env_snapshot_serde_with_optional_fields() {
        let snap = EnvSnapshot {
            os: "windows".into(),
            os_version: "10.0.19045".into(),
            hostname: "DEVBOX".into(),
            git_branch: Some("main".into()),
            git_status: None,
            git_recent_commits: vec!["abc123 initial commit".into()],
            node_version: Some("v20.10.0".into()),
            pnpm_version: None,
            npm_version: None,
            rust_version: Some("rustc 1.77.0".into()),
            python_version: None,
            ports: vec![],
        };
        let json = serde_json::to_string(&snap).unwrap();
        let restored: EnvSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.os, "windows");
        assert_eq!(restored.git_branch.as_deref(), Some("main"));
        assert!(restored.git_status.is_none());
        assert!(restored.pnpm_version.is_none());
        assert_eq!(restored.git_recent_commits.len(), 1);
    }

    // -- URL validation logic (extracted from toolkit_http_request) ------------

    #[test]
    fn url_validation_rejects_invalid_schemes() {
        let invalid_urls = ["ftp://example.com", "ws://localhost", "example.com", ""];
        for url in &invalid_urls {
            let valid = url.starts_with("http://") || url.starts_with("https://");
            assert!(!valid, "URL '{}' should be rejected", url);
        }
    }

    #[test]
    fn url_validation_accepts_valid_schemes() {
        let valid_urls = [
            "http://localhost:3000",
            "https://api.example.com/v1/data",
            "http://127.0.0.1:8080/path?q=1",
        ];
        for url in &valid_urls {
            let valid = url.starts_with("http://") || url.starts_with("https://");
            assert!(valid, "URL '{}' should be accepted", url);
        }
    }

    // -- Kill-process PID guard -----------------------------------------------

    #[tokio::test]
    async fn kill_process_rejects_system_pids() {
        // PID 0 (system idle) should be rejected
        let result = toolkit_kill_process(0).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Cannot kill system processes"),
            "Expected system process guard, got: {}",
            err
        );

        // PID 4 (system) should also be rejected
        let result = toolkit_kill_process(4).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Cannot kill system processes"));
    }

    // -- History limit clamping -----------------------------------------------

    #[test]
    fn history_limit_defaults_to_50() {
        let limit: Option<u32> = None;
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 50);
    }

    #[test]
    fn history_limit_clamps_to_200() {
        let limit: Option<u32> = Some(999);
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 200);
    }

    #[test]
    fn history_limit_passes_through_valid_value() {
        let limit: Option<u32> = Some(100);
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 100);
    }

    // -- Body truncation logic ------------------------------------------------

    #[test]
    fn body_truncation_at_500kb() {
        // Body under 500KB should not be truncated
        let small_body = "x".repeat(1000);
        let result = if small_body.len() > 500_000 {
            format!("{}...\n\n(truncated at 500KB)", &small_body[..500_000])
        } else {
            small_body.clone()
        };
        assert_eq!(result.len(), 1000);

        // Body over 500KB should be truncated
        let big_body = "y".repeat(600_000);
        let result = if big_body.len() > 500_000 {
            format!("{}...\n\n(truncated at 500KB)", &big_body[..500_000])
        } else {
            big_body.clone()
        };
        assert!(result.len() < big_body.len());
        assert!(result.contains("(truncated at 500KB)"));
        // 500_000 bytes + "...\n\n(truncated at 500KB)" suffix
        assert!(result.starts_with("yyyyy"));
    }

    // -- Port sort & dedup logic ----------------------------------------------

    #[test]
    fn port_sort_and_dedup() {
        let mut ports = vec![
            ListeningPort {
                port: 8080,
                protocol: "TCP".into(),
                pid: 10,
                process_name: "a".into(),
                address: "0.0.0.0:8080".into(),
            },
            ListeningPort {
                port: 3000,
                protocol: "TCP".into(),
                pid: 20,
                process_name: "b".into(),
                address: "0.0.0.0:3000".into(),
            },
            ListeningPort {
                port: 8080,
                protocol: "TCP".into(),
                pid: 30,
                process_name: "c".into(),
                address: "127.0.0.1:8080".into(),
            },
            ListeningPort {
                port: 443,
                protocol: "TCP".into(),
                pid: 40,
                process_name: "d".into(),
                address: "0.0.0.0:443".into(),
            },
        ];

        // Same sort + dedup logic used in toolkit_list_ports
        ports.sort_by_key(|p| p.port);
        ports.dedup_by_key(|p| p.port);

        assert_eq!(ports.len(), 3, "Duplicate port 8080 should be removed");
        assert_eq!(ports[0].port, 443);
        assert_eq!(ports[1].port, 3000);
        assert_eq!(ports[2].port, 8080);
    }

    // -- HttpProbeRequest with no body / no headers ---------------------------

    #[test]
    fn http_probe_request_minimal() {
        let req = HttpProbeRequest {
            method: "GET".into(),
            url: "http://localhost".into(),
            headers: vec![],
            body: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: HttpProbeRequest = serde_json::from_str(&json).unwrap();
        assert!(restored.body.is_none());
        assert!(restored.headers.is_empty());
    }
}
