// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! System hardware detection for local LLM model recommendations.
//!
//! Provides [`detect_hardware`] which probes RAM (via `sysinfo`) and GPU
//! (via platform-specific commands) then caches the result for the app
//! lifetime. GPU detection is best-effort — never blocks, never panics.

use serde::Serialize;
use std::sync::OnceLock;
use tracing::debug;

// ── Public types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HardwareInfo {
    pub ram_total_gb: f64,
    pub ram_available_gb: f64,
    pub gpu: Option<GpuInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GpuInfo {
    pub vendor: String,
    pub name: String,
    pub vram_mb: Option<u64>,
}

/// RAM capacity tier for model selection guidance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum RamTier {
    /// 32+ GB — can run 14B+ comfortably
    Tier32Plus,
    /// 16-31 GB — can run 14B with care, 12B comfortably
    Tier16,
    /// 8-15 GB — 8B models only
    Tier8,
    /// <8 GB — 4B or cloud-only
    TierLow,
}

// ── Cached entry point ─────────────────────────────────────────────

static HARDWARE_CACHE: OnceLock<HardwareInfo> = OnceLock::new();

/// Detect system hardware. Fast (~50ms), safe, no panics.
/// Caches result after first call (hardware doesn't change during app lifetime).
pub(crate) fn detect_hardware() -> HardwareInfo {
    HARDWARE_CACHE.get_or_init(detect_hardware_impl).clone()
}

/// Classify total RAM into a model-selection tier.
pub(crate) fn ram_tier(info: &HardwareInfo) -> RamTier {
    if info.ram_total_gb >= 32.0 {
        RamTier::Tier32Plus
    } else if info.ram_total_gb >= 16.0 {
        RamTier::Tier16
    } else if info.ram_total_gb >= 8.0 {
        RamTier::Tier8
    } else {
        RamTier::TierLow
    }
}

// ── Detection implementation ────────────────────────────────────────

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn detect_hardware_impl() -> HardwareInfo {
    use sysinfo::{MemoryRefreshKind, RefreshKind};
    let mut sys = sysinfo::System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

    let ram_total_gb = round1(sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0));
    let ram_available_gb = round1(sys.available_memory() as f64 / (1024.0 * 1024.0 * 1024.0));

    let gpu = detect_gpu();

    debug!(
        target: "4da::hardware",
        ram_total_gb,
        ram_available_gb,
        gpu_detected = gpu.is_some(),
        "Hardware detection complete"
    );

    HardwareInfo {
        ram_total_gb,
        ram_available_gb,
        gpu,
    }
}

// ── GPU detection (best-effort, platform-specific) ──────────────────

fn detect_gpu() -> Option<GpuInfo> {
    // Try NVIDIA first (cross-platform), then platform fallback.
    detect_nvidia().or_else(detect_gpu_platform)
}

/// NVIDIA GPU via `nvidia-smi`. Available on all platforms with NVIDIA drivers.
fn detect_nvidia() -> Option<GpuInfo> {
    let output = run_quiet(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ],
    )?;
    // Output: "NVIDIA GeForce RTX 4090, 24564"
    let line = output.lines().next()?;
    let (name, vram_str) = line.split_once(',')?;
    let vram_mb = vram_str.trim().parse::<u64>().ok();
    Some(GpuInfo {
        vendor: "NVIDIA".to_string(),
        name: name.trim().to_string(),
        vram_mb,
    })
}

#[cfg(target_os = "windows")]
fn detect_gpu_platform() -> Option<GpuInfo> {
    let output = run_quiet(
        "wmic",
        &[
            "path",
            "win32_VideoController",
            "get",
            "Name,AdapterRAM",
            "/format:csv",
        ],
    )?;
    // CSV output: Node,AdapterRAM,Name
    // First line is header, skip blanks.
    for line in output.lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 && parts[1].trim().parse::<u64>().is_ok() {
            let adapter_ram_bytes = parts[1].trim().parse::<u64>().unwrap_or(0);
            let name = parts[2].trim().to_string();
            if name.eq_ignore_ascii_case("Name") {
                continue; // skip header
            }
            return Some(GpuInfo {
                vendor: infer_vendor(&name),
                name,
                vram_mb: if adapter_ram_bytes > 0 {
                    Some(adapter_ram_bytes / (1024 * 1024))
                } else {
                    None
                },
            });
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_gpu_platform() -> Option<GpuInfo> {
    let output = run_quiet("system_profiler", &["SPDisplaysDataType", "-json"])?;
    let parsed: serde_json::Value = serde_json::from_str(&output).ok()?;
    let displays = parsed.get("SPDisplaysDataType")?.as_array()?;
    let gpu = displays.first()?;
    let name = gpu.get("sppci_model")?.as_str()?.to_string();
    let vram_str = gpu.get("sppci_vram")?.as_str().unwrap_or("");
    let vram_mb = vram_str
        .split_whitespace()
        .next()
        .and_then(|n| n.parse::<u64>().ok())
        .map(|n| if vram_str.contains("GB") { n * 1024 } else { n });
    Some(GpuInfo {
        vendor: infer_vendor(&name),
        name,
        vram_mb,
    })
}

#[cfg(target_os = "linux")]
fn detect_gpu_platform() -> Option<GpuInfo> {
    let output = run_quiet("lspci", &[])?;
    for line in output.lines() {
        let lower = line.to_lowercase();
        if lower.contains("vga") || lower.contains("3d") || lower.contains("display") {
            // "01:00.0 VGA compatible controller: NVIDIA Corporation ..."
            let name = line.split(':').next_back()?.trim().to_string();
            return Some(GpuInfo {
                vendor: infer_vendor(&name),
                name,
                vram_mb: None, // lspci doesn't report VRAM
            });
        }
    }
    None
}

fn infer_vendor(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro") {
        "NVIDIA".to_string()
    } else if lower.contains("amd") || lower.contains("radeon") {
        "AMD".to_string()
    } else if lower.contains("intel") {
        "Intel".to_string()
    } else if lower.contains("apple") {
        "Apple".to_string()
    } else {
        "Unknown".to_string()
    }
}

// ── Command runner (3s timeout, no window flash) ────────────────────

fn run_quiet(program: &str, args: &[&str]) -> Option<String> {
    use std::process::Command;
    use std::time::Duration;

    let mut cmd = Command::new(program);
    cmd.args(args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // 3-second timeout: spawn a thread for wait_with_output, join with deadline.
    let handle = std::thread::spawn(move || child.wait_with_output());
    let timeout = Duration::from_secs(3);
    let start = std::time::Instant::now();

    // Poll join — std::thread::JoinHandle has no timed join, so spin briefly.
    loop {
        if handle.is_finished() {
            return match handle.join() {
                Ok(Ok(output)) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                }
                _ => None,
            };
        }
        if start.elapsed() >= timeout {
            debug!(target: "4da::hardware", program, "GPU command timed out after 3s");
            return None; // Thread + child are abandoned; OS will clean up.
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_tier_boundaries() {
        let make = |gb: f64| HardwareInfo {
            ram_total_gb: gb,
            ram_available_gb: gb * 0.5,
            gpu: None,
        };
        assert_eq!(ram_tier(&make(64.0)), RamTier::Tier32Plus);
        assert_eq!(ram_tier(&make(32.0)), RamTier::Tier32Plus);
        assert_eq!(ram_tier(&make(31.9)), RamTier::Tier16);
        assert_eq!(ram_tier(&make(16.0)), RamTier::Tier16);
        assert_eq!(ram_tier(&make(15.9)), RamTier::Tier8);
        assert_eq!(ram_tier(&make(8.0)), RamTier::Tier8);
        assert_eq!(ram_tier(&make(7.9)), RamTier::TierLow);
        assert_eq!(ram_tier(&make(4.0)), RamTier::TierLow);
    }

    #[test]
    fn hardware_info_serializes() {
        let info = HardwareInfo {
            ram_total_gb: 31.8,
            ram_available_gb: 14.2,
            gpu: Some(GpuInfo {
                vendor: "NVIDIA".to_string(),
                name: "NVIDIA GeForce RTX 4090".to_string(),
                vram_mb: Some(24564),
            }),
        };
        let json = serde_json::to_value(&info).expect("serialization");
        assert_eq!(json["ram_total_gb"], 31.8);
        assert_eq!(json["gpu"]["vram_mb"], 24564);
        assert_eq!(json["gpu"]["vendor"], "NVIDIA");
    }

    #[test]
    fn hardware_info_serializes_without_gpu() {
        let info = HardwareInfo {
            ram_total_gb: 8.0,
            ram_available_gb: 3.5,
            gpu: None,
        };
        let json = serde_json::to_value(&info).expect("serialization");
        assert!(json["gpu"].is_null());
    }

    #[test]
    fn detect_hardware_returns_nonzero_ram() {
        let info = detect_hardware();
        assert!(info.ram_total_gb > 0.0, "total RAM must be positive");
        assert!(
            info.ram_available_gb >= 0.0,
            "available RAM must be non-negative"
        );
        assert!(
            info.ram_available_gb <= info.ram_total_gb,
            "available must not exceed total"
        );
    }

    #[test]
    fn detect_hardware_is_cached() {
        let a = detect_hardware();
        let b = detect_hardware();
        // Exact bit equality — same cached value.
        assert_eq!(a.ram_total_gb, b.ram_total_gb);
        assert_eq!(a.ram_available_gb, b.ram_available_gb);
    }

    #[test]
    fn round1_precision() {
        assert_eq!(round1(15.96), 16.0);
        assert_eq!(round1(7.849), 7.8);
        assert_eq!(round1(32.05), 32.1);
        assert_eq!(round1(0.0), 0.0);
    }

    #[test]
    fn infer_vendor_coverage() {
        assert_eq!(infer_vendor("NVIDIA GeForce RTX 3080"), "NVIDIA");
        assert_eq!(infer_vendor("AMD Radeon RX 7900 XTX"), "AMD");
        assert_eq!(infer_vendor("Intel UHD 770"), "Intel");
        assert_eq!(infer_vendor("Apple M2 Pro"), "Apple");
        assert_eq!(infer_vendor("Some Other GPU"), "Unknown");
        // Case insensitive
        assert_eq!(infer_vendor("geforce gtx 1080"), "NVIDIA");
        assert_eq!(infer_vendor("Quadro P4000"), "NVIDIA");
    }

    #[test]
    fn ram_tier_serializes() {
        let json = serde_json::to_value(RamTier::Tier32Plus).expect("serialization");
        assert_eq!(json, "Tier32Plus");
        let json = serde_json::to_value(RamTier::TierLow).expect("serialization");
        assert_eq!(json, "TierLow");
    }
}
