// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Where the ONNX Runtime library this platform downloads comes from, pinned
//! by SHA-256, and a check that a runtime library is built for this machine's
//! CPU (audit 2026-11-11, P4 and R10).
//!
//! The library is loaded into the app's process, so its download is trusted on
//! nothing but its digests. It comes out of Microsoft's NuGet package
//! Microsoft.ML.OnnxRuntime, one zip that holds every platform's library under
//! `runtimes/<rid>/native/`, and it is pinned twice: the package's SHA-256 is
//! checked before the package is opened, and the library's before it is given
//! its own name. The GitHub release archives this used to download went away
//! with the microsoft/onnxruntime repository, which has answered 404 since
//! about 2026-11-17. `scripts/download-ort.cjs` pins the same package and the
//! same five libraries; a test holds the two equal, so a version bump must
//! update both.
//!
//! Intel macOS has no library: Microsoft published none after 1.23.2, and the
//! `ort` crate here needs ONNX Runtime API 24 (>= 1.24). v1.0.3's Intel app
//! shipped an ARM64 library, which no Intel Mac can load; the CPU check below
//! keeps such a library from being used, so the reason a user sees is the real
//! one rather than a loader error.

use std::io::{Read, Seek as _, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

/// The ONNX Runtime release every pin below belongs to.
pub(crate) const ORT_VERSION: &str = "1.24.2";

/// Microsoft's NuGet package of that release.
pub(crate) const PACKAGE_URL: &str = "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.24.2/microsoft.ml.onnxruntime.1.24.2.nupkg";

/// The package's SHA-256 (the same bytes carry the SHA-512 `packageHash` in
/// nuget.org's own catalog entry for 1.24.2).
pub(crate) const PACKAGE_SHA256: &str =
    "782564d3d68e87269ea1812cc94710ad06fe014faf093f01a6f89d0bd58719d3";

/// The pinned package is 124,756,536 bytes. A response that says or turns out
/// to be larger than this is stopped before it can fill the disk.
pub(crate) const PACKAGE_MAX_BYTES: u64 = 128 * 1024 * 1024;

/// One platform's library in the package: its full path there and its SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrtLibrary<'a> {
    pub member: &'a str,
    pub sha256: &'a str,
}

/// Why Intel macOS has no runtime to download (the error a user is shown).
pub(crate) const INTEL_MAC_UNAVAILABLE: &str = "Local embeddings are not available on Intel Macs: \
ONNX Runtime 1.24, which this version of 4DA needs, publishes no Intel macOS build. \
Use Ollama for embeddings (Settings > AI), or run 4DA on an Apple-silicon Mac.";

/// The pinned library for the platform this binary was built for, or why there is none.
pub(crate) fn host_library() -> std::result::Result<OrtLibrary<'static>, &'static str> {
    library_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// The pinned library for an OS and CPU (`std::env::consts` names).
pub(crate) fn library_for(
    os: &str,
    arch: &str,
) -> std::result::Result<OrtLibrary<'static>, &'static str> {
    let (member, sha256) = match (os, arch) {
        ("windows", "x86_64") => (
            "runtimes/win-x64/native/onnxruntime.dll",
            "114947d633e6844ce3c4b51ef6678f776628571d08a5763859c61642c8dcca9c",
        ),
        ("windows", "aarch64") => (
            "runtimes/win-arm64/native/onnxruntime.dll",
            "d4c4d939c8bd1e93a86bc5a45b37a1fdd08dce34d8231db014a7e4a023923f5a",
        ),
        ("macos", "aarch64") => (
            "runtimes/osx-arm64/native/libonnxruntime.dylib",
            "8e88284d3b4395b1e532831ac05eb0ffa3673a62df32349e1b0fd64542cb827d",
        ),
        ("macos", "x86_64") => return Err(INTEL_MAC_UNAVAILABLE),
        ("linux", "x86_64") => (
            "runtimes/linux-x64/native/libonnxruntime.so",
            "ffc84d48e845cf0b562ba4ea5ca32aaafc0d4069019fef4f63095b307d0270ad",
        ),
        ("linux", "aarch64") => (
            "runtimes/linux-arm64/native/libonnxruntime.so",
            "e18fe095919d8613ead3a31ff78212bde4fad929418a9b48f49d61c829ed5c82",
        ),
        _ => return Err("ONNX Runtime publishes no build for this platform, so local embeddings are not available. Use Ollama for embeddings (Settings > AI)."),
    };
    Ok(OrtLibrary { member, sha256 })
}

/// Why a downloaded package did not yield a library this app will load.
#[derive(Debug, thiserror::Error)]
pub(crate) enum OrtPackageError {
    #[error("ONNX Runtime package rejected: its SHA-256 is {found}, expected {expected}. Nothing was taken from it")]
    PackageDigest { found: String, expected: String },
    #[error("ONNX Runtime library rejected: {member} in the package has SHA-256 {found}, expected {expected}")]
    LibraryDigest {
        member: String,
        found: String,
        expected: String,
    },
    #[error("ONNX Runtime package has no {0}")]
    MissingMember(String),
    #[error("ONNX Runtime package: {0}")]
    Unreadable(String),
}

/// Take `lib.member` out of the package at `package` and put it at `dest`.
///
/// The package is hashed through the same open handle the zip reader then
/// uses, so the bytes checked are the bytes read. The member is looked up by
/// its full path only: the package holds five libraries under two file names,
/// so a file of the same name elsewhere is never taken, and no name from the
/// archive is used to build a path on disk. The library is written to
/// `<dest>.tmp` and gets `dest`'s name only once its own SHA-256 matches.
pub(crate) fn extract_library(
    package: &Path,
    package_sha256: &str,
    lib: OrtLibrary<'_>,
    dest: &Path,
) -> std::result::Result<(), OrtPackageError> {
    let mut file = std::fs::File::open(package).map_err(|e| unreadable("open", e))?;
    let found = copy_hashing(&mut file, &mut std::io::sink()).map_err(|e| unreadable("read", e))?;
    if found != package_sha256 {
        return Err(OrtPackageError::PackageDigest {
            found,
            expected: package_sha256.to_string(),
        });
    }
    file.rewind().map_err(|e| unreadable("rewind", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| unreadable("not a zip", e))?;
    let entry = match archive.by_name(lib.member) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(OrtPackageError::MissingMember(lib.member.to_string()))
        }
        Err(e) => return Err(unreadable(lib.member, e)),
    };
    let tmp = tmp_path(dest);
    let written = std::fs::File::create(&tmp).and_then(|mut out| {
        let digest = copy_hashing(entry, &mut out)?;
        out.sync_all()?;
        Ok(digest)
    });
    match written {
        Ok(found) if found == lib.sha256 => {}
        Ok(found) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(OrtPackageError::LibraryDigest {
                member: lib.member.to_string(),
                found,
                expected: lib.sha256.to_string(),
            });
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(unreadable(lib.member, e));
        }
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        unreadable("rename", e)
    })
}

fn unreadable(what: &str, e: impl std::fmt::Display) -> OrtPackageError {
    OrtPackageError::Unreadable(format!("{what}: {e}"))
}

/// `<path>.tmp`, beside it.
fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}

/// Copy `from` into `to` and return the SHA-256 of what was copied.
fn copy_hashing(mut from: impl Read, to: &mut impl Write) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok(hex::encode(hasher.finalize()));
        }
        hasher.update(&buf[..n]);
        to.write_all(&buf[..n])?;
    }
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
