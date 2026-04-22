// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Toolkit — Backend commands for the Developer Toolkit.
//!
//! Provides port scanning and environment snapshots.
//! HTTP probing lives in `toolkit_http.rs`.
//! All commands are Tauri-invocable from the frontend Toolkit view.

use crate::error::{FourDaError, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use tracing::debug;

// ============================================================================
// Spawned PID Tracker — only PIDs registered here can be killed
// ============================================================================

static SPAWNED_PIDS: once_cell::sync::Lazy<Mutex<HashSet<u32>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashSet::new()));

/// Register a PID that the application has spawned.
/// Other modules should call this when creating child processes.
pub fn register_spawned_pid(pid: u32) {
    let mut pids = SPAWNED_PIDS.lock();
    pids.insert(pid);
    debug!(target: "4da::toolkit", pid, "Registered spawned PID");
}

/// Remove a PID from the spawned tracker (e.g., after process exits naturally).
pub fn unregister_spawned_pid(pid: u32) {
    let mut pids = SPAWNED_PIDS.lock();
    pids.remove(&pid);
}

/// Check if a PID was spawned by the application.
fn is_spawned_pid(pid: u32) -> bool {
    SPAWNED_PIDS.lock().contains(&pid)
}

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

// ============================================================================
// Port Scanner
// ============================================================================

#[tauri::command]
pub async fn toolkit_list_ports() -> Result<Vec<ListeningPort>> {
    tokio::task::spawn_blocking(|| {
        let mut ports = Vec::new();

        #[cfg(target_os = "windows")]
        {
            let output = {
                use std::os::windows::process::CommandExt;
                Command::new("cmd")
                    .args(["/C", "netstat -ano -p TCP"])
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .output()
                    .map_err(|e| FourDaError::Internal(format!("netstat failed: {e}")))?
            };

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
            // macOS uses lsof, Linux uses ss
            let (cmd, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
                ("lsof", &["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
            } else {
                ("ss", &["-tlnp"])
            };

            let output = Command::new(cmd)
                .args(args)
                .output()
                .map_err(|e| FourDaError::Internal(format!("{cmd} failed: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if cfg!(target_os = "macos") {
                    // lsof format: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
                    if parts.len() >= 9 {
                        let name = parts[8]; // e.g., "*:4444" or "127.0.0.1:8080"
                        if let Some(port_str) = name.rsplit(':').next() {
                            if let Ok(port) = port_str.parse::<u16>() {
                                let pid = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
                                ports.push(ListeningPort {
                                    port,
                                    protocol: "TCP".into(),
                                    pid,
                                    process_name: parts.first().unwrap_or(&"").to_string(),
                                    address: name.to_string(),
                                });
                            }
                        }
                    }
                } else {
                    // ss format: State Recv-Q Send-Q Local Address:Port Peer Address:Port Process
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
    let output = {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", &format!("tasklist /FI \"PID eq {pid}\" /NH /FO CSV")])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
    };

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .find(|l| !l.starts_with("INFO:") && l.contains(','))
                .and_then(|line| line.split(',').next())
                .map_or_else(
                    || format!("PID {pid}"),
                    |name| name.trim_matches('"').to_string(),
                )
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
    // Block system-critical PIDs
    if pid == 0 || pid == 4 {
        return Err(FourDaError::Config(
            "Cannot kill system processes".to_string(),
        ));
    }

    // Only allow killing processes that 4DA itself spawned
    if !is_spawned_pid(pid) {
        return Err(FourDaError::Config(format!(
            "Cannot kill process {pid}: not spawned by 4DA. \
             Only processes started by the application can be terminated."
        )));
    }

    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let result = {
            use std::os::windows::process::CommandExt;
            Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output()
        };

        #[cfg(not(target_os = "windows"))]
        let result = Command::new("kill").args(["-9", &pid.to_string()]).output();

        match result {
            Ok(out) if out.status.success() => {
                // Remove from tracked PIDs after successful kill
                unregister_spawned_pid(pid);
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
    let work_dir = working_dir.map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        std::path::PathBuf::from,
    );

    tokio::task::spawn_blocking(move || {
        let run = |prog: &str, args: &[&str]| -> Option<String> {
            let mut cmd = Command::new(prog);
            cmd.args(args).current_dir(&work_dir);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            cmd.output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };

        #[cfg(target_os = "windows")]
        let run_shell = |cmd: &str| -> Option<String> {
            use std::os::windows::process::CommandExt;
            Command::new("cmd")
                .args(["/C", cmd])
                .current_dir(&work_dir)
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
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

    #[tokio::test]
    async fn kill_process_rejects_untracked_pid() {
        // An arbitrary PID not registered via register_spawned_pid should be rejected
        let result = toolkit_kill_process(99999).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not spawned by 4DA"),
            "Expected spawned-PID guard, got: {}",
            err
        );
    }

    #[test]
    fn register_and_check_spawned_pid() {
        let test_pid = 777_777;
        assert!(!is_spawned_pid(test_pid));
        register_spawned_pid(test_pid);
        assert!(is_spawned_pid(test_pid));
        unregister_spawned_pid(test_pid);
        assert!(!is_spawned_pid(test_pid));
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
}
