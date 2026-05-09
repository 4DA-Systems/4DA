// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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

// ---- multilingual lesson heading tests ----

#[test]
fn test_parse_lessons_german() {
    let content = "## Lektion 1: Das Rig-Audit\nInhalt.\n## Lektion 2: Der LLM-Stack\nMehr.";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "German headings must parse");
}

#[test]
fn test_parse_lessons_japanese() {
    let content = "## レッスン 1: リグ監査\n内容。\n## レッスン 2: LLMスタック\n内容。";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "Japanese headings must parse");
}

#[test]
fn test_parse_lessons_chinese() {
    let content = "## 第 1 课：设备审计\n内容。\n## 第 2 课：LLM技术栈\n内容。";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "Chinese headings must parse");
}

#[test]
fn test_parse_lessons_arabic() {
    let content = "## الدرس 1: تدقيق\nمحتوى.\n## الدرس 2: مكدس\nمحتوى.";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "Arabic headings must parse");
}

#[test]
fn test_parse_lessons_spanish() {
    let content = "## Leccion 1: Auditoria\nContenido.\n## Leccion 2: Stack\nContenido.";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "Spanish headings must parse");
}

#[test]
fn test_parse_lessons_korean() {
    let content = "## 레슨 1: 감사\n내용.\n## 레슨 2: 스택\n내용.";
    let lessons = parse_lessons(content);
    assert_eq!(lessons.len(), 2, "Korean headings must parse");
}

#[test]
fn test_is_lesson_heading_rejects_subheadings() {
    assert!(!is_lesson_heading("### Section 1: Details"));
}

#[test]
fn test_is_lesson_heading_rejects_plain_h2() {
    assert!(!is_lesson_heading("## Module Title"));
    assert!(!is_lesson_heading("## What Comes Next"));
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
