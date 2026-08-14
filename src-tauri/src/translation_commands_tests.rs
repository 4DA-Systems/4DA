// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for translation commands — entry/status validation, override CRUD,
//! and edge cases.
//!
//! Split from translation_commands.rs to keep the module under 600 lines.

#[cfg(test)]
mod tests {
    use crate::translation_commands::*;
    use crate::translation_pipeline;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // ========================================================================
    // Test scaffolding
    // ========================================================================

    /// Locale codes are now allowlisted, so filesystem tests can no longer use
    /// throwaway `zz_*` languages — they have to use real ones, which means
    /// they share a directory with whatever the developer running the suite
    /// actually has on disk.
    ///
    /// This guard snapshots one override file, hands the test its path, and
    /// puts the original back on drop (deleting it if there was none). Each
    /// filesystem test claims a DISTINCT locale so the parallel test runner
    /// cannot make two of them collide.
    struct OverrideFileGuard {
        path: PathBuf,
        original: Option<String>,
    }

    impl OverrideFileGuard {
        fn claim(lang: &str, namespace: &str) -> Self {
            let dir = crate::i18n::translations_dir().join("overrides").join(lang);
            std::fs::create_dir_all(&dir).expect("create overrides dir");
            let path = dir.join(format!("{namespace}.json"));
            let original = std::fs::read_to_string(&path).ok();
            let _ = std::fs::remove_file(&path);
            Self { path, original }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }

        fn write(&self, content: &str) {
            std::fs::write(&self.path, content).expect("seed override file");
        }

        fn read(&self) -> String {
            std::fs::read_to_string(&self.path).expect("read override file")
        }

        fn map(&self) -> HashMap<String, String> {
            serde_json::from_str(&self.read()).expect("override file should be a JSON map")
        }
    }

    impl Drop for OverrideFileGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(content) => {
                    let _ = std::fs::write(&self.path, content);
                    return;
                }
                None => {
                    let _ = std::fs::remove_file(&self.path);
                }
            }
            // Walk back up removing directories the suite created, so a test
            // run does not leave `src-tauri/data/translations/` behind in
            // `git status`. `remove_dir` is non-recursive and fails on a
            // non-empty directory, so this can never delete real content.
            let mut dir = self.path.parent().map(std::path::Path::to_path_buf);
            for _ in 0..3 {
                match dir {
                    Some(ref d) if std::fs::remove_dir(d).is_ok() => {
                        dir = d.parent().map(std::path::Path::to_path_buf);
                    }
                    _ => break,
                }
            }
        }
    }

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
        // `de` is a real locale; no overrides dir is created for it here.
        let result = crate::translation_commands::load_overrides("de");
        assert!(result.is_ok());
    }

    #[test]
    fn load_overrides_rejects_unsafe_lang() {
        // Second-layer guard: even an internal caller cannot walk out of the
        // overrides directory.
        assert!(crate::translation_commands::load_overrides("../../etc").is_err());
        assert!(crate::translation_commands::load_overrides("zz\0evil").is_err());
    }

    // ========================================================================
    // Namespace list consistency check
    // ========================================================================

    #[test]
    fn namespace_list_is_consistent() {
        // One canonical namespace list — the override loader, the English
        // loader and the untranslated-key diff all read the same set, so an
        // override can never be writable-but-never-read.
        assert_eq!(crate::i18n::TRANSLATION_NAMESPACES.len(), 3);
        for ns in &crate::i18n::TRANSLATION_NAMESPACES {
            assert!(!ns.is_empty(), "Namespace should not be empty");
            assert!(
                crate::i18n::validate_namespace("ns", ns).is_ok(),
                "canonical namespace {ns} must pass its own validator"
            );
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
        let guard = OverrideFileGuard::claim("hi", "ui");
        guard.write(&"x".repeat(1_000_001));

        let result =
            delete_translation_override("hi".to_string(), "ui".to_string(), "some.key".to_string());

        assert!(result.is_err(), "Should error on files > 1MB");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Override file too large"),
            "Error should mention file size limit"
        );
    }

    #[test]
    fn delete_override_nonexistent_file_returns_ok() {
        let _guard = OverrideFileGuard::claim("ru", "signals");
        let result = delete_translation_override(
            "ru".to_string(),
            "signals".to_string(),
            "some.key".to_string(),
        );
        assert!(
            result.is_ok(),
            "Deleting from nonexistent file should succeed"
        );
    }

    #[test]
    fn delete_override_removes_key_from_file() {
        let guard = OverrideFileGuard::claim("it", "ui");
        let mut map = HashMap::new();
        map.insert("key.to.delete".to_string(), "Delete Me".to_string());
        map.insert("key.to.keep".to_string(), "Keep Me".to_string());
        guard.write(&serde_json::to_string_pretty(&map).unwrap());

        let result = delete_translation_override(
            "it".to_string(),
            "ui".to_string(),
            "key.to.delete".to_string(),
        );
        assert!(result.is_ok());

        let content = guard.map();
        assert!(
            !content.contains_key("key.to.delete"),
            "Deleted key should be gone"
        );
        assert_eq!(content.get("key.to.keep"), Some(&"Keep Me".to_string()));
    }

    /// Regression: this used to assert the OPPOSITE — that a file which failed
    /// to parse was silently replaced with `{}`. That behaviour was the
    /// file-truncation half of the path-injection bug: combined with an
    /// unvalidated `namespace`, ANY readable file that was not a JSON string
    /// map got overwritten with an empty object. Content we cannot parse is
    /// content we must not destroy.
    #[test]
    fn delete_override_refuses_to_clobber_unparseable_file() {
        let guard = OverrideFileGuard::claim("tr", "errors");
        let original = "not valid json {{{";
        guard.write(original);

        let result = delete_translation_override(
            "tr".to_string(),
            "errors".to_string(),
            "some.key".to_string(),
        );

        assert!(
            result.is_err(),
            "Unparseable override file must be an error, not a silent overwrite"
        );
        assert_eq!(
            guard.read(),
            original,
            "The file's contents must survive the failed delete"
        );
    }

    #[test]
    fn delete_override_treats_empty_file_as_empty_map() {
        // Nothing to destroy in a zero-byte file, so this stays recoverable
        // rather than stranding the user behind a permanent error.
        let guard = OverrideFileGuard::claim("ko", "errors");
        guard.write("   \n");

        let result = delete_translation_override(
            "ko".to_string(),
            "errors".to_string(),
            "some.key".to_string(),
        );
        assert!(result.is_ok());
        assert!(guard.map().is_empty());
    }

    // ========================================================================
    // save_translation_override — happy path
    // ========================================================================

    #[test]
    fn save_override_creates_dir_and_file() {
        let guard = OverrideFileGuard::claim("ar", "ui");

        let result = save_translation_override(
            "ar".to_string(),
            "ui".to_string(),
            "test.key".to_string(),
            "Custom Value".to_string(),
        );
        assert!(result.is_ok());
        assert!(
            guard.path().exists(),
            "Override file should have been created"
        );
        assert_eq!(
            guard.map().get("test.key"),
            Some(&"Custom Value".to_string())
        );
    }

    #[test]
    fn save_override_merges_with_existing() {
        let guard = OverrideFileGuard::claim("ja", "ui");
        let mut initial = HashMap::new();
        initial.insert("existing.key".to_string(), "Existing".to_string());
        guard.write(&serde_json::to_string_pretty(&initial).unwrap());

        let result = save_translation_override(
            "ja".to_string(),
            "ui".to_string(),
            "new.key".to_string(),
            "New Override".to_string(),
        );
        assert!(result.is_ok());

        let content = guard.map();
        assert_eq!(content.get("existing.key"), Some(&"Existing".to_string()));
        assert_eq!(content.get("new.key"), Some(&"New Override".to_string()));
    }

    #[test]
    fn save_override_refuses_to_clobber_unparseable_file() {
        let guard = OverrideFileGuard::claim("fr", "signals");
        let original = r#"["a json array, not a string map"]"#;
        guard.write(original);

        let result = save_translation_override(
            "fr".to_string(),
            "signals".to_string(),
            "some.key".to_string(),
            "Value".to_string(),
        );

        assert!(result.is_err(), "Unparseable file must not be overwritten");
        assert_eq!(guard.read(), original, "Contents must survive");
    }

    // ========================================================================
    // SECURITY — path injection via IPC parameters
    //
    // Before the fix, `lang` and `namespace` were joined into the destination
    // path with no validation at all. `Path::join` with an ABSOLUTE component
    // discards everything accumulated so far, so `namespace` alone was a fully
    // controlled write location; `create_dir_all` then built whatever directory
    // chain was named, and both the JSON key and value were caller-supplied.
    // ========================================================================

    /// The headline test. An absolute `namespace` used to relocate the write
    /// entirely — no traversal sequence required.
    #[test]
    fn save_override_rejects_absolute_namespace_and_leaves_target_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let victim = tmp.path().join("victim.json");
        let original = r#"{"important":"do not clobber"}"#;
        std::fs::write(&victim, original).expect("seed victim");

        // `save_translation_override` appends ".json", so the component names
        // the victim without its extension.
        let absolute_component = tmp
            .path()
            .join("victim")
            .to_string_lossy()
            .replace('\\', "/");

        let result = save_translation_override(
            "en".to_string(),
            absolute_component,
            "pwned".to_string(),
            "pwned".to_string(),
        );

        assert!(
            result.is_err(),
            "An absolute path component must be rejected"
        );
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            original,
            "The file outside the overrides directory must be untouched"
        );
    }

    /// The delete variant is the destructive one: it rewrites the target with
    /// the parsed map, so an unvalidated path plus a lenient parse truncated
    /// whatever it pointed at.
    #[test]
    fn delete_override_rejects_absolute_namespace_and_leaves_target_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let victim = tmp.path().join("victim.json");
        let original = "value = \"a TOML file, not a JSON map\"\n";
        std::fs::write(&victim, original).expect("seed victim");

        let absolute_component = tmp
            .path()
            .join("victim")
            .to_string_lossy()
            .replace('\\', "/");

        let result = delete_translation_override(
            "en".to_string(),
            absolute_component,
            "anything".to_string(),
        );

        assert!(result.is_err(), "Absolute component must be rejected");
        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            original,
            "Target must not be truncated to '{{}}'"
        );
    }

    #[test]
    fn override_commands_reject_traversal() {
        for ns in ["../../../etc/passwd", "..", "../ui", "sub/dir"] {
            assert!(
                save_translation_override(
                    "en".to_string(),
                    ns.to_string(),
                    "k".to_string(),
                    "v".to_string()
                )
                .is_err(),
                "namespace {ns:?} must be rejected"
            );
            assert!(
                delete_translation_override("en".to_string(), ns.to_string(), "k".to_string())
                    .is_err(),
                "namespace {ns:?} must be rejected on delete"
            );
        }
        for lang in ["../../../etc", "..", "en/../../x"] {
            assert!(
                save_translation_override(
                    lang.to_string(),
                    "ui".to_string(),
                    "k".to_string(),
                    "v".to_string()
                )
                .is_err(),
                "lang {lang:?} must be rejected"
            );
        }
    }

    #[test]
    fn override_commands_reject_null_bytes() {
        assert!(
            save_translation_override(
                "en".to_string(),
                "ui\0.txt".to_string(),
                "k".to_string(),
                "v".to_string()
            )
            .is_err(),
            "NUL in namespace must be rejected"
        );
        assert!(
            save_translation_override(
                "en\0".to_string(),
                "ui".to_string(),
                "k".to_string(),
                "v".to_string()
            )
            .is_err(),
            "NUL in lang must be rejected"
        );
        assert!(
            save_translation_override(
                "en".to_string(),
                "ui".to_string(),
                "k\0ey".to_string(),
                "v".to_string()
            )
            .is_err(),
            "NUL in key must be rejected"
        );
        assert!(
            save_translation_override(
                "en".to_string(),
                "ui".to_string(),
                "key".to_string(),
                "va\0lue".to_string()
            )
            .is_err(),
            "NUL in value must be rejected"
        );
    }

    #[test]
    fn override_commands_reject_overlong_input() {
        let guard = OverrideFileGuard::claim("es", "ui");

        assert!(
            save_translation_override(
                "en".to_string(),
                "u".repeat(5_000),
                "k".to_string(),
                "v".to_string()
            )
            .is_err(),
            "Over-long namespace must be rejected"
        );
        assert!(
            save_translation_override(
                "es".to_string(),
                "ui".to_string(),
                "k".repeat(5_000),
                "v".to_string()
            )
            .is_err(),
            "Over-long key must be rejected"
        );
        assert!(
            save_translation_override(
                "es".to_string(),
                "ui".to_string(),
                "key".to_string(),
                "v".repeat(100_000)
            )
            .is_err(),
            "Over-long value must be rejected"
        );

        assert!(
            !guard.path().exists(),
            "No rejected call may have created a file"
        );
    }

    /// Pins the deliberate asymmetry between the two locale guards.
    ///
    /// `detect_system_locale` stores whatever the OS reports, so a Dutch or
    /// Polish machine legitimately ends up with `nl` / `pl` in settings. Those
    /// codes must stay *storable* (they are only ever read back through a path
    /// join, which the component guard makes safe) while staying *unwritable*
    /// as translation overrides (which create directories). Tightening
    /// `set_locale` to `SUPPORTED_LOCALES` would silently break locale changes
    /// for those users and fix nothing.
    #[test]
    fn unshipped_system_locales_are_storable_but_not_writable() {
        for lang in ["nl", "pl", "sv", "pt", "cs"] {
            assert!(
                crate::ipc_guard::validate_path_component("language", lang).is_ok(),
                "{lang} is a real OS locale and must remain storable in settings"
            );
            assert!(
                crate::i18n::validate_locale("lang", lang).is_err(),
                "{lang} ships no translations and must not create an overrides dir"
            );
        }
    }

    #[test]
    fn override_commands_reject_unknown_locale() {
        for lang in ["zz", "nl", "pl", "pt", "EN", "en-US", ""] {
            assert!(
                save_translation_override(
                    lang.to_string(),
                    "ui".to_string(),
                    "k".to_string(),
                    "v".to_string()
                )
                .is_err(),
                "unsupported locale {lang:?} must be rejected"
            );
        }
        // ...and every shipped locale is accepted by the same validator.
        for lang in crate::i18n::SUPPORTED_LOCALES {
            assert!(
                crate::i18n::validate_locale("lang", lang).is_ok(),
                "shipped locale {lang} must pass"
            );
        }
    }

    #[test]
    fn override_commands_reject_unknown_namespace() {
        for ns in ["ui.json", "settings", "UI", ""] {
            assert!(
                save_translation_override(
                    "en".to_string(),
                    ns.to_string(),
                    "k".to_string(),
                    "v".to_string()
                )
                .is_err(),
                "unknown namespace {ns:?} must be rejected"
            );
        }
    }

    #[test]
    fn read_only_commands_reject_unsupported_locale() {
        assert!(get_translation_overrides("../../etc".to_string()).is_err());
        assert!(get_all_translations("zz".to_string()).is_err());
        assert!(get_translation_status("../secrets".to_string()).is_err());
    }

    /// Anti-drift: the compile-time allowlist must keep matching what the repo
    /// actually ships. Adding `src/locales/xx/` without adding `xx` here would
    /// otherwise produce a locale the UI offers and the backend rejects.
    #[test]
    fn locale_list_matches_shipped_files() {
        let locales_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("src")
            .join("locales");
        let Ok(entries) = std::fs::read_dir(&locales_dir) else {
            // Not a source checkout (packaged build) — nothing to compare.
            return;
        };

        let mut on_disk: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        on_disk.sort();

        let mut allowlisted: Vec<String> = crate::i18n::SUPPORTED_LOCALES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        allowlisted.sort();

        assert_eq!(
            allowlisted, on_disk,
            "i18n::SUPPORTED_LOCALES has drifted from src/locales/"
        );
    }

    // ========================================================================
    // read_override_map — the parse/clobber distinction, in isolation
    // ========================================================================

    #[test]
    fn read_override_map_distinguishes_empty_map_from_unparseable() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let empty_map = tmp.path().join("empty_map.json");
        std::fs::write(&empty_map, "{}").unwrap();
        assert!(read_override_map(&empty_map).expect("valid").is_empty());

        let blank = tmp.path().join("blank.json");
        std::fs::write(&blank, "").unwrap();
        assert!(read_override_map(&blank)
            .expect("blank is empty")
            .is_empty());

        for bad in ["[]", "null", "42", "{\"k\": 1}", "garbage"] {
            let path = tmp.path().join("bad.json");
            std::fs::write(&path, bad).unwrap();
            assert!(
                read_override_map(&path).is_err(),
                "{bad:?} must not be treated as an empty map"
            );
        }
    }
}
