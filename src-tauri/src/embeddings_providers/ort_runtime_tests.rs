// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

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

/// Each downloadable platform, with its key in `scripts/download-ort.cjs`.
const PLATFORMS: [(&str, &str, &str); 5] = [
    ("windows", "x86_64", "win32-x64"),
    ("windows", "aarch64", "win32-arm64"),
    ("macos", "aarch64", "darwin-arm64"),
    ("linux", "x86_64", "linux-x64"),
    ("linux", "aarch64", "linux-arm64"),
];

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn every_downloadable_platform_is_pinned_and_intel_macos_says_why_not() {
    assert!(PACKAGE_URL.starts_with("https://api.nuget.org/v3-flatcontainer/"));
    assert!(is_sha256(PACKAGE_SHA256));
    for (os, arch, _) in PLATFORMS {
        let lib = library_for(os, arch).expect("a pinned library");
        assert!(is_sha256(lib.sha256), "{os}/{arch}");
        assert!(
            lib.member.starts_with("runtimes/") && lib.member.contains("/native/"),
            "{os}/{arch}: {}",
            lib.member
        );
    }
    let intel = library_for("macos", "x86_64").expect_err("no Intel macOS build exists");
    assert!(
        intel.contains("Intel Macs") && intel.contains("Ollama"),
        "{intel}"
    );
    assert!(library_for("freebsd", "x86_64").is_err());
}

/// The text after `start` up to the next `end`, or which marker is missing.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Result<&'a str, String> {
    let (_, after) = text
        .split_once(start)
        .ok_or_else(|| format!("download-ort.cjs has no `{start}`"))?;
    let (inside, _) = after
        .split_once(end)
        .ok_or_else(|| format!("download-ort.cjs: nothing ends `{start}`"))?;
    Ok(inside)
}

/// `scripts/download-ort.cjs` bundles the library from the same package; the
/// two sets of pins must not drift apart.
#[test]
fn the_pins_equal_the_build_scripts() -> Result<(), String> {
    let script = include_str!("../../../scripts/download-ort.cjs");
    let version = between(script, "const ORT_VERSION = '", "'")?;
    assert_eq!(version, ORT_VERSION);

    let nuget = between(script, "const NUGET = {", "};")?;
    let url = between(nuget, "url: `", "`")?.replace("${ORT_VERSION}", version);
    assert_eq!(url, PACKAGE_URL);
    assert_eq!(between(nuget, "sha256: '", "'")?, PACKAGE_SHA256);

    let platforms = between(script, "const PLATFORMS = {", "\n};")?;
    for (os, arch, key) in PLATFORMS {
        let entry = between(platforms, &format!("'{key}': {{"), "\n  }")?;
        let lib = library_for(os, arch)?;
        assert_eq!(between(entry, "member: '", "'")?, lib.member, "{key}");
        assert_eq!(between(entry, "libSha256: '", "'")?, lib.sha256, "{key}");
    }
    // Intel macOS: neither side has a library to take.
    let intel = between(platforms, "'darwin-x64': {", "\n  }")?;
    assert!(intel.contains("bundleNothing") && !intel.contains("member:"));
    assert!(library_for("macos", "x86_64").is_err());
    Ok(())
}

const MEMBER: &str = "runtimes/win-x64/native/onnxruntime.dll";
const LIBRARY: &[u8] = b"the x64 library";

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A package laid out like the NuGet one, written to `dir`; returns its path
/// and SHA-256.
fn package(dir: &Path, entries: &[(&str, &[u8])]) -> (PathBuf, String) {
    let path = dir.join("package.nupkg");
    let file = std::fs::File::create(&path).expect("create package");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zip.start_file(*name, options).expect("start entry");
        zip.write_all(bytes).expect("write entry");
    }
    zip.finish().expect("finish package");
    let digest = sha256_hex(&std::fs::read(&path).expect("read package"));
    (path, digest)
}

/// Neither the library nor its temp file is left behind.
fn assert_nothing_at(dest: &Path) {
    assert!(!dest.exists(), "{} exists", dest.display());
    assert!(!tmp_path(dest).exists(), "a temp file was left beside it");
}

#[test]
fn the_right_member_with_both_digests_matching_is_extracted() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The real package holds two onnxruntime.dll files; arm64 comes first.
    let (pkg, pkg_sha) = package(
        dir.path(),
        &[
            (
                "runtimes/win-arm64/native/onnxruntime.dll",
                b"the arm64 library",
            ),
            (MEMBER, LIBRARY),
        ],
    );
    let dest = dir.path().join("onnxruntime.dll");
    let lib_sha = sha256_hex(LIBRARY);
    let lib = OrtLibrary {
        member: MEMBER,
        sha256: &lib_sha,
    };
    extract_library(&pkg, &pkg_sha, lib, &dest).expect("extracted");
    assert_eq!(std::fs::read(&dest).expect("read library"), LIBRARY);
    assert!(!tmp_path(&dest).exists());
}

#[test]
fn a_package_that_does_not_match_its_pin_is_never_opened() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (pkg, _) = package(dir.path(), &[(MEMBER, LIBRARY)]);
    let dest = dir.path().join("onnxruntime.dll");
    let lib_sha = sha256_hex(LIBRARY);
    let lib = OrtLibrary {
        member: MEMBER,
        sha256: &lib_sha,
    };
    let err = extract_library(&pkg, &"0".repeat(64), lib, &dest).expect_err("rejected");
    assert!(
        matches!(err, OrtPackageError::PackageDigest { .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("package rejected"), "{err}");
    assert_nothing_at(&dest);
}

#[test]
fn a_library_that_does_not_match_its_pin_never_gets_its_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (pkg, pkg_sha) = package(dir.path(), &[(MEMBER, LIBRARY)]);
    let dest = dir.path().join("onnxruntime.dll");
    let other_sha = sha256_hex(b"another library");
    let lib = OrtLibrary {
        member: MEMBER,
        sha256: &other_sha,
    };
    let err = extract_library(&pkg, &pkg_sha, lib, &dest).expect_err("rejected");
    assert!(
        matches!(&err, OrtPackageError::LibraryDigest { member, .. } if member == MEMBER),
        "{err:?}"
    );
    assert!(err.to_string().contains("library rejected"), "{err}");
    assert_nothing_at(&dest);
}

#[test]
fn a_package_without_the_member_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (pkg, pkg_sha) = package(
        dir.path(),
        &[("runtimes/linux-x64/native/libonnxruntime.so", LIBRARY)],
    );
    let dest = dir.path().join("onnxruntime.dll");
    let lib_sha = sha256_hex(LIBRARY);
    let lib = OrtLibrary {
        member: MEMBER,
        sha256: &lib_sha,
    };
    let err = extract_library(&pkg, &pkg_sha, lib, &dest).expect_err("rejected");
    assert!(
        matches!(&err, OrtPackageError::MissingMember(m) if m == MEMBER),
        "{err:?}"
    );
    assert_nothing_at(&dest);
}

/// The old lookup took the first entry whose name ended in the library's
/// file name. Only the full path counts now, even when the bytes are right.
#[test]
fn a_file_of_the_same_name_at_another_path_is_not_taken() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (pkg, pkg_sha) = package(
        dir.path(),
        &[
            ("onnxruntime.dll", LIBRARY),
            ("runtimes/win-x64/onnxruntime.dll", LIBRARY),
            ("lib/runtimes/win-x64/native/onnxruntime.dll", LIBRARY),
        ],
    );
    let dest = dir.path().join("onnxruntime.dll");
    let lib_sha = sha256_hex(LIBRARY);
    let lib = OrtLibrary {
        member: MEMBER,
        sha256: &lib_sha,
    };
    let err = extract_library(&pkg, &pkg_sha, lib, &dest).expect_err("rejected");
    assert!(matches!(err, OrtPackageError::MissingMember(_)), "{err:?}");
    assert_nothing_at(&dest);
}
