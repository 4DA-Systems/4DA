// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "4da-model-store-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Two small files standing in for the model, pinned like the real ones.
fn tiny_files(a: &'static [u8], b: &'static [u8]) -> [PinnedFile; 2] {
    let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    [
        PinnedFile {
            remote: "onnx/model.onnx",
            local: "model.onnx",
            sha256: leak(sha(a)),
            size: a.len() as u64,
        },
        PinnedFile {
            remote: "tokenizer.json",
            local: "tokenizer.json",
            sha256: leak(sha(b)),
            size: b.len() as u64,
        },
    ]
}

fn write_copy(dir: &Path, files: &[PinnedFile], bodies: &[&[u8]]) {
    std::fs::create_dir_all(dir).unwrap();
    for (f, body) in files.iter().zip(bodies) {
        std::fs::write(dir.join(f.local), body).unwrap();
    }
}

const FAST: Retry = Retry {
    attempts: 2,
    backoff: Duration::from_millis(1),
};

/// A one-route HTTP server: `GET /<remote>` returns that body, honouring
/// `Range: bytes=N-` with a 206. `cut` truncates the FIRST response after
/// that many bytes (a dropped connection), to exercise resume.
struct Server {
    url: String,
    requests: Arc<AtomicUsize>,
    ranges: Arc<parking_lot::Mutex<Vec<u64>>>,
}

fn serve(routes: Vec<(&'static str, Vec<u8>)>, cut: Option<usize>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(AtomicUsize::new(0));
    let ranges = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let (req, rng) = (requests.clone(), ranges.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let n = req.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let path = first.split_whitespace().nth(1).unwrap_or("/").to_string();
            let mut from = 0u64;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("range: bytes=") {
                    from = v.trim().trim_end_matches('-').parse().unwrap_or(0);
                    rng.lock().push(from);
                }
            }
            let body = routes
                .iter()
                .find(|(r, _)| path.ends_with(r))
                .map(|(_, b)| b.clone());
            let Some(body) = body else {
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            };
            let slice = &body[from as usize..];
            let head = if from > 0 {
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n",
                    slice.len(),
                    body.len() - 1,
                    body.len()
                )
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    slice.len()
                )
            };
            let _ = stream.write_all(head.as_bytes());
            let send = match cut {
                Some(c) if n == 0 => &slice[..c.min(slice.len())],
                _ => slice,
            };
            let _ = stream.write_all(send);
            let _ = stream.flush();
        }
    });
    Server {
        url,
        requests,
        ranges,
    }
}

// ----------------------------------------------------------------------------
// Pinning
// ----------------------------------------------------------------------------

/// The download must fetch exactly what the installer used to bundle, or a
/// user without the bundle embeds in a different space.
#[test]
fn the_download_is_pinned_to_the_installer_bundle() {
    let script = include_str!("../../../scripts/download-embedding-model.cjs");
    assert!(
        script.contains(&format!("const REVISION = '{REVISION}';")),
        "revision drifted from scripts/download-embedding-model.cjs"
    );
    assert!(script.contains(&format!("const MODEL_DIR_NAME = '{MODEL_DIR_NAME}';")));
    for f in FILES {
        assert!(
            script.contains(&format!("remote: '{}'", f.remote)),
            "{}",
            f.remote
        );
        assert!(
            script.contains(&format!("local: '{}'", f.local)),
            "{}",
            f.local
        );
        assert!(script.contains(f.sha256), "{} checksum drifted", f.local);
    }
    assert_eq!(FILES[0].local, "model_fp16.onnx");
    assert!(total_bytes(&FILES) > 270_000_000);
}

// ----------------------------------------------------------------------------
// Resolution order
// ----------------------------------------------------------------------------

#[test]
fn resolution_prefers_the_bundled_copy_then_the_cache() {
    let root = temp_dir("resolve");
    let (bundled, cache) = (root.join("bundled"), root.join("cache"));
    let files = tiny_files(b"weights", b"{}");

    assert!(
        installed(&bundled, &cache, &files).is_empty(),
        "nothing on disk"
    );

    write_copy(&cache, &files, &[b"weights", b"{}"]);
    assert_eq!(
        installed(&bundled, &cache, &files),
        vec![ModelLocation::Cache(cache.clone())],
        "a previous download is used"
    );

    write_copy(&bundled, &files, &[b"weights", b"{}"]);
    assert_eq!(
        installed(&bundled, &cache, &files),
        vec![
            ModelLocation::Bundled(bundled.clone()),
            ModelLocation::Cache(cache.clone())
        ],
        "an existing bundled copy wins, the cache stays as the fallback"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_incomplete_copy_is_skipped() {
    let root = temp_dir("incomplete");
    let (bundled, cache) = (root.join("bundled"), root.join("cache"));
    let files = tiny_files(b"weights", b"{}");
    // Bundled copy missing a file; cache copy truncated.
    std::fs::create_dir_all(&bundled).unwrap();
    std::fs::write(bundled.join("model.onnx"), b"weights").unwrap();
    write_copy(&cache, &files, &[b"weigh", b"{}"]);
    assert!(installed(&bundled, &cache, &files).is_empty());
    // A leftover .part never counts.
    std::fs::write(cache.join("model.onnx"), b"").unwrap();
    std::fs::write(cache.join("model.onnx.part"), b"weights").unwrap();
    assert!(installed(&bundled, &cache, &files).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

// ----------------------------------------------------------------------------
// Download
// ----------------------------------------------------------------------------

#[test]
fn a_download_fetches_verifies_and_names_each_file() {
    let root = temp_dir("download");
    let files = tiny_files(b"model weights here", b"{\"tok\":1}");
    let server = serve(
        vec![
            ("onnx/model.onnx", b"model weights here".to_vec()),
            ("tokenizer.json", b"{\"tok\":1}".to_vec()),
        ],
        None,
    );
    let mut last = (0, 0);
    let dir = ensure_model(&server.url, &files, &root, FAST, |d, t, _| last = (d, t)).unwrap();
    assert!(is_complete(&dir, &files));
    assert_eq!(
        last,
        (total_bytes(&files), total_bytes(&files)),
        "progress ends at 100%"
    );
    assert!(!root.join("model.onnx.part").exists());

    // A second run finds both files and fetches nothing.
    let before = server.requests.load(Ordering::SeqCst);
    ensure_model(&server.url, &files, &root, FAST, |_, _, _| {}).unwrap();
    assert_eq!(server.requests.load(Ordering::SeqCst), before);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_checksum_mismatch_is_rejected_and_leaves_no_file() {
    let root = temp_dir("mismatch");
    // Pinned to "expected bytes", served "tampered byte" (same length).
    let files = tiny_files(b"expected bytes", b"{}");
    let server = serve(
        vec![
            ("onnx/model.onnx", b"tampered bytes".to_vec()),
            ("tokenizer.json", b"{}".to_vec()),
        ],
        None,
    );
    let err = ensure_model(&server.url, &files, &root, FAST, |_, _, _| {}).unwrap_err();
    assert!(err.to_string().contains("SHA-256 mismatch"), "{err}");
    assert!(
        !root.join("model.onnx").exists(),
        "a bad file never gets its final name"
    );
    assert!(
        !root.join("model.onnx.part").exists(),
        "and its partial copy is deleted"
    );
    assert!(!is_complete(&root, &files));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_wrong_size_is_rejected() {
    let root = temp_dir("size");
    let files = tiny_files(b"short", b"{}");
    let server = serve(
        vec![
            ("onnx/model.onnx", b"much longer than pinned".to_vec()),
            ("tokenizer.json", b"{}".to_vec()),
        ],
        None,
    );
    let err = ensure_model(&server.url, &files, &root, FAST, |_, _, _| {}).unwrap_err();
    assert!(err.to_string().contains("longer than the pinned"), "{err}");
    assert!(!root.join("model.onnx").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_dropped_connection_resumes_where_it_stopped() {
    let root = temp_dir("resume");
    let body: &'static [u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let files = tiny_files(body, b"{}");
    // The first response stops after 10 bytes.
    let server = serve(
        vec![
            ("onnx/model.onnx", body.to_vec()),
            ("tokenizer.json", b"{}".to_vec()),
        ],
        Some(10),
    );
    ensure_model(&server.url, &files, &root, FAST, |_, _, _| {}).unwrap();
    assert_eq!(std::fs::read(root.join("model.onnx")).unwrap(), body);
    assert_eq!(
        *server.ranges.lock(),
        vec![10],
        "the retry asked only for the rest"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_unreachable_host_fails_honestly() {
    let root = temp_dir("unreachable");
    let files = tiny_files(b"x", b"{}");
    // Bind then drop: nothing listens on this port.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let err = ensure_model(
        &format!("http://127.0.0.1:{port}"),
        &files,
        &root,
        FAST,
        |_, _, _| {},
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("embedding model download failed (model.onnx)"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

// ----------------------------------------------------------------------------
// Degradation: which routes would ever download
// ----------------------------------------------------------------------------

#[test]
fn only_routes_that_reach_the_in_process_engine_download() {
    use crate::embeddings::EmbeddingRoute as R;
    assert!(route_uses_in_process_model(R::LocalDefault));
    assert!(route_uses_in_process_model(R::AnthropicLocal));
    assert!(
        !route_uses_in_process_model(R::Ollama),
        "explicit Ollama never falls back"
    );
    assert!(
        !route_uses_in_process_model(R::OpenAi),
        "opted-in cloud never falls back"
    );
}
