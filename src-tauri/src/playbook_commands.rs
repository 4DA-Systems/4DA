// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! STREETS playbook content helpers.
//!
//! The playbook IPC commands were removed when the STREETS tab was retired from
//! the app (the curriculum now publishes on 4da.ai). What remains is the markdown
//! lesson parser, still used to count lessons for the sovereign developer profile,
//! the personalization context, and the suns module health helpers.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookLesson {
    pub title: String,
    pub content: String,
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

#[cfg(test)]
#[path = "playbook_commands_tests.rs"]
mod tests;
