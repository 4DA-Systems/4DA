// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;

#[test]
fn embeddings_are_384_dimensional() {
    let embeddings = corpus_embeddings();
    for (i, emb) in embeddings.iter().enumerate() {
        assert_eq!(
            emb.len(),
            384,
            "Corpus item {} has dimension {} instead of 384",
            i + 1,
            emb.len()
        );
    }
}

#[test]
fn embeddings_are_normalized() {
    let embeddings = corpus_embeddings();
    for (i, emb) in embeddings.iter().enumerate() {
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "Corpus item {} has L2 norm {:.4} (expected ~1.0)",
            i + 1,
            norm
        );
    }
}

#[test]
fn embeddings_are_deterministic() {
    let first = corpus_embeddings();
    let second = corpus_embeddings();
    for (i, (a, b)) in first.iter().zip(second.iter()).enumerate() {
        assert_eq!(
            a,
            b,
            "Corpus item {} produced different embeddings on two calls",
            i + 1
        );
    }
}

#[test]
fn same_domain_items_are_similar() {
    let embeddings = corpus_embeddings();
    // Items 1 and 2 are both Systems/Rust direct matches
    let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
    assert!(
        sim > 0.5,
        "Same-domain items (1, 2) cosine similarity {:.4} should be > 0.5",
        sim
    );
}

#[test]
fn different_domain_items_are_dissimilar() {
    let embeddings = corpus_embeddings();
    // Item 1 is Systems/Rust, item 11 (index 10) is ML/Python
    let sim = cosine_similarity(&embeddings[0], &embeddings[10]);
    assert!(
        sim < 0.5,
        "Cross-domain items (Systems, ML) cosine similarity {:.4} should be < 0.5",
        sim
    );
}

#[test]
fn interest_embeddings_match_domain() {
    let embeddings = corpus_embeddings();
    // Persona 0 (rust_systems) interest should align with Systems corpus items
    let interest = interest_embedding(0);
    let sim_systems = cosine_similarity(&interest, &embeddings[0]);
    let sim_ml = cosine_similarity(&interest, &embeddings[10]);
    assert!(
        sim_systems > sim_ml,
        "Persona 0 interest more similar to ML ({:.4}) than Systems ({:.4})",
        sim_ml,
        sim_systems
    );
    assert!(
        sim_systems > 0.5,
        "Persona 0 interest-to-Systems cosine {:.4} should be > 0.5",
        sim_systems
    );
}

#[test]
fn all_corpus_items_covered() {
    let embeddings = corpus_embeddings();
    assert_eq!(
        embeddings.len(),
        220,
        "Expected 220 corpus embeddings, got {}",
        embeddings.len()
    );
}

#[test]
fn domain_signatures_are_orthogonal() {
    use DomainBlock::*;
    let blocks = [
        Systems,
        Web,
        ML,
        DevOps,
        Mobile,
        Database,
        Security,
        Distributed,
        FP,
        Career,
        Business,
        Meta,
    ];
    let names = [
        "Systems",
        "Web",
        "ML",
        "DevOps",
        "Mobile",
        "Database",
        "Security",
        "Distributed",
        "FP",
        "Career",
        "Business",
        "Meta",
    ];
    for i in 0..blocks.len() {
        for j in (i + 1)..blocks.len() {
            let sig_a = blocks[i].signature();
            let sig_b = blocks[j].signature();
            let cos = cosine_similarity(&sig_a, &sig_b);
            // 0.70 threshold catches severe overlap (old Mobile↔Web was 0.7427)
            // while allowing legitimate domain adjacency (Systems↔Database ≈ 0.62)
            assert!(
                cos <= 0.70,
                "{} <-> {} cosine = {:.4} exceeds 0.70 threshold",
                names[i],
                names[j],
                cos
            );
        }
    }
}
