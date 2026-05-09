// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::Result;
use crate::utils::sanitize_path;

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

/// Return the translated (title, description) for a module ID.
fn module_i18n(id: &str, lang: &str) -> (String, String) {
    let (title_key, desc_key) = match id {
        "S" => ("ui:streets.sovereignSetup", "ui:streets.sovereignSetupDesc"),
        "T" => ("ui:streets.technicalMoats", "ui:streets.technicalMoatsDesc"),
        "R" => ("ui:streets.revenueEngines", "ui:streets.revenueEnginesDesc"),
        "E1" => (
            "ui:streets.executionPlaybook",
            "ui:streets.executionPlaybookDesc",
        ),
        "E2" => ("ui:streets.evolvingEdge", "ui:streets.evolvingEdgeDesc"),
        "T2" => (
            "ui:streets.tacticalAutomation",
            "ui:streets.tacticalAutomationDesc",
        ),
        "S2" => (
            "ui:streets.stackingStreams",
            "ui:streets.stackingStreamsDesc",
        ),
        _ => return (id.to_string(), String::new()),
    };
    (
        crate::i18n::t(title_key, lang, &[]),
        crate::i18n::t(desc_key, lang, &[]),
    )
}

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
    let paths = crate::runtime_paths::RuntimePaths::get();
    let docs_dir = paths.streets_docs_dir();
    if docs_dir.exists() {
        return docs_dir;
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
        if is_lesson_heading(line) {
            // Save previous lesson
            if !current_title.is_empty() {
                lessons.push(PlaybookLesson {
                    title: current_title.clone(),
                    content: current_content.trim().to_string(),
                });
            }
            current_title = line.trim_start_matches('#').trim().to_string();
            current_content = String::new();
        } else if !current_title.is_empty() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }
    if !current_title.is_empty() {
        lessons.push(PlaybookLesson {
            title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    lessons
}

/// Detect lesson headings in any language.
///
/// Matches: "## Lesson 1: ...", "## Lektion 1: ...", "## レッスン 1: ...",
/// "## 第 1 课：...", "## الدرس 1: ..." — any ## heading with a digit and colon.
fn is_lesson_heading(line: &str) -> bool {
    if !line.starts_with("## ") || line.starts_with("### ") {
        return false;
    }
    let after = &line[3..];
    after.chars().any(|c| c.is_ascii_digit()) && (after.contains(':') || after.contains('\u{FF1A}'))
}

#[tauri::command]
pub fn get_playbook_modules(lang: Option<String>) -> Result<Vec<PlaybookModule>> {
    let language = lang.unwrap_or_else(crate::i18n::get_user_language);
    let content_dir = get_content_dir_for_lang(&language);
    let mut modules = Vec::new();

    for (id, _title, _desc, is_free) in MODULE_DEFS {
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

        let (translated_title, translated_desc) = module_i18n(id, &language);
        modules.push(PlaybookModule {
            id: id.to_string(),
            title: translated_title,
            description: translated_desc,
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
        .ok_or_else(|| crate::i18n::t("errors:module.unknown", &language, &[("id", &module_id)]))?;
    let path = content_dir.join(filename);

    if !path.exists() {
        let sanitized = sanitize_path(&path.to_string_lossy());
        return Err(crate::i18n::t(
            "errors:module.fileNotFound",
            &language,
            &[("path", &sanitized)],
        )
        .into());
    }

    let raw = fs::read_to_string(&path)?;

    let lessons = parse_lessons(&raw);

    // Find module metadata
    let (_, _title, _desc, is_free) = MODULE_DEFS
        .iter()
        .find(|(id, _, _, _)| *id == module_id.as_str())
        .ok_or_else(|| crate::i18n::t("errors:module.unknown", &language, &[("id", &module_id)]))?;

    let (translated_title, translated_desc) = module_i18n(&module_id, &language);
    Ok(PlaybookContent {
        module_id,
        title: translated_title,
        description: translated_desc,
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
                let topics = crate::extract_topics(&lesson.title, &lesson.content, &[]);
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

/// Translate a STREETS module's markdown content to the target language.
///
/// Reads the English source, translates via the LLM translation pipeline,
/// and saves to `docs/streets/{lang}/`. Returns the number of lessons translated.
#[tauri::command]
pub async fn translate_playbook_module(module_id: String, lang: String) -> Result<String> {
    use crate::translation_pipeline;

    if lang == "en" {
        return Ok(crate::i18n::t(
            "errors:translation.sourceIsEnglish",
            &lang,
            &[],
        ));
    }

    let filename = module_id_to_filename(&module_id)
        .ok_or_else(|| crate::i18n::t("errors:module.unknown", &lang, &[("id", &module_id)]))?;

    // Read English source
    let base_dir = get_content_dir();
    let source_path = base_dir.join(filename);
    if !source_path.exists() {
        return Err(format!("Source file not found: {}", filename).into());
    }
    let source_content = fs::read_to_string(&source_path)?;

    // Translate markdown
    let translated = translation_pipeline::translate_markdown(&source_content, &lang).await?;

    // Save to localized directory
    let target_dir = base_dir.join(&lang);
    fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(filename);
    fs::write(&target_path, &translated)?;

    let lesson_count = parse_lessons(&translated).len();
    tracing::info!(
        target: "4da::streets",
        module = %module_id,
        lang = %lang,
        lessons = lesson_count,
        "Translated STREETS module"
    );

    Ok(format!(
        "Translated {} ({} lessons) to {}",
        module_id, lesson_count, lang
    ))
}

/// Get available lesson translations for a language.
///
/// Returns a map of module_id -> bool indicating whether translated content exists.
#[tauri::command]
pub fn get_lesson_translation_status(
    lang: String,
) -> Result<std::collections::HashMap<String, bool>> {
    let base_dir = get_content_dir();
    let lang_dir = base_dir.join(&lang);

    let mut status = std::collections::HashMap::new();
    for (id, _, _, _) in MODULE_DEFS {
        let has_translation = if lang == "en" {
            true
        } else if let Some(filename) = module_id_to_filename(id) {
            lang_dir.join(filename).exists()
        } else {
            false
        };
        status.insert(id.to_string(), has_translation);
    }

    Ok(status)
}

#[cfg(test)]
#[path = "playbook_commands_tests.rs"]
mod tests;
