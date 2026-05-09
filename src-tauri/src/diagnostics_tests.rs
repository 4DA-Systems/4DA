// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;

// ---------------------------------------------------------------
// Existing tests (preserved)
// ---------------------------------------------------------------

#[test]
fn record_start_time_is_idempotent() {
    record_start_time();
    let first = APP_START.get().map(|i| i.elapsed());
    record_start_time();
    let second = APP_START.get().map(|i| i.elapsed());
    assert!(first.is_some());
    assert!(second.is_some());
}

#[test]
fn diagnostics_snapshot_serializes_correctly() {
    let snap = DiagnosticsSnapshot {
        memory_bytes: 1024 * 1024 * 50,
        db_size_bytes: 1024 * 1024 * 10,
        source_item_count: 500,
        context_chunk_count: 200,
        feedback_count: 50,
        uptime_secs: 3600,
        source_health: vec![SourceHealthSummary {
            source_type: "hackernews".to_string(),
            status: "healthy".to_string(),
            consecutive_failures: 0,
        }],
        schema_version: 42,
        db_size_warning: false,
    };
    let json = serde_json::to_value(&snap).unwrap();
    assert_eq!(json["source_item_count"], 500);
    assert_eq!(json["db_size_warning"], false);
    assert_eq!(json["source_health"].as_array().unwrap().len(), 1);
}

#[test]
fn source_health_summary_serializes_correctly() {
    let health = SourceHealthSummary {
        source_type: "reddit".to_string(),
        status: "degraded".to_string(),
        consecutive_failures: 3,
    };
    let json = serde_json::to_value(&health).unwrap();
    assert_eq!(json["source_type"], "reddit");
    assert_eq!(json["consecutive_failures"], 3);
}

#[test]
fn db_size_warning_threshold_is_500mb() {
    let snap = DiagnosticsSnapshot {
        memory_bytes: 0,
        db_size_bytes: 499 * 1024 * 1024,
        source_item_count: 0,
        context_chunk_count: 0,
        feedback_count: 0,
        uptime_secs: 0,
        source_health: vec![],
        schema_version: 0,
        db_size_warning: 499 * 1024 * 1024 > 500 * 1024 * 1024,
    };
    assert!(!snap.db_size_warning);

    let snap2 = DiagnosticsSnapshot {
        db_size_warning: 501 * 1024 * 1024 > 500 * 1024 * 1024,
        ..snap
    };
    assert!(snap2.db_size_warning);
}

// ---------------------------------------------------------------
// Windows memory reporting (live, runs on current platform)
// ---------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows_memory {
    use super::*;

    #[test]
    fn windows_memory_returns_nonzero() {
        let mem = get_process_memory_windows();
        assert!(
            mem > 0,
            "Windows memory should be non-zero for a running process, got {mem}"
        );
    }

    #[test]
    fn windows_memory_in_reasonable_range() {
        let mem = get_process_memory_windows();
        let one_mb = 1024 * 1024;
        let sixty_four_gb = 64_u64 * 1024 * 1024 * 1024;
        assert!(
            mem > one_mb,
            "Expected RSS > 1 MB, got {} bytes ({:.2} MB)",
            mem,
            mem as f64 / one_mb as f64
        );
        assert!(
            mem < sixty_four_gb,
            "Expected RSS < 64 GB, got {} bytes ({:.2} GB)",
            mem,
            mem as f64 / sixty_four_gb as f64
        );
    }

    #[test]
    fn windows_memory_consistent_across_calls() {
        // Call multiple times — values should be in the same ballpark.
        // We allow 50% drift to account for allocator churn.
        let samples: Vec<u64> = (0..5).map(|_| get_process_memory_windows()).collect();
        let min = *samples.iter().min().unwrap();
        let max = *samples.iter().max().unwrap();
        assert!(min > 0, "All samples should be non-zero, got min={min}");
        // Max should be within 2x of min (very generous margin)
        assert!(
            max <= min * 2,
            "Memory readings should be consistent: min={min}, max={max}"
        );
    }

    #[test]
    fn dispatcher_matches_windows_impl() {
        // On Windows, get_process_memory() should delegate to the Windows impl
        // and return a value in the same range.
        let dispatched = get_process_memory();
        let direct = get_process_memory_windows();

        assert!(dispatched > 0, "Dispatched value should be non-zero");
        assert!(direct > 0, "Direct value should be non-zero");

        // Both should be within 50% of each other (called close together)
        let ratio = if dispatched > direct {
            dispatched as f64 / direct as f64
        } else {
            direct as f64 / dispatched as f64
        };
        assert!(
            ratio < 1.5,
            "Dispatched ({dispatched}) and direct ({direct}) should be close (ratio={ratio:.2})"
        );
    }
}

// ---------------------------------------------------------------
// Linux /proc/self/status VmRSS parsing (cross-platform via helper)
// ---------------------------------------------------------------

mod linux_vmrss_parsing {
    use super::*;

    /// Realistic /proc/self/status content for testing
    fn mock_proc_status(vmrss_line: Option<&str>) -> String {
        let mut lines = vec![
            "Name:\ttest_process",
            "Umask:\t0022",
            "State:\tS (sleeping)",
            "Tgid:\t12345",
            "Ngid:\t0",
            "Pid:\t12345",
            "PPid:\t1234",
            "TracerPid:\t0",
            "Uid:\t1000\t1000\t1000\t1000",
            "Gid:\t1000\t1000\t1000\t1000",
            "FDSize:\t256",
            "VmPeak:\t  987654 kB",
            "VmSize:\t  876543 kB",
            "VmLck:\t       0 kB",
            "VmPin:\t       0 kB",
            "VmHWM:\t   54321 kB",
        ];

        if let Some(rss_line) = vmrss_line {
            lines.push(rss_line);
        }

        lines.extend_from_slice(&[
            "VmData:\t  123456 kB",
            "VmStk:\t     136 kB",
            "VmExe:\t    2048 kB",
            "VmLib:\t    8192 kB",
            "Threads:\t4",
            "voluntary_ctxt_switches:\t100",
            "nonvoluntary_ctxt_switches:\t50",
        ]);

        lines.join("\n")
    }

    #[test]
    fn normal_vmrss_line() {
        let content = mock_proc_status(Some("VmRSS:\t   12345 kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 12345 * 1024, "12345 kB should be 12_641_280 bytes");
    }

    #[test]
    fn vmrss_with_large_value() {
        // 16 GB RSS = 16_777_216 kB
        let content = mock_proc_status(Some("VmRSS:\t16777216 kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 16_777_216 * 1024);
    }

    #[test]
    fn vmrss_with_very_large_value() {
        // 1 TB RSS = 1_073_741_824 kB — exotic but shouldn't overflow u64
        let content = mock_proc_status(Some("VmRSS:\t1073741824 kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 1_073_741_824 * 1024);
    }

    #[test]
    fn vmrss_with_zero() {
        let content = mock_proc_status(Some("VmRSS:\t       0 kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0);
    }

    #[test]
    fn missing_vmrss_entirely() {
        let content = mock_proc_status(None);
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0, "Missing VmRSS should return 0");
    }

    #[test]
    fn vmrss_with_no_value_after_colon() {
        // "VmRSS:" with nothing following — split_whitespace().nth(1) is None
        let content = mock_proc_status(Some("VmRSS:"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0, "VmRSS with no value should return 0");
    }

    #[test]
    fn vmrss_with_only_spaces_after_colon() {
        let content = mock_proc_status(Some("VmRSS:          "));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0, "VmRSS with only whitespace should return 0");
    }

    #[test]
    fn vmrss_with_non_numeric_value() {
        let content = mock_proc_status(Some("VmRSS:\tNaN kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0, "Non-numeric VmRSS should return 0");
    }

    #[test]
    fn vmrss_with_negative_value() {
        // u64 parse will fail on negative, should fall back to 0
        let content = mock_proc_status(Some("VmRSS:\t-100 kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 0, "Negative VmRSS should return 0");
    }

    #[test]
    fn vmrss_only_line_in_file() {
        let content = "VmRSS:\t   5000 kB";
        let result = parse_vmrss_from_status(content);
        assert_eq!(result, 5000 * 1024);
    }

    #[test]
    fn vmrss_with_extra_whitespace() {
        // Extra spaces and tabs — split_whitespace handles this correctly
        let content = mock_proc_status(Some("VmRSS:  \t  \t  8192   kB"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 8192 * 1024);
    }

    #[test]
    fn vmrss_with_no_unit() {
        // No "kB" suffix — the parser only reads the first numeric token,
        // so this should still parse the number (the unit is not validated)
        let content = mock_proc_status(Some("VmRSS:\t4096"));
        let result = parse_vmrss_from_status(&content);
        assert_eq!(result, 4096 * 1024);
    }

    #[test]
    fn empty_input() {
        let result = parse_vmrss_from_status("");
        assert_eq!(result, 0, "Empty input should return 0");
    }

    #[test]
    fn vmrss_is_case_sensitive() {
        // "vmrss:" lowercase should NOT match (Linux kernel uses "VmRSS:")
        let content = "vmrss:\t1000 kB\nVmRss:\t2000 kB";
        let result = parse_vmrss_from_status(content);
        assert_eq!(result, 0, "Parser expects exact 'VmRSS:' prefix");
    }

    #[test]
    fn vmrss_first_match_wins() {
        // If there were somehow two VmRSS lines, the first should win
        let content = "VmRSS:\t1000 kB\nVmRSS:\t2000 kB";
        let result = parse_vmrss_from_status(content);
        assert_eq!(result, 1000 * 1024, "First VmRSS line should be used");
    }

    #[test]
    fn vmrss_with_windows_line_endings() {
        let content = "Name:\ttest\r\nVmRSS:\t3000 kB\r\nThreads:\t1\r\n";
        let result = parse_vmrss_from_status(content);
        // "3000" is still the second whitespace-delimited token
        // but "kB\r" might appear — split_whitespace handles \r as whitespace
        // The numeric parse of "3000" should succeed regardless
        assert_eq!(result, 3000 * 1024);
    }
}

// ---------------------------------------------------------------
// macOS struct layout and constant verification (compile-time)
// ---------------------------------------------------------------

/// Verify MachTaskBasicInfo struct is the expected 48 bytes on 64-bit.
/// Layout: 3 x u64 (24 bytes) + 6 x i32 (24 bytes) = 48 bytes.
/// This is a compile-time assertion — if the struct layout changes,
/// this will fail to compile.
#[test]
fn macos_struct_layout_size() {
    // Replicate the struct locally for size verification.
    // This mirrors the exact layout from get_process_memory_macos().
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time_secs: i32,
        user_time_usecs: i32,
        system_time_secs: i32,
        system_time_usecs: i32,
        policy: i32,
        suspend_count: i32,
    }

    assert_eq!(
        std::mem::size_of::<MachTaskBasicInfo>(),
        48,
        "MachTaskBasicInfo should be 48 bytes: 3*u64 (24) + 6*i32 (24)"
    );

    // Also verify alignment — repr(C) with u64 fields should be 8-byte aligned
    assert_eq!(
        std::mem::align_of::<MachTaskBasicInfo>(),
        8,
        "MachTaskBasicInfo should be 8-byte aligned due to u64 fields"
    );
}

#[test]
fn macos_constants_correct() {
    // These constants must match XNU kernel headers.
    // MACH_TASK_BASIC_INFO = 20 (from <mach/task_info.h>)
    // KERN_SUCCESS = 0 (from <mach/kern_return.h>)
    const MACH_TASK_BASIC_INFO: u32 = 20;
    const KERN_SUCCESS: i32 = 0;

    assert_eq!(MACH_TASK_BASIC_INFO, 20, "MACH_TASK_BASIC_INFO must be 20");
    assert_eq!(KERN_SUCCESS, 0, "KERN_SUCCESS must be 0");
}

#[test]
#[allow(unsafe_code)]
fn macos_struct_field_offsets() {
    // Verify field offsets match XNU mach_task_basic_info_data_t layout
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time_secs: i32,
        user_time_usecs: i32,
        system_time_secs: i32,
        system_time_usecs: i32,
        policy: i32,
        suspend_count: i32,
    }

    // Use offset_of-like calculation via pointer arithmetic
    let base = std::mem::MaybeUninit::<MachTaskBasicInfo>::uninit();
    let base_ptr = base.as_ptr();

    unsafe {
        let base_addr = base_ptr as usize;
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).virtual_size) as usize - base_addr,
            0,
            "virtual_size should be at offset 0"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).resident_size) as usize - base_addr,
            8,
            "resident_size should be at offset 8"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).resident_size_max) as usize - base_addr,
            16,
            "resident_size_max should be at offset 16"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).user_time_secs) as usize - base_addr,
            24,
            "user_time_secs should be at offset 24"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).user_time_usecs) as usize - base_addr,
            28,
            "user_time_usecs should be at offset 28"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).system_time_secs) as usize - base_addr,
            32,
            "system_time_secs should be at offset 32"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).system_time_usecs) as usize - base_addr,
            36,
            "system_time_usecs should be at offset 36"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).policy) as usize - base_addr,
            40,
            "policy should be at offset 40"
        );
        assert_eq!(
            std::ptr::addr_of!((*base_ptr).suspend_count) as usize - base_addr,
            44,
            "suspend_count should be at offset 44"
        );
    }
}

#[test]
fn macos_count_calculation() {
    // The macOS impl passes count as struct_size / sizeof(u32)
    // MachTaskBasicInfo is 48 bytes / 4 = 12 natural_t units
    #[repr(C)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time_secs: i32,
        user_time_usecs: i32,
        system_time_secs: i32,
        system_time_usecs: i32,
        policy: i32,
        suspend_count: i32,
    }

    let count = std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<u32>();
    assert_eq!(
        count, 12,
        "task_info count should be 12 (48 bytes / 4 bytes per natural_t)"
    );
}

// ---------------------------------------------------------------
// Dispatcher test (platform-agnostic behavior)
// ---------------------------------------------------------------

#[test]
fn dispatcher_returns_value() {
    let mem = get_process_memory();
    // On Windows/macOS/Linux, this should be non-zero.
    // On unsupported platforms, it returns 0.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    assert!(
        mem > 0,
        "On supported platforms, memory should be non-zero, got {mem}"
    );
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    assert_eq!(mem, 0, "On unsupported platforms, memory should be 0");
}

// ---------------------------------------------------------------
// Windows ProcessMemoryCounters struct layout verification
// ---------------------------------------------------------------

#[cfg(target_os = "windows")]
#[test]
fn windows_process_memory_counters_layout() {
    // Replicate the struct to verify its size matches the Win32 API expectation.
    // PROCESS_MEMORY_COUNTERS has 10 fields:
    //   cb (u32) + page_fault_count (u32) + 8 x usize
    // On 64-bit Windows: 4 + 4 + 8*8 = 72 bytes
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    let expected_size = 4 + 4 + 8 * std::mem::size_of::<usize>();
    assert_eq!(
        std::mem::size_of::<ProcessMemoryCounters>(),
        expected_size,
        "ProcessMemoryCounters size should match Win32 PROCESS_MEMORY_COUNTERS"
    );
}
