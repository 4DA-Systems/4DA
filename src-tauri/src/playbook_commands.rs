use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookModule {
    pub id: String,
    pub title: String,
    pub description: String,
    pub lesson_count: usize,
    pub is_free: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookLesson {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookContent {
    pub module_id: String,
    pub title: String,
    pub description: String,
    pub lessons: Vec<PlaybookLesson>,
    pub is_free: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookModuleProgress {
    pub module_id: String,
    pub completed_lessons: Vec<u32>,
    pub total_lessons: usize,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookProgress {
    pub modules: Vec<PlaybookModuleProgress>,
    pub overall_percentage: f32,
}

// Module metadata: (id, title, description, is_free)
const MODULE_DEFS: &[(&str, &str, &str, bool)] = &[
    (
        "S",
        "Sovereign Setup",
        "Configure your rig as a business asset",
        true,
    ),
    (
        "T",
        "Technical Moats",
        "Build what competitors can't easily copy",
        false,
    ),
    (
        "R",
        "Revenue Engines",
        "Eight ways to turn skills into income",
        false,
    ),
    (
        "E1",
        "Execution Playbook",
        "Ship your first revenue engine",
        false,
    ),
    ("E2", "Evolving Edge", "Stay ahead as markets shift", false),
    (
        "T2",
        "Tactical Automation",
        "Automate your income streams",
        false,
    ),
    (
        "S2",
        "Stacking Streams",
        "Combine engines for resilience",
        false,
    ),
];

pub(crate) fn module_id_to_filename(id: &str) -> Option<&'static str> {
    match id {
        "S" => Some("module-s-sovereign-setup.md"),
        "T" => Some("module-t-technical-moats.md"),
        "R" => Some("module-r-revenue-engines.md"),
        "E1" => Some("module-e1-execution-playbook.md"),
        "E2" => Some("module-e2-evolving-edge.md"),
        "T2" => Some("module-t2-tactical-automation.md"),
        "S2" => Some("module-s2-stacking-streams.md"),
        _ => None,
    }
}

pub(crate) fn get_content_dir() -> PathBuf {
    // Development: docs/streets/ relative to project root (CARGO_MANIFEST_DIR = src-tauri/)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = manifest_dir.parent() {
        let dev_path = root.join("docs").join("streets");
        if dev_path.exists() {
            return dev_path;
        }
    }

    // Production: relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let rel = exe_dir.join("docs").join("streets");
            if rel.exists() {
                return rel;
            }
        }
    }

    // Final fallback
    PathBuf::from("docs/streets")
}

pub(crate) fn parse_lessons(content: &str) -> Vec<PlaybookLesson> {
    let mut lessons = Vec::new();
    let mut current_title = String::new();
    let mut current_content = String::new();

    for line in content.lines() {
        if line.starts_with("## Lesson") {
            // Save previous lesson
            if !current_title.is_empty() {
                lessons.push(PlaybookLesson {
                    title: current_title.clone(),
                    content: current_content.trim().to_string(),
                });
            }
            // Extract title from "## Lesson N: Title"
            current_title = line.trim_start_matches('#').trim().to_string();
            current_content = String::new();
        } else if !current_title.is_empty() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }
    // Don't forget the last lesson
    if !current_title.is_empty() {
        lessons.push(PlaybookLesson {
            title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    lessons
}

#[tauri::command]
pub fn get_playbook_modules() -> Result<Vec<PlaybookModule>, String> {
    let content_dir = get_content_dir();
    let mut modules = Vec::new();

    for (id, title, desc, is_free) in MODULE_DEFS {
        let lesson_count = match module_id_to_filename(id) {
            Some(filename) => {
                let path = content_dir.join(filename);
                if path.exists() {
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    parse_lessons(&content).len()
                } else {
                    0
                }
            }
            None => 0,
        };

        modules.push(PlaybookModule {
            id: id.to_string(),
            title: title.to_string(),
            description: desc.to_string(),
            lesson_count,
            is_free: *is_free,
        });
    }

    Ok(modules)
}

#[tauri::command]
pub fn get_playbook_content(module_id: String) -> Result<PlaybookContent, String> {
    let content_dir = get_content_dir();
    let filename = module_id_to_filename(&module_id)
        .ok_or_else(|| format!("Unknown module: {}", module_id))?;
    let path = content_dir.join(filename);

    if !path.exists() {
        return Err(format!("Module file not found: {}", path.display()));
    }

    let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read module: {}", e))?;

    let lessons = parse_lessons(&raw);

    // Find module metadata
    let (_, title, desc, is_free) = MODULE_DEFS
        .iter()
        .find(|(id, _, _, _)| *id == module_id.as_str())
        .ok_or_else(|| format!("Unknown module: {}", module_id))?;

    Ok(PlaybookContent {
        module_id,
        title: title.to_string(),
        description: desc.to_string(),
        lessons,
        is_free: *is_free,
    })
}

#[tauri::command]
pub fn get_playbook_progress() -> Result<PlaybookProgress, String> {
    let conn = crate::open_db_connection()?;

    let content_dir = get_content_dir();
    let mut modules = Vec::new();
    let mut total_lessons = 0usize;
    let mut total_completed = 0usize;

    for (id, _, _, _) in MODULE_DEFS {
        let lesson_count = match module_id_to_filename(id) {
            Some(filename) => {
                let path = content_dir.join(filename);
                if path.exists() {
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    parse_lessons(&content).len()
                } else {
                    0
                }
            }
            None => 0,
        };

        let mut stmt = conn
            .prepare("SELECT lesson_idx FROM playbook_progress WHERE module_id = ?")
            .map_err(|e| e.to_string())?;

        let completed: Vec<u32> = stmt
            .query_map([id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let percentage = if lesson_count > 0 {
            (completed.len() as f32 / lesson_count as f32) * 100.0
        } else {
            0.0
        };

        total_lessons += lesson_count;
        total_completed += completed.len();

        modules.push(PlaybookModuleProgress {
            module_id: id.to_string(),
            completed_lessons: completed,
            total_lessons: lesson_count,
            percentage,
        });
    }

    let overall = if total_lessons > 0 {
        (total_completed as f32 / total_lessons as f32) * 100.0
    } else {
        0.0
    };

    Ok(PlaybookProgress {
        modules,
        overall_percentage: overall,
    })
}

#[tauri::command]
pub fn mark_lesson_complete(module_id: String, lesson_idx: u32) -> Result<(), String> {
    let conn = crate::open_db_connection()?;

    conn.execute(
        "INSERT OR IGNORE INTO playbook_progress (module_id, lesson_idx) VALUES (?1, ?2)",
        rusqlite::params![module_id, lesson_idx],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
