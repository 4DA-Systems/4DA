// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Cluster labels: c-TF-IDF over member titles with honesty guards, and a
//! fallback chain that names a cluster by what its members demonstrably
//! share (registry, dependency, typical member) — never "assorted".
//! Split from clustering.rs (community detection) to keep both in the size
//! gate.

use std::collections::{HashMap, HashSet};

use super::types::{GraphCluster, RawItem};

/// A term must appear in at least this share of member titles for a c-TF-IDF
/// label to be honest; below it the cluster gets a source-digest label
/// instead of three words that describe almost none of its members.
const LABEL_COVERAGE_MIN: f32 = 0.30;

/// A token present in at least this share of ONE source's items is that
/// source's boilerplate ("crates" prefixes every crates.io title) — it names
/// the source, not a topic, so it never enters labels.
const BOILERPLATE_SHARE: f32 = 0.80;

/// Sources need at least this many items in the window for a stable
/// boilerplate estimate.
const BOILERPLATE_MIN_ITEMS: usize = 5;

/// Label clusters by c-TF-IDF: a term scores by how frequent it is INSIDE the
/// cluster, discounted by how many OTHER clusters also use it. Raw frequency
/// produced junk labels ("our · middleware · second", "axios · via ·
/// prototype") because advisory boilerplate and connective words dominate
/// counts; distinctiveness against the sibling clusters is what names a topic.
///
/// Honesty guards:
/// - Source boilerplate ("crates" leads every crates.io title) is excluded
///   per source before counting (2026-07-19).
/// - Every label term must appear in at least two member titles, and the
///   best such term must cover [`LABEL_COVERAGE_MIN`] of members. The 2-hit
///   filter runs BEFORE picking the top term (2026-10-02): checking coverage
///   on the raw c-TF-IDF winner — usually a one-title word, maximally
///   "distinctive" — sent clusters with a real shared term ("rust" across a
///   Compio / tokio / concurrency trio, "sql" across two SQL-injection
///   posts) to the fallback, which was the "related items · assorted" label
///   on 6 of 24 live clusters.
/// - When no shared term qualifies, [`fallback_label`] names the cluster by
///   what its members demonstrably share — never "assorted".
pub(super) fn assign_cluster_labels(items: &[RawItem], clusters: &mut [GraphCluster]) {
    let item_map: HashMap<i64, &RawItem> = items.iter().map(|i| (i.id, i)).collect();
    let boilerplate = source_boilerplate_terms(items);
    let empty: HashSet<String> = HashSet::new();

    let keywords_of = |id: i64| -> Vec<String> {
        item_map
            .get(&id)
            .map(|item| {
                let boiler = boilerplate.get(item.source_type.as_str()).unwrap_or(&empty);
                let own_name = source_name_token(&item.source_type);
                extract_title_keywords(&item.title)
                    .into_iter()
                    .filter(|w| !boiler.contains(w) && Some(w.as_str()) != own_name)
                    .collect()
            })
            .unwrap_or_default()
    };

    // Per-cluster term frequencies.
    let tfs: Vec<HashMap<String, usize>> = clusters
        .iter()
        .map(|cluster| {
            let mut tf: HashMap<String, usize> = HashMap::new();
            for &id in &cluster.node_ids {
                for word in keywords_of(id) {
                    *tf.entry(word).or_insert(0) += 1;
                }
            }
            tf
        })
        .collect();

    // Document frequency across clusters (a "document" = one cluster).
    let mut df: HashMap<&str, usize> = HashMap::new();
    for tf in &tfs {
        for term in tf.keys() {
            *df.entry(term.as_str()).or_insert(0) += 1;
        }
    }

    let n_clusters = clusters.len().max(1) as f32;
    // Every cluster's terms in c-TF-IDF order (no hit floor): the reserve
    // `disambiguate_labels` draws from when two clusters share a label.
    let ranked: Vec<Vec<String>> = tfs
        .iter()
        .map(|tf| {
            let mut all: Vec<(&str, f32)> = tf
                .iter()
                .map(|(term, &count)| {
                    let d = df.get(term.as_str()).copied().unwrap_or(1) as f32;
                    (term.as_str(), count as f32 * (1.0 + n_clusters / d).ln())
                })
                .collect();
            // Equal scores (typically one-title terms) break toward the
            // LONGER word: "misalignment" distinguishes, "amid" does not
            // (alphabetical order crowned "openai · amid", live 2026-10-04).
            all.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.0.len().cmp(&a.0.len()))
                    .then_with(|| a.0.cmp(b.0))
            });
            all.into_iter().map(|(t, _)| t.to_string()).collect()
        })
        .collect();
    for ((cluster, tf), cluster_ranked) in clusters.iter_mut().zip(&tfs).zip(&ranked) {
        // Titles (not occurrences) carrying each term.
        let hits_of = |term: &str| {
            cluster
                .node_ids
                .iter()
                .filter(|id| keywords_of(**id).iter().any(|w| w == term))
                .count()
        };
        let mut scored: Vec<(&str, f32)> = tf
            .iter()
            .filter(|(term, _)| hits_of(term) >= 2)
            .map(|(term, &count)| {
                let d = df.get(term.as_str()).copied().unwrap_or(1) as f32;
                let idf = (1.0 + n_clusters / d).ln();
                (term.as_str(), count as f32 * idf)
            })
            .collect();
        // Deterministic: score desc, then alphabetical.
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });

        // The LEAD term is the best-scored term that covers the coverage
        // floor — not merely the top score: on an 11-member mixed cluster
        // the most distinctive 3-title term ("tauri-apps", 27%) outranked the
        // 5-title "api" and sent a real theme to the fallback (live
        // 2026-10-02). Riders follow in score order under the 2-hit floor.
        let n_members = cluster.node_ids.len().max(1) as f32;
        let lead = scored
            .iter()
            .position(|(w, _)| hits_of(w) as f32 / n_members >= LABEL_COVERAGE_MIN);

        cluster.label = if let Some(lead) = lead {
            let ordered = std::iter::once(scored[lead]).chain(
                scored
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != lead)
                    .map(|(_, s)| *s),
            );
            // A compound and its own sub-word ("tauri-apps" + "tauri"), or a
            // word and its plural ("agent" + "agents"), name the same thing
            // twice; keep the first-ranked of the pair.
            let mut chosen: Vec<&str> = Vec::new();
            for (w, _) in ordered {
                if !chosen.iter().any(|c| same_name(c, w)) {
                    chosen.push(w);
                }
                if chosen.len() == 3 {
                    break;
                }
            }
            chosen
                .iter()
                .map(|w| display_term(w))
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            fallback_label(cluster, &item_map, cluster_ranked, &keywords_of)
        };
    }
    disambiguate_labels(clusters, &ranked);
}

/// A compound and its own sub-word ("tauri-apps" + "tauri"), or a word and
/// its plural ("agent" + "agents"), name the same thing.
fn same_name(a: &str, b: &str) -> bool {
    a.split(['-', '_']).any(|part| part == b)
        || b.split(['-', '_']).any(|part| part == a)
        || a.strip_suffix('s') == Some(b)
        || b.strip_suffix('s') == Some(a)
}

/// Rounds of term-appending before the ordinal last resort.
const DISAMBIGUATE_ROUNDS: usize = 3;

/// Two clusters must never share a label: three discs all named "RUST" (live
/// 2026-10-04) tell the user nothing about how they differ. Every cluster in
/// a duplicate group gains its next distinctive term — the best-ranked
/// c-TF-IDF term not already in the label — so each reads as what sets it
/// apart ("rust · serialization", "rust · injection"). A group that still
/// collides after [`DISAMBIGUATE_ROUNDS`] (members out of words) is numbered.
fn disambiguate_labels(clusters: &mut [GraphCluster], ranked: &[Vec<String>]) {
    for round in 0..=DISAMBIGUATE_ROUNDS {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, c) in clusters.iter().enumerate() {
            groups.entry(c.label.to_lowercase()).or_default().push(i);
        }
        let mut dups: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
        if dups.is_empty() {
            return;
        }
        dups.sort();
        for group in dups {
            for (n, &i) in group.iter().enumerate() {
                if round == DISAMBIGUATE_ROUNDS {
                    // Out of words: an ordinal is still a true distinction.
                    if n > 0 {
                        clusters[i].label = format!("{} #{}", clusters[i].label, n + 1);
                    }
                    continue;
                }
                let label_lower = clusters[i].label.to_lowercase();
                let present: Vec<&str> = label_lower
                    .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
                    .filter(|w| !w.is_empty())
                    .collect();
                let next = ranked.get(i).and_then(|terms| {
                    terms.iter().find(|t| {
                        let shown = display_term(t);
                        !present.iter().any(|p| same_name(p, t) || *p == shown)
                    })
                });
                if let Some(term) = next {
                    clusters[i].label = format!("{} · {}", clusters[i].label, display_term(term));
                }
            }
        }
    }
}

/// Registry sources whose items are package releases — a cluster made only of
/// one registry's items IS that registry's releases (a group-by, stated).
const REGISTRY_SOURCES: &[&str] = &["crates_io", "npm_registry", "pypi", "go_modules"];

/// Longest representative-title label, in characters (word-boundary cut) —
/// the last-resort label for a member with no usable keyword.
const TITLE_LABEL_MAX: usize = 28;
/// A title's head before ": " replaces the whole title when at least this
/// long — "Philbin: The safest (and fastest) AEGIS library" → "Philbin".
const TITLE_HEAD_MIN: usize = 6;

/// Label for a cluster no shared title term describes. Never "assorted" —
/// a label that admits it names nothing tells the user nothing. In order:
/// 1. one registry source → "<registry> releases" (true by construction);
/// 2. a dependency linked to at least two members (and the coverage floor)
///    → that package — the members demonstrably share it;
/// 3. the two most distinctive words of the member nearest the cluster's
///    embedding centroid (its most typical member), in the cluster's own
///    c-TF-IDF order — "jwt · decoding", never a title cut mid-sentence. The
///    old cut title ("JWT Decoding vs…", "Breaking the…", live 2026-10-05)
///    read as one article's headline, not as the name of a theme.
fn fallback_label(
    cluster: &GraphCluster,
    item_map: &HashMap<i64, &RawItem>,
    cluster_ranked: &[String],
    keywords_of: &dyn Fn(i64) -> Vec<String>,
) -> String {
    let members: Vec<&RawItem> = cluster
        .node_ids
        .iter()
        .filter_map(|id| item_map.get(id).copied())
        .collect();
    if members.is_empty() {
        return String::new();
    }

    let first_source = members[0].source_type.as_str();
    if REGISTRY_SOURCES.contains(&first_source)
        && members.iter().all(|m| m.source_type == first_source)
    {
        return format!("{} releases", source_display(first_source));
    }

    let mut packages: HashMap<String, usize> = HashMap::new();
    for m in &members {
        if let Some(p) = m.matched_package.as_deref() {
            let p = p.trim().to_lowercase();
            if !p.is_empty() {
                *packages.entry(p).or_insert(0) += 1;
            }
        }
    }
    let mut packages: Vec<(String, usize)> = packages.into_iter().collect();
    packages.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if let Some((pkg, n)) = packages.first() {
        if *n >= 2 && *n as f32 / members.len() as f32 >= LABEL_COVERAGE_MIN {
            return pkg.clone();
        }
    }

    // A registry release title is a package + version ("ai v7.0.126") —
    // it names one member, not the theme. Prefer prose members when the
    // cluster has any.
    let prose: Vec<&RawItem> = members
        .iter()
        .copied()
        .filter(|m| !REGISTRY_SOURCES.contains(&m.source_type.as_str()))
        .collect();
    let pool = if prose.is_empty() { &members } else { &prose };
    let rep = representative(pool);
    let own = keywords_of(rep.id);
    let mut chosen: Vec<&str> = Vec::new();
    for term in cluster_ranked.iter().filter(|t| own.contains(t)) {
        if !chosen.iter().any(|c| same_name(c, term)) {
            chosen.push(term);
        }
        if chosen.len() == 2 {
            break;
        }
    }
    if chosen.is_empty() {
        // A title with no usable word at all (every token boilerplate).
        return short_title(&rep.title);
    }
    chosen
        .iter()
        .map(|w| display_term(w))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The member whose embedding is nearest the members' mean — deterministic
/// (ties keep the earliest member; members arrive id-sorted).
fn representative<'a>(members: &[&'a RawItem]) -> &'a RawItem {
    let dim = members[0].embedding.len();
    let mut centroid = vec![0.0f32; dim];
    for m in members {
        for (c, v) in centroid.iter_mut().zip(&m.embedding) {
            *c += v;
        }
    }
    let mut best = members[0];
    let mut best_sim = f32::MIN;
    for m in members {
        let sim = crate::utils::cosine_similarity(&m.embedding, &centroid);
        if sim > best_sim + 1e-6 {
            best = m;
            best_sim = sim;
        }
    }
    best
}

/// A title shortened for a cluster header: source/advisory prefix removed,
/// cut on a word boundary at [`TITLE_LABEL_MAX`] with an ellipsis.
pub(super) fn short_title(title: &str) -> String {
    let mut t = title.trim();
    if let Some(rest) = t.strip_prefix('[') {
        if let Some((_, after)) = rest.split_once("] ") {
            t = after.trim();
        }
    }
    for prefix in ["crates.io:", "npm:", "pypi:", "go:"] {
        if t.len() > prefix.len()
            && t.is_char_boundary(prefix.len())
            && t[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            t = t[prefix.len()..].trim();
        }
    }
    // Cut at a subtitle separator when the head alone is a usable name.
    if let Some((head, _)) = t.split_once(": ") {
        if head.chars().count() >= TITLE_HEAD_MIN {
            t = head;
        }
    }
    if t.chars().count() <= TITLE_LABEL_MAX {
        return t.to_string();
    }
    let mut out = String::new();
    for word in t.split_whitespace() {
        let next_len = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        if next_len > TITLE_LABEL_MAX - 1 {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        out = t.chars().take(TITLE_LABEL_MAX - 1).collect();
    }
    let out = out
        .trim_end_matches([',', ';', ':', '-', '—', '–'])
        .to_string();
    format!("{out}…")
}

/// Tokens normalised for counting ("next.js" → "nextjs"), shown in their
/// usual spelling.
fn display_term(term: &str) -> String {
    match term {
        "nextjs" => "next.js".to_string(),
        "nodejs" => "node.js".to_string(),
        "vuejs" => "vue.js".to_string(),
        "threejs" => "three.js".to_string(),
        other => other.to_string(),
    }
}

/// The token a registry stamps on every title it emits ("npm: …",
/// "crates.io: …"). It names the source, not a topic, at ANY corpus size —
/// the statistical boilerplate estimate needs 5+ items and missed a 2-item
/// npm cluster labelled "npm" (2026-10-02).
fn source_name_token(source_type: &str) -> Option<&'static str> {
    match source_type {
        "crates_io" => Some("crates"),
        "npm_registry" => Some("npm"),
        "pypi" => Some("pypi"),
        _ => None,
    }
}

fn source_display(source_type: &str) -> &str {
    match source_type {
        "crates_io" => "crates.io",
        "npm_registry" => "npm",
        "go_modules" => "go modules",
        "hackernews" => "hacker news",
        "papers_with_code" => "papers with code",
        other => other,
    }
}

/// Tokens appearing in ≥[`BOILERPLATE_SHARE`] of one source's items are that
/// source's template vocabulary, not topics.
fn source_boilerplate_terms(items: &[RawItem]) -> HashMap<&str, HashSet<String>> {
    let mut by_source: HashMap<&str, Vec<&RawItem>> = HashMap::new();
    for item in items {
        by_source
            .entry(item.source_type.as_str())
            .or_default()
            .push(item);
    }

    let mut out: HashMap<&str, HashSet<String>> = HashMap::new();
    for (source, members) in by_source {
        if members.len() < BOILERPLATE_MIN_ITEMS {
            continue;
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        for item in &members {
            let uniq: HashSet<String> = extract_title_keywords(&item.title).into_iter().collect();
            for w in uniq {
                *counts.entry(w).or_insert(0) += 1;
            }
        }
        let floor = (members.len() as f32 * BOILERPLATE_SHARE).ceil() as usize;
        let terms: HashSet<String> = counts
            .into_iter()
            .filter(|(_, c)| *c >= floor)
            .map(|(w, _)| w)
            .collect();
        if !terms.is_empty() {
            out.insert(source, terms);
        }
    }
    out
}

pub(super) fn extract_title_keywords(title: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a",
        "an",
        "the",
        "in",
        "of",
        "for",
        "to",
        "and",
        "is",
        "new",
        "on",
        "at",
        "by",
        "with",
        "from",
        "this",
        "that",
        "it",
        "its",
        "has",
        "have",
        "are",
        "was",
        "were",
        "been",
        "be",
        "do",
        "does",
        "did",
        "will",
        "would",
        "could",
        "should",
        "may",
        "can",
        "not",
        "no",
        "but",
        "or",
        "if",
        "how",
        "what",
        "when",
        "where",
        "who",
        "why",
        "which",
        "all",
        "each",
        "every",
        "both",
        "more",
        "most",
        "other",
        "some",
        "such",
        "than",
        "too",
        "very",
        "just",
        "about",
        "up",
        "out",
        "so",
        "show",
        "hn",
        "ask",
        "via",
        "our",
        "you",
        "your",
        "using",
        "into",
        "http",
        "https",
        "www",
        "com",
        // Generic verbs/adverbs and announcement boilerplate: maximally
        // "distinctive" to c-TF-IDF on small clusters yet topically empty —
        // live label leaks 2026-07-19: "bun · claude · NOW", "accelerated ·
        // bytecode · COME", "rewrite · GOING · rust-to-zig", "RELEASED ·
        // AHEAD · ahead-of-time", "ANNOUNCING · llvm · rust". Tech names
        // ("next", "go", "rust") stay labelable.
        "now",
        "one",
        "two",
        "three",
        "come",
        "comes",
        "coming",
        "going",
        "goes",
        "gets",
        "get",
        "got",
        "make",
        "makes",
        "made",
        "take",
        "takes",
        "like",
        "want",
        "wants",
        "really",
        "also",
        "even",
        "well",
        "say",
        "says",
        "said",
        "still",
        "back",
        "ahead",
        "today",
        "yesterday",
        "here",
        "there",
        "thing",
        "things",
        "released",
        "releases",
        "release",
        "announcing",
        "announced",
        "introducing",
        "available",
        "update",
        "updates",
        "updated",
        // Generic connective / evaluative words that crowned live labels
        // (2026-10-02: "tests · NEVER · NEXT", "concurrent · BUILDING"). A
        // label term must be a content word. "next" is the English word;
        // Next.js reaches labels as "nextjs" (see normalize_js_names).
        "never",
        "ever",
        "next",
        // Idiom filler (2026-10-03: "A batch API is not a free PASS…" and
        // "An async job is not a free PASS…" labelled an auth cluster "pass").
        "pass",
        "free",
        "why",
        "building",
        "build",
        "builds",
        "built",
        "stop",
        "real",
        "need",
        "needs",
        "know",
        "nobody",
        "everyone",
        "everything",
        "actually",
        "best",
        "better",
        "first",
        "last",
        "lessons",
        "learned",
        "tips",
        "part",
        "inside",
        "without",
        "after",
        "before",
        "over",
        "under",
        "between",
        "across",
        "only",
        "own",
        "same",
        "much",
        "many",
        "few",
        "less",
        "don",
        "doesn",
        "isn",
        "won",
        "didn",
        "aren",
        "wasn",
        "let",
        "lets",
        "per",
        "year",
        "years",
        "day",
        "days",
        "week",
        "weeks",
        "time",
        "times",
        "way",
        "ways",
        "case",
        "cases",
        "work",
        "works",
        "working",
        "use",
        "uses",
        "used",
        "written",
        "free",
        "good",
        "bad",
        "big",
        "simple",
        "easy",
        "hard",
        "yet",
        "again",
        "finally",
        "practice",
        "nothing",
        "something",
        // "true · agent" (live 2026-10-02): a literal, not a topic.
        "true",
        "false",
        // "apps · tauri · plugin" (live 2026-10-02): the sub-word of
        // "@tauri-apps/…" crowned a label; "app(s)" names nothing.
        "app",
        "apps",
    ];

    let keep = |w: &str| w.len() >= 3 && !STOPWORDS.contains(&w) && !is_numeric_noise(w);

    let mut out: Vec<String> = Vec::new();
    for token in normalize_js_names(&title.to_lowercase())
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| keep(w))
    {
        out.push(token.to_string());
        // "react.js" counts as "reactjs" AND "react", so it shares a theme
        // with plain "React" titles. ("next" itself is a stopword — the
        // English word — so Next.js is only ever "nextjs".)
        if let Some(base) = token.strip_suffix("js").filter(|_| token.len() > 4) {
            if JS_NAMES.contains(&token) && keep(base) {
                out.push(base.to_string());
            }
        }
        // Compound package names hide their shared theme inside one token:
        // "tauri-plugin-syncular" and "tauri-browser" share no whole token,
        // so a cluster of tauri crates could not be labeled "tauri". Emit
        // the sub-words too so shared prefixes become labelable.
        if token.contains(['-', '_']) {
            for sub in token.split(['-', '_']).filter(|s| keep(s)) {
                out.push(sub.to_string());
            }
        }
    }
    out
}

/// `X.js` names that are real ecosystem identifiers; their bare base is also
/// counted so "React.js" and "React" titles share a term.
const JS_NAMES: &[&str] = &["reactjs", "vuejs", "nodejs", "nextjs", "threejs", "solidjs"];

/// Fold "next.js" → "nextjs" (any `<word>.js`) before tokenizing, so the dot
/// split never yields a bare "next" / "js" pair.
fn normalize_js_names(lower: &str) -> String {
    let bytes = lower.as_bytes();
    let mut out = String::with_capacity(lower.len());
    let mut i = 0;
    while i < lower.len() {
        let is_js_dot = bytes[i] == b'.'
            && i > 0
            && bytes[i - 1].is_ascii_alphanumeric()
            && lower[i + 1..].starts_with("js")
            && !lower[i + 3..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if is_js_dot {
            i += 1; // drop the dot; "js" follows as part of the token
            continue;
        }
        let ch = lower[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Digit-led or digit-dominated tokens name nothing a human scans a map by.
///
/// Two live leak classes (2026-07-16 and 2026-07-19): long digit runs are
/// ids/timestamps/URL fragments ("116885294589687234here"), and short
/// version/count/date tokens leak into labels as junk ("152", "8th", "191k",
/// "2026-07-14", "160-post") — maximally "distinctive" to c-TF-IDF yet
/// meaningless. Noise = starts with a digit (counts, ordinals, dates,
/// versions) OR carries at least as many digits as letters. Real names with
/// incidental digits ("typescript", "sqlite3", "react19") survive.
fn is_numeric_noise(token: &str) -> bool {
    if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    let digits = token.chars().filter(char::is_ascii_digit).count();
    let alphas = token.chars().filter(char::is_ascii_alphabetic).count();
    digits >= alphas.max(1)
}
