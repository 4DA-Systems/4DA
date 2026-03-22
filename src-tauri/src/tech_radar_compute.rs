//! Radar computation engine — builds a personal tech radar from existing 4DA data.
//!
//! Extracted from `tech_radar.rs` to keep file sizes under the 600-line limit.
//! Contains the classify/score/overlay pipeline; public types and Tauri commands
//! remain in `tech_radar`.

use rusqlite::Connection;
use std::collections::HashMap;
use tracing::info;

use crate::error::Result;
use crate::tech_radar::{RadarEntry, RadarMovement, RadarQuadrant, RadarRing, TechRadar};

// ============================================================================
// Quadrant Classification
// ============================================================================

pub(crate) fn classify_quadrant(name: &str) -> RadarQuadrant {
    let n = name.to_lowercase();
    const LANGS: &[&str] = &[
        "rust",
        "typescript",
        "javascript",
        "python",
        "go",
        "golang",
        "java",
        "kotlin",
        "swift",
        "c++",
        "cpp",
        "c#",
        "csharp",
        "ruby",
        "php",
        "scala",
        "elixir",
        "haskell",
        "zig",
        "lua",
        "dart",
        "r",
        "julia",
    ];
    const FWORKS: &[&str] = &[
        "react",
        "vue",
        "angular",
        "svelte",
        "nextjs",
        "next.js",
        "nuxt",
        "tauri",
        "electron",
        "django",
        "flask",
        "fastapi",
        "rails",
        "spring",
        "express",
        "fastify",
        "actix",
        "axum",
        "gin",
        "remix",
        "astro",
        "sveltekit",
        "solidjs",
        "qwik",
        "flutter",
        "react-native",
        "wails",
        "pytorch",
        "tensorflow",
        "langchain",
        "bevy",
        "unity",
        "godot",
    ];
    const PLATS: &[&str] = &[
        "aws",
        "gcp",
        "azure",
        "vercel",
        "netlify",
        "cloudflare",
        "linux",
        "windows",
        "macos",
        "ios",
        "android",
        "heroku",
        "fly.io",
        "railway",
        "supabase",
        "firebase",
        "neon",
        "planetscale",
        "kubernetes",
        "k8s",
    ];
    if LANGS.iter().any(|l| n == *l) {
        return RadarQuadrant::Languages;
    }
    if FWORKS.iter().any(|f| n == *f || n.contains(f)) {
        return RadarQuadrant::Frameworks;
    }
    if PLATS.iter().any(|p| n == *p || n.contains(p)) {
        return RadarQuadrant::Platforms;
    }
    RadarQuadrant::Tools
}

// ============================================================================
// EntryBuilder — internal scoring helper
// ============================================================================

pub(crate) struct EntryBuilder {
    pub ring: RadarRing,
    pub quadrant: RadarQuadrant,
    pub movement: RadarMovement,
    pub signals: Vec<String>,
    pub decision_ref: Option<i64>,
    pub stack_weight: f64,
    pub engagement: f64,
    pub trend: f64,
    pub decision_boost: f64,
}

impl EntryBuilder {
    pub fn new(ring: RadarRing, stack_weight: f64) -> Self {
        Self {
            ring,
            quadrant: RadarQuadrant::Tools,
            movement: RadarMovement::Stable,
            signals: Vec::new(),
            decision_ref: None,
            stack_weight,
            engagement: 0.0,
            trend: 0.0,
            decision_boost: 0.0,
        }
    }

    pub fn score(&self) -> f64 {
        ((self.stack_weight * 0.4)
            + (self.engagement * 0.3)
            + (self.trend * 0.2)
            + (self.decision_boost * 0.1))
            .clamp(0.0, 1.0)
    }

    pub fn into_entry(self, name: String) -> RadarEntry {
        RadarEntry {
            score: self.score(),
            name,
            ring: self.ring,
            quadrant: self.quadrant,
            movement: self.movement,
            signals: self.signals,
            decision_ref: self.decision_ref,
        }
    }
}

// ============================================================================
// Radar Computation
// ============================================================================

/// Compute a personal technology radar from all available data.
pub(crate) fn compute_radar(conn: &Connection) -> Result<TechRadar> {
    let profile = crate::domain_profile::build_domain_profile(conn);
    let mut entries: HashMap<String, EntryBuilder> = HashMap::new();

    // Step 1: Seed from domain profile
    for tech in &profile.primary_stack {
        entries
            .entry(tech.clone())
            .or_insert_with(|| EntryBuilder::new(RadarRing::Adopt, 0.9));
    }
    for dep in &profile.dependency_names {
        entries
            .entry(dep.clone())
            .or_insert_with(|| EntryBuilder::new(RadarRing::Trial, 0.7));
    }
    for adj in &profile.adjacent_tech {
        entries
            .entry(adj.clone())
            .or_insert_with(|| EntryBuilder::new(RadarRing::Assess, 0.5));
    }

    // Classify quadrants
    for (name, eb) in entries.iter_mut() {
        eb.quadrant = classify_quadrant(name);
    }

    // Steps 2-5: Overlay data layers
    overlay_decisions(conn, &mut entries);
    overlay_affinities(conn, &mut entries);
    overlay_signal_trends(conn, &mut entries);
    detect_movement(conn, &mut entries);

    // Build final sorted radar
    let mut final_entries: Vec<RadarEntry> = entries
        .into_iter()
        .map(|(name, eb)| eb.into_entry(name))
        .collect();

    // Blocklist of common non-technology words that aren't real technologies
    const NOISE_WORDS: &[&str] = &[
        "conf", "config", "debug", "image", "next", "yaml", "json", "toml",
        "test", "tests", "build", "dist", "src", "lib", "bin", "docs",
        "utils", "helpers", "types", "models", "core", "base", "common",
        "main", "index", "app", "server", "client", "api", "http",
        "async", "sync", "error", "errors", "log", "logs", "data",
        "file", "files", "path", "paths", "env", "dev", "prod",
        "setup", "init", "run", "start", "stop", "clean", "lint",
        "format", "check", "publish", "deploy", "release", "version",
    ];

    final_entries.retain(|e| {
        let name_lower = e.name.to_lowercase();
        !NOISE_WORDS.contains(&name_lower.as_str())
    });

    // Filter to meaningful entries
    final_entries.retain(|e| {
        e.score >= 0.25 && e.name.len() >= 3 && !e.name.contains('/') && !e.name.starts_with('@')
    });

    // Dedup case-insensitive
    let mut seen = std::collections::HashSet::new();
    final_entries.retain(|e| seen.insert(e.name.to_lowercase()));

    // Keep top 40 by score
    final_entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    final_entries.truncate(40);

    info!(target: "4da::tech_radar", count = final_entries.len(), "Tech radar computed");
    Ok(TechRadar {
        generated_at: chrono::Utc::now().to_rfc3339(),
        entries: final_entries,
    })
}

// ============================================================================
// Overlay Helpers
// ============================================================================

/// Overlay active developer decisions onto radar entries.
fn overlay_decisions(conn: &Connection, entries: &mut HashMap<String, EntryBuilder>) {
    let mut stmt = match conn.prepare(
        "SELECT id, decision_type, subject, alternatives_rejected, status
         FROM developer_decisions WHERE status IN ('active', 'superseded')",
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    }) {
        Ok(r) => r,
        Err(_) => return,
    };

    for row in rows.flatten() {
        let (id, dtype, subject, alts_json, status) = row;
        let subj = subject.to_lowercase();
        let alts: Vec<String> = serde_json::from_str(&alts_json).unwrap_or_default();

        if dtype == "tech_choice" && status == "active" {
            if let Some(eb) = entries.get_mut(&subj) {
                eb.decision_boost = 1.0;
                eb.decision_ref = Some(id);
                eb.signals.push(format!("Active decision: {}", subject));
            }
            for alt in &alts {
                let alt_lower = alt.to_lowercase();
                let eb = entries
                    .entry(alt_lower.clone())
                    .or_insert_with(|| EntryBuilder::new(RadarRing::Hold, 0.2));
                eb.ring = RadarRing::Hold;
                eb.quadrant = classify_quadrant(&alt_lower);
                eb.decision_ref = Some(id);
                eb.signals.push(format!("Rejected in favor of {}", subject));
            }
        }
        if status == "superseded" {
            if let Some(eb) = entries.get_mut(&subj) {
                eb.ring = RadarRing::Hold;
                eb.decision_boost = 0.0;
                eb.signals.push("Superseded by newer decision".into());
            }
        }
    }
}

/// Overlay topic affinity engagement data.
fn overlay_affinities(conn: &Connection, entries: &mut HashMap<String, EntryBuilder>) {
    let mut stmt = match conn
        .prepare("SELECT topic, affinity_score FROM topic_affinities WHERE affinity_score > 0.3")
    {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return,
    };
    for row in rows.flatten() {
        let (topic, score) = row;
        if let Some(eb) = entries.get_mut(&topic.to_lowercase()) {
            eb.engagement = score.clamp(0.0, 1.0);
            if score > 0.7 && matches!(eb.ring, RadarRing::Trial | RadarRing::Assess) {
                eb.ring = RadarRing::Adopt;
                eb.signals
                    .push(format!("High engagement (affinity {:.2})", score));
            } else if score < 0.4 && matches!(eb.ring, RadarRing::Adopt | RadarRing::Trial) {
                eb.ring = RadarRing::Hold;
                eb.signals
                    .push(format!("Declining engagement (affinity {:.2})", score));
            }
        }
    }
}

/// Overlay signal trend data from source_items mentions in last 30 days.
///
/// Uses a single batch query to fetch all recent titles, then counts mentions
/// per tech in-memory. Replaces the previous N+1 pattern (one query per tech).
fn overlay_signal_trends(conn: &Connection, entries: &mut HashMap<String, EntryBuilder>) {
    // Batch: fetch all titles from last 30 days in one query
    let mut titles: Vec<String> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT LOWER(title) FROM source_items WHERE created_at >= datetime('now', '-30 days')",
    ) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            titles = rows.flatten().collect();
        }
    }

    // Count mentions per tech in-memory
    for (tech, eb) in entries.iter_mut() {
        let count = titles.iter().filter(|t| t.contains(tech.as_str())).count() as i64;
        eb.trend = match count {
            0 => 0.0,
            1..=3 => 0.3,
            4..=10 => 0.6,
            _ => 1.0,
        };
        if count > 5 {
            eb.movement = RadarMovement::Up;
            eb.signals
                .push(format!("{} mentions in last 30 days", count));
        }
    }
}

/// Detect movement from previous radar snapshots in temporal_events.
fn detect_movement(conn: &Connection, entries: &mut HashMap<String, EntryBuilder>) {
    let prev: Option<String> = conn
        .query_row(
            "SELECT data FROM temporal_events WHERE event_type = 'radar_snapshot'
         ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    let prev = match prev {
        Some(d) => d,
        None => return,
    };
    let prev_map: HashMap<String, String> = serde_json::from_str(&prev).unwrap_or_default();

    let ring_ord = |r: &str| match r {
        "adopt" => 3,
        "trial" => 2,
        "assess" => 1,
        _ => 0,
    };
    for (name, eb) in entries.iter_mut() {
        if eb.movement != RadarMovement::Stable {
            continue;
        }
        if let Some(prev_ring) = prev_map.get(name) {
            let cur = match &eb.ring {
                RadarRing::Adopt => "adopt",
                RadarRing::Trial => "trial",
                RadarRing::Assess => "assess",
                RadarRing::Hold => "hold",
            };
            let (c, p) = (ring_ord(cur), ring_ord(prev_ring));
            eb.movement = if c > p {
                RadarMovement::Up
            } else if c < p {
                RadarMovement::Down
            } else {
                RadarMovement::Stable
            };
        }
    }
}
