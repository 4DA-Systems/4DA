// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The ONNX Runtime archive this platform downloads, pinned by SHA-256, and a
//! check that a runtime library is built for this machine's CPU (audit
//! 2026-11-11, P4 and R10).
//!
//! The library is loaded into the app's process, and its download was trusted
//! on HTTP success alone. The digests are GitHub's own for the v1.24.2 release
//! assets (`gh api repos/microsoft/onnxruntime/releases/tags/v1.24.2`, field
//! `digest`), checked 2026-11-11; `scripts/download-ort.cjs` pins the same
//! five, and a version bump must update both.
//!
//! Intel macOS has no archive: Microsoft published none after 1.23.2, and the
//! `ort` crate here needs ONNX Runtime API 24 (>= 1.24). v1.0.3's Intel app
//! shipped an ARM64 library, which no Intel Mac can load; the CPU check below
//! keeps such a library from being used, so the reason a user sees is the real
//! one rather than a loader error.

/// Archive format of a runtime download: decides how the library is extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveKind {
    Zip,
    Tgz,
}

/// A pinned ONNX Runtime release archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrtArchive {
    pub url: &'static str,
    pub sha256: &'static str,
    pub kind: ArchiveKind,
}

/// Why Intel macOS has no runtime to download (the error a user is shown).
pub(crate) const INTEL_MAC_UNAVAILABLE: &str = "Local embeddings are not available on Intel Macs: \
ONNX Runtime 1.24, which this version of 4DA needs, publishes no Intel macOS build. \
Use Ollama for embeddings (Settings > AI), or run 4DA on an Apple-silicon Mac.";

/// The pinned archive for the platform this binary was built for, or why there is none.
pub(crate) fn host_archive() -> std::result::Result<OrtArchive, &'static str> {
    archive_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// A v1.24.2 release asset URL.
macro_rules! ort_url {
    ($file:literal) => {
        concat!(
            "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/",
            $file
        )
    };
}

/// The pinned archive for an OS and CPU (`std::env::consts` names).
pub(crate) fn archive_for(os: &str, arch: &str) -> std::result::Result<OrtArchive, &'static str> {
    let (url, sha256, kind) = match (os, arch) {
        ("windows", "x86_64") => (
            ort_url!("onnxruntime-win-x64-1.24.2.zip"),
            "8e3e9c826375352e29cb2614fe44f3d7a4b0ff7b8028ad7a456af9d949a7e8b0",
            ArchiveKind::Zip,
        ),
        ("windows", "aarch64") => (
            ort_url!("onnxruntime-win-arm64-1.24.2.zip"),
            "dd8180d98e5a0ead7ead99029acc80b86a8b905b9aba4cc978e388039bb5823b",
            ArchiveKind::Zip,
        ),
        ("macos", "aarch64") => (
            ort_url!("onnxruntime-osx-arm64-1.24.2.tgz"),
            "0af4fa503e8ea285245b47ee42d0a7461b8156a81270857da0c1d4ecf858abde",
            ArchiveKind::Tgz,
        ),
        ("macos", "x86_64") => return Err(INTEL_MAC_UNAVAILABLE),
        ("linux", "x86_64") => (
            ort_url!("onnxruntime-linux-x64-1.24.2.tgz"),
            "43725474ba5663642e17684717946693850e2005efbd724ac72da278fead25e6",
            ArchiveKind::Tgz,
        ),
        ("linux", "aarch64") => (
            ort_url!("onnxruntime-linux-aarch64-1.24.2.tgz"),
            "6715b3d19965a2a6981e78ed4ba24f17a8c30d2d26420dbed10aac7ceca0085e",
            ArchiveKind::Tgz,
        ),
        _ => return Err("ONNX Runtime publishes no build for this platform, so local embeddings are not available. Use Ollama for embeddings (Settings > AI)."),
    };
    Ok(OrtArchive { url, sha256, kind })
}

/// The CPU a native library is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibCpu {
    X64,
    Arm64,
    /// A macOS fat (universal) binary: runs on either.
    Universal,
}

/// The CPU a library's header names: PE (Windows), Mach-O 64-bit or fat
/// (macOS), ELF (Linux). `None` for a header this does not recognise.
pub(crate) fn lib_cpu(header: &[u8]) -> Option<LibCpu> {
    let u16_le = |at: usize| {
        header
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_le = |at: usize| {
        header
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if header.starts_with(b"MZ") {
        let pe = u32_le(0x3c)? as usize;
        if u32_le(pe)? != 0x0000_4550 {
            return None;
        }
        return match u16_le(pe + 4)? {
            0x8664 => Some(LibCpu::X64),
            0xaa64 => Some(LibCpu::Arm64),
            _ => None,
        };
    }
    if u32_le(0)? == 0xfeed_facf {
        return match u32_le(4)? {
            0x0100_0007 => Some(LibCpu::X64),
            0x0100_000c => Some(LibCpu::Arm64),
            _ => None,
        };
    }
    if header.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        return Some(LibCpu::Universal);
    }
    if header.starts_with(b"\x7fELF") {
        return match u16_le(18)? {
            0x3e => Some(LibCpu::X64),
            0xb7 => Some(LibCpu::Arm64),
            _ => None,
        };
    }
    None
}

/// Whether a library built for `found` runs on `host` (`std::env::consts::ARCH`).
pub(crate) fn runs_on(found: Option<LibCpu>, host: &str) -> bool {
    match found {
        Some(LibCpu::Universal) => true,
        Some(LibCpu::X64) => host == "x86_64",
        Some(LibCpu::Arm64) => host == "aarch64",
        // A header this does not recognise: let the loader decide, as before.
        None => true,
    }
}

/// Whether the library at `path` is built for this machine's CPU (a library
/// that cannot be read is left for the loader to report).
pub(crate) fn lib_runs_here(path: &std::path::Path) -> bool {
    use std::io::Read as _;
    let mut header = vec![0u8; 4096];
    let read = std::fs::File::open(path).and_then(|mut f| f.read(&mut header));
    match read {
        Ok(n) => runs_on(lib_cpu(&header[..n]), std::env::consts::ARCH),
        Err(_) => true,
    }
}

#[cfg(test)]
#[path = "ort_runtime_tests.rs"]
mod tests;
