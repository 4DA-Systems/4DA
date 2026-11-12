// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Hardware Monitor Sun -- re-runs hardware detection daily, updates sovereign_profile.

use super::SunResult;
use tracing::debug;

use crate::error::Result;

pub fn execute() -> SunResult {
    let mut facts_found = 0;

    #[cfg(target_os = "windows")]
    {
        // Try PowerShell first (Windows 11+), fall back to wmic (Windows 10).
        // All PowerShell calls below use .or_else(|_| run_cmd("wmic ...")) so
        // hardware detection works even when PowerShell is unavailable or restricted.
        debug!(target: "4da::suns", "Windows hardware detection: PowerShell primary, wmic fallback");
        let cpu_output = run_cmd("powershell -NoProfile -Command \"Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Name\"")
            .or_else(|_| run_cmd("wmic cpu get name,numberofcores /format:list"));
        if let Ok(output) = cpu_output {
            crate::sovereign_facts::store_facts_from_execution("cpu info", &output, "sun:hardware");
            facts_found += 1;
        }

        let mem_output = run_cmd("powershell -NoProfile -Command \"(Get-CimInstance Win32_PhysicalMemory | Measure-Object -Property Capacity -Sum).Sum\"")
            .or_else(|_| run_cmd("wmic memorychip get capacity /format:list"));
        if let Ok(output) = mem_output {
            crate::sovereign_facts::store_facts_from_execution(
                "memory info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        let disk_output = run_cmd("powershell -NoProfile -Command \"Get-CimInstance Win32_DiskDrive | Select-Object -Property Size,Model | Format-List\"")
            .or_else(|_| run_cmd("wmic diskdrive get size,model /format:list"));
        if let Ok(output) = disk_output {
            crate::sovereign_facts::store_facts_from_execution(
                "disk info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }
    }

    #[cfg(target_os = "linux")]
    {
        // CPU model name (e.g. "AMD Ryzen 9 7950X")
        let cpu_output = run_cmd("lscpu | grep 'Model name'")
            .or_else(|_| run_cmd("nproc").map(|n| format!("CPU cores: {}", n.trim())));
        if let Ok(output) = cpu_output {
            crate::sovereign_facts::store_facts_from_execution("cpu info", &output, "sun:hardware");
            facts_found += 1;
        }

        // Memory info
        if let Ok(output) = run_cmd("free -h") {
            crate::sovereign_facts::store_facts_from_execution(
                "memory info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        // Disk info (physical drives with size and model)
        if let Ok(output) = run_cmd("lsblk -d -o NAME,SIZE,MODEL --noheadings") {
            crate::sovereign_facts::store_facts_from_execution(
                "disk info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        // GPU info (VGA/3D/display controllers)
        if let Ok(output) = run_cmd("lspci | grep -iE 'vga|3d|display'") {
            crate::sovereign_facts::store_facts_from_execution("gpu info", &output, "sun:hardware");
            facts_found += 1;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = run_cmd("sysctl -n hw.ncpu") {
            crate::sovereign_facts::store_facts_from_execution(
                "sysctl -n hw.ncpu",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }
        if let Ok(output) = run_cmd("sysctl -n hw.memsize") {
            crate::sovereign_facts::store_facts_from_execution(
                "sysctl -n hw.memsize",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }
    }

    let disk_alert = check_disk_space();

    SunResult {
        success: true,
        message: format!("Updated {facts_found} hardware facts"),
        data: disk_alert.map(|msg| serde_json::json!({ "alert": msg })),
    }
}

/// Longest a system command may run. A `Get-CimInstance` can hang on a sick
/// WMI service; with no limit it held the sun registry, and before the
/// scheduler was supervised, the whole scheduler (audit O2).
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Run a shell command and capture stdout, killed after [`COMMAND_TIMEOUT`].
pub(super) fn run_cmd(cmd: &str) -> Result<String> {
    run_cmd_with_timeout(cmd, COMMAND_TIMEOUT)
}

fn run_cmd_with_timeout(cmd: &str, timeout: std::time::Duration) -> Result<String> {
    use std::io::Read;
    use std::process::Stdio;

    debug!(target: "4da::suns", cmd, "Running system command");

    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]).creation_flags(0x08000000); // CREATE_NO_WINDOW
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    // Drain both pipes on their own threads so a chatty command can never
    // block on a full pipe while we wait for it.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );

    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                kill_tree(&mut child);
                // The reader threads finish when the last holder of the pipes
                // exits; they are not waited for.
                return Err(
                    format!("`{cmd}` did not finish within {} s", timeout.as_secs()).into(),
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if status.success() {
        Ok(String::from_utf8_lossy(&out).to_string())
    } else {
        Err(String::from_utf8_lossy(&err).to_string().into())
    }
}

/// Kill a timed-out command and what it started (`cmd /C powershell ...`
/// leaves PowerShell running when only `cmd` dies).
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn check_disk_space() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // Try PowerShell first (Windows 11+), fall back to wmic (Windows 10)
        let disk_output = run_cmd("powershell -NoProfile -Command \"Get-CimInstance Win32_LogicalDisk | Select-Object -Property FreeSpace | Format-List\"")
            .or_else(|_| run_cmd("wmic logicaldisk get freespace,size /format:list"));
        if let Ok(output) = disk_output {
            // Parse free space lines: FreeSpace=<bytes> or FreeSpace : <bytes>
            for line in output.lines() {
                let trimmed = line.trim();
                let val = trimmed
                    .strip_prefix("FreeSpace=")
                    .or_else(|| trimmed.strip_prefix("FreeSpace :").map(str::trim));
                if let Some(val) = val {
                    if let Ok(free_bytes) = val.trim().parse::<u64>() {
                        let free_gb = free_bytes / (1024 * 1024 * 1024);
                        if free_gb < 50 {
                            return Some(format!(
                                "Low disk space: {free_gb}GB free on system drive"
                            ));
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // BSD df: -g flag displays sizes in gigabyte blocks
        if let Ok(output) = run_cmd("df -g / | tail -1") {
            // df output: Filesystem 1G-blocks Used Available Capacity Mounted
            let parts: Vec<&str> = output.split_whitespace().collect();
            if parts.len() >= 4 {
                if let Ok(avail_gb) = parts[3].parse::<u64>() {
                    if avail_gb < 50 {
                        return Some(format!(
                            "Low disk space: {}GB free on root partition",
                            avail_gb
                        ));
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Check the partition containing 4DA's data directory.
        //
        // SECURITY: Previous implementation shelled out via `sh -c` with the
        // path interpolated via format!() — a Linux username containing shell
        // metacharacters (POSIX permits anything except '/' and NUL) would
        // execute arbitrary commands. Now we invoke df directly without a
        // shell and parse in-process. The last line of df's multi-line
        // output is the data row we want; no pipe to `tail` is needed.
        // Ref: docs/ADVERSARIAL-AUDIT-2026-04-19.md self-audit HIGH-3.
        let data_dir = dirs::data_dir()
            .map(|d| d.join("4da"))
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        if let Ok(out) = std::process::Command::new("df")
            .arg("-BG")
            .arg(&data_dir)
            .output()
        {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // df output: header line + one data line per filesystem.
                // We want the last data line (covers the data_dir's mount).
                if let Some(last) = stdout.lines().rfind(|l| !l.is_empty()) {
                    // df output: Filesystem 1G-blocks Used Available Use% Mounted
                    let parts: Vec<&str> = last.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let avail_str = parts[3].trim_end_matches('G');
                        if let Ok(avail_gb) = avail_str.parse::<u64>() {
                            if avail_gb < 50 {
                                return Some(format!(
                                    "Low disk space: {}GB free on data partition",
                                    avail_gb
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command that never returns used to block `run_cmd` (and with it
    /// the sun registry) forever (audit O2).
    #[test]
    fn a_hung_command_is_killed_at_the_timeout() {
        #[cfg(target_os = "windows")]
        let slow = "ping -n 30 127.0.0.1";
        #[cfg(not(target_os = "windows"))]
        let slow = "sleep 30";
        let started = std::time::Instant::now();
        let r = run_cmd_with_timeout(slow, std::time::Duration::from_millis(500));
        assert!(r.is_err(), "a hung command is an error");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "returned at the timeout, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_quick_command_returns_its_output() {
        let out = run_cmd_with_timeout("echo fourda", std::time::Duration::from_secs(20)).unwrap();
        assert!(out.contains("fourda"), "{out}");
    }
}
