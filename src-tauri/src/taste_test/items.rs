//! Taste Test Items — 15 maximally-discriminating calibration cards.
//!
//! Each item is selected from the simulation corpus for maximum information
//! gain: items where personas disagree most about relevance.
//!
//! Persona order (columns): [rust, python, typescript, devops, mobile,
//!   bootstrap, power, switcher, niche]

use super::TasteCard;

/// Get the calibration items as owned TasteCard values.
///
/// Items selected from corpus IDs: 1, 11, 16, 28, 19, 21, 4, 24, 6, 96, 8, 17, 45, 91, 142
pub fn calibration_items() -> Vec<TasteCard> {
    vec![
        // Slot 0: Rust 2024 Edition — rust_systems discriminator
        TasteCard { id: 1, slot: 0, title: "Rust 2024 Edition: What's New".into(), snippet: "New language features including improved async support, new syntax for lifetime annotations, and better error messages.".into(), source_hint: "Hacker News".into(), category_hint: "Systems".into() },
        // Slot 1: PyTorch 2.0 — python_ml discriminator
        TasteCard { id: 11, slot: 1, title: "PyTorch 2.0 performance benchmarks".into(), snippet: "torch.compile() delivers 1.5-2x speedup on training workloads with minimal code changes.".into(), source_hint: "Hacker News".into(), category_hint: "ML/AI".into() },
        // Slot 2: Kubernetes 1.30 — devops_sre discriminator
        TasteCard { id: 16, slot: 2, title: "Kubernetes 1.30: What's new for operators".into(), snippet: "Better CRD validation, improved leader election, and new admission policies for the operator pattern.".into(), source_hint: "Hacker News".into(), category_hint: "DevOps".into() },
        // Slot 3: Next.js App Router — fullstack_ts discriminator
        TasteCard { id: 28, slot: 3, title: "Next.js App Router: Server components deep dive".into(), snippet: "React Server Components in Next.js App Router: data fetching patterns, caching strategies, and streaming.".into(), source_hint: "Hacker News".into(), category_hint: "Web".into() },
        // Slot 4: React Native — mobile_dev discriminator
        TasteCard { id: 19, slot: 4, title: "React Native 0.74 new architecture".into(), snippet: "Fabric renderer and JSI bridge replacement. Performance improvements and better TypeScript integration.".into(), source_hint: "Hacker News".into(), category_hint: "Mobile".into() },
        // Slot 5: GHC 9.8 — niche_specialist discriminator
        TasteCard { id: 21, slot: 5, title: "Haskell GHC 9.8 improvements".into(), snippet: "Improved type-level programming, better error messages, and performance improvements for compiled code.".into(), source_hint: "Hacker News".into(), category_hint: "Languages".into() },
        // Slot 6: tokio internals — rust+power_user separator
        TasteCard { id: 4, slot: 6, title: "tokio async runtime internals explained".into(), snippet: "Deep dive into task scheduling, executor pools, IO drivers, and waker mechanics in tokio.".into(), source_hint: "Hacker News".into(), category_hint: "Systems".into() },
        // Slot 7: Go generics — context_switcher discriminator
        TasteCard { id: 24, slot: 7, title: "Go generics: Practical patterns after 2 years".into(), snippet: "Type constraints, generic data structures, and where Go generics shine two years after introduction.".into(), source_hint: "Hacker News".into(), category_hint: "Languages".into() },
        // Slot 8: WASM + Rust — rust+fullstack overlap test
        TasteCard { id: 6, slot: 8, title: "WebAssembly + Rust: Production case study".into(), snippet: "Running Rust compiled to WASM in production: performance-critical browser code and the component model.".into(), source_hint: "Blog".into(), category_hint: "Systems/Web".into() },
        // Slot 9: Career noise — noise calibrator
        TasteCard { id: 96, slot: 9, title: "Senior Rust Engineer at Cloudflare — remote".into(), snippet: "5+ years experience, distributed systems background required. $180-250k.".into(), source_hint: "Hacker News".into(), category_hint: "Jobs".into() },
        // Slot 10: sqlite-vec — power_user breadth test
        TasteCard { id: 8, slot: 10, title: "sqlite-vec: Vector search for SQLite".into(), snippet: "Store and query embeddings directly in SQLite using cosine similarity without a separate vector database.".into(), source_hint: "Hacker News".into(), category_hint: "Databases".into() },
        // Slot 11: eBPF — devops_sre depth test
        TasteCard { id: 17, slot: 11, title: "eBPF for production observability".into(), snippet: "Zero-overhead observability: trace system calls, network packets, and application behavior without modifying code.".into(), source_hint: "Hacker News".into(), category_hint: "DevOps".into() },
        // Slot 12: gRPC vs REST — context_switcher breadth test
        TasteCard { id: 45, slot: 12, title: "gRPC vs REST: When to choose each".into(), snippet: "Comparing gRPC and REST for service communication: performance, schema evolution, streaming support.".into(), source_hint: "Hacker News".into(), category_hint: "Architecture".into() },
        // Slot 13: Vector databases — borderline test
        TasteCard { id: 91, slot: 13, title: "Vector databases: The hype and the reality".into(), snippet: "Pinecone, Weaviate, and Chroma vs SQLite-vec or pgvector for semantic search workloads.".into(), source_hint: "Hacker News".into(), category_hint: "Databases".into() },
        // Slot 14: Show HN side project — meta noise calibrator
        TasteCard { id: 142, slot: 14, title: "Show HN: My side project — another note-taking app".into(), snippet: "Note-taking app built over 6 months. Like Notion but simpler. Built with React and Node.js.".into(), source_hint: "Hacker News".into(), category_hint: "Show HN".into() },
    ]
}

/// Likelihood matrix: P(interested | persona_j) for each item.
///
/// Rows = items (slots 0-14), Columns = personas in canonical order:
/// [rust, python, typescript, devops, mobile, bootstrap, power, switcher, niche]
///
/// Derived from corpus ExpectedOutcome:
///   StrongRelevant → 0.90
///   WeakRelevant   → 0.65
///   MildBorderline → 0.40
///   NotRelevant    → 0.10
///   Excluded       → 0.05
pub static LIKELIHOOD_MATRIX: [[f64; 9]; 15] = [
    // Slot 0: Rust 2024 [S, N, N, N, N, N, S, S, B]
    [0.90, 0.10, 0.10, 0.10, 0.10, 0.10, 0.90, 0.90, 0.40],
    // Slot 1: PyTorch [N, S, N, N, N, N, S, B, N]
    [0.10, 0.90, 0.10, 0.10, 0.10, 0.10, 0.90, 0.40, 0.10],
    // Slot 2: K8s [N, N, N, S, N, N, S, W, N]
    [0.10, 0.10, 0.10, 0.90, 0.10, 0.10, 0.90, 0.65, 0.10],
    // Slot 3: Next.js [N, N, S, N, N, S, S, W, N]
    [0.10, 0.10, 0.90, 0.10, 0.10, 0.90, 0.90, 0.65, 0.10],
    // Slot 4: React Native [N, N, W, N, S, B, W, W, N]
    [0.10, 0.10, 0.65, 0.10, 0.90, 0.40, 0.65, 0.65, 0.10],
    // Slot 5: GHC [N, N, N, N, N, N, W, N, S]
    [0.10, 0.10, 0.10, 0.10, 0.10, 0.10, 0.65, 0.10, 0.90],
    // Slot 6: tokio [S, N, N, N, N, N, S, S, B]
    [0.90, 0.10, 0.10, 0.10, 0.10, 0.10, 0.90, 0.90, 0.40],
    // Slot 7: Go generics [W, N, N, N, N, N, W, S, N]
    [0.65, 0.10, 0.10, 0.10, 0.10, 0.10, 0.65, 0.90, 0.10],
    // Slot 8: WASM+Rust [S, N, W, N, N, B, S, W, N]
    [0.90, 0.10, 0.65, 0.10, 0.10, 0.40, 0.90, 0.65, 0.10],
    // Slot 9: Job posting — noise calibrator (non-uniform for discrimination)
    [0.35, 0.05, 0.05, 0.05, 0.05, 0.05, 0.20, 0.20, 0.05],
    // Slot 10: sqlite-vec [S, W, N, N, N, N, S, S, N]
    [0.90, 0.65, 0.10, 0.10, 0.10, 0.10, 0.90, 0.90, 0.10],
    // Slot 11: eBPF [N, N, N, S, N, N, S, W, N]
    [0.10, 0.10, 0.10, 0.90, 0.10, 0.10, 0.90, 0.65, 0.10],
    // Slot 12: gRPC vs REST [W, N, W, W, N, W, W, S, N]
    [0.65, 0.10, 0.65, 0.65, 0.10, 0.65, 0.65, 0.90, 0.10],
    // Slot 13: Vector DBs [W, W, B, N, N, N, S, W, N]
    [0.65, 0.65, 0.40, 0.10, 0.10, 0.10, 0.90, 0.65, 0.10],
    // Slot 14: Show HN side project — noise (non-uniform for discrimination)
    [0.05, 0.05, 0.25, 0.05, 0.05, 0.40, 0.10, 0.05, 0.05],
];

/// Mapping from item slot to corpus item ID.
pub static SLOT_TO_CORPUS_ID: [u64; 15] = [1, 11, 16, 28, 19, 21, 4, 24, 6, 96, 8, 17, 45, 91, 142];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_15_items_present() {
        let items = calibration_items();
        assert_eq!(items.len(), 15);
    }

    #[test]
    fn test_likelihood_matrix_valid() {
        for (i, row) in LIKELIHOOD_MATRIX.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(&val),
                    "LIKELIHOOD_MATRIX[{i}][{j}] = {val} is out of [0.0, 1.0]"
                );
            }
        }
    }

    #[test]
    fn test_items_have_discrimination() {
        for (i, row) in LIKELIHOOD_MATRIX.iter().enumerate() {
            let mean: f64 = row.iter().sum::<f64>() / 9.0;
            let variance: f64 = row.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / 9.0;
            assert!(
                variance > 0.01,
                "Item slot {i} has insufficient discrimination: variance = {variance:.4}"
            );
        }
    }

    #[test]
    fn test_items_have_nonempty_fields() {
        for item in calibration_items() {
            assert!(!item.title.is_empty(), "Item {} has empty title", item.id);
            assert!(
                !item.snippet.is_empty(),
                "Item {} has empty snippet",
                item.id
            );
            assert!(
                !item.source_hint.is_empty(),
                "Item {} has empty source_hint",
                item.id
            );
            assert!(
                !item.category_hint.is_empty(),
                "Item {} has empty category_hint",
                item.id
            );
        }
    }
}
