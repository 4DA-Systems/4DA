//! Domain-Specific Embedding Generation for Simulation
//!
//! Generates deterministic, domain-aware pseudo-embeddings for the corpus items.
//! Uses a taxonomy of 12 domain blocks, each with a characteristic signature vector.
//! All simulation tests use these embeddings, enabling cosine-similarity-based
//! scoring that discriminates in-domain from off-domain content.

const EMBEDDING_DIM: usize = 384;
const SIGNATURE_LEN: usize = 32;

// ============================================================================
// Domain Blocks (12 categories)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DomainBlock {
    Systems,     // Rust, C/C++, OS, kernel, memory management
    Web,         // React, TypeScript, Node.js, CSS, HTML, frontend
    ML,          // Python, PyTorch, TensorFlow, ML, LLM, data science
    DevOps,      // Kubernetes, Docker, Terraform, CI/CD, monitoring
    Mobile,      // React Native, iOS, Android, Expo, Flutter
    Database,    // SQL, PostgreSQL, SQLite, Redis, MongoDB
    Security,    // CVE, vulnerability, encryption, auth, pentest
    Distributed, // microservices, message queues, consensus, gRPC
    FP,          // Haskell, functional, type theory, category theory, Nix
    Career,      // hiring, compensation, interview, remote work
    Business,    // funding, startup, pricing, acquisition
    Meta,        // culture, opinion, philosophy, burnout, productivity
}

impl DomainBlock {
    /// Returns a 32-float signature unique to this domain block.
    /// Each block has a distinct pattern that tiles across the 384-dim space.
    fn signature(&self) -> [f32; SIGNATURE_LEN] {
        match self {
            Self::Systems => [
                0.8, 0.6, 0.3, 0.1, -0.2, 0.5, 0.7, 0.4, 0.2, -0.1, 0.6, 0.3, 0.5, 0.7, 0.1, -0.3,
                0.4, 0.8, 0.2, 0.6, 0.3, -0.1, 0.5, 0.7, 0.4, 0.2, 0.6, 0.8, 0.1, -0.2, 0.3, 0.5,
            ],
            Self::Web => [
                -0.1, 0.7, 0.8, 0.5, 0.3, -0.2, 0.1, 0.6, 0.4, 0.8, -0.1, 0.5, 0.7, 0.2, 0.6, 0.3,
                -0.2, 0.4, 0.8, 0.1, 0.5, 0.7, 0.3, -0.1, 0.6, 0.8, 0.2, 0.4, 0.5, 0.3, -0.2, 0.7,
            ],
            Self::ML => [
                0.3, -0.1, 0.5, 0.8, 0.7, 0.4, -0.2, 0.6, 0.8, 0.2, 0.5, -0.1, 0.3, 0.7, 0.4, 0.8,
                0.6, -0.2, 0.1, 0.5, 0.7, 0.3, -0.1, 0.8, 0.4, 0.6, -0.2, 0.2, 0.8, 0.5, 0.7, 0.1,
            ],
            Self::DevOps => [
                0.4, 0.2, -0.1, 0.6, 0.3, 0.8, 0.5, -0.2, 0.7, 0.4, 0.1, 0.8, -0.1, 0.5, 0.3, 0.6,
                0.8, 0.2, -0.2, 0.7, 0.4, 0.1, 0.8, 0.3, -0.1, 0.5, 0.7, 0.6, 0.2, 0.8, 0.4, -0.2,
            ],
            // Mobile — orthogonal to Web: emphasize platform/native slots, suppress web slots
            Self::Mobile => [
                0.05, -0.1, 0.05, 0.05, 0.95, 0.85, 0.05, -0.1, 0.10, -0.1, 0.80, 0.10, -0.1, 0.05,
                0.90, 0.80, 0.05, 0.10, -0.1, 0.10, 0.05, -0.1, 0.05, 0.10, -0.1, -0.1, 0.10, 0.05,
                0.85, 0.05, 0.10, -0.1,
            ],
            Self::Database => [
                0.6, 0.4, 0.2, -0.1, 0.5, 0.3, 0.8, 0.1, 0.7, -0.2, 0.4, 0.6, 0.8, 0.3, -0.1, 0.5,
                0.2, 0.7, 0.6, 0.4, -0.2, 0.8, 0.1, 0.3, 0.5, 0.7, 0.4, -0.1, 0.6, 0.2, 0.8, 0.3,
            ],
            Self::Security => [
                0.2, 0.8, -0.1, 0.4, 0.6, 0.1, 0.3, 0.7, 0.5, 0.8, -0.2, 0.6, 0.1, 0.4, 0.8, -0.1,
                0.7, 0.3, 0.5, 0.2, 0.8, -0.2, 0.6, 0.4, 0.1, 0.3, 0.8, 0.7, -0.1, 0.5, 0.2, 0.6,
            ],
            Self::Distributed => [
                0.5, 0.1, 0.7, -0.2, 0.4, 0.6, 0.2, 0.8, -0.1, 0.3, 0.7, 0.5, 0.4, 0.8, -0.2, 0.1,
                0.6, 0.3, 0.7, -0.1, 0.2, 0.8, 0.5, 0.4, 0.3, -0.2, 0.1, 0.7, 0.8, 0.6, 0.5, 0.4,
            ],
            Self::FP => [
                0.1, 0.3, 0.6, 0.7, -0.2, 0.8, 0.4, 0.2, 0.5, -0.1, 0.8, 0.7, 0.3, -0.2, 0.6, 0.4,
                0.1, 0.5, 0.8, 0.7, -0.1, 0.3, 0.6, 0.2, -0.2, 0.4, 0.5, 0.8, 0.7, 0.1, 0.6, -0.1,
            ],
            Self::Career => [
                -0.3, 0.4, 0.1, 0.2, 0.6, -0.1, 0.8, 0.3, 0.5, 0.7, 0.2, -0.2, 0.4, 0.1, 0.6, 0.8,
                -0.3, 0.5, 0.3, 0.7, 0.1, -0.2, 0.4, 0.8, 0.6, 0.2, 0.3, -0.1, 0.5, 0.7, -0.3, 0.4,
            ],
            Self::Business => [
                0.7, -0.2, 0.4, 0.1, 0.3, 0.5, -0.1, 0.8, 0.6, 0.2, -0.3, 0.4, 0.7, 0.5, 0.1, -0.2,
                0.3, 0.8, 0.6, -0.1, 0.4, 0.2, 0.7, -0.3, 0.5, 0.1, 0.8, 0.3, -0.2, 0.6, 0.4, 0.7,
            ],
            Self::Meta => [
                -0.1, 0.2, 0.5, -0.3, 0.7, 0.4, 0.1, 0.3, 0.8, 0.6, -0.2, 0.5, -0.1, 0.3, 0.7, 0.2,
                0.4, -0.3, 0.6, 0.8, 0.1, 0.5, -0.2, 0.3, 0.7, 0.4, -0.1, 0.6, 0.2, -0.3, 0.8, 0.5,
            ],
        }
    }
}

// ============================================================================
// Embedding Spec
// ============================================================================

pub(super) struct EmbeddingSpec {
    blocks: &'static [(DomainBlock, f32)],
}

// ============================================================================
// Deterministic PRNG (xorshift64)
// ============================================================================

struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        // Avoid zero state which would be degenerate
        Self {
            state: seed.wrapping_add(1),
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Returns a float in [-0.05, 0.05] for noise injection
    fn next_noise(&mut self) -> f32 {
        let bits = self.next();
        let unit = (bits & 0xFFFFFF) as f32 / 0x1000000 as f32;
        (unit - 0.5) * 0.1
    }
}

// ============================================================================
// Core generation
// ============================================================================

/// Generate a 384-dim embedding from a spec and seed, deterministically.
fn generate_embedding(spec: &EmbeddingSpec, seed: u64) -> Vec<f32> {
    let mut rng = Xorshift64::new(seed);
    let mut embedding = vec![0.0_f32; EMBEDDING_DIM];

    // Step 1: Fill with small deterministic noise
    for v in embedding.iter_mut() {
        *v = rng.next_noise();
    }

    // Step 2: Add weighted domain block signatures (tiled across 384 dims)
    for &(block, weight) in spec.blocks {
        let sig = block.signature();
        for (i, v) in embedding.iter_mut().enumerate() {
            *v += sig[i % SIGNATURE_LEN] * weight;
        }
    }

    // Step 3: L2-normalize to unit length
    l2_normalize(&mut embedding);
    embedding
}

fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ============================================================================
// Static Embedding Specs (by corpus ID range)
// ============================================================================

use DomainBlock::*;

// -- DirectMatch: Systems/Rust (IDs 1-10)
static SPEC_SYSTEMS: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 1.0), (Database, 0.3)],
};

// -- DirectMatch: ML/Python (IDs 11-13)
static SPEC_ML: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 1.0)],
};

// -- DirectMatch: Web/TS (IDs 14-15)
static SPEC_WEB: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 1.0)],
};

// -- DirectMatch: DevOps (IDs 16-18)
static SPEC_DEVOPS: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 1.0)],
};

// -- DirectMatch: Mobile (IDs 19-20)
static SPEC_MOBILE: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Mobile, 1.0), (Web, 0.3)],
};

// -- DirectMatch: FP/Niche (IDs 21-23)
static SPEC_FP: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(FP, 1.0)],
};

// -- DirectMatch: Go/Context Switcher (ID 24)
static SPEC_GO_BACKEND: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.5), (Distributed, 0.7)],
};

// -- DirectMatch: More Rust (IDs 25-26)
static SPEC_SYSTEMS_STRONG: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 1.0)],
};

// -- DirectMatch: More Python (ID 27)
static SPEC_ML_PYTHON: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 0.9), (Distributed, 0.2)],
};

// -- DirectMatch: More Web (ID 28)
static SPEC_WEB_FULLSTACK: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.9), (Database, 0.2)],
};

// -- DirectMatch: More DevOps (ID 29)
static SPEC_DEVOPS_OBS: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 0.9), (Distributed, 0.2)],
};

// -- DirectMatch: More Mobile (ID 30)
static SPEC_MOBILE_CICD: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Mobile, 0.8), (DevOps, 0.3)],
};

// -- AdjacentMatch (IDs 31-50): mixed with moderate signals
static SPEC_ADJ_SYSTEMS_WEB: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.5), (Web, 0.5)],
};
static SPEC_ADJ_ML_DB: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 0.5), (Database, 0.4)],
};
static SPEC_ADJ_DEVOPS_SEC: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 0.5), (Security, 0.4)],
};
static SPEC_ADJ_WEB_MOBILE: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.5), (Mobile, 0.4)],
};
static SPEC_ADJ_DISTRIBUTED: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Distributed, 0.6), (Systems, 0.3)],
};

// -- CrossDomainNoise (IDs 51-75): category opposite to primary personas
static SPEC_CDN_ML_ONLY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 0.8), (Meta, 0.2)],
};
static SPEC_CDN_WEB_ONLY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.8), (Career, 0.1)],
};
static SPEC_CDN_DEVOPS_ONLY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 0.7), (Business, 0.2)],
};
static SPEC_CDN_MOBILE_ONLY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Mobile, 0.7), (Meta, 0.2)],
};
static SPEC_CDN_FP_ONLY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(FP, 0.7), (Career, 0.2)],
};

// -- Borderline (IDs 76-95): weak mixed signals
static SPEC_BORDERLINE_A: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.3), (Web, 0.2), (Meta, 0.2)],
};
static SPEC_BORDERLINE_B: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 0.3), (DevOps, 0.2), (Career, 0.2)],
};
static SPEC_BORDERLINE_C: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.3), (Database, 0.2), (Business, 0.1)],
};
static SPEC_BORDERLINE_D: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Distributed, 0.3), (Security, 0.2), (Meta, 0.2)],
};

// -- Career (IDs 96-110)
static SPEC_CAREER: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Career, 1.0)],
};

// -- Security (IDs 111-125)
static SPEC_SECURITY: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Security, 0.9), (Systems, 0.2)],
};

// -- IntroductoryNoise (IDs 126-140): weak generic signals
static SPEC_INTRO: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.2), (Career, 0.2), (Meta, 0.3)],
};

// -- ShowHNNoise (IDs 141-155): mixed general
static SPEC_SHOW_HN: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Web, 0.3), (Business, 0.3), (Meta, 0.2)],
};

// -- MetaNoise (IDs 156-170)
static SPEC_META: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Meta, 1.0)],
};

// -- BusinessNoise (IDs 171-180)
static SPEC_BUSINESS: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Business, 0.9), (Career, 0.2)],
};

// -- ReleaseNotes (IDs 181-185)
static SPEC_RELEASE_SYS: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.4), (Web, 0.3), (DevOps, 0.2)],
};

// -- HNDiscussion (IDs 186-195)
static SPEC_HN_DISC: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Meta, 0.4), (Career, 0.2), (Web, 0.2)],
};

// -- DistantlyRelevant (IDs 196-205)
static SPEC_DISTANT: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Business, 0.3), (Meta, 0.3), (Career, 0.2)],
};

// -- ReverseEngineering (IDs 206-215)
static SPEC_REVERSE: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.5), (Security, 0.5)],
};

// -- ReleaseNotes expansion (IDs 216-220): domain-specific releases
static SPEC_RELEASE_GO: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(Systems, 0.5), (Distributed, 0.5)],
};
static SPEC_RELEASE_HASKELL: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(FP, 1.0)],
};
static SPEC_RELEASE_GRAFANA: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 0.9), (Distributed, 0.2)],
};
static SPEC_RELEASE_DOCKER: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(DevOps, 1.0)],
};
static SPEC_RELEASE_HF: EmbeddingSpec = EmbeddingSpec {
    blocks: &[(ML, 1.0)],
};

// ============================================================================
// Corpus Item -> EmbeddingSpec Mapping
// ============================================================================

fn corpus_item_spec(item_id: u64) -> &'static EmbeddingSpec {
    match item_id {
        // DirectMatch: Systems/Rust
        1..=10 => &SPEC_SYSTEMS,
        // DirectMatch: ML/Python
        11..=13 => &SPEC_ML,
        // DirectMatch: Web/TypeScript
        14..=15 => &SPEC_WEB,
        // DirectMatch: DevOps
        16..=18 => &SPEC_DEVOPS,
        // DirectMatch: Mobile
        19..=20 => &SPEC_MOBILE,
        // DirectMatch: FP/Niche
        21..=23 => &SPEC_FP,
        // DirectMatch: Go/backend
        24 => &SPEC_GO_BACKEND,
        // DirectMatch: More Rust
        25..=26 => &SPEC_SYSTEMS_STRONG,
        // DirectMatch: More Python
        27 => &SPEC_ML_PYTHON,
        // DirectMatch: More Web
        28 => &SPEC_WEB_FULLSTACK,
        // DirectMatch: More DevOps
        29 => &SPEC_DEVOPS_OBS,
        // DirectMatch: More Mobile
        30 => &SPEC_MOBILE_CICD,
        // AdjacentMatch (31-50): cycle through adjacent specs
        31..=34 => &SPEC_ADJ_SYSTEMS_WEB,
        35..=38 => &SPEC_ADJ_ML_DB,
        39..=42 => &SPEC_ADJ_DEVOPS_SEC,
        43..=46 => &SPEC_ADJ_WEB_MOBILE,
        47..=50 => &SPEC_ADJ_DISTRIBUTED,
        // CrossDomainNoise (51-75): domains opposite to primary personas
        51..=55 => &SPEC_CDN_ML_ONLY,
        56..=60 => &SPEC_CDN_WEB_ONLY,
        61..=65 => &SPEC_CDN_DEVOPS_ONLY,
        66..=70 => &SPEC_CDN_MOBILE_ONLY,
        71..=75 => &SPEC_CDN_FP_ONLY,
        // Borderline (76-95): weak mixed signals
        76..=80 => &SPEC_BORDERLINE_A,
        81..=85 => &SPEC_BORDERLINE_B,
        86..=90 => &SPEC_BORDERLINE_C,
        91..=95 => &SPEC_BORDERLINE_D,
        // Career (96-110)
        96..=110 => &SPEC_CAREER,
        // Security (111-125)
        111..=125 => &SPEC_SECURITY,
        // IntroductoryNoise (126-140)
        126..=140 => &SPEC_INTRO,
        // ShowHNNoise (141-155)
        141..=155 => &SPEC_SHOW_HN,
        // MetaNoise (156-170)
        156..=170 => &SPEC_META,
        // BusinessNoise (171-180)
        171..=180 => &SPEC_BUSINESS,
        // ReleaseNotes (181-185)
        181..=185 => &SPEC_RELEASE_SYS,
        // HNDiscussion (186-195)
        186..=195 => &SPEC_HN_DISC,
        // DistantlyRelevant (196-205)
        196..=205 => &SPEC_DISTANT,
        // ReverseEngineering (206-215)
        206..=215 => &SPEC_REVERSE,
        // ReleaseNotes expansion (216-220)
        216 => &SPEC_RELEASE_GO,
        217 => &SPEC_RELEASE_HASKELL,
        218 => &SPEC_RELEASE_GRAFANA,
        219 => &SPEC_RELEASE_DOCKER,
        220 => &SPEC_RELEASE_HF,
        // Fallback: weak noise
        _ => &SPEC_INTRO,
    }
}

// ============================================================================
// Backward-compatible API (used by existing callers)
// ============================================================================

/// Generate a domain-specific embedding for a persona domain index (0-8).
/// Backward compatible with the band-based approach.
pub(super) fn domain_embedding(domain_idx: usize) -> Vec<f32> {
    interest_embedding(domain_idx)
}

/// Generate a content embedding similar to a given domain.
/// Adds controlled noise via item_seed so it is close but not identical.
pub(super) fn content_embedding_for_domain(domain_idx: usize, item_seed: u64) -> Vec<f32> {
    // Map persona domain index to the primary domain block
    let spec = persona_domain_spec(domain_idx);
    generate_embedding(spec, 20000 + item_seed)
}

/// Generate a blended embedding between two domain indices.
pub(super) fn adjacent_content_embedding(
    primary_domain: usize,
    adjacent_domain: usize,
    blend: f32,
) -> Vec<f32> {
    let a = interest_embedding(primary_domain);
    let b = interest_embedding(adjacent_domain);
    let mut emb = vec![0.0_f32; EMBEDDING_DIM];
    for i in 0..EMBEDDING_DIM {
        emb[i] = a[i] * (1.0 - blend) + b[i] * blend;
    }
    l2_normalize(&mut emb);
    emb
}

/// Generate a noise embedding unrelated to any domain.
pub(super) fn noise_embedding(seed: u64) -> Vec<f32> {
    static NOISE_SPEC: EmbeddingSpec = EmbeddingSpec {
        blocks: &[(Meta, 0.1)],
    };
    generate_embedding(&NOISE_SPEC, 30000 + seed)
}

/// Zero embedding for bootstrap / no-context scenarios.
pub(super) fn zero_embedding() -> Vec<f32> {
    vec![0.0_f32; EMBEDDING_DIM]
}

/// Wrong-dimension embedding for error handling tests.
pub(super) fn wrong_dimension_embedding() -> Vec<f32> {
    vec![0.5_f32; 128]
}

/// Helper: map persona index to a static EmbeddingSpec
fn persona_domain_spec(domain_idx: usize) -> &'static EmbeddingSpec {
    static PERSONA_SPECS: [EmbeddingSpec; 9] = [
        // 0: rust_systems -> Systems + Database
        EmbeddingSpec {
            blocks: &[(Systems, 1.0), (Database, 0.4)],
        },
        // 1: python_ml -> ML
        EmbeddingSpec {
            blocks: &[(ML, 1.0)],
        },
        // 2: fullstack_ts -> Web + Database
        EmbeddingSpec {
            blocks: &[(Web, 1.0), (Database, 0.3)],
        },
        // 3: devops_sre -> DevOps + Distributed
        EmbeddingSpec {
            blocks: &[(DevOps, 1.0), (Distributed, 0.3)],
        },
        // 4: mobile_dev -> Mobile + Web
        EmbeddingSpec {
            blocks: &[(Mobile, 1.0), (Web, 0.4)],
        },
        // 5: bootstrap -> Web (light)
        EmbeddingSpec {
            blocks: &[(Web, 0.5)],
        },
        // 6: power_user -> broad multi-domain
        EmbeddingSpec {
            blocks: &[(Systems, 0.6), (ML, 0.5), (Web, 0.4), (Distributed, 0.4)],
        },
        // 7: context_switcher -> Systems + Distributed
        EmbeddingSpec {
            blocks: &[(Systems, 0.7), (Distributed, 0.7)],
        },
        // 8: niche_specialist -> FP
        EmbeddingSpec {
            blocks: &[(FP, 1.0)],
        },
    ];
    PERSONA_SPECS.get(domain_idx).unwrap_or(&PERSONA_SPECS[0])
}

// ============================================================================
// Public API
// ============================================================================

/// Generate embeddings for all 215 corpus items.
/// Each embedding is a 384-dim unit vector with domain-specific signal.
pub(super) fn corpus_embeddings() -> Vec<Vec<f32>> {
    (1..=220)
        .map(|id| {
            let spec = corpus_item_spec(id);
            generate_embedding(spec, id)
        })
        .collect()
}

/// Generate an interest embedding for a persona by index.
///
/// Persona mapping (canonical order from mod.rs):
///   0=rust_systems, 1=python_ml, 2=fullstack_ts, 3=devops_sre,
///   4=mobile_dev, 5=bootstrap, 6=power_user, 7=context_switcher,
///   8=niche_specialist
pub(super) fn interest_embedding(persona_idx: usize) -> Vec<f32> {
    let spec = persona_domain_spec(persona_idx);
    // Use a distinct seed space (offset by 10000) to avoid correlation with corpus
    generate_embedding(spec, 10000 + persona_idx as u64)
}

/// Generate a topic embedding from a known topic string.
/// Falls back to a weak Meta embedding for unrecognized topics.
pub(super) fn topic_embedding(topic: &str) -> Vec<f32> {
    let lower = topic.to_lowercase();
    let spec: &EmbeddingSpec = match lower.as_str() {
        "rust" | "systems programming" | "tauri" | "memory management" | "wasm" | "webassembly" => {
            &SPEC_SYSTEMS
        }
        "react" | "typescript" | "node.js" | "nodejs" | "next.js" | "nextjs" | "css" | "html"
        | "frontend" | "graphql" => &SPEC_WEB,
        "python" | "pytorch" | "tensorflow" | "machine learning" | "ml" | "llm"
        | "data science" | "ai" => &SPEC_ML,
        "kubernetes" | "docker" | "terraform" | "ci/cd" | "devops" | "observability"
        | "monitoring" | "prometheus" | "grafana" => &SPEC_DEVOPS,
        "react native" | "ios" | "android" | "expo" | "flutter" | "mobile"
        | "mobile development" => &SPEC_MOBILE,
        "sql" | "postgresql" | "sqlite" | "redis" | "mongodb" | "databases" | "database" => {
            static DB_SPEC: EmbeddingSpec = EmbeddingSpec {
                blocks: &[(Database, 1.0)],
            };
            &DB_SPEC
        }
        "security" | "encryption" | "auth" | "cve" | "vulnerability" => &SPEC_SECURITY,
        "microservices" | "grpc" | "distributed systems" | "kafka" | "consensus" => {
            static DIST_SPEC: EmbeddingSpec = EmbeddingSpec {
                blocks: &[(Distributed, 1.0)],
            };
            &DIST_SPEC
        }
        "haskell" | "functional programming" | "type theory" | "category theory" | "nix" | "fp" => {
            &SPEC_FP
        }
        "hiring" | "compensation" | "interview" | "remote work" | "career" => &SPEC_CAREER,
        "funding" | "startup" | "pricing" | "acquisition" | "business" => &SPEC_BUSINESS,
        "go" | "golang" | "backend" => &SPEC_GO_BACKEND,
        _ => {
            static FALLBACK_SPEC: EmbeddingSpec = EmbeddingSpec {
                blocks: &[(Meta, 0.3)],
            };
            &FALLBACK_SPEC
        }
    };

    let seed = topic_hash(topic);
    generate_embedding(spec, seed)
}

/// Simple deterministic hash for topic strings.
fn topic_hash(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
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
}
