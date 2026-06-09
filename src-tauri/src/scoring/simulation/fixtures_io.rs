// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Self-documenting binary (de)serialization for the real-embedding simulation
//! fixtures (recall investigation, `calibrated-sim` / `generate-sim-fixtures`).
//!
//! Two committed artefacts live in `fixtures/`:
//!   - `corpus_embeddings.bin`  — keyed by corpus item id (u32)   -> Vec<f32>
//!   - `topic_embeddings.bin`   — keyed by topic/interest string  -> Vec<f32>
//!
//! Both are produced by the `generate-sim-fixtures` test (`fixtures_gen.rs`) from
//! the SAME fastembed path (`crate::fastembed_sync` + `pad_and_normalize`) that
//! `benchmark_calibration` uses, so the `.bin` reproduce deterministically.
//!
//! Format (little-endian throughout, no external deps — keeps Cargo.lock clean):
//!   magic:   8 bytes   b"4DASIMv1"
//!   kind:    1 byte   0 = u32-keyed (corpus), 1 = string-keyed (topics)
//!   dim:     u32      embedding dimension (must equal crate::EMBEDDING_DIMS)
//!   count:   u32      number of records
//!   records: count × {
//!       key  : u32-keyed -> u32 id
//!              string-keyed -> u32 byte-len + UTF-8 bytes
//!       vec  : dim × f32  (raw little-endian bits)
//!   }
//!
//! Only compiled when one of the two simulation-fixture features is on, so the
//! default build never pulls this code in (and never warns about it).

#![cfg(any(feature = "calibrated-sim", feature = "generate-sim-fixtures"))]

use std::io::{self, Read, Write};

const MAGIC: &[u8; 8] = b"4DASIMv1";
const KIND_U32: u8 = 0;
const KIND_STR: u8 = 1;

fn write_header(out: &mut Vec<u8>, kind: u8, dim: u32, count: u32) {
    out.extend_from_slice(MAGIC);
    out.push(kind);
    out.extend_from_slice(&dim.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
}

fn write_vec(out: &mut Vec<u8>, v: &[f32]) {
    out.reserve(v.len() * 4);
    for &x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

/// Serialize u32-keyed embeddings (corpus items by id).
#[cfg(feature = "generate-sim-fixtures")]
pub(super) fn serialize_u32_keyed(dim: u32, records: &[(u32, Vec<f32>)]) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, KIND_U32, dim, records.len() as u32);
    for (id, v) in records {
        out.extend_from_slice(&id.to_le_bytes());
        write_vec(&mut out, v);
    }
    out
}

/// Serialize string-keyed embeddings (topics / interests by name).
#[cfg(feature = "generate-sim-fixtures")]
pub(super) fn serialize_str_keyed(dim: u32, records: &[(String, Vec<f32>)]) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, KIND_STR, dim, records.len() as u32);
    for (key, v) in records {
        let bytes = key.as_bytes();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
        write_vec(&mut out, v);
    }
    out
}

#[cfg(feature = "calibrated-sim")]
fn read_exact_into<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<()> {
    r.read_exact(buf)
}

#[cfg(feature = "calibrated-sim")]
fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut b = [0u8; 1];
    read_exact_into(r, &mut b)?;
    Ok(b[0])
}

#[cfg(feature = "calibrated-sim")]
fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    read_exact_into(r, &mut b)?;
    Ok(u32::from_le_bytes(b))
}

#[cfg(feature = "calibrated-sim")]
fn read_vec<R: Read>(r: &mut R, dim: usize) -> io::Result<Vec<f32>> {
    let mut raw = vec![0u8; dim * 4];
    read_exact_into(r, &mut raw)?;
    let mut v = Vec::with_capacity(dim);
    for chunk in raw.chunks_exact(4) {
        v.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(v)
}

#[cfg(feature = "calibrated-sim")]
fn read_and_check_header<R: Read>(r: &mut R, expect_kind: u8) -> io::Result<(usize, u32)> {
    let mut magic = [0u8; 8];
    read_exact_into(r, &mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "simulation fixture: bad magic",
        ));
    }
    let kind = read_u8(r)?;
    if kind != expect_kind {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "simulation fixture: wrong record kind",
        ));
    }
    let dim = read_u32(r)? as usize;
    if dim != crate::EMBEDDING_DIMS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "simulation fixture: dim {dim} != EMBEDDING_DIMS {}",
                crate::EMBEDDING_DIMS
            ),
        ));
    }
    let count = read_u32(r)?;
    Ok((dim, count))
}

/// Deserialize u32-keyed embeddings into (id, vec) pairs.
#[cfg(feature = "calibrated-sim")]
pub(super) fn deserialize_u32_keyed(bytes: &[u8]) -> io::Result<Vec<(u32, Vec<f32>)>> {
    let mut r = bytes;
    let (dim, count) = read_and_check_header(&mut r, KIND_U32)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = read_u32(&mut r)?;
        let v = read_vec(&mut r, dim)?;
        out.push((id, v));
    }
    Ok(out)
}

/// Deserialize string-keyed embeddings into (key, vec) pairs.
#[cfg(feature = "calibrated-sim")]
pub(super) fn deserialize_str_keyed(bytes: &[u8]) -> io::Result<Vec<(String, Vec<f32>)>> {
    let mut r = bytes;
    let (dim, count) = read_and_check_header(&mut r, KIND_STR)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let key_len = read_u32(&mut r)? as usize;
        let mut key_bytes = vec![0u8; key_len];
        read_exact_into(&mut r, &mut key_bytes)?;
        let key = String::from_utf8(key_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let v = read_vec(&mut r, dim)?;
        out.push((key, v));
    }
    Ok(out)
}

/// Write bytes to a fixture file under `src/scoring/simulation/fixtures/`.
#[cfg(feature = "generate-sim-fixtures")]
pub(super) fn write_fixture(file_name: &str, bytes: &[u8]) -> io::Result<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("scoring")
        .join("simulation")
        .join("fixtures");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name);
    let mut f = std::fs::File::create(&path)?;
    f.write_all(bytes)?;
    f.flush()?;
    Ok(path)
}
