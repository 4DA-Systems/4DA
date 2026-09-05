// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Committed real-model embeddings for the scenario benchmark.
//!
//! The calibrated benchmark used to embed every scenario text at test time.
//! On GitHub's runners that made the gate non-deterministic: on 2026-09-05 the
//! same commit passed one \`Rust (default)\` run and failed the next, with two
//! scenarios flipping an axis confirmation (reg_express_fullstack 0.439 ->
//! 0.174) from embedding jitter across runner hardware. v29's honest context
//! axis sits scenarios near the confirmation threshold, so the 2-signal cliff
//! turns that jitter into a band failure. The persona simulation solved the
//! same problem with committed \`.bin\` fixtures (\`simulation/fixtures\`); this
//! is the same shape for the benchmark.
//!
//! The fixture key carries a digest of the embedded TEXT, so an edited title
//! or body reads as missing and the loader fails loudly instead of measuring
//! a stale vector. Regenerate (real model required) whenever a scenario or a
//! profile topic changes:
//!
//! \`\`\`text
//! cargo test --lib --features generate-sim-fixtures \
//!     scoring::benchmark_calibration::fixtures::generate::generate_scenario_embedding_fixtures \
//!     -- --ignored --nocapture
//! \`\`\`
//!
//! \`real_model_embeds_one_text\` (mod.rs) keeps CI proving the real model still
//! loads; the benchmark itself no longer depends on the runner's CPU.
use std::collections::HashMap;

use super::Scenario;
use crate::error::FourDaError;

const ITEM_FIXTURE: &[u8] = include_bytes!("fixtures/scenario_item_embeddings.bin");
const TOPIC_FIXTURE: &[u8] = include_bytes!("fixtures/scenario_topic_embeddings.bin");
const MAGIC: &[u8; 4] = b"4DAE";

/// The one command that refreshes both files. The test lives in the nested
/// `generate` module, so the path carries that segment — a filter without it
/// matches nothing and reports "0 passed" while leaving the fixtures stale
/// (2026-09-06: one run lost to exactly that).
pub(super) const REGENERATE: &str = "cargo test --lib --features generate-sim-fixtures \
     scoring::benchmark_calibration::fixtures::generate::generate_scenario_embedding_fixtures \
     -- --ignored --nocapture";

pub(super) type EmbeddingMaps = (HashMap<String, Vec<f32>>, HashMap<String, Vec<f32>>);

/// Fixture key for a scenario: its id plus a digest of the embedded text.
pub(super) fn item_key(scenario: &Scenario) -> String {
    format!(
        "{}#{:016x}",
        scenario.id,
        fnv1a64(&super::embeddings::scenario_text(scenario))
    )
}

/// FNV-1a 64: tiny, stable across Rust versions and platforms (unlike
/// \`DefaultHasher\`), which is all a fixture key needs.
fn fnv1a64(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn stale(what: &str, why: &str) -> FourDaError {
    FourDaError::Internal(format!(
        "scenario embedding fixture ({what}) {why}; regenerate with: {REGENERATE}"
    ))
}

fn take<'a>(bytes: &'a [u8], pos: usize, n: usize, what: &str) -> crate::error::Result<&'a [u8]> {
    bytes
        .get(pos..pos + n)
        .ok_or_else(|| stale(what, "is truncated"))
}

fn read_u32(bytes: &[u8], pos: usize, what: &str) -> crate::error::Result<u32> {
    let b = take(bytes, pos, 4, what)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Encode \`(key, vector)\` records: magic, count, dim, then per record a u32
/// key length, the UTF-8 key, and \`dim\` little-endian f32s.
pub(super) fn encode(dim: usize, records: &[(String, Vec<f32>)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + records.len() * (8 + dim * 4));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(records.len() as u32).to_le_bytes());
    out.extend_from_slice(&(dim as u32).to_le_bytes());
    for (key, vec) in records {
        out.extend_from_slice(&(key.len() as u32).to_le_bytes());
        out.extend_from_slice(key.as_bytes());
        for x in vec.iter().take(dim) {
            out.extend_from_slice(&x.to_le_bytes());
        }
        for _ in vec.len()..dim {
            out.extend_from_slice(&0.0_f32.to_le_bytes());
        }
    }
    out
}

pub(super) fn decode(bytes: &[u8], what: &str) -> crate::error::Result<Vec<(String, Vec<f32>)>> {
    if bytes.len() < 12 || &bytes[0..4] != MAGIC {
        return Err(stale(what, "is missing or empty"));
    }
    let count = read_u32(bytes, 4, what)? as usize;
    let dim = read_u32(bytes, 8, what)? as usize;
    let mut pos = 12;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let key_len = read_u32(bytes, pos, what)? as usize;
        pos += 4;
        let key = std::str::from_utf8(take(bytes, pos, key_len, what)?)
            .map_err(|_| stale(what, "has a non-UTF-8 key"))?
            .to_string();
        pos += key_len;
        let raw = take(bytes, pos, dim * 4, what)?;
        let vec: Vec<f32> = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        pos += dim * 4;
        out.push((key, vec));
    }
    Ok(out)
}

/// Load the committed embeddings and check they cover every scenario (by id
/// AND text digest) and every profile topic the runner will look up. A stale
/// fixture is an error, never a silent fallback to live generation: that would
/// put runner jitter back into the gate exactly when someone forgot to
/// regenerate.
pub(super) fn load_fixture_embeddings(
    scenarios: &[Scenario],
) -> crate::error::Result<EmbeddingMaps> {
    let items: HashMap<String, Vec<f32>> = decode(ITEM_FIXTURE, "items")?.into_iter().collect();
    let topics: HashMap<String, Vec<f32>> = decode(TOPIC_FIXTURE, "topics")?.into_iter().collect();

    let mut item_map = HashMap::with_capacity(scenarios.len());
    let mut missing_items: Vec<&str> = Vec::new();
    for s in scenarios {
        match items.get(&item_key(s)) {
            Some(v) => {
                item_map.insert(s.id.clone(), v.clone());
            }
            None => missing_items.push(s.id.as_str()),
        }
    }
    let missing_topics: Vec<String> = super::embeddings::profile_topics()
        .into_iter()
        .filter(|t| !topics.contains_key(t))
        .collect();
    if !missing_items.is_empty() || !missing_topics.is_empty() {
        return Err(FourDaError::Internal(format!(
            "scenario embedding fixture is STALE: {} scenario(s) missing or edited [{}], \
             {} profile topic(s) missing [{}]; regenerate with: {REGENERATE}",
            missing_items.len(),
            missing_items.join(", "),
            missing_topics.len(),
            missing_topics.join(", ")
        )));
    }
    Ok((item_map, topics))
}

#[cfg(feature = "generate-sim-fixtures")]
fn write_fixture(file_name: &str, bytes: &[u8]) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("scoring")
        .join("benchmark_calibration")
        .join("fixtures");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name);
    let mut f = std::fs::File::create(&path)?;
    f.write_all(bytes)?;
    f.flush()?;
    Ok(path)
}

#[cfg(all(test, feature = "generate-sim-fixtures"))]
mod generate {
    use super::*;

    /// Regenerate both fixtures from the real model. Run ONCE per scenario or
    /// profile-topic change; commit the \`.bin\` files with that change.
    #[test]
    #[ignore = "requires the fastembed model; run explicitly to regenerate the committed fixtures"]
    fn generate_scenario_embedding_fixtures() {
        let scenarios = crate::scoring::benchmark_scenarios::load_scenarios();
        let (items, topics) =
            crate::scoring::benchmark_calibration::embeddings::generate_all_embeddings(&scenarios)
                .expect("fastembed model must be available to regenerate fixtures");
        let item_records: Vec<(String, Vec<f32>)> = scenarios
            .iter()
            .map(|s| (item_key(s), items[&s.id].clone()))
            .collect();
        let mut topic_records: Vec<(String, Vec<f32>)> = topics.into_iter().collect();
        topic_records.sort_by(|a, b| a.0.cmp(&b.0));
        let dim = crate::EMBEDDING_DIMS;
        let p1 = write_fixture("scenario_item_embeddings.bin", &encode(dim, &item_records))
            .expect("write items");
        let p2 = write_fixture(
            "scenario_topic_embeddings.bin",
            &encode(dim, &topic_records),
        )
        .expect("write topics");
        eprintln!(
            "wrote {} items -> {} and {} topics -> {}",
            item_records.len(),
            p1.display(),
            topic_records.len(),
            p2.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stale-fixture guard that runs in every CI leg: the committed files
    /// must cover every scenario text and every profile topic as of this build.
    #[test]
    fn fixture_covers_every_scenario_and_topic() {
        let scenarios = crate::scoring::benchmark_scenarios::load_scenarios();
        let (items, topics) = load_fixture_embeddings(&scenarios).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(items.len(), scenarios.len());
        assert!(items.values().all(|v| v.len() == crate::EMBEDDING_DIMS));
        assert!(topics.values().all(|v| v.len() == crate::EMBEDDING_DIMS));
        for v in items.values() {
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 1e-3,
                "fixture vectors are L2-normalized (got {norm})"
            );
        }
    }

    #[test]
    fn codec_round_trips_and_pads() {
        let records = vec![
            ("a#0".to_string(), vec![0.5, -0.25, 1.0]),
            ("b#1".to_string(), vec![1.0]),
        ];
        let bytes = encode(4, &records);
        let back = decode(&bytes, "test").expect("decode");
        assert_eq!(back[0].0, "a#0");
        assert_eq!(back[0].1, vec![0.5, -0.25, 1.0, 0.0]);
        assert_eq!(back[1].1, vec![1.0, 0.0, 0.0, 0.0]);
        assert!(decode(b"", "test").is_err());
        assert!(
            decode(&bytes[..20], "test").is_err(),
            "truncated input is an error, not a panic"
        );
    }

    #[test]
    fn fnv1a64_is_the_reference_function() {
        // Reference vectors for FNV-1a 64.
        assert_eq!(fnv1a64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64("a"), 0xaf63_dc4c_8601_ec8c);
    }
}
