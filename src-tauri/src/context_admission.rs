// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Canonical context-admission policy — the single answer to "may this text
//! become part of the developer's technical-identity grounding corpus, and how
//! much should it count?".
//!
//! This is the chunk-level twin of [`crate::project_inclusion`] (which governs
//! which PROJECTS feed intelligence). Every writer of `context_chunks` funnels
//! through [`crate::db::Database::upsert_context_weighted`], which consults this
//! module; every embedding grounding read funnels through
//! [`crate::db::Database::find_similar_contexts`], which filters on the
//! provenance this module assigns. No indexer — present or future — can bypass
//! it. That is the structural guarantee: the pollution class is closed at the
//! narrowest point every path must cross, not patched call-site by call-site.
//!
//! # Why this exists (2026-07-14)
//!
//! 46% of a live user's 44,695-chunk context index was a Spanish/Portuguese
//! business course (`module-e1-execution-playbook.md` alone = 3,455 chunks),
//! surfacing `Similar to your code: "Nunca perca entregas de clientes"` on a
//! Docker tool. The prior defenses were all **path/name blocklists**
//! (`collect_context_files` SKIP_DIRS/SKIP_FILES/`is_meta_doc`,
//! `project_inclusion` path tiers). A lowercase-kebab `.md` essay in a legit
//! repo slips through every one of them — each new content type finds a new gap.
//!
//! # Why provenance is decided by EXTENSION, not content
//!
//! Content-density classification was measured against real files and does NOT
//! separate course-prose from technical docs: `R-revenue-engines.md` (a course
//! on making money) scored a HIGHER software-term density (0.052) than the
//! genuine `PASIFA-WHITEPAPER.md` (0.040). The vocabulary overlaps too much.
//! What IS reliable is (a) file extension (code vs not-code) and (b) volume
//! (one file contributing thousands of chunks is never an identity). The policy
//! rests on those two reliable signals plus an immune-system health check.
//!
//! # The three invariants
//!
//! 1. **Grounding is code-only.** Only [`ContextClass::Code`]/[`ContextClass::Config`]
//!    feed embedding relevance. Docs/prose can never surface as "your code" nor
//!    move the context score — their embeddings are semantic wildcards.
//! 2. **No single doc source dominates.** A doc file contributes at most
//!    [`MAX_DOC_CHUNKS_PER_SOURCE`] chunks. Content-agnostic: catches any giant
//!    essay/course/dataset, whatever a future user drops in their tree.
//! 3. **Composition is watched.** [`assess_corpus`] trips when docs or any single
//!    source dominate — logged, asserted in debug, auto-quarantined in release —
//!    so a recurrence self-announces instead of silently poisoning for weeks.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Provenance class of a context chunk — what the text IS, decided from its
/// source extension (reliable) rather than its content (measured unreliable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextClass {
    /// Source code in a recognized language. Full grounding weight; the ONLY
    /// class (with [`Config`](ContextClass::Config)) eligible as "Similar to
    /// your code" and to move the embedding context score.
    Code,
    /// Manifests / configuration (Cargo.toml, package.json, *.yaml). Identity-
    /// defining; grounding-eligible at near-full weight.
    Config,
    /// Natural-language documentation or prose (`.md`, `.txt`, README, courses,
    /// marketing, essays). Admitted at reduced weight and a strict per-source
    /// cap, but NEVER feeds embedding grounding — prose embeddings match
    /// arbitrary news ("Nunca perca entregas de clientes" ↔ a Docker tool).
    Doc,
    /// Not context at all — binaries, data blobs, unusable content. Never stored.
    Reject,
}

impl ContextClass {
    /// Stable string persisted in `context_chunks.source_type` and matched by
    /// the grounding read filter. Changing these strings is a migration.
    pub fn source_type(self) -> &'static str {
        match self {
            ContextClass::Code => "code",
            ContextClass::Config => "config",
            ContextClass::Doc => "doc",
            ContextClass::Reject => "reject",
        }
    }

    /// May this chunk be stored in `context_chunks` at all?
    pub fn is_admitted(self) -> bool {
        !matches!(self, ContextClass::Reject)
    }

    /// May this chunk feed embedding relevance grounding — the "Similar to your
    /// code" evidence AND the context score that both pipelines read from
    /// [`crate::db::Database::find_similar_contexts`]? Code and config only.
    pub fn grounding_eligible(self) -> bool {
        matches!(self, ContextClass::Code | ContextClass::Config)
    }

    /// Recover the class from a persisted `source_type` string. `None` for an
    /// unrecognized value (e.g. the legacy `'text'` default before the reconcile
    /// runs), which is treated as non-grounding by callers. Keeps the enum the
    /// single source of truth for the grounding read filter.
    pub fn from_source_type(source_type: &str) -> Option<Self> {
        match source_type {
            "code" => Some(ContextClass::Code),
            "config" => Some(ContextClass::Config),
            "doc" => Some(ContextClass::Doc),
            "reject" => Some(ContextClass::Reject),
            _ => None,
        }
    }

    /// Weight multiplier applied on top of any caller weight at admission.
    pub fn weight_multiplier(self) -> f32 {
        match self {
            ContextClass::Code => 1.0,
            ContextClass::Config => 0.9,
            ContextClass::Doc => 0.5,
            ContextClass::Reject => 0.0,
        }
    }
}

/// Max chunks any single doc source_file may contribute to the corpus. A real
/// README has a handful of sections; a 3,455-chunk course module is not an
/// identity. Content-agnostic proportionality guard — the reliable half of the
/// structural fix. Code/config are NOT capped here (many files legitimately
/// share a basename like `mod.rs`; aggregate dominance is caught by
/// [`assess_corpus`] instead).
pub const MAX_DOC_CHUNKS_PER_SOURCE: usize = 40;

/// Corpus is unhealthy if docs exceed this fraction of the whole grounding
/// corpus (docs should be a garnish on code, never the main dish).
pub const MAX_DOC_FRACTION: f64 = 0.35;

/// Corpus is unhealthy if any single source_file exceeds this fraction — a
/// single file defining a plurality of "your context" is the pollution shape.
pub const MAX_SINGLE_SOURCE_FRACTION: f64 = 0.15;

/// Fraction/dominance checks only apply once the corpus is large enough for a
/// share to be meaningful. Below this a single file legitimately dominates
/// (cold start: a new user with one README and two source files) — flagging it
/// would be a false alarm the immune system could not act on. The absolute
/// per-file cap ([`MAX_DOC_CHUNKS_PER_SOURCE`]) still applies at any size.
pub const MIN_CORPUS_FOR_DOMINANCE: usize = 500;

// ── Extension tables ────────────────────────────────────────────────────────

/// Recognized source-code extensions (grounding-eligible).
const CODE_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "pyi", "go", "java", "kt", "kts", "scala",
    "c", "h", "cpp", "cxx", "cc", "hpp", "hxx", "cs", "rb", "php", "swift", "m", "mm", "lua", "r",
    "dart", "ex", "exs", "erl", "clj", "cljs", "hs", "ml", "mli", "fs", "vue", "svelte", "sql",
    "proto", "sh", "bash", "zsh", "ps1", "pl", "pm", "groovy", "zig", "nim", "jl", "v", "sol",
    "elm", "gleam", "rkt", "scm", "asm",
];

/// Recognized config / manifest extensions (grounding-eligible; identity-
/// defining because they name the stack).
const CONFIG_EXTS: &[&str] = &[
    "toml",
    "yaml",
    "yml",
    "json",
    "jsonc",
    "ini",
    "cfg",
    "conf",
    "properties",
    "env",
    "lock",
    "xml",
    "csproj",
    "gradle",
    "tf",
    "dockerfile",
];

/// Recognized documentation / prose extensions (admitted, capped, NOT grounding).
const DOC_EXTS: &[&str] = &[
    "md", "markdown", "mdx", "txt", "text", "rst", "adoc", "asciidoc", "org", "tex", "rtf",
    "textile", "wiki",
];

/// Extensionless filenames that are really code or config.
fn classify_by_bare_name(name: &str) -> Option<ContextClass> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "dockerfile" | "makefile" | "cmakelists.txt" | "gemfile" | "rakefile" | "procfile"
        | "vagrantfile" | "brewfile" | "justfile" => Some(ContextClass::Config),
        _ => None,
    }
}

/// Extract `(bare_name, extension)` from a stored `source_file`, which may be a
/// bare filename (`module-e1.md`), a full path (`D:\proj\src\mod.rs`), or a
/// readme-indexer key (`/proj/README.md#Features`). Everything after the first
/// `#` is a section anchor, not part of the filename.
fn split_name_ext(source_file: &str) -> (String, Option<String>) {
    let no_anchor = source_file.split('#').next().unwrap_or(source_file);
    let base = no_anchor
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(no_anchor)
        .trim();
    let ext = base
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        // A leading-dot name (`.gitignore`) has no real extension.
        .filter(|_| !base.starts_with('.') || base.matches('.').count() > 1);
    (base.to_string(), ext)
}

/// The policy: classify a chunk's provenance from its source path. Pure and
/// deterministic — the reliable signal, unlike content density.
pub fn classify_source(source_file: &str) -> ContextClass {
    let (name, ext) = split_name_ext(source_file);
    if name.is_empty() {
        return ContextClass::Reject;
    }
    if let Some(c) = classify_by_bare_name(&name) {
        return c;
    }
    match ext.as_deref() {
        Some(e) if CODE_EXTS.contains(&e) => ContextClass::Code,
        Some(e) if CONFIG_EXTS.contains(&e) => ContextClass::Config,
        Some(e) if DOC_EXTS.contains(&e) => ContextClass::Doc,
        // Unknown extension or none: treat as a doc (bounded, non-grounding).
        // Never promoted to Code — an unknown blob must not ground the feed.
        _ => ContextClass::Doc,
    }
}

/// Log a rejected / capped source once per process, at admission time. Silent
/// permanent exclusion is not acceptable (accuracy-first); this is the audit
/// trail. Mirrors `project_inclusion::log_tier2_exclusion`.
pub fn log_admission_skip(source_file: &str, reason: &str) {
    static LOGGED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let key = format!("{source_file}|{reason}");
    let mutex = LOGGED.get_or_init(|| Mutex::new(HashSet::new()));
    let newly = match mutex.lock() {
        Ok(mut set) => set.insert(key),
        Err(_) => false,
    };
    if newly {
        tracing::info!(
            target: "4da::context_admission",
            source_file = %source_file,
            reason = %reason,
            "context chunk not admitted to grounding corpus"
        );
    }
}

// ── Corpus health (the immune system) ───────────────────────────────────────

/// Per-source tally used to assess corpus composition.
#[derive(Debug, Clone)]
pub struct SourceTally {
    pub source_file: String,
    pub source_type: String,
    pub count: usize,
}

/// A snapshot verdict on the grounding corpus. `healthy == false` means the
/// composition has drifted into the pollution shape and remediation should fire.
#[derive(Debug, Clone)]
pub struct CorpusHealth {
    pub total: usize,
    pub doc_fraction: f64,
    pub top_source: Option<(String, usize)>,
    pub top_source_fraction: f64,
    /// Doc sources exceeding [`MAX_DOC_CHUNKS_PER_SOURCE`] — the quarantine list.
    pub over_cap_doc_sources: Vec<SourceTally>,
    pub healthy: bool,
    pub issues: Vec<String>,
}

/// Assess corpus composition from per-source tallies. Pure — the caller supplies
/// the tallies (from a `GROUP BY source_file, source_type` query) so this is
/// unit-testable without a database.
pub fn assess_corpus(tallies: &[SourceTally]) -> CorpusHealth {
    let total: usize = tallies.iter().map(|t| t.count).sum();
    let doc_count: usize = tallies
        .iter()
        .filter(|t| t.source_type == "doc" || t.source_type == "reject")
        .map(|t| t.count)
        .sum();
    let doc_fraction = if total > 0 {
        doc_count as f64 / total as f64
    } else {
        0.0
    };
    let top = tallies.iter().max_by_key(|t| t.count);
    let top_source = top.map(|t| (t.source_file.clone(), t.count));
    let top_source_fraction = match (top, total) {
        (Some(t), n) if n > 0 => t.count as f64 / n as f64,
        _ => 0.0,
    };
    let over_cap_doc_sources: Vec<SourceTally> = tallies
        .iter()
        .filter(|t| t.source_type == "doc" && t.count > MAX_DOC_CHUNKS_PER_SOURCE)
        .cloned()
        .collect();

    let mut issues = Vec::new();
    // Dominance (fraction) checks are only meaningful on a substantial corpus —
    // below the floor a single file legitimately dominates (cold start).
    let dominance_meaningful = total >= MIN_CORPUS_FOR_DOMINANCE;
    if dominance_meaningful && doc_fraction > MAX_DOC_FRACTION {
        issues.push(format!(
            "docs are {:.1}% of the grounding corpus (max {:.0}%)",
            doc_fraction * 100.0,
            MAX_DOC_FRACTION * 100.0
        ));
    }
    if dominance_meaningful && top_source_fraction > MAX_SINGLE_SOURCE_FRACTION {
        if let Some((name, count)) = &top_source {
            issues.push(format!(
                "single source '{}' is {:.1}% of the corpus ({} chunks, max {:.0}%)",
                name,
                top_source_fraction * 100.0,
                count,
                MAX_SINGLE_SOURCE_FRACTION * 100.0
            ));
        }
    }
    if !over_cap_doc_sources.is_empty() {
        issues.push(format!(
            "{} doc source(s) exceed the {}-chunk per-file cap",
            over_cap_doc_sources.len(),
            MAX_DOC_CHUNKS_PER_SOURCE
        ));
    }

    CorpusHealth {
        total,
        doc_fraction,
        top_source,
        top_source_fraction,
        over_cap_doc_sources,
        healthy: issues.is_empty(),
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_extensions_are_code_and_grounding_eligible() {
        for f in [
            "mod.rs",
            r"D:\4DA\src-tauri\src\scoring\pipeline_v2.rs",
            "/proj/src/store.ts",
            "app.py",
            "server.go",
            "Component.tsx",
            "query.sql",
        ] {
            let c = classify_source(f);
            assert_eq!(c, ContextClass::Code, "{f} should be Code");
            assert!(c.grounding_eligible(), "{f} must ground");
            assert_eq!(c.source_type(), "code");
        }
    }

    #[test]
    fn config_extensions_are_config_and_ground() {
        for f in ["Cargo.toml", "package.json", "config.yaml", "Dockerfile"] {
            let c = classify_source(f);
            assert_eq!(c, ContextClass::Config, "{f} should be Config");
            assert!(c.grounding_eligible());
        }
    }

    #[test]
    fn the_streets_course_files_are_docs_and_never_ground() {
        // The exact files that were 46% of a live corpus and produced the
        // "Nunca perca entregas de clientes" grounding garbage. Whatever their
        // content, by extension they are Doc and can NEVER surface as "your code".
        for f in [
            "module-e1-execution-playbook.md",
            "T-technical-moats.md",
            "R-revenue-engines.md",
            r"D:\4DA\docs\streets\module-t-technical-moats.md",
            "streets-course/modules/E1-execution-playbook.md",
        ] {
            let c = classify_source(f);
            assert_eq!(c, ContextClass::Doc, "{f} should be Doc");
            assert!(!c.grounding_eligible(), "{f} must NOT ground");
            assert!(
                c.is_admitted(),
                "docs are still admitted (capped), not dropped"
            );
            assert_eq!(c.source_type(), "doc");
        }
    }

    #[test]
    fn readmes_are_docs_including_the_section_anchor_key_form() {
        // readme_indexing stores `path#Heading` keys.
        for f in [
            "README.md",
            r"D:\4DA\README.md#Features",
            "/proj/README.md#Getting Started",
            "docs/architecture.md",
        ] {
            assert_eq!(classify_source(f), ContextClass::Doc, "{f} should be Doc");
        }
    }

    #[test]
    fn split_name_ext_handles_paths_anchors_and_dotfiles() {
        assert_eq!(
            split_name_ext(r"D:\proj\src\mod.rs"),
            ("mod.rs".to_string(), Some("rs".to_string()))
        );
        assert_eq!(
            split_name_ext("/proj/README.md#Features"),
            ("README.md".to_string(), Some("md".to_string()))
        );
        assert_eq!(
            split_name_ext(".gitignore"),
            (".gitignore".to_string(), None)
        );
        assert_eq!(
            split_name_ext("archive.tar.gz"),
            ("archive.tar.gz".to_string(), Some("gz".to_string()))
        );
    }

    #[test]
    fn unknown_and_empty_sources_never_ground() {
        assert_eq!(classify_source("data.bin"), ContextClass::Doc); // unknown → doc, non-grounding
        assert!(!classify_source("data.bin").grounding_eligible());
        assert_eq!(classify_source("noextfile"), ContextClass::Doc);
        assert_eq!(classify_source(""), ContextClass::Reject);
        assert!(!classify_source("").is_admitted());
    }

    #[test]
    fn extensionless_code_and_config_names() {
        assert_eq!(classify_source("Makefile"), ContextClass::Config);
        assert_eq!(classify_source("Dockerfile"), ContextClass::Config);
        assert_eq!(classify_source(r"D:\proj\Gemfile"), ContextClass::Config);
    }

    #[test]
    fn weights_and_grounding_types_are_consistent() {
        assert!(ContextClass::Code.weight_multiplier() > ContextClass::Doc.weight_multiplier());
        assert_eq!(ContextClass::Reject.weight_multiplier(), 0.0);
        // source_type() and from_source_type() must round-trip, and the read
        // filter (from_source_type + grounding_eligible) must agree with the
        // enum's own grounding_eligible() — no drift between write and read.
        for c in [
            ContextClass::Code,
            ContextClass::Config,
            ContextClass::Doc,
            ContextClass::Reject,
        ] {
            assert_eq!(ContextClass::from_source_type(c.source_type()), Some(c));
            let via_read = ContextClass::from_source_type(c.source_type())
                .is_some_and(ContextClass::grounding_eligible);
            assert_eq!(via_read, c.grounding_eligible(), "{c:?} read/flag mismatch");
        }
        // The legacy default is not grounding-eligible until reclassified.
        assert_eq!(ContextClass::from_source_type("text"), None);
    }

    #[test]
    fn healthy_corpus_passes() {
        // A realistic corpus: many code files none dominating, a few small docs.
        let mut tallies: Vec<SourceTally> = (0..12)
            .map(|i| SourceTally {
                source_file: format!("src/file_{i}.rs"),
                source_type: "code".into(),
                count: 50,
            })
            .collect();
        tallies.push(SourceTally {
            source_file: "README.md".into(),
            source_type: "doc".into(),
            count: 20,
        });
        tallies.push(SourceTally {
            source_file: "docs/arch.md".into(),
            source_type: "doc".into(),
            count: 15,
        });
        let h = assess_corpus(&tallies);
        assert!(
            h.total >= MIN_CORPUS_FOR_DOMINANCE,
            "corpus must clear the floor"
        );
        assert!(h.healthy, "issues: {:?}", h.issues);
        assert!(h.doc_fraction < MAX_DOC_FRACTION);
        assert!(h.top_source_fraction < MAX_SINGLE_SOURCE_FRACTION);
    }

    #[test]
    fn small_corpus_never_false_alarms_on_dominance() {
        // Cold start: one README dominates a tiny corpus — must NOT be flagged
        // (the immune system could not act on it, and it is legitimate).
        let tallies = vec![
            SourceTally {
                source_file: "README.md".into(),
                source_type: "doc".into(),
                count: 30,
            },
            SourceTally {
                source_file: "main.rs".into(),
                source_type: "code".into(),
                count: 10,
            },
        ];
        let h = assess_corpus(&tallies);
        assert!(h.total < MIN_CORPUS_FOR_DOMINANCE);
        assert!(h.healthy, "small corpus must not alarm: {:?}", h.issues);
    }

    #[test]
    fn the_pollution_shape_is_caught() {
        // The live 2026-07-14 shape: one doc source dominating the corpus.
        let tallies = vec![
            SourceTally {
                source_file: "module-e1-execution-playbook.md".into(),
                source_type: "doc".into(),
                count: 3455,
            },
            SourceTally {
                source_file: "mod.rs".into(),
                source_type: "code".into(),
                count: 578,
            },
        ];
        let h = assess_corpus(&tallies);
        assert!(!h.healthy);
        assert!(h.doc_fraction > MAX_DOC_FRACTION);
        assert_eq!(h.over_cap_doc_sources.len(), 1);
        assert!(h.top_source_fraction > MAX_SINGLE_SOURCE_FRACTION);
        assert!(
            h.issues.len() >= 2,
            "expected multiple issues: {:?}",
            h.issues
        );
    }

    #[test]
    fn empty_corpus_is_vacuously_healthy() {
        let h = assess_corpus(&[]);
        assert!(h.healthy);
        assert_eq!(h.total, 0);
        assert_eq!(h.doc_fraction, 0.0);
    }
}
