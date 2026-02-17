//! Technology Radar — Computed personal tech radar from existing 4DA data
//!
//! Synthesizes a ThoughtWorks-style Technology Radar from domain profile,
//! developer decisions, topic affinities, and source item mentions.
//! This is a computed view — nothing is stored.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarRing {
    Adopt,
    Trial,
    Assess,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarMovement {
    Up,
    Down,
    Stable,
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarQuadrant {
    Languages,
    Frameworks,
    Tools,
    Platforms,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RadarEntry {
    pub name: String,
    pub ring: RadarRing,
    pub quadrant: RadarQuadrant,
    pub movement: RadarMovement,
    pub signals: Vec<String>,
    pub decision_ref: Option<i64>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TechRadar {
    pub generated_at: String,
    pub entries: Vec<RadarEntry>,
}

// ============================================================================
// Quadrant Classification
// ============================================================================

fn classify_quadrant(name: &str) -> RadarQuadrant {
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
// Radar Computation
// ============================================================================

struct EntryBuilder {
    ring: RadarRing,
    quadrant: RadarQuadrant,
    movement: RadarMovement,
    signals: Vec<String>,
    decision_ref: Option<i64>,
    stack_weight: f64,
    engagement: f64,
    trend: f64,
    decision_boost: f64,
}

impl EntryBuilder {
    fn new(ring: RadarRing, stack_weight: f64) -> Self {
        Self {
            ring,
            quadrant: RadarQuadrant::Tools,
            movement: RadarMovement::New,
            signals: Vec::new(),
            decision_ref: None,
            stack_weight,
            engagement: 0.0,
            trend: 0.0,
            decision_boost: 0.0,
        }
    }

    fn score(&self) -> f64 {
        ((self.stack_weight * 0.4)
            + (self.engagement * 0.3)
            + (self.trend * 0.2)
            + (self.decision_boost * 0.1))
            .clamp(0.0, 1.0)
    }

    fn into_entry(self, name: String) -> RadarEntry {
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

/// Compute a personal technology radar from all available data.
pub(crate) fn compute_radar(conn: &Connection) -> Result<TechRadar, String> {
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
    final_entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    info!(target: "4da::tech_radar", count = final_entries.len(), "Tech radar computed");
    Ok(TechRadar {
        generated_at: chrono::Utc::now().to_rfc3339(),
        entries: final_entries,
    })
}

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
fn overlay_signal_trends(conn: &Connection, entries: &mut HashMap<String, EntryBuilder>) {
    let tech_names: Vec<String> = entries.keys().cloned().collect();
    for tech in &tech_names {
        let pattern = format!("%{}%", tech);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
             WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
             AND created_at >= datetime('now', '-30 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if let Some(eb) = entries.get_mut(tech) {
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
        if eb.movement != RadarMovement::New {
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

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_tech_radar() -> Result<TechRadar, String> {
    let conn = crate::open_db_connection()?;
    compute_radar(&conn)
}

#[tauri::command]
pub async fn get_radar_entry(name: String) -> Result<Option<RadarEntry>, String> {
    let conn = crate::open_db_connection()?;
    let radar = compute_radar(&conn)?;
    Ok(radar
        .entries
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(&name)))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tech_stack (id INTEGER PRIMARY KEY, technology TEXT NOT NULL UNIQUE);
             CREATE TABLE detected_tech (id INTEGER PRIMARY KEY, name TEXT NOT NULL, category TEXT, confidence REAL DEFAULT 0.5);
             CREATE TABLE explicit_interests (id INTEGER PRIMARY KEY, topic TEXT NOT NULL);
             CREATE TABLE project_dependencies (id INTEGER PRIMARY KEY, project_path TEXT, manifest_type TEXT, package_name TEXT, version TEXT, is_dev INTEGER DEFAULT 0, language TEXT, last_scanned TEXT DEFAULT (datetime('now')), UNIQUE(project_path, package_name));
             CREATE TABLE developer_decisions (id INTEGER PRIMARY KEY AUTOINCREMENT, decision_type TEXT NOT NULL, subject TEXT NOT NULL, decision TEXT NOT NULL, rationale TEXT, alternatives_rejected TEXT DEFAULT '[]', context_tags TEXT DEFAULT '[]', confidence REAL DEFAULT 0.8, status TEXT DEFAULT 'active', superseded_by INTEGER, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')));
             CREATE TABLE source_items (id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT NOT NULL, source_id TEXT NOT NULL, url TEXT, title TEXT NOT NULL, content TEXT DEFAULT '', content_hash TEXT DEFAULT '', embedding BLOB DEFAULT x'00', created_at TEXT DEFAULT (datetime('now')), last_seen TEXT DEFAULT (datetime('now')), UNIQUE(source_type, source_id));",
        ).unwrap();
        conn
    }

    #[test]
    fn test_classify_quadrant() {
        assert_eq!(classify_quadrant("rust"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("typescript"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("python"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("react"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("tauri"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("django"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("aws"), RadarQuadrant::Platforms);
        assert_eq!(classify_quadrant("vercel"), RadarQuadrant::Platforms);
        assert_eq!(classify_quadrant("docker"), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("webpack"), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("obscure-lib"), RadarQuadrant::Tools);
    }

    #[test]
    fn test_compute_radar_with_profile() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tech_stack (technology) VALUES ('typescript')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language)
             VALUES ('/proj', 'cargo', 'serde', '1.0', 0, 'rust')", [],
        ).unwrap();

        let radar = compute_radar(&conn).unwrap();
        assert!(!radar.entries.is_empty());

        let rust = radar.entries.iter().find(|e| e.name == "rust").unwrap();
        assert_eq!(rust.ring, RadarRing::Adopt);
        assert!(rust.score > 0.3);

        let ts = radar
            .entries
            .iter()
            .find(|e| e.name == "typescript")
            .unwrap();
        assert_eq!(ts.ring, RadarRing::Adopt);

        assert!(radar.entries.iter().any(|e| e.name == "serde"));
    }

    #[test]
    fn test_decision_overlay() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('sqlite')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, alternatives_rejected, status)
             VALUES ('tech_choice', 'sqlite', 'Use SQLite', '[\"postgresql\", \"mysql\"]', 'active')", [],
        ).unwrap();

        let radar = compute_radar(&conn).unwrap();

        let sqlite = radar.entries.iter().find(|e| e.name == "sqlite").unwrap();
        assert_eq!(sqlite.ring, RadarRing::Adopt);
        assert!(sqlite.decision_ref.is_some());

        let pg = radar
            .entries
            .iter()
            .find(|e| e.name == "postgresql")
            .unwrap();
        assert_eq!(pg.ring, RadarRing::Hold);
        assert!(pg.signals.iter().any(|s| s.contains("Rejected")));

        let mysql = radar.entries.iter().find(|e| e.name == "mysql").unwrap();
        assert_eq!(mysql.ring, RadarRing::Hold);
    }

    #[test]
    fn test_signal_trends() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        for i in 0..8 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content)
                 VALUES ('hackernews', ?1, ?2, 'Rust programming language news')",
                params![format!("hn-{}", i), format!("Rust {} release notes", i)],
            )
            .unwrap();
        }

        let radar = compute_radar(&conn).unwrap();
        let rust = radar.entries.iter().find(|e| e.name == "rust").unwrap();
        assert_eq!(rust.movement, RadarMovement::Up);
        assert!(rust.signals.iter().any(|s| s.contains("mentions")));
    }
}
