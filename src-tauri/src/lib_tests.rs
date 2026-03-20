#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    /// Count entries in generate_handler![] by parsing lib.rs source.
    /// This catches accidental deletions of command registrations.
    fn count_handler_entries() -> usize {
        let content = include_str!("lib.rs");
        // Find generate_handler![ ... ] block
        let start = content
            .find("generate_handler![")
            .expect("generate_handler![ not found");
        let block = &content[start..];
        let end = block.find(']').expect("Closing ] not found");
        let handler_block = &block[..end];

        // Count lines with module::function pattern (the actual command entries)
        handler_block
            .lines()
            .filter(|line| {
                let stripped = line.trim();
                // Strip comments
                let code = stripped.split("//").next().unwrap_or("").trim();
                // Must contain :: (module::function) and end with comma or be the entry
                code.contains("::") && !code.starts_with("//")
            })
            .count()
    }

    /// Extract command function names from generate_handler![] block.
    fn extract_handler_names() -> Vec<String> {
        let content = include_str!("lib.rs");
        let start = content
            .find("generate_handler![")
            .expect("generate_handler![ not found");
        let block = &content[start..];
        let end = block.find(']').expect("Closing ] not found");
        let handler_block = &block[..end];

        handler_block
            .lines()
            .filter_map(|line| {
                let stripped = line.trim();
                let code = stripped.split("//").next().unwrap_or("").trim();
                if code.contains("::") && !code.starts_with("//") {
                    // Extract function name after last ::
                    let clean = code.trim_end_matches(',').trim();
                    clean.split("::").last().map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    // ========================================================================
    // Test 1: Handler registration count stays above known minimum
    // ========================================================================

    #[test]
    fn generate_handler_count_above_minimum() {
        let count = count_handler_entries();
        // As of 2026-03-21, there are 309 registered commands.
        // This test catches accidental mass-deletion of registrations.
        // Update the minimum when intentionally removing commands.
        assert!(
            count >= 250,
            "Expected at least 250 handler entries, got {count}. \
             Commands may have been accidentally removed from generate_handler![]."
        );
    }

    // ========================================================================
    // Test 2: No duplicate command registrations
    // ========================================================================

    #[test]
    fn no_duplicate_commands_in_handler() {
        let names = extract_handler_names();
        let mut seen = HashSet::new();
        let mut duplicates = Vec::new();

        for name in &names {
            if !seen.insert(name.as_str()) {
                duplicates.push(name.clone());
            }
        }

        assert!(
            duplicates.is_empty(),
            "Duplicate command registrations found: {:?}",
            duplicates
        );
    }

    // ========================================================================
    // Test 3: Event subscriptions present in setup()
    // ========================================================================

    #[test]
    fn event_subscriptions_registered() {
        let content = include_str!("lib.rs");

        // These are the critical event listeners wired in setup()
        let expected_events = [
            "tray-analyze",
            "tray-toggle-monitoring",
            "scheduled-analysis",
            "deep-link://new-url",
        ];

        for event in &expected_events {
            assert!(
                content.contains(event),
                "Missing event subscription for '{event}' in lib.rs setup()"
            );
        }
    }

    // ========================================================================
    // Test 4: Tray setup failure is non-fatal (warn path, not panic)
    // ========================================================================

    #[test]
    fn tray_setup_failure_handled_gracefully() {
        let content = include_str!("lib.rs");

        // The tray setup must use a match with Ok/Err, not unwrap/expect
        // This ensures tray failures don't crash the app
        assert!(
            content.contains("match monitoring::setup_tray"),
            "Tray setup should use match (not unwrap) for graceful failure"
        );
        assert!(
            content.contains("System tray setup failed, continuing without tray"),
            "Tray failure should log a warning and continue"
        );
        // Verify it stores None on failure (not panicking)
        assert!(
            content.contains("None\n"),
            "Tray failure path should produce None"
        );
    }

    // ========================================================================
    // Test 5: Module declarations compile (structural marker)
    // ========================================================================

    #[test]
    fn module_declarations_are_valid() {
        let content = include_str!("lib.rs");

        // Core modules that must always be declared
        let required_modules = [
            "mod analysis;",
            "mod commands;",
            "mod embeddings;",
            "mod events;",
            "mod state;",
            "mod types;",
            "mod utils;",
            "mod ace;",
            "mod ace_commands;",
            "mod db;",
        ];

        for module in &required_modules {
            assert!(
                content.contains(module),
                "Required module declaration missing: {module}"
            );
        }

        // Count total mod declarations to catch mass deletions
        let mod_count = content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                (trimmed.starts_with("mod ") || trimmed.starts_with("pub mod "))
                    && trimmed.ends_with(';')
                    && !trimmed.contains("//")
            })
            .count();

        assert!(
            mod_count >= 100,
            "Expected at least 100 module declarations, got {mod_count}. \
             Modules may have been accidentally removed."
        );
    }
}
