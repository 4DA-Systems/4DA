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
            crate::sovereign_profile::store_facts_from_execution(
                "cpu info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        let mem_output = run_cmd("powershell -NoProfile -Command \"(Get-CimInstance Win32_PhysicalMemory | Measure-Object -Property Capacity -Sum).Sum\"")
            .or_else(|_| run_cmd("wmic memorychip get capacity /format:list"));
        if let Ok(output) = mem_output {
            crate::sovereign_profile::store_facts_from_execution(
                "memory info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        let disk_output = run_cmd("powershell -NoProfile -Command \"Get-CimInstance Win32_DiskDrive | Select-Object -Property Size,Model | Format-List\"")
            .or_else(|_| run_cmd("wmic diskdrive get size,model /format:list"));
        if let Ok(output) = disk_output {
            crate::sovereign_profile::store_facts_from_execution(
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
            crate::sovereign_profile::store_facts_from_execution(
                "cpu info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        // Memory info
        if let Ok(output) = run_cmd("free -h") {
            crate::sovereign_profile::store_facts_from_execution(
                "memory info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        // Disk info (physical drives with size and model)
        if let Ok(output) = run_cmd("lsblk -d -o NAME,SIZE,MODEL --noheadings") {
            crate::sovereign_profile::store_facts_from_execution(
                "disk info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }

        // GPU info (VGA/3D/display controllers)
        if let Ok(output) = run_cmd("lspci | grep -iE 'vga|3d|display'") {
            crate::sovereign_profile::store_facts_from_execution(
                "gpu info",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = run_cmd("sysctl -n hw.ncpu") {
            crate::sovereign_profile::store_facts_from_execution(
                "sysctl -n hw.ncpu",
                &output,
                "sun:hardware",
            );
            facts_found += 1;
        }
        if let Ok(output) = run_cmd("sysctl -n hw.memsize") {
            crate::sovereign_profile::store_facts_from_execution(
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

/// Run a shell command and capture stdout.
pub(super) fn run_cmd(cmd: &str) -> Result<String> {
    debug!(target: "4da::suns", cmd, "Running system command");

    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("cmd").args(["/C", cmd]).output();

    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("sh").args(["-c", cmd]).output();

    match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).to_string().into()),
        Err(e) => Err(e.to_string().into()),
    }
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
                if let Some(last) = stdout.lines().filter(|l| !l.is_empty()).next_back() {
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
