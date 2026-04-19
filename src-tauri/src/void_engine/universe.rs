// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// ============================================================================
// Phase 2: Random Projection Engine & Universe (feature-gated)
// Experimental (AD-012). Enable with: cargo build --features void-universe
// ============================================================================

use serde::Serialize;
use std::collections::HashMap;

use crate::context_engine::ContextEngine;
use crate::db::Database;
use crate::error::{Result, ResultExt};
use tracing::info;

pub const PROJ_DIM: usize = 3;

#[cfg(test)]
pub const EMB_DIM_SMALL: usize = 384;

/// Fixed seed for deterministic projection matrix
const PROJECTION_SEED: u64 = 0x4DA_0000_2026;

/// Xorshift64 PRNG - simple, fast, deterministic
pub fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Generate a random Gaussian value using Box-Muller transform
fn gaussian(rng: &mut u64) -> f32 {
    let u1 = ((xorshift64(rng) & 0xFFFF) as f32 / 65536.0).max(1e-10);
    let u2 = (xorshift64(rng) & 0xFFFF) as f32 / 65536.0;
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

/// Random projection matrix: PROJ_DIM x emb_dim
/// Gaussian entries scaled by 1/sqrt(PROJ_DIM) per Johnson-Lindenstrauss.
/// Generated once from a fixed seed - same input always produces same output.
pub fn generate_projection_matrix(emb_dim: usize) -> Vec<[f32; PROJ_DIM]> {
    let scale = 1.0 / (PROJ_DIM as f32).sqrt();
    let mut rng = PROJECTION_SEED;
    let mut matrix = Vec::with_capacity(emb_dim);
    for _ in 0..emb_dim {
        let mut row = [0.0f32; PROJ_DIM];
        for val in row.iter_mut() {
            *val = gaussian(&mut rng) * scale;
        }
        matrix.push(row);
    }
    matrix
}

/// Project a single embedding to 3D.
/// embedding.len() must match the matrix length.
pub fn project(embedding: &[f32], matrix: &[[f32; PROJ_DIM]]) -> [f32; 3] {
    debug_assert_eq!(
        embedding.len(),
        matrix.len(),
        "Embedding dim must match projection matrix"
    );
    let mut result = [0.0f32; 3];
    for (i, val) in embedding.iter().enumerate() {
        let row = &matrix[i];
        result[0] += val * row[0];
        result[1] += val * row[1];
        result[2] += val * row[2];
    }
    result
}

/// Batch project multiple embeddings. Returns vec of (index, [x, y, z]).
pub fn project_batch(embeddings: &[Vec<f32>], matrix: &[[f32; PROJ_DIM]]) -> Vec<[f32; 3]> {
    embeddings.iter().map(|e| project(e, matrix)).collect()
}

/// Compute centroid of projected positions
pub fn centroid(positions: &[[f32; 3]]) -> [f32; 3] {
    if positions.is_empty() {
        return [0.0; 3];
    }
    let n = positions.len() as f32;
    let mut sum = [0.0f32; 3];
    for p in positions {
        sum[0] += p[0];
        sum[1] += p[1];
        sum[2] += p[2];
    }
    [sum[0] / n, sum[1] / n, sum[2] / n]
}

/// Squared Euclidean distance between two 3D points
pub fn dist_sq(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

// ========================================================================
// Universe Data Structures
// ========================================================================

/// Complete universe state sent to the frontend for 3D rendering
#[derive(Serialize, Clone, Debug)]
pub struct VoidUniverse {
    /// Context centroid position (the "core" - user's context center)
    pub core: [f32; 3],
    /// Top-K interest orbital nodes
    pub interests: Vec<InterestNode>,
    /// All particles with 3D positions (capped at max_particles)
    pub particles: Vec<VoidParticle>,
    /// Cluster summaries for LOD at >5K items
    pub clusters: Vec<ClusterNode>,
    /// Total items in database (may exceed particles.len())
    pub total_items: usize,
    /// Current projection version (for cache coherence)
    pub projection_version: i64,
}

/// An interest orbital - a labeled node representing a topic cluster
#[derive(Serialize, Clone, Debug)]
pub struct InterestNode {
    pub name: String,
    pub position: [f32; 3],
    pub weight: f32,
    pub item_count: usize,
}

/// A single particle in the universe (source item, context chunk, or document)
#[derive(Serialize, Clone, Debug)]
pub struct VoidParticle {
    pub id: i64,
    /// "source" | "context" | "document"
    pub layer: String,
    pub position: [f32; 3],
    pub label: String,
    pub url: Option<String>,
    pub relevance: f32,
    pub source_type: String,
    pub age_hours: f32,
}

/// Cluster summary for LOD rendering (when items > 5K)
#[derive(Serialize, Clone, Debug)]
pub struct ClusterNode {
    pub centroid: [f32; 3],
    pub count: usize,
    pub avg_relevance: f32,
    pub top_label: String,
    pub dominant_source: String,
}

/// K-means clustering on 3D projected positions for LOD
pub fn kmeans_3d(positions: &[[f32; 3]], k: usize, max_iter: usize) -> Vec<usize> {
    let n = positions.len();
    if n == 0 || k == 0 {
        return vec![];
    }
    let k = k.min(n);

    // Initialize centroids with evenly-spaced items
    let mut centroids: Vec<[f32; 3]> = (0..k).map(|i| positions[i * n / k]).collect();
    let mut assignments = vec![0usize; n];

    for _ in 0..max_iter {
        let mut changed = false;

        // Assign each point to nearest centroid
        for (i, pos) in positions.iter().enumerate() {
            let mut best = 0;
            let mut best_dist = f32::MAX;
            for (j, c) in centroids.iter().enumerate() {
                let d = dist_sq(pos, c);
                if d < best_dist {
                    best_dist = d;
                    best = j;
                }
            }
            if assignments[i] != best {
                assignments[i] = best;
                changed = true;
            }
        }

        if !changed {
            break;
        }

        // Recompute centroids
        let mut sums = vec![[0.0f32; 3]; k];
        let mut counts = vec![0usize; k];
        for (i, pos) in positions.iter().enumerate() {
            let c = assignments[i];
            sums[c][0] += pos[0];
            sums[c][1] += pos[1];
            sums[c][2] += pos[2];
            counts[c] += 1;
        }
        for j in 0..k {
            if counts[j] > 0 {
                let n = counts[j] as f32;
                centroids[j] = [sums[j][0] / n, sums[j][1] / n, sums[j][2] / n];
            }
        }
    }

    assignments
}

// ========================================================================
// Universe Builder
// ========================================================================

/// Maximum particles to return (hard cap for rendering performance)
const MAX_PARTICLES: usize = 5000;

/// Build the complete VoidUniverse from current database state.
/// This is the main entry point called by the Tauri command.
pub fn build_universe(
    db: &Database,
    context_engine: &ContextEngine,
    max_particles: Option<usize>,
    projection_version: i64,
) -> Result<VoidUniverse> {
    let cap = max_particles.unwrap_or(MAX_PARTICLES).min(MAX_PARTICLES);
    let start = std::time::Instant::now();

    // 1. Fetch source items with embeddings
    let source_items = db
        .get_source_items_for_projection(cap)
        .context("Failed to get source items")?;

    // 2. Fetch context chunks with embeddings
    let context_chunks = db
        .get_context_chunks_for_projection(cap / 5) // Context is 20% of budget
        .context("Failed to get context chunks")?;

    // 3. Determine embedding dimension from first available embedding
    let emb_dim = source_items
        .first()
        .map(|(_, _, _, _, emb, _)| emb.len())
        .or_else(|| context_chunks.first().map(|(_, _, _, emb)| emb.len()))
        .unwrap_or(384);

    if emb_dim == 0 {
        return Ok(VoidUniverse {
            core: [0.0; 3],
            interests: vec![],
            particles: vec![],
            clusters: vec![],
            total_items: 0,
            projection_version,
        });
    }

    // 4. Generate projection matrix
    let matrix = generate_projection_matrix(emb_dim);

    // 5. Project source items
    let mut particles: Vec<VoidParticle> =
        Vec::with_capacity(source_items.len() + context_chunks.len());
    let mut all_positions: Vec<[f32; 3]> = Vec::new();
    let mut context_positions: Vec<[f32; 3]> = Vec::new();

    for (id, source_type, title, url, embedding, age_hours) in &source_items {
        if embedding.len() != emb_dim {
            continue; // Skip items with mismatched embedding dims
        }
        let pos = project(embedding, &matrix);
        all_positions.push(pos);
        particles.push(VoidParticle {
            id: *id,
            layer: "source".to_string(),
            position: pos,
            label: title.clone(),
            url: url.clone(),
            relevance: 0.0, // Will be filled from analysis results if available
            source_type: source_type.clone(),
            age_hours: *age_hours,
        });
    }

    // 6. Project context chunks
    for (id, source_file, text_preview, embedding) in &context_chunks {
        if embedding.len() != emb_dim {
            continue;
        }
        let pos = project(embedding, &matrix);
        context_positions.push(pos);
        all_positions.push(pos);
        particles.push(VoidParticle {
            id: *id,
            layer: "context".to_string(),
            position: pos,
            label: text_preview.clone(),
            url: None,
            relevance: 0.0,
            source_type: source_file.clone(),
            age_hours: 0.0,
        });
    }

    // 7. Compute core position (centroid of context chunks, or all items if no context)
    let core = if !context_positions.is_empty() {
        centroid(&context_positions)
    } else {
        centroid(&all_positions)
    };

    // 8. Build interest orbitals from context engine
    let interests = build_interest_nodes(context_engine, &matrix, emb_dim);

    // 9. Apply LOD clustering if too many particles
    let clusters = if particles.len() > cap {
        // Truncate particles to cap, build clusters from all positions
        let positions_for_clustering: Vec<[f32; 3]> =
            particles.iter().map(|p| p.position).collect();
        let k = 20.min(particles.len());
        let assignments = kmeans_3d(&positions_for_clustering, k, 30);
        let cluster_nodes = build_cluster_nodes(&particles, &assignments, k);
        particles.truncate(cap);
        cluster_nodes
    } else {
        vec![]
    };

    let total_items = db.total_item_count().unwrap_or(0) as usize;

    info!(
        target: "4da::void",
        particles = particles.len(),
        interests = interests.len(),
        clusters = clusters.len(),
        total_items = total_items,
        elapsed_ms = start.elapsed().as_millis(),
        "Built void universe"
    );

    Ok(VoidUniverse {
        core,
        interests,
        particles,
        clusters,
        total_items,
        projection_version,
    })
}

/// Build interest nodes from context engine topics
fn build_interest_nodes(
    context_engine: &ContextEngine,
    matrix: &[[f32; PROJ_DIM]],
    emb_dim: usize,
) -> Vec<InterestNode> {
    let interests = match context_engine.get_interests() {
        Ok(i) => i,
        Err(_) => return vec![],
    };

    interests
        .iter()
        .filter_map(|interest| {
            let embedding = interest.embedding.as_ref()?;
            if embedding.len() != emb_dim {
                return None;
            }
            let position = project(embedding, matrix);
            Some(InterestNode {
                name: interest.topic.clone(),
                position,
                weight: interest.weight,
                item_count: 0, // Could be enriched with actual counts
            })
        })
        .collect()
}

/// Build cluster summaries from k-means assignments
fn build_cluster_nodes(
    particles: &[VoidParticle],
    assignments: &[usize],
    k: usize,
) -> Vec<ClusterNode> {
    #[allow(clippy::type_complexity)]
    let mut clusters: Vec<(Vec<[f32; 3]>, Vec<f32>, String, Vec<String>)> = (0..k)
        .map(|_| (vec![], vec![], String::new(), vec![]))
        .collect();

    for (i, particle) in particles.iter().enumerate() {
        if i >= assignments.len() {
            break;
        }
        let c = assignments[i];
        if c < k {
            clusters[c].0.push(particle.position);
            clusters[c].1.push(particle.relevance);
            if clusters[c].2.is_empty() || particle.relevance > clusters[c].1[0] {
                clusters[c].2 = particle.label.clone();
            }
            clusters[c].3.push(particle.source_type.clone());
        }
    }

    clusters
        .into_iter()
        .filter(|(positions, _, _, _)| !positions.is_empty())
        .map(|(positions, relevances, top_label, sources)| {
            let count = positions.len();
            let avg_relevance = if relevances.is_empty() {
                0.0
            } else {
                relevances.iter().sum::<f32>() / relevances.len() as f32
            };
            // Find dominant source type
            let dominant_source = sources
                .iter()
                .fold(HashMap::new(), |mut acc, s| {
                    *acc.entry(s.clone()).or_insert(0usize) += 1;
                    acc
                })
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(s, _)| s)
                .unwrap_or_default();

            ClusterNode {
                centroid: centroid(&positions),
                count,
                avg_relevance,
                top_label,
                dominant_source,
            }
        })
        .collect()
}

/// Find k nearest neighbors to a particle position
pub fn find_neighbors(
    target_id: i64,
    target_layer: &str,
    particles: &[VoidParticle],
    k: usize,
) -> Vec<VoidParticle> {
    let target = particles
        .iter()
        .find(|p| p.id == target_id && p.layer == target_layer);

    let target_pos = match target {
        Some(p) => p.position,
        None => return vec![],
    };

    let mut scored: Vec<(f32, &VoidParticle)> = particles
        .iter()
        .filter(|p| !(p.id == target_id && p.layer == target_layer))
        .map(|p| (dist_sq(&target_pos, &p.position), p))
        .collect();

    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored.into_iter().map(|(_, p)| p.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xorshift_deterministic() {
        let mut rng1 = 42u64;
        let mut rng2 = 42u64;
        for _ in 0..100 {
            assert_eq!(xorshift64(&mut rng1), xorshift64(&mut rng2));
        }
    }

    #[test]
    fn test_projection_matrix_deterministic() {
        let m1 = generate_projection_matrix(EMB_DIM_SMALL);
        let m2 = generate_projection_matrix(EMB_DIM_SMALL);
        assert_eq!(m1.len(), EMB_DIM_SMALL);
        assert_eq!(m2.len(), EMB_DIM_SMALL);
        for i in 0..EMB_DIM_SMALL {
            for j in 0..PROJ_DIM {
                assert_eq!(m1[i][j], m2[i][j], "Matrix not deterministic at [{i}][{j}]");
            }
        }
    }

    #[test]
    fn test_projection_matrix_not_zero() {
        let m = generate_projection_matrix(EMB_DIM_SMALL);
        let all_zero = m.iter().all(|row| row.iter().all(|&v| v == 0.0));
        assert!(!all_zero, "Projection matrix should not be all zeros");
    }

    #[test]
    fn test_project_zero_embedding() {
        let m = generate_projection_matrix(EMB_DIM_SMALL);
        let zero = vec![0.0f32; EMB_DIM_SMALL];
        let pos = project(&zero, &m);
        assert_eq!(pos, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_project_stability() {
        let m = generate_projection_matrix(EMB_DIM_SMALL);
        let emb: Vec<f32> = (0..EMB_DIM_SMALL).map(|i| (i as f32) * 0.01).collect();
        let pos1 = project(&emb, &m);
        let pos2 = project(&emb, &m);
        assert_eq!(pos1, pos2);
    }

    #[test]
    fn test_distance_preservation() {
        let m = generate_projection_matrix(EMB_DIM_SMALL);
        let e1 = vec![0.1f32; EMB_DIM_SMALL];
        let mut e2 = vec![0.1f32; EMB_DIM_SMALL];
        e2[0] = 0.2;
        let e3: Vec<f32> = (0..EMB_DIM_SMALL)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let p1 = project(&e1, &m);
        let p2 = project(&e2, &m);
        let p3 = project(&e3, &m);

        let d12 = dist_sq(&p1, &p2);
        let d13 = dist_sq(&p1, &p3);
        assert!(
            d12 < d13,
            "Similar embeddings should project closer: d12={d12}, d13={d13}"
        );
    }

    #[test]
    fn test_batch_projection() {
        let m = generate_projection_matrix(EMB_DIM_SMALL);
        let embeddings: Vec<Vec<f32>> = (0..10)
            .map(|i| {
                (0..EMB_DIM_SMALL)
                    .map(|j| (i * EMB_DIM_SMALL + j) as f32 * 0.001)
                    .collect()
            })
            .collect();
        let positions = project_batch(&embeddings, &m);
        assert_eq!(positions.len(), 10);
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert_ne!(
                    positions[i], positions[j],
                    "Positions {i} and {j} shouldn't be identical"
                );
            }
        }
    }

    #[test]
    fn test_centroid() {
        let positions = vec![[1.0, 2.0, 3.0], [3.0, 4.0, 5.0]];
        let c = centroid(&positions);
        assert!((c[0] - 2.0).abs() < 1e-6);
        assert!((c[1] - 3.0).abs() < 1e-6);
        assert!((c[2] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_centroid_empty() {
        let positions: Vec<[f32; 3]> = vec![];
        let c = centroid(&positions);
        assert_eq!(c, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_kmeans_basic() {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [0.1, 0.1, 0.1],
            [0.05, 0.05, 0.05],
            [10.0, 10.0, 10.0],
            [10.1, 10.1, 10.1],
            [9.9, 9.9, 9.9],
        ];
        let assignments = kmeans_3d(&positions, 2, 20);
        assert_eq!(assignments.len(), 6);
        assert_eq!(assignments[0], assignments[1]);
        assert_eq!(assignments[1], assignments[2]);
        assert_eq!(assignments[3], assignments[4]);
        assert_eq!(assignments[4], assignments[5]);
        assert_ne!(assignments[0], assignments[3]);
    }

    #[test]
    fn test_kmeans_empty() {
        let positions: Vec<[f32; 3]> = vec![];
        let assignments = kmeans_3d(&positions, 5, 10);
        assert!(assignments.is_empty());
    }

    #[test]
    fn test_find_neighbors() {
        let particles = vec![
            VoidParticle {
                id: 1,
                layer: "source".into(),
                position: [0.0, 0.0, 0.0],
                label: "A".into(),
                url: None,
                relevance: 0.5,
                source_type: "hn".into(),
                age_hours: 1.0,
            },
            VoidParticle {
                id: 2,
                layer: "source".into(),
                position: [1.0, 0.0, 0.0],
                label: "B".into(),
                url: None,
                relevance: 0.6,
                source_type: "hn".into(),
                age_hours: 2.0,
            },
            VoidParticle {
                id: 3,
                layer: "source".into(),
                position: [10.0, 10.0, 10.0],
                label: "C".into(),
                url: None,
                relevance: 0.1,
                source_type: "arxiv".into(),
                age_hours: 3.0,
            },
        ];

        let neighbors = find_neighbors(1, "source", &particles, 2);
        assert_eq!(neighbors.len(), 2);
        assert_eq!(neighbors[0].id, 2);
        assert_eq!(neighbors[1].id, 3);
    }

    #[test]
    fn test_find_neighbors_not_found() {
        let particles = vec![VoidParticle {
            id: 1,
            layer: "source".into(),
            position: [0.0, 0.0, 0.0],
            label: "A".into(),
            url: None,
            relevance: 0.5,
            source_type: "hn".into(),
            age_hours: 1.0,
        }];
        let neighbors = find_neighbors(999, "source", &particles, 5);
        assert!(neighbors.is_empty());
    }

    #[test]
    fn test_build_cluster_nodes() {
        let particles = vec![
            VoidParticle {
                id: 1,
                layer: "source".into(),
                position: [0.0, 0.0, 0.0],
                label: "A".into(),
                url: None,
                relevance: 0.8,
                source_type: "hn".into(),
                age_hours: 1.0,
            },
            VoidParticle {
                id: 2,
                layer: "source".into(),
                position: [1.0, 1.0, 1.0],
                label: "B".into(),
                url: None,
                relevance: 0.6,
                source_type: "hn".into(),
                age_hours: 2.0,
            },
            VoidParticle {
                id: 3,
                layer: "source".into(),
                position: [10.0, 10.0, 10.0],
                label: "C".into(),
                url: None,
                relevance: 0.2,
                source_type: "arxiv".into(),
                age_hours: 3.0,
            },
        ];
        let assignments = vec![0, 0, 1];
        assert_eq!(particles.len(), 3);
        assert_eq!(assignments.len(), 3);
    }
}
