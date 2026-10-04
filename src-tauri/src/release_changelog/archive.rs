// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry package archive reader (gzip tar) — a port of the MCP server's
//! `package-archive.ts`, with its fixtures in `archive_tests.rs`.
//!
//! The lane reads the changelog a package ships INSIDE its own registry
//! archive (npm `.tgz`, crates.io `.crate`), so the only host contacted is the
//! package's own registry (Decision 1, option A).
//!
//! Nothing in an archive is ever written to disk or executed: entries are read
//! into memory, only root-level text files whose basename names a changelog
//! are kept, and their bytes are only ever parsed as text. A path with a `..`
//! segment, an absolute path or a backslash is skipped, so a hostile name can
//! neither escape the package directory nor masquerade as a root file.
//!
//! The tar subset is what real registry archives use: ustar headers with the
//! `prefix` field, PAX `x` headers (`path=` for long names — npm's packer
//! emits these) and GNU `L` long names (older cargo / GNU tar). Everything else
//! is skipped, not guessed.
//!
//! Hard caps, because the archive is third-party input: compressed ≤ 5 MB
//! (checked on the declared length AND on the bytes actually read, in
//! `fetch.rs`), uncompressed ≤ 64 MB (inflation stops at the cap, so a gzip
//! bomb cannot exhaust memory), single file ≤ 2 MB.

use std::collections::BTreeMap;
use std::io::Read;

/// Largest compressed archive read. Measured registry tarballs for the
/// user-facing release rows ranged 15 KB – 2.9 MB (2026-09-27 probe).
pub(crate) const MAX_COMPRESSED_BYTES: usize = 5 * 1024 * 1024;
/// Largest uncompressed archive inflated.
pub(crate) const MAX_UNCOMPRESSED_BYTES: usize = 64 * 1024 * 1024;
/// Largest single file kept.
pub(crate) const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

const BLOCK: usize = 512;

/// A refusal: cap exceeded, not gzip, corrupt tar. Permanent for a given
/// published archive, which never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveError(pub String);

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One regular-file entry with its resolved name.
#[derive(Debug)]
pub(crate) struct TarEntry<'a> {
    pub path: String,
    pub kind: u8,
    pub data: &'a [u8],
}

fn read_string(buf: &[u8], start: usize, len: usize) -> String {
    let slice = buf.get(start..start + len).unwrap_or(&[]);
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn read_octal(buf: &[u8], start: usize, len: usize) -> Result<usize, ArchiveError> {
    if buf.get(start).is_some_and(|b| b & 0x80 != 0) {
        // GNU base-256 size: only needed for entries ≥ 8 GB, far over every cap.
        return Err(ArchiveError(
            "tar entry uses a base-256 size field (entry too large)".into(),
        ));
    }
    let text = read_string(buf, start, len);
    let text = text.trim();
    if text.is_empty() {
        return Ok(0);
    }
    if !text.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Err(ArchiveError(format!(
            "corrupt tar header: size field \"{text}\""
        )));
    }
    usize::from_str_radix(text, 8)
        .map_err(|_| ArchiveError(format!("corrupt tar header: size field \"{text}\"")))
}

fn checksum_matches(header: &[u8]) -> bool {
    let stored = read_string(header, 148, 8);
    let stored = stored.trim();
    let Ok(want) = u64::from_str_radix(stored, 8) else {
        return false;
    };
    let sum: u64 = header
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if (148..156).contains(&i) {
                0x20
            } else {
                u64::from(b)
            }
        })
        .sum();
    sum == want
}

/// PAX records are `"<len> <key>=<value>\n"`, where len counts the whole
/// record in BYTES.
pub(crate) fn parse_pax_headers(data: &[u8]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut pos = 0;
    while pos < data.len() {
        let Some(space) = data[pos..].iter().position(|&b| b == b' ').map(|p| p + pos) else {
            break;
        };
        let Some(len) = std::str::from_utf8(&data[pos..space])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&l| l > 0 && pos + l <= data.len() && space < pos + l)
        else {
            break;
        };
        let record = String::from_utf8_lossy(&data[space + 1..pos + len]);
        let record = record.strip_suffix('\n').unwrap_or(&record);
        if let Some((k, v)) = record.split_once('=').filter(|(k, _)| !k.is_empty()) {
            out.insert(k.to_string(), v.to_string());
        }
        pos += len;
    }
    out
}

/// Walk an uncompressed tar buffer, returning every non-metadata entry with
/// its resolved name.
pub(crate) fn iterate_tar(tar: &[u8]) -> Result<Vec<TarEntry<'_>>, ArchiveError> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut pax_path: Option<String> = None;
    let mut gnu_long_name: Option<String> = None;
    while offset + BLOCK <= tar.len() {
        let header = &tar[offset..offset + BLOCK];
        if header.iter().all(|&b| b == 0) {
            break; // end-of-archive marker
        }
        if !checksum_matches(header) {
            return Err(ArchiveError("corrupt tar header: checksum mismatch".into()));
        }
        let size = read_octal(header, 124, 12)?;
        let kind = if header[156] == 0 { b'0' } else { header[156] };
        let data_start = offset + BLOCK;
        let data_end = data_start
            .checked_add(size)
            .filter(|&end| end <= tar.len())
            .ok_or_else(|| ArchiveError("truncated tar entry".into()))?;
        let data = &tar[data_start..data_end];
        offset = data_start + size.div_ceil(BLOCK) * BLOCK;
        match kind {
            b'x' => pax_path = parse_pax_headers(data).remove("path"),
            b'L' => gnu_long_name = Some(read_string(data, 0, data.len())),
            b'g' | b'K' => {}
            _ => {
                let name = read_string(header, 0, 100);
                let magic = read_string(header, 257, 6);
                let prefix = if magic.starts_with("ustar") {
                    read_string(header, 345, 155)
                } else {
                    String::new()
                };
                let path = pax_path
                    .take()
                    .or_else(|| gnu_long_name.take())
                    .unwrap_or_else(|| {
                        if prefix.is_empty() {
                            name
                        } else {
                            format!("{prefix}/{name}")
                        }
                    });
                pax_path = None;
                gnu_long_name = None;
                out.push(TarEntry { path, kind, data });
            }
        }
    }
    Ok(out)
}

/// The path's segments with `./` and empty segments removed, or `None` when
/// the path could escape the package directory (absolute, `..`, backslash,
/// drive letter).
fn safe_segments(path: &str) -> Option<Vec<&str>> {
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return None;
    }
    let parts: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.contains(&"..") {
        return None;
    }
    Some(parts)
}

/// Inflate a gzip buffer, refusing at `MAX_UNCOMPRESSED_BYTES`.
pub(crate) fn gunzip_capped(gz: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    if gz.len() > MAX_COMPRESSED_BYTES {
        return Err(ArchiveError(format!(
            "archive is {} bytes compressed (cap {MAX_COMPRESSED_BYTES})",
            gz.len()
        )));
    }
    if gz.len() < 2 || gz[0] != 0x1f || gz[1] != 0x8b {
        return Err(ArchiveError("archive is not valid gzip".into()));
    }
    let mut out = Vec::new();
    let cap = MAX_UNCOMPRESSED_BYTES as u64 + 1;
    flate2::read::MultiGzDecoder::new(gz)
        .take(cap)
        .read_to_end(&mut out)
        .map_err(|e| ArchiveError(format!("archive is not valid gzip: {e}")))?;
    if out.len() > MAX_UNCOMPRESSED_BYTES {
        return Err(ArchiveError(format!(
            "archive exceeds {MAX_UNCOMPRESSED_BYTES} bytes uncompressed"
        )));
    }
    Ok(out)
}

/// Root-level text files from a gzip tar whose basename `accept` approves.
/// "Root-level" = at most one directory deep, because registry archives wrap
/// everything in a single top directory (`package/`, `<name>-<version>/`).
/// Keys are the archive paths as stored. Oversized or binary (NUL-containing)
/// files are skipped.
pub(crate) fn read_root_text_files(
    gz: &[u8],
    accept: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, String>, ArchiveError> {
    let tar = gunzip_capped(gz)?;
    let mut files = BTreeMap::new();
    for entry in iterate_tar(&tar)? {
        if entry.kind != b'0' && entry.kind != b'7' {
            continue;
        }
        let Some(parts) = safe_segments(&entry.path) else {
            continue;
        };
        if parts.is_empty() || parts.len() > 2 {
            continue;
        }
        if !parts.last().is_some_and(|base| accept(base)) {
            continue;
        }
        if entry.data.len() > MAX_FILE_BYTES || entry.data.contains(&0) {
            continue;
        }
        files.insert(
            entry.path.clone(),
            String::from_utf8_lossy(entry.data).into_owned(),
        );
    }
    Ok(files)
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
