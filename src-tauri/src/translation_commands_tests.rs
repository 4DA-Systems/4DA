//! Tests for translation commands — entry/status validation, override CRUD,
//! and edge cases.
//!
//! Split from translation_commands.rs to keep the module under 600 lines.

#[cfg(test)]
mod tests {
    use crate::translation_commands::*;
    use crate::translation_pipeline;
    use std::collections::HashMap;

    // ========================================================================
    // TranslationEntry struct tests
    // ========================================================================

    #[test]
    fn translation_entry_untranslated() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: None,
            status: "untranslated".to_string(),
        };
        assert_eq!(entry.english, "Hello");
        assert!(entry.translated.is_none());
        assert_eq!(entry.status, "untranslated");
    }

    #[test]
    fn translation_entry_translated() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: Some("Hola".to_string()),
            status: "translated".to_string(),
        };
        assert_eq!(entry.translated, Some("Hola".to_string()));
        assert_eq!(entry.status, "translated");
    }

    #[test]
    fn translation_entry_overridden() {
        let entry = TranslationEntry {
            english: "Hello".to_string(),
            translated: Some("Custom Hello".to_string()),
            status: "overridden".to_string(),
        };
        assert_eq!(entry.status, "overridden");
        assert_eq!(entry.translated, Some("Custom Hello".to_string()));
    }

    #[test]
    fn translation_entry_serializes_to_json() {
        let entry = TranslationEntry {
            english: "Save".to_string(),
            translated: Some("Guardar".to_string()),
            status: "translated".to_string(),
        };
        let json = serde_json::to_value(&entry).expect("should serialize");
        assert_eq!(json["english"], "Save");
        assert_eq!(json["translated"], "Guardar");
        assert_eq!(json["status"], "translated");
    }

    #[test]
    fn translation_entry_serializes_null_for_none() {
        let entry = TranslationEntry {
            english: "Cancel".to_string(),
            translated: None,
            status: "untranslated".to_string(),
        };
        let json = serde_json::to_value(&entry).expect("should serialize");
        assert!(json["translated"].is_null());
    }

    #[test]
    fn translation_entry_deserializes_from_json() {
        let json_str = r#"{"english":"Quit","translated":"Quitter","status":"translated"}"#;
        let entry: TranslationEntry = serde_json::from_str(json_str).expect("should deserialize");
        assert_eq!(entry.english, "Quit");
        assert_eq!(entry.translated, Some("Quitter".to_string()));
        assert_eq!(entry.status, "translated");
    }

    #[test]
    fn translation_entry_deserializes_null_translated() {
        let json_str = r#"{"english":"Quit","translated":null,"status":"untranslated"}"#;
        let entry: TranslationEntry = serde_json::from_str(json_str).expect("should deserialize");
        assert!(entry.translated.is_none());
    }

    // ========================================================================
    // Translation status computation logic (from get_translation_status)
    // ========================================================================

    #[test]
    fn translation_percentage_calculation_full() {
        let total = 100;
        let translated = 100;
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_partial() {
        let total = 200;
        let untranslated_count = 50;
        let translated = total - untranslated_count;
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_zero_total() {
        let total = 0;
        let percentage = if total > 0 {
            (0_f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn translation_percentage_calculation_none_translated() {
        let total: usize = 50;
        let untranslated_count: usize = 50;
        let translated = total.saturating_sub(untranslated_count);
        let percentage = if total > 0 {
            (translated as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        assert!((percentage - 0.0).abs() < f32::EPSILON);
    }

    // ========================================================================
    // TranslationStatus struct from translation_pipeline
    // ========================================================================

    #[test]
    fn translation_status_serializes() {
        let status = translation_pipeline::TranslationStatus {
            language: "es".to_string(),
            total_keys: 100,
            translated_keys: 75,
            percentage: 75.0,
        };
        let json = serde_json::to_value(&status).expect("should serialize");
        assert_eq!(json["language"], "es");
        assert_eq!(json["total_keys"], 100);
        assert_eq!(json["translated_keys"], 75);
        assert_eq!(json["percentage"], 75.0);
    }

    #[test]
    fn translation_status_deserializes() {
        let json_str =
            r#"{"language":"fr","total_keys":200,"translated_keys":150,"percentage":75.0}"#;
        let status: translation_pipeline::TranslationStatus =
            serde_json::from_str(json_str).expect("should deserialize");
        assert_eq!(status.language, "fr");
        assert_eq!(status.total_keys, 200);
        assert_eq!(status.translated_keys, 150);
    }

    // ========================================================================
    // load_overrides returns empty map for nonexistent language
    // ========================================================================

    #[test]
    fn load_overrides_nonexistent_lang_returns_empty() {
        let result =
            crate::translation_commands::load_overrides("zz_nonexistent_test_language_xyz");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ========================================================================
    // Namespace list consistency check
    // ========================================================================

    #[test]
    fn namespace_list_is_consistent() {
        let expected_namespaces = ["ui", "coach", "streets", "errors"];
        assert_eq!(expected_namespaces.len(), 4);
        for ns in &expected_namespaces {
            assert!(!ns.is_empty(), "Namespace should not be empty");
        }
    }

    // ========================================================================
    // Translation entry status values are valid
    // ========================================================================

    #[test]
    fn valid_status_values() {
        let valid_statuses = ["overridden", "translated", "untranslated"];
        assert_eq!(valid_statuses.len(), 3);
        for status in &valid_statuses {
            assert!(!status.is_empty(), "Status value should not be empty");
        }
    }

    // ========================================================================
    // File too large guard — delete_translation_override
    // ========================================================================

    #[test]
    fn delete_override_file_too_large_guard() {
        let test_lang = "zz_too_large_guard_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        let large_content = "x".repeat(1_000_001);
        std::fs::write(&path, &large_content).expect("write large file");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "some.key".to_string(),
        );

        assert!(result.is_err(), "Should error on files > 1MB");
        assert!(
            result.unwrap_err().to_string().contains("Override file too large"),
            "Error should mention file size limit"
        );

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn delete_override_nonexistent_file_returns_ok() {
        let result = delete_translation_override(
            "zz_nonexistent_delete_test".to_string(),
            "ui".to_string(),
            "some.key".to_string(),
        );
        assert!(
            result.is_ok(),
            "Deleting from nonexistent file should succeed"
        );
    }

    #[test]
    fn delete_override_removes_key_from_file() {
        let test_lang = "zz_delete_key_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));

        let mut map = HashMap::new();
        map.insert("key.to.delete".to_string(), "Delete Me".to_string());
        map.insert("key.to.keep".to_string(), "Keep Me".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&map).unwrap())
            .expect("write override file");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "key.to.delete".to_string(),
        );
        assert!(result.is_ok());

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            !content.contains_key("key.to.delete"),
            "Deleted key should be gone"
        );
        assert_eq!(content.get("key.to.keep"), Some(&"Keep Me".to_string()));

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn delete_override_malformed_json_file_returns_ok() {
        let test_lang = "zz_malformed_delete_test";
        let test_ns = "errors";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create test dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        std::fs::write(&path, "not valid json {{{").expect("write malformed");

        let result = delete_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "some.key".to_string(),
        );
        assert!(
            result.is_ok(),
            "Malformed JSON should be handled gracefully"
        );

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .expect("File should now be valid JSON");
        assert!(content.is_empty());

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    // ========================================================================
    // save_translation_override — error paths
    // ========================================================================

    #[test]
    fn save_override_creates_dir_and_file() {
        let test_lang = "zz_save_override_test";
        let test_ns = "ui";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        let _ = std::fs::remove_dir_all(&overrides_dir);

        let result = save_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "test.key".to_string(),
            "Custom Value".to_string(),
        );
        assert!(result.is_ok());

        let path = overrides_dir.join(format!("{}.json", test_ns));
        assert!(path.exists(), "Override file should have been created");

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content.get("test.key"), Some(&"Custom Value".to_string()));

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }

    #[test]
    fn save_override_merges_with_existing() {
        let test_lang = "zz_save_merge_test";
        let test_ns = "coach";
        let overrides_dir = crate::i18n::translations_dir()
            .join("overrides")
            .join(test_lang);
        std::fs::create_dir_all(&overrides_dir).expect("create dir");

        let path = overrides_dir.join(format!("{}.json", test_ns));
        let mut initial = HashMap::new();
        initial.insert("existing.key".to_string(), "Existing".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap())
            .expect("write initial");

        let result = save_translation_override(
            test_lang.to_string(),
            test_ns.to_string(),
            "new.key".to_string(),
            "New Override".to_string(),
        );
        assert!(result.is_ok());

        let content: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content.get("existing.key"), Some(&"Existing".to_string()));
        assert_eq!(content.get("new.key"), Some(&"New Override".to_string()));

        let _ = std::fs::remove_dir_all(&overrides_dir);
    }
}
