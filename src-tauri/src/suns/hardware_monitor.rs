//! Hardware Monitor Sun -- re-runs hardware detection daily, updates sovereign_profile.

use super::SunResult;
use tracing::debug;

use crate::error::Result;

pub fn execute() -> SunResult {
    let mut facts_found = 0;

    #[cfg(target_os = "windows")]
    {
        // Try PowerShell first (Windows 11+), fall back to wmic (Windows 10)
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
        if let Ok(output) = run_cmd("nproc") {
            crate::sovereign_profile::store_facts_from_execution("nproc", &output, "sun:hardware");
            facts_found += 1;
        }
        if let Ok(output) = run_cmd("free -h") {
            crate::sovereign_profile::store_facts_from_execution(
                "free -h",
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
        message: format!("Updated {} hardware facts", facts_found),
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
                    .or_else(|| trimmed.strip_prefix("FreeSpace :").map(|s| s.trim()));
                if let Some(val) = val {
                    if let Ok(free_bytes) = val.trim().parse::<u64>() {
                        let free_gb = free_bytes / (1024 * 1024 * 1024);
                        if free_gb < 50 {
                            return Some(format!(
                                "Low disk space: {}GB free on system drive",
                                free_gb
                            ));
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = run_cmd("df -BG / | tail -1") {
            // df output: Filesystem 1G-blocks Used Available Use% Mounted
            let parts: Vec<&str> = output.split_whitespace().collect();
            if parts.len() >= 4 {
                let avail_str = parts[3].trim_end_matches('G');
                if let Ok(avail_gb) = avail_str.parse::<u64>() {
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

    None
}
