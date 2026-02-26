//! Sovereign Profile — accumulates hardware/system facts from STREETS commands.
//!
//! Facts are extracted automatically from command output and stored in the
//! `sovereign_profile` table. The module also generates a "Sovereign Stack
//! Document" (STREETS Lesson 6 deliverable) from accumulated facts.

use crate::error::{FourDaError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignFact {
    pub category: String,
    pub key: String,
    pub value: String,
    pub source_lesson: Option<String>,
    pub confidence: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignProfileData {
    pub facts: Vec<SovereignFact>,
    pub categories: Vec<CategorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: String,
    pub fact_count: usize,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCompleteness {
    pub total_categories: usize,
    pub filled_categories: usize,
    pub percentage: f64,
    pub missing: Vec<String>,
}

/// All categories tracked by the sovereign profile.
const ALL_CATEGORIES: &[&str] = &[
    "cpu", "ram", "gpu", "storage", "network", "os", "llm", "legal", "budget",
];

// ============================================================================
// Fact Extraction from Command Output
// ============================================================================

/// Parse command stdout to extract structured `(category, key, value)` facts.
pub fn extract_facts_from_output(command: &str, stdout: &str) -> Vec<(String, String, String)> {
    let mut f = Vec::new();
    let cl = command.to_lowercase();

    // Helper: push a fact tuple
    macro_rules! fact {
        ($cat:expr, $key:expr, $val:expr) => {
            f.push(($cat.into(), $key.into(), $val.into()))
        };
    }

    // CPU (lscpu / Get-CimInstance Win32_Processor)
    if cl.contains("lscpu") || (cl.contains("get-ciminstance") && cl.contains("processor")) {
        for line in stdout.lines() {
            let t = line.trim();
            if let Some(v) = t.strip_prefix("Model name:") {
                fact!("cpu", "model", v.trim());
            } else if let Some(v) = t.strip_prefix("CPU(s):") {
                fact!("cpu", "cores", v.trim());
            } else if let Some(v) = t.strip_prefix("Thread(s) per core:") {
                fact!("cpu", "threads_per_core", v.trim());
            } else if let Some(v) = t.strip_prefix("Architecture:") {
                fact!("cpu", "architecture", v.trim());
            }
            // Windows CIM "Name : ..." / "NumberOfCores : ..."
            if t.starts_with("Name") && t.contains(':') {
                if let Some(v) = t.split(':').nth(1).map(str::trim).filter(|v| !v.is_empty()) {
                    fact!("cpu", "model", v);
                }
            }
            if t.starts_with("NumberOfCores") && t.contains(':') {
                if let Some(v) = t.split(':').nth(1) {
                    fact!("cpu", "cores", v.trim());
                }
            }
        }
    }

    // RAM (free -h)
    if cl.contains("free") {
        for line in stdout.lines() {
            if line.trim().starts_with("Mem:") {
                let p: Vec<&str> = line.split_whitespace().collect();
                if p.len() >= 2 {
                    fact!("ram", "total", p[1]);
                }
                if p.len() >= 4 {
                    fact!("ram", "available", p[3]);
                }
            }
        }
    }

    // GPU (nvidia-smi)
    if cl.contains("nvidia-smi") {
        if cl.contains("--query-gpu") {
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() >= 2 {
                let parts: Vec<&str> = lines[1].split(',').map(str::trim).collect();
                if !parts.is_empty() {
                    fact!("gpu", "name", parts[0]);
                }
                if parts.len() >= 2 {
                    fact!("gpu", "memory_total", parts[1]);
                }
            }
        } else {
            for line in stdout.lines() {
                let t = line.trim();
                if t.contains("Driver Version:") {
                    if let Some(v) = t
                        .split("Driver Version:")
                        .nth(1)
                        .and_then(|s| s.split_whitespace().next())
                        .filter(|v| !v.is_empty())
                    {
                        fact!("gpu", "driver_version", v);
                    }
                }
                if t.starts_with('|') && t.contains("NVIDIA") {
                    let inner = t.trim_start_matches('|').trim();
                    if let Some(end) = inner.find("  ") {
                        let name = inner[..end].trim();
                        if !name.is_empty() {
                            fact!("gpu", "name", name);
                        }
                    }
                }
            }
        }
    }

    // AMD GPU (rocm-smi)
    if cl.contains("rocm-smi") && stdout.contains("GFX Version") {
        fact!("gpu", "type", "AMD ROCm");
    }

    // Storage (df -h)
    if cl.contains("df ") {
        let total_gb: f64 = stdout
            .lines()
            .filter(|l| l.trim().starts_with('/') || l.contains(':'))
            .filter_map(|l| l.split_whitespace().nth(1).and_then(parse_size_to_gb))
            .sum();
        if total_gb > 0.0 {
            fact!("storage", "total", format!("{:.0} GB", total_gb));
        }
    }

    // OS (uname -a)
    if cl.contains("uname") {
        if let Some(first) = stdout
            .lines()
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            fact!("os", "uname", first);
            if let Some(k) = first.split_whitespace().next() {
                fact!("os", "kernel", k);
            }
        }
    }

    // Windows systeminfo
    if cl.contains("systeminfo") {
        for line in stdout.lines() {
            let t = line.trim();
            if t.starts_with("OS Name:") {
                if let Some(v) = t.split(':').nth(1) {
                    fact!("os", "name", v.trim());
                }
            } else if t.starts_with("OS Version:") {
                if let Some(v) = t.split(':').nth(1) {
                    fact!("os", "version", v.trim());
                }
            } else if t.starts_with("Total Physical Memory:") {
                if let Some(v) = t.split(':').nth(1) {
                    fact!("ram", "total", v.trim());
                }
            }
        }
    }

    // Ollama
    if cl.contains("ollama") {
        if cl.contains("--version") || cl.contains("-v") {
            let v = stdout.trim();
            if !v.is_empty() {
                fact!("llm", "ollama_version", v);
            }
        }
        if cl.contains("list") {
            let models: Vec<&str> = stdout
                .lines()
                .skip(1)
                .filter_map(|l| l.split_whitespace().next().filter(|n| !n.is_empty()))
                .collect();
            if !models.is_empty() {
                fact!("llm", "installed_models", models.join(", "));
            }
        }
    }

    // Network (speedtest)
    if cl.contains("speedtest") {
        for line in stdout.lines() {
            let t = line.trim();
            if t.starts_with("Download:") {
                fact!("network", "download_speed", t);
            } else if t.starts_with("Upload:") {
                fact!("network", "upload_speed", t);
            }
        }
    }

    f
}

/// Parse size strings like "500G", "1.2T", "256M" to approximate GB.
fn parse_size_to_gb(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.ends_with('T') || s.ends_with("Ti") {
        s.trim_end_matches("Ti")
            .trim_end_matches('T')
            .parse::<f64>()
            .ok()
            .map(|v| v * 1024.0)
    } else if s.ends_with('G') || s.ends_with("Gi") {
        s.trim_end_matches("Gi")
            .trim_end_matches('G')
            .parse::<f64>()
            .ok()
    } else if s.ends_with('M') || s.ends_with("Mi") {
        s.trim_end_matches("Mi")
            .trim_end_matches('M')
            .parse::<f64>()
            .ok()
            .map(|v| v / 1024.0)
    } else {
        None
    }
}

// ============================================================================
// Public Helper — called from streets_commands after execution
// ============================================================================

/// Extract and store facts from a command execution's stdout.
/// Non-fatal: logs warnings on failure instead of propagating errors.
pub fn store_facts_from_execution(command: &str, stdout: &str, source_lesson: &str) {
    let facts = extract_facts_from_output(command, stdout);
    if facts.is_empty() {
        return;
    }

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::sovereign", error = %e, "Cannot store sovereign facts: DB unavailable");
            return;
        }
    };

    for (category, key, value) in &facts {
        if let Err(e) = conn.execute(
            "INSERT INTO sovereign_profile (category, key, value, source_command, source_lesson, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, 1.0)
             ON CONFLICT(category, key) DO UPDATE SET
                value = excluded.value,
                source_command = excluded.source_command,
                source_lesson = excluded.source_lesson,
                updated_at = datetime('now')",
            params![category, key, value, command, source_lesson],
        ) {
            warn!(target: "4da::sovereign", error = %e, category, key, "Failed to store sovereign fact");
        }
    }

    info!(target: "4da::sovereign",
        count = facts.len(),
        source = source_lesson,
        "Stored sovereign facts from command execution"
    );
}

/// Log a command execution to the execution log table.
#[allow(clippy::too_many_arguments)]
pub fn log_command_execution(
    module_id: &str,
    lesson_idx: usize,
    command_id: &str,
    command_text: &str,
    success: bool,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    duration_ms: u64,
) {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::sovereign", error = %e, "Cannot log command execution: DB unavailable");
            return;
        }
    };

    if let Err(e) = conn.execute(
        "INSERT INTO command_execution_log
            (module_id, lesson_idx, command_id, command_text, success, exit_code, stdout, stderr, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            module_id,
            lesson_idx as i64,
            command_id,
            command_text,
            success as i32,
            exit_code,
            stdout,
            stderr,
            duration_ms as i64,
        ],
    ) {
        warn!(target: "4da::sovereign", error = %e, "Failed to log command execution");
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_sovereign_profile() -> Result<SovereignProfileData> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    let mut stmt = conn
        .prepare(
            "SELECT category, key, value, source_lesson, confidence, updated_at
             FROM sovereign_profile ORDER BY category, key",
        )
        .map_err(FourDaError::Db)?;

    let facts: Vec<SovereignFact> = stmt
        .query_map([], |row| {
            Ok(SovereignFact {
                category: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                source_lesson: row.get(3)?,
                confidence: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect();

    // Build category summaries
    let mut cat_map: std::collections::HashMap<String, (usize, Option<String>)> =
        std::collections::HashMap::new();
    for fact in &facts {
        let entry = cat_map.entry(fact.category.clone()).or_insert((0, None));
        entry.0 += 1;
        if entry.1.is_none() || entry.1.as_deref() < Some(&fact.updated_at) {
            entry.1 = Some(fact.updated_at.clone());
        }
    }

    let categories = cat_map
        .into_iter()
        .map(|(cat, (count, last))| CategorySummary {
            category: cat,
            fact_count: count,
            last_updated: last,
        })
        .collect();

    Ok(SovereignProfileData { facts, categories })
}

#[tauri::command]
pub async fn get_sovereign_profile_completeness() -> Result<ProfileCompleteness> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    let mut stmt = conn
        .prepare("SELECT DISTINCT category FROM sovereign_profile")
        .map_err(FourDaError::Db)?;

    let filled: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect();

    let total = ALL_CATEGORIES.len();
    let filled_count = ALL_CATEGORIES
        .iter()
        .filter(|c| filled.contains(**c))
        .count();
    let missing: Vec<String> = ALL_CATEGORIES
        .iter()
        .filter(|c| !filled.contains(**c))
        .map(|c| c.to_string())
        .collect();

    let percentage = if total > 0 {
        (filled_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(ProfileCompleteness {
        total_categories: total,
        filled_categories: filled_count,
        percentage,
        missing,
    })
}

#[tauri::command]
pub async fn generate_sovereign_stack_document() -> Result<String> {
    let profile = get_sovereign_profile().await?;
    let completeness = get_sovereign_profile_completeness().await?;

    let mut doc = String::new();
    doc.push_str("# Sovereign Stack Document\n\n");
    doc.push_str(&format!(
        "**Profile Completeness:** {:.0}% ({}/{} categories)\n\n",
        completeness.percentage, completeness.filled_categories, completeness.total_categories,
    ));
    doc.push_str("---\n\n");

    // Group facts by category
    let mut by_category: std::collections::BTreeMap<String, Vec<&SovereignFact>> =
        std::collections::BTreeMap::new();
    for fact in &profile.facts {
        by_category
            .entry(fact.category.clone())
            .or_default()
            .push(fact);
    }

    let category_labels: std::collections::HashMap<&str, &str> = [
        ("cpu", "CPU / Processor"),
        ("ram", "Memory (RAM)"),
        ("gpu", "GPU / Accelerator"),
        ("storage", "Storage"),
        ("network", "Network"),
        ("os", "Operating System"),
        ("llm", "LLM Infrastructure"),
        ("legal", "Legal Entity"),
        ("budget", "Budget / Runway"),
    ]
    .into_iter()
    .collect();

    for cat in ALL_CATEGORIES {
        let label = category_labels.get(cat).unwrap_or(cat);
        doc.push_str(&format!("## {}\n\n", label));

        if let Some(facts) = by_category.get(*cat) {
            for fact in facts {
                doc.push_str(&format!("- **{}:** {}\n", fact.key, fact.value));
            }
        } else {
            doc.push_str(
                "_No data collected yet. Run the relevant STREETS commands to populate._\n",
            );
        }
        doc.push('\n');
    }

    doc.push_str("---\n\n");
    doc.push_str(
        "_Generated by 4DA Sovereign Profile. Run STREETS Module S commands to auto-populate._\n",
    );

    debug!(target: "4da::sovereign", doc_len = doc.len(), "Generated Sovereign Stack Document");
    Ok(doc)
}

#[tauri::command]
pub async fn save_sovereign_fact(category: String, key: String, value: String) -> Result<()> {
    if category.is_empty() || key.is_empty() || value.is_empty() {
        return Err(FourDaError::Config(
            "Category, key, and value are all required".into(),
        ));
    }

    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    conn.execute(
        "INSERT INTO sovereign_profile (category, key, value, source_command, source_lesson, confidence)
         VALUES (?1, ?2, ?3, 'manual', 'manual', 1.0)
         ON CONFLICT(category, key) DO UPDATE SET
            value = excluded.value,
            source_command = 'manual',
            source_lesson = 'manual',
            updated_at = datetime('now')",
        params![category, key, value],
    )
    .map_err(FourDaError::Db)?;

    info!(target: "4da::sovereign", category = %category, key = %key, "Saved manual sovereign fact");
    Ok(())
}

#[tauri::command]
pub async fn get_execution_log(
    module_id: String,
    lesson_idx: Option<usize>,
) -> Result<Vec<serde_json::Value>> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    let rows: Vec<serde_json::Value> = if let Some(idx) = lesson_idx {
        let mut stmt = conn
            .prepare(
                "SELECT id, module_id, lesson_idx, command_id, command_text, success,
                        exit_code, stdout, stderr, duration_ms, executed_at
                 FROM command_execution_log
                 WHERE module_id = ?1 AND lesson_idx = ?2
                 ORDER BY executed_at DESC LIMIT 100",
            )
            .map_err(FourDaError::Db)?;
        let result: Vec<serde_json::Value> = stmt
            .query_map(params![module_id, idx as i64], row_to_json)
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, module_id, lesson_idx, command_id, command_text, success,
                        exit_code, stdout, stderr, duration_ms, executed_at
                 FROM command_execution_log
                 WHERE module_id = ?1
                 ORDER BY executed_at DESC LIMIT 100",
            )
            .map_err(FourDaError::Db)?;
        let result: Vec<serde_json::Value> = stmt
            .query_map(params![module_id], row_to_json)
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    Ok(rows)
}

fn row_to_json(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, i64>(0)?,
        "module_id": row.get::<_, String>(1)?,
        "lesson_idx": row.get::<_, i64>(2)?,
        "command_id": row.get::<_, String>(3)?,
        "command_text": row.get::<_, String>(4)?,
        "success": row.get::<_, i32>(5)? != 0,
        "exit_code": row.get::<_, Option<i32>>(6)?,
        "stdout": row.get::<_, Option<String>>(7)?,
        "stderr": row.get::<_, Option<String>>(8)?,
        "duration_ms": row.get::<_, Option<i64>>(9)?,
        "executed_at": row.get::<_, Option<String>>(10)?,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cpu_lscpu() {
        let stdout = "Architecture:          x86_64\nCPU(s):                12\nModel name:            AMD Ryzen 5 3600\n";
        let facts = extract_facts_from_output("lscpu", stdout);
        assert!(facts.iter().any(|(c, k, _)| c == "cpu" && k == "model"));
        assert!(facts.iter().any(|(c, k, _)| c == "cpu" && k == "cores"));
        assert!(facts
            .iter()
            .any(|(c, k, _)| c == "cpu" && k == "architecture"));
    }

    #[test]
    fn test_extract_ram_free() {
        let stdout = "              total        used        free      shared\nMem:           31Gi       12Gi       15Gi       0.5Gi\n";
        let facts = extract_facts_from_output("free -h", stdout);
        assert!(facts.iter().any(|(c, k, _)| c == "ram" && k == "total"));
    }

    #[test]
    fn test_extract_ollama_version() {
        let stdout = "ollama version is 0.1.35\n";
        let facts = extract_facts_from_output("ollama --version", stdout);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].0, "llm");
        assert_eq!(facts[0].1, "ollama_version");
    }

    #[test]
    fn test_extract_nvidia_csv() {
        let stdout = "name, memory.total [MiB]\nNVIDIA GeForce RTX 3080, 10240 MiB\n";
        let facts = extract_facts_from_output(
            "nvidia-smi --query-gpu=name,memory.total --format=csv",
            stdout,
        );
        assert!(facts.iter().any(|(c, k, _)| c == "gpu" && k == "name"));
        assert!(facts
            .iter()
            .any(|(c, k, _)| c == "gpu" && k == "memory_total"));
    }

    #[test]
    fn test_extract_empty_returns_nothing() {
        let facts = extract_facts_from_output("echo hello", "hello\n");
        assert!(facts.is_empty());
    }

    #[test]
    fn test_parse_size_to_gb() {
        assert_eq!(parse_size_to_gb("500G"), Some(500.0));
        assert_eq!(parse_size_to_gb("1T"), Some(1024.0));
        assert_eq!(parse_size_to_gb("512M"), Some(0.5));
    }
}
