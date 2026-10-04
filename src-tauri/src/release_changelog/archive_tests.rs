// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Parity with the MCP server's `package-archive.test.ts` (ustar prefix, PAX
//! `path=`, GNU `L`, root-level selection, every cap), plus the safety cases
//! this port adds: path traversal and absolute paths are never "root files".
//! Archives are built in memory — no network, no fixtures on disk.

use std::io::Write;

use super::*;

struct Spec<'a> {
    name: &'a str,
    body: Vec<u8>,
    kind: u8,
    prefix: &'a str,
}

fn file<'a>(name: &'a str, body: &str) -> Spec<'a> {
    Spec {
        name,
        body: body.as_bytes().to_vec(),
        kind: b'0',
        prefix: "",
    }
}

fn put(h: &mut [u8], at: usize, s: &[u8]) {
    h[at..at + s.len()].copy_from_slice(s);
}

fn header(name: &str, size: usize, kind: u8, prefix: &str) -> Vec<u8> {
    let mut h = vec![0u8; 512];
    let name_bytes = name.as_bytes();
    put(&mut h, 0, &name_bytes[..name_bytes.len().min(100)]);
    put(&mut h, 100, b"0000644\0");
    put(&mut h, 108, b"0000000\0");
    put(&mut h, 116, b"0000000\0");
    put(&mut h, 124, format!("{size:011o}\0").as_bytes());
    put(&mut h, 136, b"00000000000\0");
    h[156] = kind;
    put(&mut h, 257, b"ustar\0");
    put(&mut h, 263, b"00");
    put(&mut h, 345, prefix.as_bytes());
    h[148..156].fill(b' ');
    let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
    put(&mut h, 148, format!("{sum:06o}\0 ").as_bytes());
    h
}

fn pad(mut data: Vec<u8>) -> Vec<u8> {
    let rem = data.len() % 512;
    if rem != 0 {
        data.resize(data.len() + 512 - rem, 0);
    }
    data
}

fn pax_record(key: &str, value: &str) -> String {
    let body = format!(" {key}={value}\n");
    let mut len = body.len() + 1;
    while len.to_string().len() + body.len() != len {
        len = len.to_string().len() + body.len();
    }
    format!("{len}{body}")
}

fn build_tar(specs: Vec<Spec<'_>>) -> Vec<u8> {
    let mut out = Vec::new();
    for s in specs {
        out.extend(header(s.name, s.body.len(), s.kind, s.prefix));
        out.extend(pad(s.body));
    }
    out.extend(vec![0u8; 1024]);
    out
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn accept(base: &str) -> bool {
    let lower = base.to_lowercase();
    lower.starts_with("changelog") || lower.starts_with("readme")
}

#[test]
fn pax_records_are_byte_length_prefixed_including_multi_byte_values() {
    let data = format!(
        "{}{}",
        pax_record("path", "package/ÇHANGELOG.md"),
        pax_record("mtime", "1")
    );
    let got = parse_pax_headers(data.as_bytes());
    assert_eq!(
        got.get("path").map(String::as_str),
        Some("package/ÇHANGELOG.md")
    );
    assert_eq!(got.get("mtime").map(String::as_str), Some("1"));
}

#[test]
fn joins_the_ustar_prefix_with_the_name() {
    let mut spec = file("CHANGELOG.md", "x");
    spec.prefix = "package";
    let tar = build_tar(vec![spec]);
    let paths: Vec<String> = iterate_tar(&tar)
        .unwrap()
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert_eq!(paths, ["package/CHANGELOG.md"]);
}

#[test]
fn a_pax_path_applies_to_the_next_entry_only() {
    let long = format!("package/{}.md", "a".repeat(120));
    let pax = pax_record("path", &long);
    let tar = build_tar(vec![
        Spec {
            name: "PaxHeader",
            body: pax.into_bytes(),
            kind: b'x',
            prefix: "",
        },
        file("truncated-name", "one"),
        file("package/second.md", "two"),
    ]);
    let paths: Vec<String> = iterate_tar(&tar)
        .unwrap()
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert_eq!(paths, [long, "package/second.md".to_string()]);
}

#[test]
fn a_gnu_long_name_applies_to_the_next_entry() {
    let long = format!("crate-1.0.0/{}.txt", "b".repeat(150));
    let tar = build_tar(vec![
        Spec {
            name: "././@LongLink",
            body: format!("{long}\0").into_bytes(),
            kind: b'L',
            prefix: "",
        },
        file("short", "data"),
    ]);
    let entries = iterate_tar(&tar).unwrap();
    assert_eq!(entries[0].path, long);
    assert_eq!(entries[0].data, b"data");
}

#[test]
fn rejects_a_header_whose_checksum_does_not_match() {
    let mut tar = build_tar(vec![file("package/a", "x")]);
    tar[0] = b'z';
    assert!(iterate_tar(&tar).is_err());
}

#[test]
fn rejects_an_entry_whose_data_runs_past_the_archive() {
    let tar = build_tar(vec![file("package/a", &"x".repeat(600))]);
    let err = iterate_tar(&tar[..700]).unwrap_err();
    assert!(err.0.contains("truncated"), "{err}");
}

#[test]
fn returns_accepted_root_files_and_skips_nested_binary_and_unaccepted() {
    let gz = gzip(&build_tar(vec![
        file("package/CHANGELOG.md", "## 1.0.0\n- a"),
        file("package/README.md", "hi"),
        file("package/docs/CHANGELOG.md", "nested"),
        Spec {
            name: "package/changelog.bin",
            body: vec![0x43, 0, 0x44],
            kind: b'0',
            prefix: "",
        },
        file("package/index.js", "code"),
        Spec {
            name: "package/dir",
            body: Vec::new(),
            kind: b'5',
            prefix: "",
        },
    ]));
    let files = read_root_text_files(&gz, accept).unwrap();
    let keys: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(keys, ["package/CHANGELOG.md", "package/README.md"]);
    assert!(files["package/CHANGELOG.md"].contains("## 1.0.0"));
}

#[test]
fn finds_a_root_file_whose_name_only_fits_in_a_pax_header() {
    let pax = pax_record("path", "fastembed-7.1.0/CHANGELOG.md");
    let gz = gzip(&build_tar(vec![
        Spec {
            name: "PaxHeader",
            body: pax.into_bytes(),
            kind: b'x',
            prefix: "",
        },
        file("x", "## 7.1.0"),
    ]));
    let files = read_root_text_files(&gz, accept).unwrap();
    assert_eq!(
        files
            .get("fastembed-7.1.0/CHANGELOG.md")
            .map(String::as_str),
        Some("## 7.1.0")
    );
}

#[test]
fn path_traversal_and_absolute_names_are_never_root_files() {
    let pax = pax_record("path", "package/../../CHANGELOG.md");
    let gz = gzip(&build_tar(vec![
        file("../CHANGELOG.md", "escape"),
        file("/CHANGELOG.md", "absolute"),
        file("package\\..\\CHANGELOG.md", "backslash"),
        file("C:/CHANGELOG.md", "drive"),
        Spec {
            name: "PaxHeader",
            body: pax.into_bytes(),
            kind: b'x',
            prefix: "",
        },
        file("y", "pax escape"),
        file("package/CHANGELOG.md", "ok"),
    ]));
    let files = read_root_text_files(&gz, accept).unwrap();
    let keys: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(keys, ["package/CHANGELOG.md"]);
}

#[test]
fn symlinks_and_hardlinks_are_not_read() {
    let gz = gzip(&build_tar(vec![
        Spec {
            name: "package/CHANGELOG.md",
            body: Vec::new(),
            kind: b'2',
            prefix: "",
        },
        Spec {
            name: "package/CHANGES.md",
            body: Vec::new(),
            kind: b'1',
            prefix: "",
        },
    ]));
    assert!(read_root_text_files(&gz, accept).unwrap().is_empty());
}

#[test]
fn skips_a_single_file_over_the_per_file_cap() {
    let body = vec![b'a'; MAX_FILE_BYTES + 1];
    let gz = gzip(&build_tar(vec![Spec {
        name: "package/CHANGELOG.md",
        body,
        kind: b'0',
        prefix: "",
    }]));
    assert!(read_root_text_files(&gz, accept).unwrap().is_empty());
}

#[test]
fn stops_inflating_a_gzip_bomb_at_the_uncompressed_cap() {
    // 65 MB of zeros compresses to ~64 KB: under the compressed cap, over the uncompressed one.
    let bomb = gzip(&vec![0u8; MAX_UNCOMPRESSED_BYTES + 1024 * 1024]);
    assert!(bomb.len() < MAX_COMPRESSED_BYTES);
    let err = read_root_text_files(&bomb, accept).unwrap_err();
    assert!(err.0.contains("uncompressed"), "{err}");
}

#[test]
fn refuses_an_archive_over_the_compressed_cap() {
    let big = vec![0x1f; MAX_COMPRESSED_BYTES + 1];
    let err = read_root_text_files(&big, accept).unwrap_err();
    assert!(err.0.contains("compressed"), "{err}");
}

#[test]
fn rejects_input_that_is_not_gzip() {
    assert!(read_root_text_files(b"not gzip", accept).is_err());
    // A gzip magic followed by garbage is refused too, not panicked on.
    assert!(read_root_text_files(&[0x1f, 0x8b, 0x08, 0, 1, 2, 3], accept).is_err());
}

#[test]
fn a_zip_or_plain_tar_is_not_gzip() {
    let tar = build_tar(vec![file("package/CHANGELOG.md", "## 1.0.0")]);
    assert!(read_root_text_files(&tar, accept).is_err());
    assert!(read_root_text_files(b"PK\x03\x04rest", accept).is_err());
}
