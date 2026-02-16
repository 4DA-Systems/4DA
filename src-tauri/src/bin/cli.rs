// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1). See LICENSE file.

//! 4DA CLI — Terminal access to your intelligence feed
//!
//! Reads from the same SQLite database as the desktop app.
//! All data stays local. This binary is read-only.
//!
//! Usage:
//!   4da briefing          Show latest AI briefing
//!   4da signals            Show recent items with signal classifications
//!   4da signals --critical Only critical/high priority signals
//!   4da gaps               Show knowledge gaps in your dependencies
//!   4da health             Show project dependency health
//!   4da status             Show database stats and last analysis time

use std::path::PathBuf;
use std::process;

// ============================================================================
// Database Path Resolution
// ============================================================================

/// Resolve the 4DA database path. Checks (in order):
/// 1. FOURDA_DB_PATH env var
/// 2. data/4da.db relative to working directory
/// 3. Platform-specific Tauri app data directory
fn resolve_db_path() -> Option<PathBuf> {
    // 1. Environment variable
    if let Ok(path) = std::env::var("FOURDA_DB_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Relative to CWD (development)
    let cwd = std::env::current_dir().ok()?;
    let dev_path = cwd.join("data").join("4da.db");
    if dev_path.exists() {
        return Some(dev_path);
    }

    // Also check parent (if running from src-tauri/)
    if let Some(parent) = cwd.parent() {
        let parent_path = parent.join("data").join("4da.db");
        if parent_path.exists() {
            return Some(parent_path);
        }
    }

    // 3. Platform-specific Tauri app data
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata)
                .join("com.4da.app")
                .join("data")
                .join("4da.db");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            let p = home
                .join("Library")
                .join("Application Support")
                .join("com.4da.app")
                .join("data")
                .join("4da.db");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            let p = home
                .join(".local")
                .join("share")
                .join("com.4da.app")
                .join("data")
                .join("4da.db");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Open SQLite connection with sqlite-vec extension
fn open_db(path: &PathBuf) -> Result<rusqlite::Connection, String> {
    #[allow(clippy::missing_transmute_annotations)]
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    let conn =
        rusqlite::Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))?;

    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| format!("Failed to set busy_timeout: {e}"))?;

    Ok(conn)
}

// ============================================================================
// Commands
// ============================================================================

#[allow(clippy::type_complexity)]
fn cmd_briefing(conn: &rusqlite::Connection) {
    let result: Result<Option<(String, Option<String>, i64, String)>, _> = conn
        .query_row(
            "SELECT content, model, item_count, created_at
         FROM briefings ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional();

    match result {
        Ok(Some((content, model, item_count, created_at))) => {
            let model_str = model.as_deref().unwrap_or("unknown");
            println!("--- 4DA Briefing ({created_at}) ---");
            println!("Items analyzed: {item_count} | Model: {model_str}\n");
            println!("{content}");
        }
        Ok(None) => {
            println!("No briefing available. Run an analysis from the 4DA desktop app first.");
        }
        Err(e) => {
            eprintln!("Error reading briefing: {e}");
        }
    }
}

fn cmd_signals(conn: &rusqlite::Connection, critical_only: bool) {
    // Get recent source items and check for signal patterns
    let query = "SELECT id, source_type, title, url, created_at
                 FROM source_items
                 ORDER BY created_at DESC
                 LIMIT 200";

    let mut stmt = match conn.prepare(query) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    // Load declared tech for signal classification
    let declared_tech: Vec<String> = conn
        .prepare("SELECT topic FROM interests")
        .and_then(|mut s| {
            s.query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    // Load detected tech
    let detected_tech: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT technology FROM detected_technologies
             WHERE confidence > 0.5 ORDER BY confidence DESC LIMIT 50",
        )
        .and_then(|mut s| {
            s.query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    // Load dependencies for matching
    let deps: Vec<String> = conn
        .prepare("SELECT DISTINCT package_name FROM project_dependencies")
        .and_then(|mut s| {
            s.query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    let mut signal_count = 0;

    for row in rows.flatten() {
        let (_id, source_type, title, url, created_at) = row;
        let title_lower = title.to_lowercase();
        let combined = title_lower.clone();

        // Inline signal classification (simplified version of signals.rs logic)
        let (priority, signal_type, action) = classify_signal(
            &combined,
            &title_lower,
            &declared_tech,
            &detected_tech,
            &deps,
        );

        if priority.is_none() {
            continue;
        }

        let pri = priority.unwrap();
        if critical_only && pri != "critical" && pri != "high" {
            continue;
        }

        let icon = match pri {
            "critical" => "!!",
            "high" => " !",
            "medium" => " -",
            _ => "  ",
        };

        let sig = signal_type.unwrap_or("signal");
        let date = &created_at[..10]; // Just the date part

        println!("[{icon}] [{pri:<8}] [{sig:<18}] [{source_type:<12}] {date}  {title}");
        if let Some(act) = action {
            println!("    Action: {act}");
        }
        if let Some(ref u) = url {
            println!("    {u}");
        }
        println!();
        signal_count += 1;
    }

    if signal_count == 0 {
        if critical_only {
            println!("No critical/high signals found. Your stack is clean.");
        } else {
            println!("No signals detected in recent items.");
        }
    } else {
        println!("--- {signal_count} signal(s) found ---");
    }
}

/// Simplified signal classification (mirrors signals.rs patterns)
fn classify_signal<'a>(
    combined: &str,
    title_lower: &str,
    declared_tech: &[String],
    detected_tech: &[String],
    deps: &[String],
) -> (Option<&'a str>, Option<&'a str>, Option<String>) {
    let all_tech: Vec<&str> = declared_tech
        .iter()
        .chain(detected_tech.iter())
        .chain(deps.iter())
        .map(|s| s.as_str())
        .collect();

    let matches_tech = |text: &str| -> bool {
        all_tech.iter().any(|t| {
            let t_lower = t.to_lowercase();
            text.contains(&t_lower)
        })
    };

    // Critical: Security alerts for YOUR stack
    let security_keywords = [
        "cve-",
        "vulnerability",
        "security advisory",
        "remote code execution",
        "sql injection",
        "xss",
        "csrf",
        "supply chain attack",
        "malware",
        "backdoor",
        "zero-day",
        "0day",
        "rce found",
    ];
    for kw in &security_keywords {
        if combined.contains(kw) && matches_tech(combined) {
            return (
                Some("critical"),
                Some("security_alert"),
                Some("Review and patch immediately".to_string()),
            );
        }
    }

    // High: Breaking changes in YOUR deps
    let breaking_keywords = [
        "breaking change",
        "deprecated",
        "end of life",
        "eol",
        "migration guide",
        "upgrade guide",
    ];
    for kw in &breaking_keywords {
        if combined.contains(kw) && matches_tech(combined) {
            return (
                Some("high"),
                Some("breaking_change"),
                Some("Check if your code is affected".to_string()),
            );
        }
    }

    // High: New major releases of YOUR deps
    if (title_lower.contains("released")
        || title_lower.contains("v2")
        || title_lower.contains("v3")
        || title_lower.contains("v4")
        || title_lower.contains("v5"))
        && matches_tech(combined)
    {
        return (
            Some("high"),
            Some("new_release"),
            Some("Review changelog for your usage".to_string()),
        );
    }

    // Medium: General security news
    for kw in &security_keywords {
        if combined.contains(kw) {
            return (Some("medium"), Some("security_alert"), None);
        }
    }

    // Medium: Your tech discussed
    if matches_tech(title_lower) {
        return (Some("low"), Some("tech_discussion"), None);
    }

    (None, None, None)
}

fn cmd_gaps(conn: &rusqlite::Connection) {
    // Get dependencies that have no matching content coverage
    let deps: Vec<(String, String, String)> = conn
        .prepare(
            "SELECT package_name, language, version
             FROM project_dependencies
             ORDER BY package_name",
        )
        .and_then(|mut s| {
            s.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?
                        .unwrap_or_else(|| "?".to_string()),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    if deps.is_empty() {
        println!("No project dependencies found. Run an ACE scan from the desktop app first.");
        return;
    }

    // Check which deps have recent content coverage
    let mut gaps = Vec::new();
    let mut covered = 0;

    for (name, lang, version) in &deps {
        let name_lower = name.to_lowercase();
        // Skip generic packages
        if ["typescript", "react", "node", "python", "rust"].contains(&name_lower.as_str()) {
            covered += 1;
            continue;
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1",
                [format!("%{name_lower}%")],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count == 0 {
            gaps.push((name.clone(), lang.clone(), version.clone()));
        } else {
            covered += 1;
        }
    }

    let total = deps.len();
    let coverage = if total > 0 {
        (covered as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    println!("--- Knowledge Gap Analysis ---");
    println!(
        "Dependencies: {total} | Covered: {covered} | Gaps: {} | Coverage: {coverage}%\n",
        gaps.len()
    );

    if gaps.is_empty() {
        println!("No knowledge gaps detected. All dependencies have recent content coverage.");
    } else {
        println!("Blind spots (dependencies with zero content coverage):\n");
        for (name, lang, version) in &gaps {
            println!("  {name} ({lang}) v{version}");
        }
        println!(
            "\nThese packages are in your dependency tree but 4DA has found zero\n\
             articles, discussions, or advisories about them."
        );
    }
}

fn cmd_health(conn: &rusqlite::Connection) {
    // Get all projects and their dependency counts
    let projects: Vec<(String, i64)> = conn
        .prepare(
            "SELECT project_path, COUNT(*) as dep_count
             FROM project_dependencies
             GROUP BY project_path
             ORDER BY dep_count DESC",
        )
        .and_then(|mut s| {
            s.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    if projects.is_empty() {
        println!("No projects found. Run an ACE scan from the desktop app first.");
        return;
    }

    println!("--- Project Health ---\n");

    for (path, dep_count) in &projects {
        // Get the project name from path
        let name = PathBuf::from(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        // Count dev vs prod deps
        let dev_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_dependencies
                 WHERE project_path = ?1 AND is_dev = 1",
                [path],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let prod_count = dep_count - dev_count;

        // Get languages used
        let languages: Vec<String> = conn
            .prepare(
                "SELECT DISTINCT language FROM project_dependencies
                 WHERE project_path = ?1",
            )
            .and_then(|mut s| {
                s.query_map([path], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default();

        let langs = languages.join(", ");

        println!("  {name}");
        println!("    Path: {path}");
        println!("    Dependencies: {prod_count} prod + {dev_count} dev = {dep_count} total");
        println!("    Languages: {langs}");
        println!();
    }
}

fn cmd_status(conn: &rusqlite::Connection) {
    let item_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
        .unwrap_or(0);

    let context_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
        .unwrap_or(0);

    let dep_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM project_dependencies", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let briefing_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM briefings", [], |row| row.get(0))
        .unwrap_or(0);

    let last_item: String = conn
        .query_row(
            "SELECT created_at FROM source_items ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "never".to_string());

    let interest_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM interests", [], |row| row.get(0))
        .unwrap_or(0);

    // Source breakdown
    let sources: Vec<(String, i64)> = conn
        .prepare(
            "SELECT source_type, COUNT(*) FROM source_items
             GROUP BY source_type ORDER BY COUNT(*) DESC",
        )
        .and_then(|mut s| {
            s.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_default();

    println!("--- 4DA Status ---\n");
    println!("  Source items:   {item_count}");
    println!("  Context chunks: {context_count}");
    println!("  Dependencies:   {dep_count}");
    println!("  Interests:      {interest_count}");
    println!("  Briefings:      {briefing_count}");
    println!("  Last fetch:     {last_item}\n");

    if !sources.is_empty() {
        println!("  Sources:");
        for (src, count) in &sources {
            println!("    {src:<14} {count}");
        }
    }
}

// ============================================================================
// Main
// ============================================================================

use rusqlite::OptionalExtension;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(0);
    }

    let command = args[1].as_str();

    if command == "--help" || command == "-h" || command == "help" {
        print_usage();
        process::exit(0);
    }

    // Resolve database
    let db_path = match resolve_db_path() {
        Some(p) => p,
        None => {
            eprintln!("4DA database not found.");
            eprintln!("Make sure the 4DA desktop app has been run at least once,");
            eprintln!("or set FOURDA_DB_PATH to point to your 4da.db file.");
            process::exit(1);
        }
    };

    let conn = match open_db(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to open database: {e}");
            process::exit(1);
        }
    };

    match command {
        "briefing" | "brief" | "b" => cmd_briefing(&conn),
        "signals" | "signal" | "s" => {
            let critical = args.iter().any(|a| a == "--critical" || a == "-c");
            cmd_signals(&conn, critical);
        }
        "gaps" | "gap" | "g" => cmd_gaps(&conn),
        "health" | "h" => cmd_health(&conn),
        "status" | "st" => cmd_status(&conn),
        other => {
            eprintln!("Unknown command: {other}\n");
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!(
        "4DA CLI — All signal. No feed.\n\
         \n\
         Usage: 4da <command> [options]\n\
         \n\
         Commands:\n\
         \n\
           briefing, b       Show latest AI briefing\n\
           signals, s        Show items with signal classifications\n\
             --critical, -c   Only critical/high priority\n\
           gaps, g           Show knowledge gaps in your dependencies\n\
           health, h         Show project dependency health\n\
           status, st        Show database stats\n\
         \n\
         Environment:\n\
         \n\
           FOURDA_DB_PATH    Path to 4da.db (auto-detected if not set)\n\
         \n\
         The CLI reads from the same database as the 4DA desktop app.\n\
         All data stays on your machine."
    );
}
