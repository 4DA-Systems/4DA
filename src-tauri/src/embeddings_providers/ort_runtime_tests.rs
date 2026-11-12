// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use super::*;

/// The first bytes of real libraries, as each format lays them out.
fn pe(machine: u16) -> Vec<u8> {
    let mut h = vec![0u8; 0x200];
    h[0] = b'M';
    h[1] = b'Z';
    h[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    h[0x80..0x84].copy_from_slice(b"PE\0\0");
    h[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
    h
}

fn macho(cputype: u32) -> Vec<u8> {
    let mut h = 0xfeed_facfu32.to_le_bytes().to_vec();
    h.extend_from_slice(&cputype.to_le_bytes());
    h.extend_from_slice(&[0u8; 24]);
    h
}

fn elf(machine: u16) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"\x7fELF");
    h[18..20].copy_from_slice(&machine.to_le_bytes());
    h
}

#[test]
fn reads_the_cpu_of_each_library_format() {
    assert_eq!(lib_cpu(&pe(0x8664)), Some(LibCpu::X64));
    assert_eq!(lib_cpu(&pe(0xaa64)), Some(LibCpu::Arm64));
    assert_eq!(lib_cpu(&macho(0x0100_0007)), Some(LibCpu::X64));
    assert_eq!(lib_cpu(&macho(0x0100_000c)), Some(LibCpu::Arm64));
    assert_eq!(
        lib_cpu(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2]),
        Some(LibCpu::Universal)
    );
    assert_eq!(lib_cpu(&elf(0x3e)), Some(LibCpu::X64));
    assert_eq!(lib_cpu(&elf(0xb7)), Some(LibCpu::Arm64));
    assert_eq!(lib_cpu(b"not a library"), None);
    assert_eq!(lib_cpu(&[]), None);
    // A PE header pointer past the end of what was read is not a crash.
    let mut short = pe(0x8664);
    short.truncate(0x60);
    assert_eq!(lib_cpu(&short), None);
}

/// v1.0.3's Intel macOS app: an ARM64 library beside x86_64 binaries. It must
/// not be used on an Intel Mac; the same library is right on Apple silicon.
#[test]
fn an_arm64_library_does_not_run_on_an_intel_mac() {
    let shipped = lib_cpu(&macho(0x0100_000c));
    assert!(!runs_on(shipped, "x86_64"));
    assert!(runs_on(shipped, "aarch64"));
    assert!(runs_on(Some(LibCpu::Universal), "x86_64"));
    assert!(!runs_on(Some(LibCpu::X64), "aarch64"));
    // Unknown header: the loader decides, as before this check existed.
    assert!(runs_on(None, "x86_64"));
}

#[test]
fn every_downloadable_platform_is_pinned_and_intel_macos_says_why_not() {
    for (os, arch, kind) in [
        ("windows", "x86_64", ArchiveKind::Zip),
        ("windows", "aarch64", ArchiveKind::Zip),
        ("macos", "aarch64", ArchiveKind::Tgz),
        ("linux", "x86_64", ArchiveKind::Tgz),
        ("linux", "aarch64", ArchiveKind::Tgz),
    ] {
        let a = archive_for(os, arch).expect("a pinned archive");
        assert_eq!(a.kind, kind, "{os}/{arch}");
        assert_eq!(a.sha256.len(), 64, "{os}/{arch}");
        assert!(
            a.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "{os}/{arch}"
        );
        assert!(a
            .url
            .starts_with("https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/"));
        let tgz = a.url.ends_with(".tgz");
        assert_eq!(
            tgz,
            kind == ArchiveKind::Tgz,
            "{os}/{arch}: the kind follows the archive, not the temp file name"
        );
    }
    let intel = archive_for("macos", "x86_64").expect_err("no Intel macOS build exists");
    assert!(
        intel.contains("Intel Macs") && intel.contains("Ollama"),
        "{intel}"
    );
    assert!(archive_for("freebsd", "x86_64").is_err());
}

#[test]
fn the_digests_match_the_build_scripts_pins() {
    // scripts/download-ort.cjs pins the same five archives; a version bump must change both.
    let script = include_str!("../../../scripts/download-ort.cjs");
    for (os, arch) in [
        ("windows", "x86_64"),
        ("windows", "aarch64"),
        ("macos", "aarch64"),
        ("linux", "x86_64"),
        ("linux", "aarch64"),
    ] {
        let a = archive_for(os, arch).expect("a pinned archive");
        assert!(
            script.contains(a.sha256),
            "{os}/{arch}: {} is not pinned in download-ort.cjs",
            a.sha256
        );
    }
}
