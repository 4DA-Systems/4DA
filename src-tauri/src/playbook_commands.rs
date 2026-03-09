use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::Result;

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

/// Returns the content directory for a specific language.
///
/// English content lives directly in `docs/streets/`.
/// Localized content lives in `docs/streets/{lang}/` (e.g. `docs/streets/es/`).
/// Falls back to the base directory (English) if no localized directory exists.
fn get_content_dir_for_lang(lang: &str) -> PathBuf {
    let base = get_content_dir();
    if lang != "en" {
        let localized = base.join(lang);
        if localized.exists() {
            return localized;
        }
    }
    base
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
pub fn get_playbook_modules(lang: Option<String>) -> Result<Vec<PlaybookModule>> {
    let language = lang.unwrap_or_else(crate::i18n::get_user_language);
    let content_dir = get_content_dir_for_lang(&language);
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
pub fn get_playbook_content(module_id: String, lang: Option<String>) -> Result<PlaybookContent> {
    let language = lang.unwrap_or_else(crate::i18n::get_user_language);
    let content_dir = get_content_dir_for_lang(&language);
    let filename = module_id_to_filename(&module_id)
        .ok_or_else(|| format!("Unknown module: {}", module_id))?;
    let path = content_dir.join(filename);

    if !path.exists() {
        return Err(format!("Module file not found: {}", path.display()).into());
    }

    let raw = fs::read_to_string(&path)?;

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
pub fn get_playbook_progress() -> Result<PlaybookProgress> {
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

        let mut stmt =
            conn.prepare("SELECT lesson_idx FROM playbook_progress WHERE module_id = ?")?;

        let completed: Vec<u32> = stmt
            .query_map([id], |row| row.get(0))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in playbook_commands: {e}");
                    None
                }
            })
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
pub fn mark_lesson_complete(
    app: tauri::AppHandle,
    module_id: String,
    lesson_idx: u32,
) -> Result<()> {
    use tauri::Emitter;

    let conn = crate::open_db_connection()?;

    conn.execute(
        "INSERT OR IGNORE INTO playbook_progress (module_id, lesson_idx) VALUES (?1, ?2)",
        rusqlite::params![module_id, lesson_idx],
    )?;

    // Extract topics from lesson content for affinity learning.
    // STREETS completions are strong positive signals — record them as
    // topic affinities so the scoring pipeline learns what the user cares about.
    if let Some(filename) = module_id_to_filename(&module_id) {
        let content_dir = get_content_dir();
        let path = content_dir.join(filename);
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let lessons = parse_lessons(&raw);
            if let Some(lesson) = lessons.get(lesson_idx as usize) {
                let topics = crate::extract_topics(&lesson.title, &lesson.content);
                if let Ok(ace) = crate::get_ace_engine() {
                    for topic in topics.iter().take(5) {
                        let topic_lower = topic.to_lowercase();
                        let _ = ace.record_interaction(
                            0,                                // No specific item_id for STREETS lessons
                            crate::ace::BehaviorAction::Save, // Save = strongest positive signal (1.0)
                            vec![topic_lower],
                            "streets".to_string(),
                        );
                    }
                    tracing::debug!(
                        target: "4da::streets",
                        module = %module_id,
                        lesson = lesson_idx,
                        topic_count = topics.len().min(5),
                        "Recorded STREETS lesson topics as affinity signals"
                    );
                }
            }
        }
    }

    // Notify frontend that profile data has changed
    if let Err(e) = app.emit("profile-updated", "lesson-complete") {
        tracing::warn!("Failed to emit 'profile-updated': {e}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- module_id_to_filename tests ----

    #[test]
    fn test_module_id_to_filename_known_ids() {
        assert_eq!(
            module_id_to_filename("S"),
            Some("module-s-sovereign-setup.md")
        );
        assert_eq!(
            module_id_to_filename("T"),
            Some("module-t-technical-moats.md")
        );
        assert_eq!(
            module_id_to_filename("R"),
            Some("module-r-revenue-engines.md")
        );
        assert_eq!(
            module_id_to_filename("E1"),
            Some("module-e1-execution-playbook.md")
        );
        assert_eq!(
            module_id_to_filename("E2"),
            Some("module-e2-evolving-edge.md")
        );
        assert_eq!(
            module_id_to_filename("T2"),
            Some("module-t2-tactical-automation.md")
        );
        assert_eq!(
            module_id_to_filename("S2"),
            Some("module-s2-stacking-streams.md")
        );
    }

    #[test]
    fn test_module_id_to_filename_unknown_returns_none() {
        assert_eq!(module_id_to_filename("X"), None);
        assert_eq!(module_id_to_filename(""), None);
        assert_eq!(module_id_to_filename("s"), None); // case-sensitive
    }

    // ---- parse_lessons tests ----

    #[test]
    fn test_parse_lessons_empty_input() {
        let lessons = parse_lessons("");
        assert!(lessons.is_empty());
    }

    #[test]
    fn test_parse_lessons_no_lesson_headers() {
        let content = "# Module Title\nSome intro text\nMore text";
        let lessons = parse_lessons(content);
        assert!(lessons.is_empty());
    }

    #[test]
    fn test_parse_lessons_single_lesson() {
        let content = "## Lesson 1: Getting Started\nThis is the first lesson.\nIt has two lines.";
        let lessons = parse_lessons(content);
        assert_eq!(lessons.len(), 1);
        assert_eq!(lessons[0].title, "Lesson 1: Getting Started");
        assert_eq!(
            lessons[0].content,
            "This is the first lesson.\nIt has two lines."
        );
    }

    #[test]
    fn test_parse_lessons_multiple_lessons() {
        let content = "\
## Lesson 1: First
Content of first lesson.
## Lesson 2: Second
Content of second lesson.
## Lesson 3: Third
Content of third lesson.";
        let lessons = parse_lessons(content);
        assert_eq!(lessons.len(), 3);
        assert_eq!(lessons[0].title, "Lesson 1: First");
        assert_eq!(lessons[1].title, "Lesson 2: Second");
        assert_eq!(lessons[2].title, "Lesson 3: Third");
    }

    #[test]
    fn test_parse_lessons_content_trimmed() {
        let content = "## Lesson 1: Test\n\n  Content with whitespace  \n\n";
        let lessons = parse_lessons(content);
        assert_eq!(lessons.len(), 1);
        // Content is trimmed by the parser
        assert_eq!(lessons[0].content, "Content with whitespace");
    }

    #[test]
    fn test_parse_lessons_ignores_content_before_first_lesson() {
        let content = "\
# Module Title
Some preamble text
## Lesson 1: Actual Lesson
Lesson body here.";
        let lessons = parse_lessons(content);
        assert_eq!(lessons.len(), 1);
        assert_eq!(lessons[0].title, "Lesson 1: Actual Lesson");
        assert_eq!(lessons[0].content, "Lesson body here.");
    }

    // ---- struct construction & serialization tests ----

    #[test]
    fn test_playbook_module_serialization() {
        let module = PlaybookModule {
            id: "S".to_string(),
            title: "Sovereign Setup".to_string(),
            description: "Configure your rig".to_string(),
            lesson_count: 5,
            is_free: true,
        };
        let json = serde_json::to_value(&module).expect("serialize");
        assert_eq!(json["id"], "S");
        assert_eq!(json["lesson_count"], 5);
        assert_eq!(json["is_free"], true);
    }

    #[test]
    fn test_playbook_module_deserialization() {
        let json = r#"{"id":"T","title":"Technical Moats","description":"Build moats","lesson_count":3,"is_free":false}"#;
        let module: PlaybookModule = serde_json::from_str(json).expect("deserialize");
        assert_eq!(module.id, "T");
        assert_eq!(module.lesson_count, 3);
        assert!(!module.is_free);
    }

    #[test]
    fn test_playbook_progress_serialization() {
        let progress = PlaybookProgress {
            modules: vec![PlaybookModuleProgress {
                module_id: "S".to_string(),
                completed_lessons: vec![0, 1, 2],
                total_lessons: 5,
                percentage: 60.0,
            }],
            overall_percentage: 60.0,
        };
        let json = serde_json::to_value(&progress).expect("serialize");
        assert_eq!(json["overall_percentage"], 60.0);
        assert_eq!(
            json["modules"][0]["completed_lessons"],
            serde_json::json!([0, 1, 2])
        );
    }

    #[test]
    fn test_playbook_content_struct() {
        let content = PlaybookContent {
            module_id: "R".to_string(),
            title: "Revenue Engines".to_string(),
            description: "Eight ways".to_string(),
            lessons: vec![PlaybookLesson {
                title: "Lesson 1".to_string(),
                content: "Body".to_string(),
            }],
            is_free: false,
        };
        let json = serde_json::to_value(&content).expect("serialize");
        assert_eq!(json["lessons"].as_array().expect("lessons array").len(), 1);
        assert!(!json["is_free"].as_bool().expect("is_free bool"));
    }

    // ---- MODULE_DEFS constant tests ----

    #[test]
    fn test_module_defs_has_seven_modules() {
        assert_eq!(MODULE_DEFS.len(), 7);
    }

    #[test]
    fn test_module_defs_only_first_is_free() {
        // Per the STREETS design, only the "S" module is marked free in the constant
        let free_modules: Vec<&str> = MODULE_DEFS
            .iter()
            .filter(|(_, _, _, is_free)| *is_free)
            .map(|(id, _, _, _)| *id)
            .collect();
        assert_eq!(free_modules, vec!["S"]);
    }

    #[test]
    fn test_module_defs_ids_match_filename_mapping() {
        // Every module ID in MODULE_DEFS should have a valid filename mapping
        for (id, _, _, _) in MODULE_DEFS {
            assert!(
                module_id_to_filename(id).is_some(),
                "Module ID '{}' should have a filename mapping",
                id
            );
        }
    }

    // ---- get_content_dir test ----

    #[test]
    fn test_get_content_dir_returns_path() {
        let dir = get_content_dir();
        // Should end with docs/streets regardless of whether it exists
        let path_str = dir.to_string_lossy();
        assert!(
            path_str.contains("streets"),
            "Content dir '{}' should contain 'streets'",
            path_str
        );
    }
}
