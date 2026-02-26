//! Uptime Monitor Sun -- tracks system uptime (every 5 min).

use super::SunResult;

pub fn execute() -> SunResult {
    let uptime_secs = get_system_uptime();

    let hours = uptime_secs / 3600;
    let days = hours / 24;

    // Store uptime fact in sovereign profile
    if uptime_secs > 0 {
        crate::sovereign_profile::store_facts_from_execution(
            "uptime",
            &format!(
                "{} seconds ({} days, {} hours)",
                uptime_secs,
                days,
                hours % 24
            ),
            "sun:uptime",
        );
    }

    SunResult {
        success: true,
        message: format!("System uptime: {}d {}h", days, hours % 24),
        data: Some(serde_json::json!({
            "uptime_secs": uptime_secs,
            "uptime_hours": hours,
            "uptime_days": days,
        })),
    }
}

fn get_system_uptime() -> u64 {
    #[cfg(target_os = "windows")]
    {
        // Parse wmic os get lastbootuptime output
        if let Ok(output) =
            super::hardware_monitor::run_cmd("wmic os get lastbootuptime /format:list")
        {
            for line in output.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("LastBootUpTime=") {
                    // Format: 20250101120000.000000+000
                    // Parse first 14 chars as YYYYMMDDHHmmss
                    let ts = val.trim();
                    if ts.len() >= 14 {
                        if let Ok(boot_time) =
                            chrono::NaiveDateTime::parse_from_str(&ts[..14], "%Y%m%d%H%M%S")
                        {
                            let now = chrono::Utc::now().naive_utc();
                            let duration = now.signed_duration_since(boot_time);
                            return duration.num_seconds().max(0) as u64;
                        }
                    }
                }
            }
        }
        0
    }

    #[cfg(target_os = "linux")]
    {
        return std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|s| {
                s.split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
            })
            .map(|s| s as u64)
            .unwrap_or(0);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = super::hardware_monitor::run_cmd("sysctl -n kern.boottime") {
            // Format: { sec = 1234567890, usec = 0 }
            if let Some(sec_start) = output.find("sec = ") {
                let after_sec = &output[sec_start + 6..];
                if let Some(comma) = after_sec.find(',') {
                    if let Ok(boot_sec) = after_sec[..comma].trim().parse::<u64>() {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        return now.saturating_sub(boot_sec);
                    }
                }
            }
        }
        return 0;
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        0
    }
}
