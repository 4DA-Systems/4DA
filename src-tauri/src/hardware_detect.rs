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

/// The display-adapter device class. Each adapter is a numbered subkey that
/// carries its `DriverDesc` and, from WDDM drivers, a 64-bit
/// `HardwareInformation.qwMemorySize`.
#[cfg(target_os = "windows")]
const DISPLAY_CLASS_KEY: &str =
    r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Non-NVIDIA Windows GPUs. The registry is read first: `Win32_VideoController
/// .AdapterRAM` is a 32-bit field that saturates at 4 GiB, so WMI reported a
/// 16 GB card as 4,095 MB (and `wmic` is removed from current Windows 11).
/// Local judges are offered by VRAM tier, so a wrong 4 GB hid them from every
/// large AMD or Intel card.
#[cfg(target_os = "windows")]
fn detect_gpu_platform() -> Option<GpuInfo> {
    detect_gpu_registry().or_else(detect_gpu_wmic)
}

#[cfg(target_os = "windows")]
fn detect_gpu_registry() -> Option<GpuInfo> {
    let query = |value: &str| run_quiet("reg", &["query", DISPLAY_CLASS_KEY, "/s", "/v", value]);
    let sizes = query("HardwareInformation.qwMemorySize")?;
    let names = query("DriverDesc").unwrap_or_default();
    largest_registry_adapter(&sizes, &names)
}

#[cfg(target_os = "windows")]
fn detect_gpu_wmic() -> Option<GpuInfo> {
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
    largest_wmic_adapter(&output)
}

#[cfg(any(target_os = "windows", test))]
/// `reg query <key> /s /v <name>` output as (subkey path, raw value) pairs.
/// A value line follows its key line: `    <name>    <REG_TYPE>    <data>`.
fn parse_reg_values(output: &str) -> Vec<(String, String, String)> {
    let mut current_key = String::new();
    let mut values = Vec::new();
    for line in output.lines() {
        if line.starts_with("HKEY_") {
            current_key = line.trim().to_string();
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(_name), Some(kind), Some(first)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if !kind.starts_with("REG_") || current_key.is_empty() {
            continue;
        }
        let data = std::iter::once(first)
            .chain(parts)
            .collect::<Vec<_>>()
            .join(" ");
        values.push((current_key.clone(), kind.to_string(), data));
    }
    values
}

#[cfg(any(target_os = "windows", test))]
/// Bytes from a registry memory-size value: `REG_QWORD`/`REG_DWORD` as hex
/// (`0x3ff800000`), or `REG_BINARY` as little-endian hex bytes.
fn reg_bytes(kind: &str, data: &str) -> Option<u64> {
    match kind {
        "REG_QWORD" | "REG_DWORD" => {
            u64::from_str_radix(data.trim().trim_start_matches("0x"), 16).ok()
        }
        "REG_BINARY" => {
            let hex = data.trim();
            if hex.is_empty() || hex.len() > 16 || hex.len() % 2 != 0 {
                return None;
            }
            (0..hex.len()).step_by(2).rev().try_fold(0u64, |acc, i| {
                u8::from_str_radix(hex.get(i..i + 2)?, 16)
                    .ok()
                    .map(|b| (acc << 8) | u64::from(b))
            })
        }
        _ => None,
    }
}

#[cfg(any(target_os = "windows", test))]
/// The adapter with the most dedicated memory. Virtual displays (VR, remote
/// desktop) report no memory size, so they never win.
fn largest_registry_adapter(sizes: &str, names: &str) -> Option<GpuInfo> {
    let names = parse_reg_values(names);
    parse_reg_values(sizes)
        .into_iter()
        .filter_map(|(key, kind, data)| Some((key, reg_bytes(&kind, &data)?)))
        .filter(|(_, bytes)| *bytes > 0)
        .max_by_key(|(_, bytes)| *bytes)
        .map(|(key, bytes)| {
            let name = names
                .iter()
                .find(|(k, _, _)| k.eq_ignore_ascii_case(&key))
                .map(|(_, _, desc)| desc.clone())
                .unwrap_or_else(|| "Unknown GPU".to_string());
            GpuInfo {
                vendor: infer_vendor(&name),
                name,
                vram_mb: Some(bytes / (1024 * 1024)),
            }
        })
}

#[cfg(any(target_os = "windows", test))]
/// `AdapterRAM` at or above this is the 32-bit field saturating, not a size.
const WMI_ADAPTER_RAM_SATURATED: u64 = 0xFFF0_0000;

#[cfg(any(target_os = "windows", test))]
/// WMIC CSV (`Node,AdapterRAM,Name`): the adapter with the most memory. A
/// saturated `AdapterRAM` means "4 GiB or more", so its size is unknown.
fn largest_wmic_adapter(output: &str) -> Option<GpuInfo> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            let bytes = parts.get(1)?.trim().parse::<u64>().ok()?;
            let name = parts.get(2)?.trim();
            (!name.is_empty() && !name.eq_ignore_ascii_case("Name"))
                .then(|| (name.to_string(), bytes))
        })
        .max_by_key(|(_, bytes)| *bytes)
        .map(|(name, bytes)| GpuInfo {
            vendor: infer_vendor(&name),
            name,
            vram_mb: (bytes > 0 && bytes < WMI_ADAPTER_RAM_SATURATED)
                .then_some(bytes / (1024 * 1024)),
        })
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

    // `reg query` output captured on a machine with three virtual displays
    // ahead of the real card (2026-09-25). WMI reported this card as 4,095 MB.
    const REG_SIZES: &str = r"
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0002
    HardwareInformation.qwMemorySize    REG_QWORD    0x3ff800000

End of search: 1 match(es) found.
";
    const REG_NAMES: &str = r"
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000
    DriverDesc    REG_SZ    Virtual Desktop Monitor

HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0002
    DriverDesc    REG_SZ    NVIDIA GeForce RTX 4080 SUPER

End of search: 2 match(es) found.
";

    #[test]
    fn registry_reports_the_real_card_past_the_32bit_limit() {
        let gpu = largest_registry_adapter(REG_SIZES, REG_NAMES).expect("adapter");
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4080 SUPER");
        assert_eq!(gpu.vendor, "NVIDIA");
        assert_eq!(gpu.vram_mb, Some(16376), "same figure nvidia-smi reports");
    }

    #[test]
    fn registry_picks_the_largest_adapter_not_the_first() {
        let sizes = r"HKEY_LOCAL_MACHINE\X\0000
    HardwareInformation.qwMemorySize    REG_QWORD    0x20000000
HKEY_LOCAL_MACHINE\X\0001
    HardwareInformation.qwMemorySize    REG_QWORD    0x400000000
";
        let names = r"HKEY_LOCAL_MACHINE\X\0000
    DriverDesc    REG_SZ    Intel(R) UHD Graphics 770
HKEY_LOCAL_MACHINE\X\0001
    DriverDesc    REG_SZ    AMD Radeon RX 7800 XT
";
        let gpu = largest_registry_adapter(sizes, names).expect("adapter");
        assert_eq!(gpu.name, "AMD Radeon RX 7800 XT");
        assert_eq!(gpu.vendor, "AMD");
        assert_eq!(gpu.vram_mb, Some(16384));
    }

    #[test]
    fn registry_binary_and_dword_sizes_decode() {
        // 16 GiB as REG_BINARY little-endian, and 2 GiB as REG_DWORD.
        assert_eq!(
            reg_bytes("REG_BINARY", "0000000004000000"),
            Some(16 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            reg_bytes("REG_DWORD", "0x80000000"),
            Some(2 * 1024 * 1024 * 1024)
        );
        assert_eq!(reg_bytes("REG_SZ", "whatever"), None);
        assert_eq!(reg_bytes("REG_BINARY", "abc"), None);
        assert!(largest_registry_adapter("", "").is_none());
    }

    #[test]
    fn wmic_saturated_adapter_ram_is_unknown_not_4gb() {
        let csv = "
Node,AdapterRAM,Name
PC,,Virtual Desktop Monitor
PC,4293918720,NVIDIA GeForce RTX 4080 SUPER
PC,1073741824,Intel(R) UHD Graphics
";
        let gpu = largest_wmic_adapter(csv).expect("adapter");
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4080 SUPER");
        assert_eq!(gpu.vram_mb, None, "a saturated 32-bit field is not a size");
        let small = "Node,AdapterRAM,Name
PC,2147483648,AMD Radeon Vega 8
";
        assert_eq!(
            largest_wmic_adapter(small).expect("adapter").vram_mb,
            Some(2048)
        );
    }

    #[test]
    fn ram_tier_serializes() {
        let json = serde_json::to_value(RamTier::Tier32Plus).expect("serialization");
        assert_eq!(json, "Tier32Plus");
        let json = serde_json::to_value(RamTier::TierLow).expect("serialization");
        assert_eq!(json, "TierLow");
    }
}
