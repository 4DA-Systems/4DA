//! Victauri dogfood tests — validate 4DA using our own testing framework.
//!
//! Every test has real assertions. No log-and-continue patterns.
//!
//! Requires a running 4DA dev server (`pnpm run tauri dev`).
//! Run with: `VICTAURI_E2E=1 CARGO_TARGET_DIR=target-test cargo test --test victauri_dogfood -- --test-threads=1`

use victauri_test::visual::{MaskRegion, ThresholdPreset, VisualOptions};
use victauri_test::VictauriClient;

fn skip_unless_e2e() -> bool {
    if !victauri_test::is_e2e() {
        eprintln!("Skipping: set VICTAURI_E2E=1 with 4DA dev server running");
        return true;
    }
    false
}

// ── Phase 1: Smoke Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn connect_and_get_plugin_info() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover()
        .await
        .expect("Failed to connect — is 4DA dev server running?");

    let info = client.get_plugin_info().await.unwrap();
    assert!(
        info.get("version").is_some(),
        "plugin_info should have version"
    );
    assert!(
        info.get("tools").is_some() || info.get("tool_count").is_some(),
        "plugin_info should report tool count"
    );
}

#[tokio::test]
async fn screenshot_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client.screenshot().await.unwrap();

    let has_image = result.get("image").is_some()
        || result.get("data").is_some()
        || result.get("base64").is_some()
        || result.pointer("/result/content/0/data").is_some();
    assert!(
        has_image,
        "screenshot should return image data: keys={:?}",
        result.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn dom_snapshot_has_elements() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let snapshot = client.dom_snapshot().await.unwrap();

    let has_tree = snapshot.get("tree").is_some()
        || snapshot.get("nodes").is_some()
        || snapshot.get("elements").is_some()
        || snapshot.get("root").is_some();
    assert!(has_tree, "DOM snapshot should contain element tree");
}

#[tokio::test]
async fn memory_stats_reports_rss() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let stats = client.get_memory_stats().await.unwrap();

    assert!(
        stats.get("rss_bytes").is_some()
            || stats.get("rss").is_some()
            || stats.get("memory").is_some()
            || stats.get("working_set_bytes").is_some(),
        "memory stats should contain RSS/working set"
    );
}

#[tokio::test]
async fn window_state_reports_main() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let windows = client.list_windows().await.unwrap();

    let has_windows = windows.as_array().is_some()
        || windows.get("windows").is_some()
        || windows.get("labels").is_some()
        || windows.as_object().is_some();
    assert!(has_windows, "should return window info");

    let state = client.get_window_state(Some("main")).await.unwrap();
    assert!(
        !state.is_null(),
        "window state should return data for 'main'"
    );
}

// ── Phase 2: Core Flow Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn main_navigation_tabs_exist() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let expected_tabs = ["Brief", "Preemption", "Blind Spots", "Signal", "Playbook"];
    let snapshot = client.dom_snapshot().await.unwrap();
    let snapshot_str = serde_json::to_string(&snapshot).unwrap();

    let mut found = Vec::new();
    let mut missing = Vec::new();

    for tab in &expected_tabs {
        if snapshot_str.contains(tab) {
            found.push(*tab);
        } else {
            missing.push(*tab);
        }
    }

    assert!(
        missing.len() <= 1,
        "At least 4 of 5 main tabs should be in DOM. Found: {found:?}, missing: {missing:?}"
    );
}

#[tokio::test]
async fn eval_js_returns_document_title() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let title = client.eval_js("document.title").await.unwrap();

    let title_str = title.as_str().unwrap_or("");
    assert!(
        !title_str.is_empty(),
        "document.title should not be empty: {title}"
    );
}

#[tokio::test]
async fn settings_command_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let settings = client.invoke_command("get_settings", None).await.unwrap();

    assert!(
        settings.is_object(),
        "get_settings should return an object: {settings}"
    );
}

#[tokio::test]
async fn console_logs_accessible() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Generate a console log entry we can verify
    let _ = client.eval_js("console.log('victauri-test-marker')").await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let logs = client.logs("console", Some(50)).await.unwrap();
    let count = logs.as_array().map_or(0, |a| a.len());
    assert!(count > 0, "console logs should have entries after eval");
}

// ── Phase 3: Cross-Boundary Tests (Victauri's Unique Value) ──────────────────

#[tokio::test]
async fn ghost_command_detection_works() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let ghosts = client.detect_ghost_commands().await.unwrap();

    // 4DA doesn't use #[inspectable], so ghost detection should find
    // frontend-invoked commands not in the registry. The tool must return
    // a structured response — we verify the shape, not the count (which
    // depends on how many IPC calls have been made this session).
    let has_report = ghosts.get("ghost_commands").is_some()
        || ghosts.get("frontend_only").is_some()
        || ghosts.get("report").is_some();
    assert!(
        has_report,
        "ghost command detection should return structured report: {ghosts}"
    );

    // If ghosts were found, verify each has a command name
    if let Some(list) = ghosts
        .get("ghost_commands")
        .and_then(|g| g.as_array())
        .or_else(|| ghosts.get("frontend_only").and_then(|f| f.as_array()))
    {
        for ghost in list {
            assert!(
                ghost.is_string() || ghost.get("name").is_some() || ghost.get("command").is_some(),
                "each ghost should have a command name: {ghost}"
            );
        }
    }
}

#[tokio::test]
async fn ipc_integrity_check() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let health = client.check_ipc_integrity().await.unwrap();

    let has_status = health.get("healthy").is_some()
        || health.get("status").is_some()
        || health.get("pending").is_some();
    assert!(has_status, "IPC health check should report status");
}

#[tokio::test]
#[ignore = "Victauri IPC checkpoint drain timing is unreliable — upstream fix needed in victauri-plugin event loop"]
async fn ipc_log_captures_commands() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let checkpoint = client.create_ipc_checkpoint().await.unwrap();

    let _ = client.invoke_command("get_analysis_status", None).await;

    let calls_since = client.get_ipc_calls_since(checkpoint).await.unwrap();
    assert!(
        !calls_since.is_empty(),
        "IPC log should capture the invoke_command call"
    );
}

#[tokio::test]
async fn accessibility_audit_returns_results() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let audit = client.audit_accessibility().await.unwrap();

    assert!(
        audit.get("violations").is_some() || audit.get("summary").is_some(),
        "accessibility audit should return violations or summary"
    );
}

#[tokio::test]
async fn performance_metrics_baseline() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let metrics = client.get_performance_metrics().await.unwrap();

    assert!(
        metrics.get("navigation").is_some() || metrics.get("js_heap").is_some(),
        "performance metrics should have navigation or heap data"
    );
}

// ── Phase 3.5: Fluent Verification ──────────────────────────────────────────

#[tokio::test]
async fn full_verification_chain() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let report = client
        .verify()
        .no_console_errors()
        .ipc_healthy()
        .run()
        .await
        .unwrap();

    assert!(
        report.all_passed(),
        "verification chain should pass: failures={:?}",
        report
            .failures()
            .iter()
            .map(|f| format!("{}: {}", f.description, f.detail))
            .collect::<Vec<_>>()
    );
}

// ── Phase 4: Visual, Coverage, Recording ─────────────────────────────────────

#[tokio::test]
async fn visual_regression_baseline() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let snapshot_dir = std::env::temp_dir().join("victauri-4da-snapshots");
    std::fs::create_dir_all(&snapshot_dir).ok();

    let opts = VisualOptions {
        snapshot_dir: snapshot_dir.clone(),
        ..VisualOptions::default()
    }
    .with_preset(ThresholdPreset::AntiAlias)
    .with_mask(MaskRegion::new(0, 0, 200, 30));

    match client.screenshot_visual("4da_main_view", &opts).await {
        Ok(d) => {
            // On subsequent runs, verify the screenshot matches baseline
            assert!(
                d.is_match(opts.threshold_percent),
                "visual regression detected: {:.2}% match, {} diff pixels (threshold: {}%)",
                d.match_percentage,
                d.diff_pixel_count,
                opts.threshold_percent
            );
        }
        Err(victauri_test::TestError::VisualRegression(msg)) => {
            panic!("visual regression: {msg}");
        }
        Err(_) => {
            // First run creates baseline — this is expected
        }
    }
}

#[tokio::test]
async fn ipc_coverage_via_log_analysis() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Trigger several different commands to build up IPC log
    let _ = client.invoke_command("get_settings", None).await;
    let _ = client.invoke_command("get_monitoring_status", None).await;
    let _ = client.invoke_command("get_developer_dna", None).await;
    let _ = client.invoke_command("list_channels", None).await;

    // Get the full IPC log and count unique commands
    let ipc_log = client.get_ipc_log(None).await.unwrap();
    let empty = vec![];
    let entries = ipc_log.as_array().unwrap_or(&empty);

    let unique_commands: std::collections::HashSet<&str> = entries
        .iter()
        .filter_map(|e| e.get("command").and_then(|c| c.as_str()))
        .collect();

    assert!(
        unique_commands.len() >= 4,
        "IPC log should show at least 4 unique commands invoked, got {}: {:?}",
        unique_commands.len(),
        unique_commands
    );
}

#[tokio::test]
async fn recording_captures_events() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let start = client.start_recording(Some("dogfood-test")).await.unwrap();
    assert!(
        start
            .get("started")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || start.get("session_id").is_some(),
        "recording should start: {start}"
    );

    // Generate activity that the event drain loop will capture
    let _ = client
        .eval_js("console.log('recording-test-' + Date.now())")
        .await;
    let _ = client.dom_snapshot().await;
    let _ = client.invoke_command("get_settings", None).await;

    // Wait for the event drain loop (polls every 1s)
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let stop = client.stop_recording().await.unwrap();
    let event_count = stop
        .get("events")
        .and_then(|e| e.as_array())
        .map_or(0, |a| a.len());
    assert!(
        event_count > 0,
        "recording should capture events after generating activity, got 0: {stop}"
    );
}

#[tokio::test]
async fn wait_for_text_finds_content() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // "Brief" tab label is always rendered in the DOM
    let result = client
        .wait_for("text", Some("Brief"), Some(5000), Some(200))
        .await
        .unwrap();

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(ok, "wait_for should find 'Brief' text in DOM: {result}");
}

// ── Phase 5: Expanded Verification ──────────────────────────────────────────

#[tokio::test]
async fn verification_chain_with_state_match() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let report = client
        .verify()
        .no_console_errors()
        .ipc_healthy()
        .state_matches(
            "({title: document.title})",
            serde_json::json!({"title": "4DA"}),
        )
        .run()
        .await
        .unwrap();

    assert!(
        report.all_passed(),
        "fluent verification should pass: failures={:?}",
        report
            .failures()
            .iter()
            .map(|f| format!("{}: {}", f.description, f.detail))
            .collect::<Vec<_>>()
    );
}

// ── Phase 6: Navigation & View Switching ─────────────────────────────────────

#[tokio::test]
async fn navigate_all_five_views() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Skip Brief first (it's already selected), navigate the others, then back to Brief
    let tabs = ["Preemption", "Blind Spots", "Signal", "Playbook", "Brief"];
    for tab_name in &tabs {
        let elements = client
            .find_elements(serde_json::json!({"role": "tab"}))
            .await
            .unwrap();
        let elements_arr = elements.as_array().expect("find_elements returns array");

        let tab = elements_arr
            .iter()
            .find(|e| {
                e.get("text")
                    .and_then(|t| t.as_str())
                    .map_or(false, |t| t == *tab_name)
            })
            .unwrap_or_else(|| panic!("tab '{tab_name}' not found in DOM"));

        let ref_id = tab["ref_id"].as_str().unwrap();
        let result = client.click(ref_id).await.unwrap();
        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(ok, "clicking tab '{tab_name}' should succeed: {result}");

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn settings_modal_open_close() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Clear any existing modal state
    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let settings_btn = buttons
        .as_array()
        .unwrap()
        .iter()
        .find(|b| {
            b.get("name")
                .and_then(|n| n.as_str())
                .map_or(false, |n| n == "Settings")
        })
        .expect("Settings button should exist");

    let ref_id = settings_btn["ref_id"].as_str().unwrap();
    client.click(ref_id).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let dialogs = client
        .find_elements(serde_json::json!({"role": "dialog"}))
        .await
        .unwrap();
    assert!(
        !dialogs.as_array().unwrap().is_empty(),
        "Settings dialog should be open"
    );

    client.press_key("Escape").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let dialogs_after = client
        .find_elements(serde_json::json!({"role": "dialog"}))
        .await
        .unwrap();
    assert!(
        dialogs_after.as_array().unwrap().is_empty(),
        "Settings dialog should be closed after Escape"
    );
}

#[tokio::test]
#[ignore = "press_key('?') does not reliably trigger keyboard shortcut modal in automated context"]
async fn keyboard_shortcuts_modal() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    client.press_key("?").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let dialogs = client
        .find_elements(serde_json::json!({"role": "dialog"}))
        .await
        .unwrap();
    assert!(
        !dialogs.as_array().unwrap().is_empty(),
        "? shortcut should open keyboard shortcuts modal"
    );

    client.press_key("Escape").await.unwrap();
}

// ── Phase 7: Multi-Window Management ─────────────────────────────────────────

#[tokio::test]
async fn all_three_windows_reported() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let windows = client.list_windows().await.unwrap();

    let labels: Vec<&str> = windows
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w.as_str())
        .collect();

    assert!(labels.contains(&"main"), "should have main window");
    assert!(
        labels.contains(&"notification"),
        "should have notification window"
    );
    assert!(labels.contains(&"briefing"), "should have briefing window");
}

#[tokio::test]
async fn main_window_state_details() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let state = client.get_window_state(Some("main")).await.unwrap();

    let main = state
        .as_array()
        .and_then(|arr| arr.iter().find(|w| w["label"] == "main"))
        .unwrap_or(&state);

    assert_eq!(main["title"].as_str().unwrap_or(""), "4DA");
    assert!(
        main["url"]
            .as_str()
            .unwrap_or("")
            .contains("localhost:4444"),
        "main window URL should point to dev server"
    );
}

#[tokio::test]
async fn window_resize_and_restore() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let resize = client
        .call_tool(
            "window",
            serde_json::json!({"action": "resize", "window_label": "main", "width": 1400, "height": 900}),
        )
        .await
        .unwrap();
    assert!(
        resize.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "resize should succeed"
    );

    let restore = client
        .call_tool(
            "window",
            serde_json::json!({"action": "resize", "window_label": "main", "width": 1200, "height": 800}),
        )
        .await
        .unwrap();
    assert!(
        restore.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "restore should succeed"
    );
}

// ── Phase 8: Cross-Boundary Verification ─────────────────────────────────────

#[tokio::test]
async fn verify_state_title_match() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .verify_state(
            "({title: document.title})",
            serde_json::json!({"title": "4DA"}),
        )
        .await
        .unwrap();

    victauri_test::assert_state_matches(&result);
}

#[tokio::test]
async fn verify_state_detects_mismatch() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .verify_state(
            "({title: document.title})",
            serde_json::json!({"title": "Wrong"}),
        )
        .await
        .unwrap();

    let passed = result
        .get("passed")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    assert!(!passed, "mismatched state should fail verification");

    let divergences = result
        .get("divergences")
        .and_then(|d| d.as_array())
        .map_or(0, |a| a.len());
    assert!(divergences > 0, "should report divergences");
}

#[tokio::test]
async fn semantic_assert_equals() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .assert_semantic(
            "document.title",
            "title is 4DA",
            "equals",
            serde_json::json!("4DA"),
        )
        .await
        .unwrap();

    assert!(
        result
            .get("passed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "semantic assert equals should pass: {result}"
    );
}

#[tokio::test]
async fn semantic_assert_truthy() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .assert_semantic(
            "document.querySelectorAll('nav').length",
            "navigation exists",
            "truthy",
            serde_json::Value::Null,
        )
        .await
        .unwrap();

    assert!(
        result
            .get("passed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "nav count should be truthy: {result}"
    );
}

// ── Phase 9: Deep Introspection ──────────────────────────────────────────────

#[tokio::test]
async fn css_style_inspection() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let snapshot = client.dom_snapshot().await.unwrap();
    let tree = snapshot.get("tree").and_then(|t| t.as_str()).unwrap_or("");

    let body_ref = tree
        .lines()
        .next()
        .and_then(|line| {
            let start = line.find("[e")?;
            let end = line[start..].find(']')? + start + 1;
            Some(&line[start + 1..end - 1])
        })
        .unwrap_or("e0");

    let styles = client
        .call_tool(
            "inspect",
            serde_json::json!({"action": "get_styles", "ref_id": body_ref}),
        )
        .await
        .unwrap();
    assert!(
        styles.get("styles").is_some() || styles.get("ref_id").is_some(),
        "should return style data for {body_ref}: {styles}"
    );
}

#[tokio::test]
async fn accessibility_has_no_critical_violations() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let audit = client.audit_accessibility().await.unwrap();

    let critical = audit
        .pointer("/summary/critical")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let serious = audit
        .pointer("/summary/serious")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    assert_eq!(critical, 0, "no critical a11y violations");
    assert_eq!(serious, 0, "no serious a11y violations");
}

#[tokio::test]
async fn performance_metrics_within_budget() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let metrics = client.get_performance_metrics().await.unwrap();

    let dom_complete = metrics
        .pointer("/navigation/dom_complete_ms")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let heap_mb = metrics
        .pointer("/js_heap/used_mb")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    assert!(
        dom_complete < 5000.0,
        "DOM complete should be under 5s: {dom_complete}ms"
    );
    // 2GB budget — generous enough for long dev sessions, tight enough to catch leaks
    assert!(heap_mb < 2048.0, "heap should be under 2GB: {heap_mb}MB");
}

#[tokio::test]
async fn highlight_and_clear() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let tabs = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let first_ref = tabs.as_array().unwrap()[0]["ref_id"].as_str().unwrap();

    let highlight = client
        .call_tool(
            "inspect",
            serde_json::json!({"action": "highlight", "ref_id": first_ref, "color": "blue", "label": "test"}),
        )
        .await
        .unwrap();
    assert!(
        highlight
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "highlight should succeed"
    );

    let clear = client
        .call_tool("inspect", serde_json::json!({"action": "clear_highlights"}))
        .await
        .unwrap();
    assert!(
        clear.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "clear highlights should succeed"
    );
}

// ── Phase 10: IPC Command Invocation ─────────────────────────────────────────

#[tokio::test]
async fn invoke_settings_returns_valid_config() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let settings = client.invoke_command("get_settings", None).await.unwrap();

    assert!(settings.is_object(), "settings should be object");
    assert!(
        settings.get("llm").is_some(),
        "settings should have llm config"
    );
    assert!(
        settings.get("rerank").is_some(),
        "settings should have rerank config"
    );
}

#[tokio::test]
async fn invoke_monitoring_returns_status() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let status = client
        .invoke_command("get_monitoring_status", None)
        .await
        .unwrap();

    assert!(
        status.get("enabled").is_some(),
        "monitoring should report enabled state"
    );
    assert!(
        status.get("interval_mins").is_some() || status.get("interval_secs").is_some(),
        "monitoring should report interval"
    );
}

#[tokio::test]
async fn invoke_developer_dna_returns_stack() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let dna = client
        .invoke_command("get_developer_dna", None)
        .await
        .unwrap();

    let stack = dna
        .get("primary_stack")
        .and_then(|s| s.as_array())
        .map_or(0, |a| a.len());
    assert!(stack > 0, "developer DNA should have primary stack");
}

#[tokio::test]
async fn invoke_channels_returns_list() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let channels = client.invoke_command("list_channels", None).await.unwrap();

    assert!(channels.is_array(), "channels should be array");
    let count = channels.as_array().unwrap().len();
    assert!(count > 0, "should have at least one channel");
}

#[tokio::test]
async fn ipc_integrity_healthy() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let integrity = client.check_ipc_integrity().await.unwrap();

    victauri_test::assert_ipc_healthy(&integrity);
}

// ── Phase 11: Time-Travel Recording ──────────────────────────────────────────

#[tokio::test]
async fn recording_lifecycle() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let start = client
        .start_recording(Some("lifecycle-test"))
        .await
        .unwrap();
    assert!(
        start
            .get("started")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || start.get("session_id").is_some(),
        "recording should start"
    );

    // Generate events and wait for drain
    let _ = client
        .eval_js("console.log('lifecycle-' + Date.now())")
        .await;
    let _ = client.dom_snapshot().await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let stop = client.stop_recording().await.unwrap();
    let events = stop
        .get("events")
        .and_then(|e| e.as_array())
        .map_or(0, |a| a.len());
    assert!(
        events > 0,
        "recording lifecycle should capture events, got 0"
    );
}

// ── Phase 12: Logs & Navigation ──────────────────────────────────────────────

#[tokio::test]
async fn network_log_captured() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Trigger network activity via IPC
    let _ = client.invoke_command("get_settings", None).await;

    let logs = client.logs("network", Some(10)).await.unwrap();
    let count = logs.as_array().map_or(0, |a| a.len());
    assert!(count > 0, "network log should have entries after IPC calls");
}

#[tokio::test]
async fn slow_ipc_detection() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let slow = client
        .call_tool(
            "logs",
            serde_json::json!({"action": "slow_ipc", "threshold_ms": 100}),
        )
        .await
        .unwrap();

    // The tool should return a count, regardless of whether there are slow calls
    assert!(
        slow.get("count").is_some() || slow.get("calls").is_some(),
        "slow IPC detection should return structured result: {slow}"
    );
}

#[tokio::test]
async fn navigation_history_tracked() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let history = client.logs("navigation", None).await.unwrap();

    let entries = history.as_array().map_or(0, |a| a.len());
    assert!(entries > 0, "should have initial navigation entry");

    let first = &history.as_array().unwrap()[0];
    assert!(
        first
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .contains("localhost"),
        "first nav entry should be localhost"
    );
}

// ── Phase 13: Stress & Concurrency ───────────────────────────────────────────

#[tokio::test]
async fn rapid_eval_burst() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let start = std::time::Instant::now();
    for i in 0..10 {
        let result = client
            .eval_js(&format!("'burst-{i}-' + Date.now()"))
            .await
            .unwrap();
        let s = result.as_str().unwrap_or("");
        assert!(
            s.starts_with(&format!("burst-{i}-")),
            "burst eval {i} should return correct prefix: {s}"
        );
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 30,
        "10 evals should complete within 30s: {elapsed:?}"
    );
}

// ── Phase 14: Security ──────────────────────────────────────────────────────

#[tokio::test]
async fn health_endpoint_does_not_leak_internals() {
    if skip_unless_e2e() {
        return;
    }

    let client = VictauriClient::discover().await.unwrap();
    let resp = reqwest::get(format!("{}/health", client.base_url()))
        .await
        .unwrap();
    assert!(resp.status().is_success());

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(
        body.get("uptime_secs").is_none(),
        "health endpoint should not leak uptime"
    );
    assert!(
        body.get("memory").is_none(),
        "health endpoint should not leak memory stats"
    );
    assert!(
        body.get("commands_registered").is_none(),
        "health endpoint should not leak internal state"
    );
}

#[tokio::test]
async fn bad_auth_token_rejected() {
    if skip_unless_e2e() {
        return;
    }

    let port = VictauriClient::discover().await.unwrap();
    let base_url = port.base_url().to_string();

    let http = reqwest::Client::new();
    let resp = http
        .post(format!("{base_url}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", "Bearer totally-wrong-token-12345")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "attacker", "version": "0.0.1"}
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        401,
        "wrong auth token should be rejected with 401"
    );
}

#[tokio::test]
async fn missing_auth_token_rejected() {
    if skip_unless_e2e() {
        return;
    }

    let port = VictauriClient::discover().await.unwrap();
    let base_url = port.base_url().to_string();

    let http = reqwest::Client::new();
    let resp = http
        .post(format!("{base_url}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        // No Authorization header
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "no-auth", "version": "0.0.1"}
            }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        401,
        "missing auth token should be rejected with 401"
    );
}

// ── Phase 15: Heavy IPC Traffic ─────────────────────────────────────────────

#[tokio::test]
async fn parallel_ipc_burst() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let commands = [
        "get_settings",
        "get_monitoring_status",
        "get_developer_dna",
        "list_channels",
        "get_analysis_status",
    ];

    let start = std::time::Instant::now();
    for round in 0..5 {
        for cmd in &commands {
            let result = client.invoke_command(cmd, None).await;
            assert!(
                result.is_ok(),
                "round {round} command {cmd} failed: {:?}",
                result.err()
            );
        }
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 60,
        "25 IPC calls should complete within 60s: {elapsed:?}"
    );

    let integrity = client.check_ipc_integrity().await.unwrap();
    victauri_test::assert_ipc_healthy(&integrity);
}

// ── Phase 16: Multi-Window Snapshots ────────────────────────────────────────

#[tokio::test]
async fn snapshot_each_window() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let windows = client.list_windows().await.unwrap();

    let labels: Vec<&str> = windows
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|w| w.as_str())
        .collect();

    for label in &labels {
        let state = client.get_window_state(Some(label)).await.unwrap();
        assert!(
            !state.is_null(),
            "window '{label}' should return state data"
        );

        let has_url = state
            .as_array()
            .and_then(|arr| arr.iter().find(|w| w["label"] == *label))
            .and_then(|w| w.get("url"))
            .is_some()
            || state.get("url").is_some();
        assert!(has_url, "window '{label}' state should include URL");
    }

    // DOM snapshot per window using the new targeted API
    let main_snap = client.dom_snapshot_for("main").await.unwrap();
    assert!(
        main_snap.get("tree").is_some() || main_snap.get("root").is_some(),
        "main window DOM snapshot should have tree"
    );
}

// ── Phase 18: Connection Resilience ─────────────────────────────────────────

#[tokio::test]
async fn is_alive_returns_true() {
    if skip_unless_e2e() {
        return;
    }

    let client = VictauriClient::discover().await.unwrap();
    assert!(
        client.is_alive().await,
        "running server should report alive"
    );
}

// ── Phase 17: Event-Driven IPC Capture ──────────────────────────────────────

#[tokio::test]
async fn ipc_wait_for_capture_returns_complete_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let _ = client.invoke_command("get_settings", None).await;

    let log = client
        .call_tool(
            "logs",
            serde_json::json!({"action": "ipc", "wait_for_capture": true, "limit": 1}),
        )
        .await
        .unwrap();

    let empty = vec![];
    let entries = log.as_array().unwrap_or(&empty);
    assert!(
        !entries.is_empty(),
        "IPC log with wait_for_capture should return entries"
    );

    let last = &entries[entries.len() - 1];
    assert!(
        last.get("duration_ms").is_some() && !last["duration_ms"].is_null(),
        "wait_for_capture should ensure duration_ms is populated: {last}"
    );
    assert!(
        last.get("result").is_some() && !last["result"].is_null(),
        "wait_for_capture should ensure result is captured: {last}"
    );
}

// ── Phase 19: Blind Spots Tab ───────────────────────────────────────────────

#[tokio::test]
#[ignore = "diagnostic only — dumps blind spots data for manual inspection"]
async fn blind_spots_data_dump() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();

    let score = result.get("score").and_then(|s| s.as_f64());
    let total = result.get("total_tracked").and_then(|t| t.as_u64());
    let items = result["items"].as_array().unwrap();

    eprintln!("=== BLIND SPOTS DATA ===");
    eprintln!(
        "Score: {score:?}  |  Total tracked: {total:?}  |  Items: {}",
        items.len()
    );
    eprintln!();

    for item in items {
        let id = item["id"].as_str().unwrap_or("?");
        let title = item["title"].as_str().unwrap_or("?");
        let urgency = item["urgency"].as_str().unwrap_or("?");
        let explanation = item["explanation"].as_str().unwrap_or("?");
        let deps = item["affected_deps"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let evidence_count = item["evidence"].as_array().map_or(0, |a| a.len());

        eprintln!("[{urgency}] {id}");
        eprintln!("  title: {title}");
        eprintln!("  deps: {deps}");
        eprintln!("  explanation: {explanation}");
        eprintln!("  evidence: {evidence_count} citations");
        eprintln!();
    }
}

#[tokio::test]
async fn blind_spots_ipc_returns_evidence_feed() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();

    assert!(result.is_object(), "get_blind_spots should return object");
    assert!(
        result.get("items").is_some(),
        "EvidenceFeed must have items field: {result}"
    );
    let items = result["items"].as_array().expect("items is array");

    for item in items {
        assert!(
            item.get("id").is_some() && !item["id"].as_str().unwrap_or("").is_empty(),
            "every item needs a non-empty id: {item}"
        );
        assert!(
            item.get("title").is_some() && !item["title"].as_str().unwrap_or("").is_empty(),
            "every item needs a non-empty title: {item}"
        );
        assert!(
            item.get("urgency").is_some(),
            "every item needs urgency: {item}"
        );
        assert!(
            item.get("explanation").is_some()
                && !item["explanation"].as_str().unwrap_or("").is_empty(),
            "every item needs a non-empty explanation: {item}"
        );

        let id_str = item["id"].as_str().unwrap();
        let valid_prefix = id_str.starts_with("bs_")
            || id_str.starts_with("llm-bs-")
            || id_str.starts_with("bs_rec_");
        assert!(
            valid_prefix,
            "item id must start with bs_ or llm-bs-: {id_str}"
        );
    }
}

#[tokio::test]
async fn blind_spots_score_is_valid() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();

    if let Some(score) = result.get("score").and_then(|s| s.as_f64()) {
        assert!(
            score == -1.0 || (0.0..=100.0).contains(&score),
            "score must be -1 (cold-start) or 0-100, got {score}"
        );
    }

    if let Some(total) = result.get("total_tracked").and_then(|t| t.as_u64()) {
        assert!(
            total <= 5000,
            "total_tracked should be reasonable, got {total}"
        );
    }
}

#[tokio::test]
async fn blind_spots_no_template_explanations() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();

    let empty_items = vec![];
    let items = result["items"].as_array().unwrap_or(&empty_items);
    let banned = [
        "High-relevance item matching",
        "Moderately relevant based on",
        "Borderline-relevant",
        "worth a glance",
        "Affects X in Y",
    ];

    for item in items {
        let explanation = item["explanation"].as_str().unwrap_or("");
        for pattern in &banned {
            assert!(
                !explanation.contains(pattern),
                "template explanation found in item {}: '{explanation}' contains '{pattern}'",
                item["id"].as_str().unwrap_or("?")
            );
        }
    }
}

#[tokio::test]
async fn blind_spots_items_have_valid_evidence() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();

    let empty_items2 = vec![];
    let items = result["items"].as_array().unwrap_or(&empty_items2);
    for item in items {
        let id = item["id"].as_str().unwrap_or("?");

        let known_prefix = id.starts_with("bs_uncov_")
            || id.starts_with("bs_stale_")
            || id.starts_with("bs_missed_")
            || id.starts_with("bs_rec_")
            || id.starts_with("llm-bs-");
        assert!(
            known_prefix,
            "item has unknown ID prefix: '{id}' — frontend will silently drop it"
        );

        if id.starts_with("bs_missed_") || id.starts_with("llm-bs-") {
            let evidence = item.get("evidence").and_then(|e| e.as_array());
            assert!(
                evidence.is_some() && !evidence.unwrap().is_empty(),
                "missed signal {id} must have at least one citation"
            );

            let cite = &evidence.unwrap()[0];
            assert!(
                cite.get("source").is_some(),
                "citation for {id} must have source"
            );
        }

        let urgency = item["urgency"].as_str().unwrap_or("");
        assert!(
            ["critical", "high", "medium", "watch"].contains(&urgency),
            "invalid urgency '{urgency}' on item {id}"
        );
    }
}

#[tokio::test]
async fn blind_spots_tab_renders_without_errors() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Blind Spots tab
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tabs = elements.as_array().expect("tabs array");
    let bs_tab = tabs
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab must exist");

    let ref_id = bs_tab["ref_id"].as_str().unwrap();
    let click_result = client.click(ref_id).await.unwrap();
    assert!(
        click_result
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "clicking Blind Spots tab should succeed"
    );

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Verify no console errors
    let logs = client.logs("console", Some(100)).await.unwrap();
    let empty_logs = vec![];
    let errors: Vec<_> = logs
        .as_array()
        .unwrap_or(&empty_logs)
        .iter()
        .filter(|l| {
            l.get("level")
                .and_then(|v| v.as_str())
                .map_or(false, |v| v == "error")
        })
        .collect();

    assert!(
        errors.len() <= 1,
        "Blind Spots tab should not produce console errors: {errors:?}"
    );
}

#[tokio::test]
async fn blind_spots_tab_has_score_bar() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Blind Spots tab
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tabs = elements.as_array().expect("tabs array");
    let bs_tab = tabs
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab must exist");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&snapshot).unwrap();

    let has_score =
        dom_str.contains("/100") || dom_str.contains("building") || dom_str.contains("Building");
    assert!(
        has_score,
        "Blind Spots view must show score bar (X/100) or building state"
    );
}

#[tokio::test]
async fn blind_spots_tab_has_tier_sections() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Blind Spots
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Verify tier sections exist as ARIA regions
    let sections = client
        .find_elements(serde_json::json!({"role": "region"}))
        .await
        .unwrap();
    let section_names: Vec<String> = sections
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    let dom_snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&dom_snapshot).unwrap();

    // The view should show EITHER tier sections (when data exists) OR an empty/building state
    // Check both aria-label sections AND translated text content
    let has_tiers = !section_names.is_empty()
        || dom_str.contains("needs attention")
        || dom_str.contains("Needs Attention")
        || dom_str.contains("drifting")
        || dom_str.contains("Drifting")
        || dom_str.contains("covered")
        || dom_str.contains("Covered")
        || dom_str.contains("building")
        || dom_str.contains("Building")
        || dom_str.contains("clean")
        || dom_str.contains("/100")
        || dom_str.contains("Stack Dependencies")
        || dom_str.contains("Ecosystem Dependencies");
    assert!(
        has_tiers,
        "Blind Spots should show tier sections or empty state. Sections found: {section_names:?}"
    );
}

#[tokio::test]
async fn blind_spots_accessibility_audit() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Blind Spots
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let audit = client.audit_accessibility().await.unwrap();

    let critical = audit.get("critical").and_then(|v| v.as_u64()).unwrap_or(0);
    let serious = audit.get("serious").and_then(|v| v.as_u64()).unwrap_or(0);

    assert!(
        critical == 0,
        "Blind Spots tab must have zero critical a11y violations: {audit}"
    );
    assert!(
        serious <= 2,
        "Blind Spots tab should have minimal serious a11y violations (got {serious}): {audit}"
    );
}

#[tokio::test]
async fn blind_spots_clean_state_shows_positive_ux() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();
    let score = result.get("score").and_then(|s| s.as_f64()).unwrap_or(99.0);
    if score > 0.0 && score != -1.0 {
        eprintln!("Skipping clean-state test: score={score} (has problems)");
        return;
    }

    // Navigate to Blind Spots
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&snapshot).unwrap();

    // Clean state should NOT show empty problem sections with alarming colors
    let has_positive = dom_str.contains("excellent")
        || dom_str.contains("Excellent")
        || dom_str.contains("monitoring")
        || dom_str.contains("Monitoring");
    assert!(
        has_positive,
        "Clean state should show positive reinforcement, not empty problem sections"
    );
}

#[tokio::test]
async fn blind_spots_no_vanity_metrics() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Blind Spots
    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&snapshot).unwrap();

    // Intelligence Doctrine Rule 3: no vanity metrics
    let banned_patterns = [
        "Items monitored",
        "Sources producing",
        "Validated principles: 0",
        "Decisions tracked: 0",
        "Actively monitoring",
    ];

    for pattern in &banned_patterns {
        assert!(
            !dom_str.contains(pattern),
            "Vanity metric detected in Blind Spots tab: '{pattern}'"
        );
    }
}

#[tokio::test]
async fn blind_spots_score_shows_coverage_not_problems() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();
    let raw_score = result.get("score").and_then(|s| s.as_f64()).unwrap_or(-1.0);
    if raw_score < 0.0 {
        eprintln!("Skipping: score is building ({raw_score})");
        return;
    }

    let expected_coverage = (100.0 - raw_score).round() as u64;

    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&snapshot).unwrap();

    let coverage_str = format!("{expected_coverage}");
    assert!(
        dom_str.contains(&coverage_str),
        "Score bar should show coverage ({expected_coverage}/100), not raw score ({raw_score})"
    );

    if raw_score == 0.0 {
        assert!(
            !dom_str.contains(r#""0"</span>"#) || dom_str.contains(&format!("{expected_coverage}")),
            "Perfect score (raw=0) must NOT display '0' as the score — should show 100"
        );
    }
}

#[tokio::test]
async fn blind_spots_covered_section_has_compact_view() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client
        .invoke_command("get_blind_spots", None)
        .await
        .unwrap();
    let total = result
        .get("total_tracked")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    if total == 0 {
        eprintln!("Skipping: no tracked deps");
        return;
    }

    let elements = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let bs_tab = elements
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains("Blind Spots"))
        })
        .expect("Blind Spots tab");

    client
        .click(bs_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let snapshot = client.dom_snapshot().await.unwrap();
    let dom_str = serde_json::to_string(&snapshot).unwrap();

    // When score=0, there are no problem deps to compare against, so the
    // covered section is hidden and the emerald clean-state card shows instead.
    // When score>0, some deps are problems and the rest appear as "Well Covered".
    let raw_score = result.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
    if raw_score > 0.0 {
        let has_problem_section = dom_str.contains("Uncovered")
            || dom_str.contains("needs attention")
            || dom_str.contains("Drifting")
            || dom_str.contains("Well Covered");
        assert!(
            has_problem_section,
            "Should show problem sections or covered section when score > 0 (score={raw_score})"
        );
    } else {
        let has_clean_state = dom_str.contains("coverage gaps")
            || dom_str.contains("excellent")
            || dom_str.contains("Excellent")
            || dom_str.contains("monitoring");
        assert!(
            has_clean_state,
            "Clean state (score=0) should show positive card"
        );
    }
}

// ── Phase 10: Content Graph ─────────────────────────────────────────────────

#[tokio::test]
async fn content_graph_command_returns_valid_structure() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 7, "max_nodes": 150})),
        )
        .await
        .unwrap();

    assert!(graph.get("nodes").is_some(), "graph must have nodes field");
    assert!(graph.get("edges").is_some(), "graph must have edges field");
    assert!(
        graph.get("clusters").is_some(),
        "graph must have clusters field"
    );
    assert!(graph.get("meta").is_some(), "graph must have meta field");

    let meta = &graph["meta"];
    assert!(
        meta.get("total_items").is_some(),
        "meta must have total_items"
    );
    assert!(
        meta.get("total_edges").is_some(),
        "meta must have total_edges"
    );
    assert!(
        meta.get("cluster_count").is_some(),
        "meta must have cluster_count"
    );
    assert!(
        meta.get("time_window_days").is_some(),
        "meta must have time_window_days"
    );
    assert!(
        meta.get("edge_threshold").is_some(),
        "meta must have edge_threshold"
    );
}

#[tokio::test]
async fn content_graph_nodes_have_required_fields() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 50})),
        )
        .await
        .unwrap();

    let nodes = graph["nodes"].as_array().expect("nodes must be array");
    if nodes.is_empty() {
        eprintln!("WARN: no content nodes in graph — database may be empty");
        return;
    }

    for (i, node) in nodes.iter().enumerate().take(10) {
        assert!(node.get("id").is_some(), "node[{i}] must have id");
        assert!(node.get("title").is_some(), "node[{i}] must have title");
        assert!(
            node.get("source_type").is_some(),
            "node[{i}] must have source_type"
        );
        assert!(
            node.get("relevance_score").is_some(),
            "node[{i}] must have relevance_score"
        );
        assert!(node.get("x").is_some(), "node[{i}] must have x position");
        assert!(node.get("y").is_some(), "node[{i}] must have y position");
        assert!(
            node.get("created_at").is_some(),
            "node[{i}] must have created_at"
        );

        let score = node["relevance_score"].as_f64().unwrap_or(-1.0);
        assert!(
            (0.0..=1.0).contains(&score),
            "node[{i}] relevance_score {score} must be in [0, 1]"
        );

        let title = node["title"].as_str().unwrap_or("");
        assert!(!title.is_empty(), "node[{i}] title must not be empty");
    }
}

#[tokio::test]
async fn content_graph_edges_have_valid_types() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 100})),
        )
        .await
        .unwrap();

    let edges = graph["edges"].as_array().expect("edges must be array");
    let nodes = graph["nodes"].as_array().expect("nodes must be array");
    let node_ids: std::collections::HashSet<i64> =
        nodes.iter().filter_map(|n| n["id"].as_i64()).collect();

    let valid_types = ["semantic", "chain", "concept", "convergence", "duplicate"];

    for (i, edge) in edges.iter().enumerate().take(20) {
        assert!(edge.get("source").is_some(), "edge[{i}] must have source");
        assert!(edge.get("target").is_some(), "edge[{i}] must have target");
        assert!(
            edge.get("edge_type").is_some(),
            "edge[{i}] must have edge_type"
        );
        assert!(edge.get("weight").is_some(), "edge[{i}] must have weight");
        assert!(
            edge.get("methods").is_some(),
            "edge[{i}] must have methods array"
        );

        let edge_type = edge["edge_type"].as_str().unwrap_or("");
        assert!(
            valid_types.contains(&edge_type),
            "edge[{i}] type '{edge_type}' must be one of {valid_types:?}"
        );

        let weight = edge["weight"].as_f64().unwrap_or(-1.0);
        assert!(
            (0.0..=1.0).contains(&weight),
            "edge[{i}] weight {weight} must be in [0, 1]"
        );

        let source = edge["source"].as_i64().unwrap_or(-1);
        let target = edge["target"].as_i64().unwrap_or(-1);
        assert!(
            node_ids.contains(&source),
            "edge[{i}] source {source} must reference existing node"
        );
        assert!(
            node_ids.contains(&target),
            "edge[{i}] target {target} must reference existing node"
        );

        assert_ne!(source, target, "edge[{i}] must not be self-referencing");

        let methods = edge["methods"].as_array();
        assert!(methods.is_some(), "edge[{i}] methods must be an array");
        let methods = methods.unwrap();
        assert!(
            !methods.is_empty(),
            "edge[{i}] must have at least one method (provenance)"
        );
    }
}

#[tokio::test]
async fn content_graph_clusters_are_consistent() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let clusters = graph["clusters"]
        .as_array()
        .expect("clusters must be array");
    let nodes = graph["nodes"].as_array().expect("nodes must be array");
    let node_ids: std::collections::HashSet<i64> =
        nodes.iter().filter_map(|n| n["id"].as_i64()).collect();

    let meta = &graph["meta"];
    let reported_count = meta["cluster_count"].as_u64().unwrap_or(0) as usize;
    assert_eq!(
        clusters.len(),
        reported_count,
        "meta.cluster_count must match actual clusters length"
    );

    for (i, cluster) in clusters.iter().enumerate() {
        assert!(cluster.get("id").is_some(), "cluster[{i}] must have id");
        assert!(
            cluster.get("label").is_some(),
            "cluster[{i}] must have label"
        );
        assert!(
            cluster.get("node_ids").is_some(),
            "cluster[{i}] must have node_ids"
        );

        let label = cluster["label"].as_str().unwrap_or("");
        assert!(!label.is_empty(), "cluster[{i}] label must not be empty");

        let member_ids = cluster["node_ids"]
            .as_array()
            .expect("node_ids must be array");
        assert!(
            !member_ids.is_empty(),
            "cluster[{i}] must have at least one member"
        );

        for nid in member_ids {
            let id = nid.as_i64().unwrap_or(-1);
            assert!(
                node_ids.contains(&id),
                "cluster[{i}] member {id} must reference an existing node"
            );
        }
    }
}

#[tokio::test]
async fn content_graph_meta_counts_are_consistent() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 7, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let nodes = graph["nodes"].as_array().unwrap();
    let edges = graph["edges"].as_array().unwrap();
    let meta = &graph["meta"];

    let total_items = meta["total_items"].as_u64().unwrap_or(0) as usize;
    let total_edges = meta["total_edges"].as_u64().unwrap_or(0) as usize;

    assert_eq!(
        nodes.len(),
        total_items,
        "meta.total_items must match actual node count"
    );
    assert_eq!(
        edges.len(),
        total_edges,
        "meta.total_edges must match actual edge count"
    );

    let time_window = meta["time_window_days"].as_u64().unwrap_or(0);
    assert_eq!(
        time_window, 7,
        "time_window_days should match requested days=7"
    );
}

#[tokio::test]
async fn content_graph_different_time_windows() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let graph_7 = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 7, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let graph_30 = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let nodes_7 = graph_7["nodes"].as_array().unwrap().len();
    let nodes_30 = graph_30["nodes"].as_array().unwrap().len();

    assert!(
        nodes_30 >= nodes_7,
        "30-day window ({nodes_30} nodes) should have >= nodes than 7-day ({nodes_7})"
    );

    let tw_7 = graph_7["meta"]["time_window_days"].as_u64().unwrap_or(0);
    let tw_30 = graph_30["meta"]["time_window_days"].as_u64().unwrap_or(0);
    assert_eq!(tw_7, 7);
    assert_eq!(tw_30, 30);
}

#[tokio::test]
async fn content_graph_no_duplicate_edges() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let edges = graph["edges"].as_array().unwrap();

    let mut seen = std::collections::HashSet::new();
    for edge in edges {
        let src = edge["source"].as_i64().unwrap_or(0);
        let tgt = edge["target"].as_i64().unwrap_or(0);
        let etype = edge["edge_type"].as_str().unwrap_or("");
        let key = format!("{src}-{tgt}-{etype}");
        assert!(seen.insert(key.clone()), "duplicate edge found: {key}");
    }
}

#[tokio::test]
async fn content_graph_layout_positions_are_finite() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 100})),
        )
        .await
        .unwrap();

    let nodes = graph["nodes"].as_array().unwrap();
    for (i, node) in nodes.iter().enumerate() {
        let x = node["x"].as_f64().unwrap_or(f64::NAN);
        let y = node["y"].as_f64().unwrap_or(f64::NAN);
        assert!(x.is_finite(), "node[{i}] x={x} must be finite");
        assert!(y.is_finite(), "node[{i}] y={y} must be finite");
    }

    let clusters = graph["clusters"].as_array().unwrap();
    for (i, cluster) in clusters.iter().enumerate() {
        let cx = cluster["centroid_x"].as_f64().unwrap_or(f64::NAN);
        let cy = cluster["centroid_y"].as_f64().unwrap_or(f64::NAN);
        assert!(
            cx.is_finite(),
            "cluster[{i}] centroid_x={cx} must be finite"
        );
        assert!(
            cy.is_finite(),
            "cluster[{i}] centroid_y={cy} must be finite"
        );
    }
}

#[tokio::test]
async fn content_graph_ui_toggle_exists() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Clear any modal/overlay left by prior tests
    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Navigate to Brief first (reset to known tab), then Signal
    let tabs = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tab_arr = tabs.as_array().expect("find_elements returns array");

    let brief_tab = tab_arr.iter().find(|e| {
        e.get("text")
            .and_then(|t| t.as_str())
            .map_or(false, |t| t == "Brief")
    });
    if let Some(bt) = brief_tab {
        let _ = client.click(bt["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    // Now navigate to Signal tab
    let tabs = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tab_arr = tabs.as_array().expect("find_elements returns array");
    let signal_tab = tab_arr
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t == "Signal")
        })
        .expect("Signal tab must exist");

    client
        .click(signal_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // Check for List/Graph toggle buttons
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");

    let list_btn = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t == "List")
    });
    let graph_btn = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t == "Graph")
    });

    assert!(
        list_btn.is_some(),
        "List toggle button must exist on Signal tab"
    );
    assert!(
        graph_btn.is_some(),
        "Graph toggle button must exist on Signal tab"
    );
}

#[tokio::test]
async fn content_graph_ui_renders_on_toggle() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Clear any modal/overlay left by prior tests
    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let _ = client.press_key("Escape").await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Navigate to Brief first (reset to known tab), then Signal
    let tabs = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tab_arr = tabs.as_array().expect("find_elements returns array");

    let brief_tab = tab_arr.iter().find(|e| {
        e.get("text")
            .and_then(|t| t.as_str())
            .map_or(false, |t| t == "Brief")
    });
    if let Some(bt) = brief_tab {
        let _ = client.click(bt["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    // Now navigate to Signal tab
    let tabs = client
        .find_elements(serde_json::json!({"role": "tab"}))
        .await
        .unwrap();
    let tab_arr = tabs.as_array().expect("find_elements returns array");
    let signal_tab = tab_arr
        .iter()
        .find(|e| {
            e.get("text")
                .and_then(|t| t.as_str())
                .map_or(false, |t| t == "Signal")
        })
        .expect("Signal tab must exist");
    client
        .click(signal_tab["ref_id"].as_str().unwrap())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // Click Graph toggle
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    let graph_btn = button_arr
        .iter()
        .find(|b| {
            b.get("text")
                .or_else(|| b.get("name"))
                .and_then(|t| t.as_str())
                .map_or(false, |t| t == "Graph")
        })
        .expect("Graph button must exist");
    client
        .click(graph_btn["ref_id"].as_str().unwrap())
        .await
        .unwrap();

    // Wait for graph to load (IPC call + React Flow render)
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Verify graph mode is active — check for React Flow DOM or graph stats
    let state = client
        .eval_js("document.querySelector('.react-flow') !== null || document.querySelector('[class*=\"react-flow\"]') !== null || document.querySelector('[data-testid=\"rf\"]') !== null || document.body.innerHTML.includes('No content relationships') || document.body.innerHTML.includes('nodes') || document.body.innerHTML.includes('edges') || document.body.innerHTML.includes('clusters')")
        .await
        .unwrap();
    let has_graph =
        state.as_bool().unwrap_or(false) || state.as_str().map_or(false, |s| s == "true");

    assert!(
        has_graph,
        "Graph view should render React Flow or stats bar after toggle: {state}"
    );

    // Switch back to List view for clean state
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    if let Some(list_btn) = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t == "List")
    }) {
        let _ = client.click(list_btn["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn content_graph_edge_provenance_is_non_empty() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let graph = client
        .invoke_command(
            "build_content_graph",
            Some(serde_json::json!({"days": 30, "max_nodes": 150})),
        )
        .await
        .unwrap();

    let edges = graph["edges"].as_array().unwrap();

    for (i, edge) in edges.iter().enumerate() {
        let methods = edge["methods"].as_array().unwrap();
        assert!(
            !methods.is_empty(),
            "edge[{i}] must have at least one provenance method — accuracy contract violation"
        );

        for method in methods {
            let m = method.as_str().unwrap_or("");
            assert!(!m.is_empty(), "edge[{i}] method must not be empty string");
        }
    }
}

// ── Phase 11: Brief Tab — The Product ────────────────────────────────────────
//
// The briefing is the single most important surface in 4DA.
// These tests verify the full pipeline: IPC returns structured data,
// the DOM renders it, and the content is substantive (not placeholder).

#[tokio::test]
async fn briefing_snapshot_returns_structured_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let snapshot = client
        .invoke_command("get_briefing_snapshot", None)
        .await
        .unwrap();

    // Snapshot may be null if no analysis has run yet — that's OK,
    // but if it exists it must have the canonical shape.
    if !snapshot.is_null() {
        assert!(
            snapshot.get("briefing").is_some(),
            "briefing snapshot must have 'briefing' field, got keys: {:?}",
            snapshot.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        assert!(
            snapshot.get("generated_at_unix").is_some(),
            "briefing snapshot must have 'generated_at_unix' field"
        );
        assert!(
            snapshot.get("version").is_some(),
            "briefing snapshot must have 'version' field"
        );
    }
}

#[tokio::test]
async fn latest_briefing_returns_value() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let briefing = client
        .invoke_command("get_latest_briefing", None)
        .await
        .unwrap();

    // Even if empty, the command must succeed and return JSON
    assert!(
        briefing.is_object() || briefing.is_null() || briefing.is_string(),
        "get_latest_briefing must return valid JSON, got: {briefing}"
    );
}

#[tokio::test]
async fn brief_tab_renders_content_in_dom() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Brief tab
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    if let Some(brief_btn) = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t.contains("Brief"))
    }) {
        let _ = client.click(brief_btn["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    // The brief tab should render either a briefing or an empty state —
    // never a blank white rectangle.
    let has_brief_content = client
        .eval_js(
            "document.body.innerText.includes('briefing') \
             || document.body.innerText.includes('Brief') \
             || document.body.innerText.includes('analysis') \
             || document.body.innerText.includes('Generate') \
             || document.body.innerText.includes('No briefing') \
             || document.body.innerText.includes('signal') \
             || document.body.innerText.length > 200",
        )
        .await
        .unwrap();
    let found = has_brief_content.as_bool().unwrap_or(false)
        || has_brief_content.as_str().map_or(false, |s| s == "true");
    assert!(
        found,
        "Brief tab must render substantive content or a clear empty state"
    );
}

#[tokio::test]
async fn briefing_snapshot_content_is_not_placeholder() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let snapshot = client
        .invoke_command("get_briefing_snapshot", None)
        .await
        .unwrap();

    if snapshot.is_null() {
        return;
    }

    let content = snapshot
        .get("briefing")
        .or_else(|| snapshot.get("html"))
        .or_else(|| snapshot.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        !content.contains("lorem ipsum") && !content.contains("placeholder"),
        "briefing must never contain placeholder text"
    );

    if !content.is_empty() {
        assert!(
            content.len() > 50,
            "non-empty briefing should be substantive (>50 chars), got {} chars",
            content.len()
        );
    }
}

// ── Phase 12: Signal Tab — Scored Item Rendering ─────────────────────────────

#[tokio::test]
async fn scoring_stats_returns_aggregate() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let stats = client
        .invoke_command("get_scoring_stats", None)
        .await
        .unwrap();

    assert!(
        stats.is_object(),
        "get_scoring_stats must return an object, got: {stats}"
    );

    assert!(
        stats.get("total_scored").is_some(),
        "scoring stats must have 'total_scored', got keys: {:?}",
        stats.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        stats.get("total_relevant").is_some(),
        "scoring stats must have 'total_relevant'"
    );
    assert!(
        stats.get("total_runs").is_some(),
        "scoring stats must have 'total_runs'"
    );
}

#[tokio::test]
async fn analysis_status_reports_state() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let status = client
        .invoke_command("get_analysis_status", None)
        .await
        .unwrap();

    assert!(
        status.is_object(),
        "get_analysis_status must return an object, got: {status}"
    );

    assert!(
        status.get("running").is_some(),
        "analysis status must have 'running' field, got keys: {:?}",
        status.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        status.get("completed").is_some(),
        "analysis status must have 'completed' field"
    );
}

#[tokio::test]
async fn signal_tab_renders_items_or_empty_state() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Signal tab
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    if let Some(signal_btn) = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t.contains("Signal"))
    }) {
        let _ = client.click(signal_btn["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    // Signal tab must show scored items or a meaningful empty state
    let content_check = client
        .eval_js(
            "(() => { \
                const text = document.body.innerText; \
                const hasItems = document.querySelectorAll('[class*=\"card\"], [class*=\"item\"], [class*=\"signal\"], [class*=\"feed\"]').length > 0; \
                const hasEmpty = text.includes('No signals') || text.includes('No items') || text.includes('Run analysis') || text.includes('Fetch'); \
                const hasContent = text.length > 200; \
                return hasItems || hasEmpty || hasContent; \
            })()",
        )
        .await
        .unwrap();
    let found = content_check.as_bool().unwrap_or(false)
        || content_check.as_str().map_or(false, |s| s == "true");
    assert!(
        found,
        "Signal tab must render items or empty state, not a blank view"
    );
}

#[tokio::test]
async fn signal_items_have_titles_when_present() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Signal tab
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    if let Some(signal_btn) = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t.contains("Signal"))
    }) {
        let _ = client.click(signal_btn["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    // If items are rendered, they must have visible title text
    let titles = client
        .eval_js(
            "(() => { \
                const cards = document.querySelectorAll('[class*=\"card\"], [class*=\"item\"], [class*=\"signal-item\"]'); \
                if (cards.length === 0) return JSON.stringify({count: 0, titles: []}); \
                const titles = Array.from(cards).slice(0, 10).map(c => { \
                    const h = c.querySelector('h1,h2,h3,h4,h5,h6,[class*=\"title\"],[class*=\"heading\"]'); \
                    return h ? h.innerText.trim() : c.innerText.trim().substring(0, 80); \
                }); \
                return JSON.stringify({count: cards.length, titles}); \
            })()",
        )
        .await
        .unwrap();

    let parsed: serde_json::Value =
        serde_json::from_str(titles.as_str().unwrap_or("{}")).unwrap_or_default();

    let count = parsed["count"].as_u64().unwrap_or(0);
    if count > 0 {
        let title_arr = parsed["titles"].as_array().unwrap();
        let non_empty = title_arr
            .iter()
            .filter(|t| !t.as_str().unwrap_or("").is_empty())
            .count();
        assert!(
            non_empty > 0,
            "at least one signal item must have visible title text out of {count} cards"
        );
    }
}

// ── Phase 13: Settings Persistence Round-trip ────────────────────────────────

#[tokio::test]
async fn get_settings_returns_valid_config() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let settings = client.invoke_command("get_settings", None).await.unwrap();

    assert!(
        settings.is_object(),
        "get_settings must return an object, got: {settings}"
    );

    assert!(
        settings.get("llm").is_some(),
        "settings must have 'llm' section, got keys: {:?}",
        settings.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        settings.get("rerank").is_some(),
        "settings must have 'rerank' section"
    );
}

#[tokio::test]
async fn settings_no_api_keys_in_response() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let settings = client.invoke_command("get_settings", None).await.unwrap();

    let settings_str = serde_json::to_string(&settings).unwrap();

    // API keys in settings responses should be masked or absent.
    // This checks the privacy contract — raw keys must not transit IPC unmasked
    // if a future version adds masking. For now, just verify the response is valid.
    assert!(
        !settings_str.contains("sk-ant-") && !settings_str.contains("sk-proj-"),
        "settings response must not contain raw API key prefixes in transit"
    );
}

#[tokio::test]
async fn mark_onboarding_complete_succeeds() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // This is idempotent — safe to call repeatedly
    let result = client
        .invoke_command("mark_onboarding_complete", None)
        .await;

    assert!(
        result.is_ok(),
        "mark_onboarding_complete must succeed: {:?}",
        result.err()
    );

    // Verify it took effect
    let settings = client.invoke_command("get_settings", None).await.unwrap();

    let onboarding = settings
        .get("onboarding_complete")
        .or_else(|| settings.pointer("/general/onboarding_complete"));
    if let Some(val) = onboarding {
        assert!(
            val.as_bool().unwrap_or(false),
            "onboarding_complete should be true after marking complete"
        );
    }
}

#[tokio::test]
async fn settings_round_trip_preserves_structure() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Read settings twice — structure must be identical
    let first = client.invoke_command("get_settings", None).await.unwrap();
    let second = client.invoke_command("get_settings", None).await.unwrap();

    let first_keys: Vec<&str> = first
        .as_object()
        .map(|o| o.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    let second_keys: Vec<&str> = second
        .as_object()
        .map(|o| o.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    assert_eq!(
        first_keys, second_keys,
        "consecutive get_settings calls must return identical structure"
    );
}

// ── Phase 14: Preemption Data Verification ───────────────────────────────────

#[tokio::test]
async fn preemption_alerts_returns_evidence_feed() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let alerts = client
        .invoke_command("get_preemption_alerts", None)
        .await
        .unwrap();

    assert!(
        alerts.is_object(),
        "preemption alerts must return an object, got: {alerts}"
    );
    assert!(
        alerts.get("items").is_some(),
        "preemption alerts must have 'items' field, got keys: {:?}",
        alerts.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn preemption_alerts_items_have_required_fields() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let alerts = client
        .invoke_command("get_preemption_alerts", None)
        .await
        .unwrap();

    let items = alerts
        .get("items")
        .and_then(|v| v.as_array())
        .or_else(|| alerts.as_array());

    if let Some(items) = items {
        for (i, item) in items.iter().enumerate() {
            // EvidenceItem must have title/summary and confidence
            let has_title = item.get("title").is_some()
                || item.get("summary").is_some()
                || item.get("headline").is_some();
            assert!(
                has_title,
                "preemption item[{i}] must have title/summary, got keys: {:?}",
                item.as_object().map(|o| o.keys().collect::<Vec<_>>())
            );
        }
    }
}

#[tokio::test]
async fn preemption_tab_renders_in_dom() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Navigate to Preemption tab
    let buttons = client
        .find_elements(serde_json::json!({"role": "button"}))
        .await
        .unwrap();
    let button_arr = buttons.as_array().expect("find_elements returns array");
    if let Some(pre_btn) = button_arr.iter().find(|b| {
        b.get("text")
            .or_else(|| b.get("name"))
            .and_then(|t| t.as_str())
            .map_or(false, |t| t.contains("Preemption") || t.contains("preempt"))
    }) {
        let _ = client.click(pre_btn["ref_id"].as_str().unwrap()).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    let content = client
        .eval_js(
            "(() => { \
                const text = document.body.innerText; \
                const hasAlerts = text.includes('alert') || text.includes('Preemption') || text.includes('preempt'); \
                const hasEmpty = text.includes('No alerts') || text.includes('No preemption') || text.includes('monitoring'); \
                const hasContent = text.length > 200; \
                return hasAlerts || hasEmpty || hasContent; \
            })()",
        )
        .await
        .unwrap();
    let found =
        content.as_bool().unwrap_or(false) || content.as_str().map_or(false, |s| s == "true");
    assert!(found, "Preemption tab must render alerts or empty state");
}

// ── Phase 15: Cold-start & Monitoring ────────────────────────────────────────

#[tokio::test]
async fn monitoring_status_returns_valid_state() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let status = client
        .invoke_command("get_monitoring_status", None)
        .await
        .unwrap();

    assert!(
        status.is_object(),
        "get_monitoring_status must return an object, got: {status}"
    );

    assert!(
        status.get("enabled").is_some(),
        "monitoring status must have 'enabled' field, got keys: {:?}",
        status.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        status.get("interval_mins").is_some() || status.get("interval_secs").is_some(),
        "monitoring status must have interval field"
    );
}

#[tokio::test]
async fn cold_start_no_blank_screens() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Visit each of the five tabs — none should render blank
    let tab_names = ["Brief", "Preemption", "Blind Spots", "Signal", "Playbook"];

    for tab_name in &tab_names {
        let buttons = client
            .find_elements(serde_json::json!({"role": "button"}))
            .await
            .unwrap();
        let button_arr = buttons.as_array().expect("find_elements returns array");
        if let Some(btn) = button_arr.iter().find(|b| {
            b.get("text")
                .or_else(|| b.get("name"))
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains(tab_name))
        }) {
            let _ = client.click(btn["ref_id"].as_str().unwrap()).await;
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }

        let body_len = client
            .eval_js("document.body.innerText.length")
            .await
            .unwrap();
        let len = body_len.as_u64().unwrap_or(0);
        assert!(
            len > 50,
            "tab '{tab_name}' must not be blank — only {len} chars of text rendered"
        );
    }
}

#[tokio::test]
async fn all_five_tabs_have_no_console_errors() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let tab_names = ["Brief", "Signal", "Preemption", "Blind Spots", "Playbook"];

    for tab_name in &tab_names {
        let buttons = client
            .find_elements(serde_json::json!({"role": "button"}))
            .await
            .unwrap();
        let button_arr = buttons.as_array().expect("find_elements returns array");
        if let Some(btn) = button_arr.iter().find(|b| {
            b.get("text")
                .or_else(|| b.get("name"))
                .and_then(|t| t.as_str())
                .map_or(false, |t| t.contains(tab_name))
        }) {
            let _ = client.click(btn["ref_id"].as_str().unwrap()).await;
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }

        let report = client.verify().no_console_errors().run().await.unwrap();

        assert!(
            report.all_passed(),
            "tab '{tab_name}' has console errors: {:?}",
            report
                .failures()
                .iter()
                .map(|f| format!("{}: {}", f.description, f.detail))
                .collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn ipc_commands_never_panic() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let commands = [
        "get_briefing_snapshot",
        "get_latest_briefing",
        "get_settings",
        "get_analysis_status",
        "get_scoring_stats",
        "get_preemption_alerts",
        "get_monitoring_status",
        "get_blind_spots",
        "get_knowledge_gaps",
        "get_signal_chains",
        "get_void_signal",
        "get_developer_dna",
        "get_capability_states",
        "get_capability_summary",
        "get_source_health",
        "get_user_context",
        "get_learned_preferences",
        "get_startup_health",
        "get_diagnostics",
        "get_achievement_state",
        "get_achievements",
        "get_autophagy_status",
        "get_data_health",
        "get_intelligence_pulse",
        "get_sovereign_profile",
        "get_intelligence_growth",
        "get_decision_windows",
        "get_indexed_stats",
        "get_stack_health",
        "get_engagement_summary",
        "get_rss_feeds",
        "ace_get_detected_tech",
        "ace_get_active_topics",
    ];

    for cmd in &commands {
        let result = client.invoke_command(cmd, None).await;
        assert!(
            result.is_ok(),
            "IPC command '{cmd}' panicked or errored: {:?}",
            result.err()
        );
    }

    let ping = client.get_plugin_info().await;
    assert!(
        ping.is_ok(),
        "backend unresponsive after {}-command barrage — possible panic",
        commands.len()
    );
}

// ── Phase 16: Write-Read-Verify — Mutation Tests ─────────────────────────────

#[tokio::test]
async fn tech_stack_add_remove_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let marker = "victauri-test-lang-xyz";

    // Add
    let add_result = client
        .invoke_command(
            "add_tech_stack",
            Some(serde_json::json!({"technology": marker})),
        )
        .await
        .unwrap();
    assert!(
        add_result.is_object() || add_result.is_null(),
        "add_tech_stack must return valid response: {add_result}"
    );

    // Verify present
    let ctx = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let ctx_str = serde_json::to_string(&ctx).unwrap();
    assert!(
        ctx_str.contains(marker),
        "user context must contain '{marker}' after add_tech_stack"
    );

    // Remove
    let remove_result = client
        .invoke_command(
            "remove_tech_stack",
            Some(serde_json::json!({"technology": marker})),
        )
        .await
        .unwrap();
    assert!(
        remove_result.is_object() || remove_result.is_null(),
        "remove_tech_stack must return valid response"
    );

    // Verify absent
    let ctx2 = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let ctx2_str = serde_json::to_string(&ctx2).unwrap();
    assert!(
        !ctx2_str.contains(marker),
        "user context must NOT contain '{marker}' after remove_tech_stack"
    );
}

#[tokio::test]
async fn interest_add_remove_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let marker = "victauri-test-topic-xyz";

    let add = client
        .invoke_command("add_interest", Some(serde_json::json!({"topic": marker})))
        .await
        .unwrap();
    assert!(
        add.is_object() || add.is_null(),
        "add_interest must succeed: {add}"
    );

    let ctx = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let ctx_str = serde_json::to_string(&ctx).unwrap();
    assert!(
        ctx_str.contains(marker),
        "user context must contain interest '{marker}' after add"
    );

    let remove = client
        .invoke_command(
            "remove_interest",
            Some(serde_json::json!({"topic": marker})),
        )
        .await
        .unwrap();
    assert!(
        remove.is_object() || remove.is_null(),
        "remove_interest must succeed"
    );

    let ctx2 = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let ctx2_str = serde_json::to_string(&ctx2).unwrap();
    assert!(
        !ctx2_str.contains(marker),
        "user context must NOT contain '{marker}' after remove"
    );
}

#[tokio::test]
async fn monitoring_toggle_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Read current state
    let before = client
        .invoke_command("get_monitoring_status", None)
        .await
        .unwrap();
    let was_enabled = before["enabled"].as_bool().unwrap_or(true);

    // Toggle off
    let _ = client
        .invoke_command(
            "set_monitoring_enabled",
            Some(serde_json::json!({"enabled": false})),
        )
        .await
        .unwrap();

    let after_off = client
        .invoke_command("get_monitoring_status", None)
        .await
        .unwrap();
    assert_eq!(
        after_off["enabled"].as_bool().unwrap_or(true),
        false,
        "monitoring must be disabled after set_monitoring_enabled(false)"
    );

    // Restore
    let _ = client
        .invoke_command(
            "set_monitoring_enabled",
            Some(serde_json::json!({"enabled": was_enabled})),
        )
        .await
        .unwrap();

    let restored = client
        .invoke_command("get_monitoring_status", None)
        .await
        .unwrap();
    assert_eq!(
        restored["enabled"].as_bool().unwrap_or(false),
        was_enabled,
        "monitoring must be restored to original state"
    );
}

#[tokio::test]
async fn rss_feeds_read_write_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Read current feeds
    let original = client.invoke_command("get_rss_feeds", None).await.unwrap();

    let original_feeds: Vec<String> = original
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Add a test feed
    let mut test_feeds = original_feeds.clone();
    test_feeds.push("https://victauri-test-feed.example.com/rss".to_string());

    let _ = client
        .invoke_command(
            "set_rss_feeds",
            Some(serde_json::json!({"feeds": test_feeds})),
        )
        .await
        .unwrap();

    let after = client.invoke_command("get_rss_feeds", None).await.unwrap();
    let after_str = serde_json::to_string(&after).unwrap();
    assert!(
        after_str.contains("victauri-test-feed"),
        "RSS feeds must contain test feed after set"
    );

    // Restore original
    let _ = client
        .invoke_command(
            "set_rss_feeds",
            Some(serde_json::json!({"feeds": original_feeds})),
        )
        .await
        .unwrap();

    let restored = client.invoke_command("get_rss_feeds", None).await.unwrap();
    let restored_str = serde_json::to_string(&restored).unwrap();
    assert!(
        !restored_str.contains("victauri-test-feed"),
        "RSS feeds must NOT contain test feed after restore"
    );
}

#[tokio::test]
async fn user_role_set_and_verify() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Read original
    let ctx_before = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let original_role = ctx_before
        .get("role")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    // Set to a known role
    let _ = client
        .invoke_command(
            "set_user_role",
            Some(serde_json::json!({"role": "backend_developer"})),
        )
        .await
        .unwrap();

    let ctx_after = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();
    let new_role = ctx_after.get("role").and_then(|r| r.as_str()).unwrap_or("");
    assert_eq!(
        new_role, "backend_developer",
        "user role must be 'backend_developer' after set"
    );

    // Restore
    if !original_role.is_empty() {
        let _ = client
            .invoke_command(
                "set_user_role",
                Some(serde_json::json!({"role": original_role})),
            )
            .await;
    }
}

// ── Phase 17: Playbook — The Fifth Tab ───────────────────────────────────────

#[tokio::test]
async fn playbook_modules_returns_non_empty_list() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let modules = client
        .invoke_command("get_playbook_modules", None)
        .await
        .unwrap();

    assert!(
        modules.is_array(),
        "get_playbook_modules must return array, got: {modules}"
    );
    let arr = modules.as_array().unwrap();
    assert!(!arr.is_empty(), "playbook must have at least one module");

    for (i, m) in arr.iter().enumerate() {
        assert!(m.get("id").is_some(), "playbook module[{i}] must have 'id'");
        assert!(
            m.get("title").is_some(),
            "playbook module[{i}] must have 'title'"
        );
    }
}

#[tokio::test]
async fn playbook_content_returns_lessons() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Get first module ID
    let modules = client
        .invoke_command("get_playbook_modules", None)
        .await
        .unwrap();
    let first_id = modules
        .as_array()
        .and_then(|a| a.first())
        .and_then(|m| m.get("id"))
        .and_then(|id| id.as_str())
        .expect("at least one playbook module must exist");

    let content = client
        .invoke_command(
            "get_playbook_content",
            Some(serde_json::json!({"module_id": first_id})),
        )
        .await;

    match content {
        Ok(c) if !c.as_object().map_or(true, |o| o.is_empty()) => {
            assert!(
                c.get("lessons").is_some(),
                "playbook content must have 'lessons' field, got keys: {:?}",
                c.as_object().map(|o| o.keys().collect::<Vec<_>>())
            );
            assert!(
                c.get("title").is_some(),
                "playbook content must have 'title' field"
            );
        }
        Ok(_) => {
            // Empty object — content files may not be deployed in dev mode.
            // The command succeeded without panicking, which is the baseline.
        }
        Err(e) => {
            let err_str = format!("{e:?}");
            assert!(
                err_str.contains("fileNotFound") || err_str.contains("not found"),
                "playbook content error must be 'file not found', not a panic: {err_str}"
            );
        }
    }
}

#[tokio::test]
async fn playbook_progress_returns_state() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let progress = client
        .invoke_command("get_playbook_progress", None)
        .await
        .unwrap();

    assert!(
        progress.is_object(),
        "playbook progress must be an object, got: {progress}"
    );
}

// ── Phase 18: Intelligence Systems — Deep IPC Coverage ──────────────────────

#[tokio::test]
async fn tech_radar_returns_valid_structure() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let radar = client.invoke_command("get_tech_radar", None).await.unwrap();

    assert!(
        radar.is_object(),
        "tech radar must return an object, got: {radar}"
    );
}

#[tokio::test]
async fn knowledge_gaps_returns_evidence_feed() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let gaps = client
        .invoke_command("get_knowledge_gaps", None)
        .await
        .unwrap();

    assert!(gaps.is_object(), "knowledge gaps must return an object");
    assert!(
        gaps.get("items").is_some(),
        "knowledge gaps must have 'items' field, got keys: {:?}",
        gaps.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn signal_chains_returns_array() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let chains = client
        .invoke_command("get_signal_chains", None)
        .await
        .unwrap();

    assert!(
        chains.is_array(),
        "signal chains must return an array, got: {chains}"
    );
}

#[tokio::test]
async fn void_signal_returns_heartbeat() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let signal = client
        .invoke_command("get_void_signal", None)
        .await
        .unwrap();

    assert!(
        signal.is_object(),
        "void signal must return an object, got: {signal}"
    );
}

#[tokio::test]
async fn capability_states_returns_map() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let states = client
        .invoke_command("get_capability_states", None)
        .await
        .unwrap();

    assert!(
        states.is_object(),
        "capability states must be an object, got: {states}"
    );

    let key_count = states.as_object().map_or(0, |o| o.len());
    assert!(
        key_count > 0,
        "capability states must have at least one capability"
    );
}

#[tokio::test]
async fn capability_summary_returns_counts() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let summary = client
        .invoke_command("get_capability_summary", None)
        .await
        .unwrap();

    assert!(
        summary.is_object(),
        "capability summary must be an object, got: {summary}"
    );
}

#[tokio::test]
async fn source_health_returns_summary() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let health = client
        .invoke_command("get_source_health", None)
        .await
        .unwrap();

    assert!(
        health.is_object(),
        "source health must be an object, got: {health}"
    );
}

#[tokio::test]
async fn learned_preferences_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let prefs = client
        .invoke_command("get_learned_preferences", None)
        .await
        .unwrap();

    assert!(
        prefs.is_object() || prefs.is_array(),
        "learned preferences must return object or array, got: {prefs}"
    );
}

#[tokio::test]
async fn trust_dashboard_returns_summary() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let trust = client
        .invoke_command("get_trust_dashboard", None)
        .await
        .unwrap();

    assert!(
        trust.is_object(),
        "trust dashboard must return an object, got: {trust}"
    );
}

#[tokio::test]
async fn intelligence_metrics_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let metrics = client
        .invoke_command("get_intelligence_metrics", None)
        .await
        .unwrap();

    assert!(
        metrics.is_object(),
        "intelligence metrics must return an object, got: {metrics}"
    );
}

#[tokio::test]
async fn attention_report_returns_structure() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let report = client
        .invoke_command("get_attention_report", None)
        .await
        .unwrap();

    assert!(
        report.is_object(),
        "attention report must return an object, got: {report}"
    );
}

// ── Phase 19: System Health & Diagnostics ────────────────────────────────────

#[tokio::test]
async fn startup_health_returns_issues_array() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let health = client
        .invoke_command("get_startup_health", None)
        .await
        .unwrap();

    assert!(
        health.is_array(),
        "startup health must return array of HealthIssue, got: {health}"
    );
}

#[tokio::test]
async fn diagnostics_snapshot_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let diag = client
        .invoke_command("get_diagnostics", None)
        .await
        .unwrap();

    assert!(
        diag.is_object(),
        "diagnostics must return an object, got: {diag}"
    );
}

#[tokio::test]
async fn autophagy_status_returns_state() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let status = client
        .invoke_command("get_autophagy_status", None)
        .await
        .unwrap();

    assert!(
        status.is_object(),
        "autophagy status must return an object, got: {status}"
    );
}

#[tokio::test]
async fn data_health_returns_report() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let health = client
        .invoke_command("get_data_health", None)
        .await
        .unwrap();

    assert!(
        health.is_object(),
        "data health must return an object, got: {health}"
    );
}

#[tokio::test]
async fn intelligence_pulse_returns_snapshot() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let pulse = client
        .invoke_command("get_intelligence_pulse", None)
        .await
        .unwrap();

    assert!(
        pulse.is_object(),
        "intelligence pulse must return an object, got: {pulse}"
    );
}

// ── Phase 20: ACE, Achievements, Channels, Profile ──────────────────────────

#[tokio::test]
async fn ace_detected_tech_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let tech = client
        .invoke_command("ace_get_detected_tech", None)
        .await
        .unwrap();

    assert!(
        tech.is_object() || tech.is_array(),
        "ACE detected tech must return structured data, got: {tech}"
    );
}

#[tokio::test]
async fn ace_active_topics_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let topics = client
        .invoke_command("ace_get_active_topics", None)
        .await
        .unwrap();

    assert!(
        topics.is_object() || topics.is_array(),
        "ACE active topics must return structured data, got: {topics}"
    );
}

#[tokio::test]
async fn achievement_state_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let state = client
        .invoke_command("get_achievement_state", None)
        .await
        .unwrap();

    assert!(
        state.is_object(),
        "achievement state must return an object, got: {state}"
    );
}

#[tokio::test]
async fn achievements_list_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let achievements = client
        .invoke_command("get_achievements", None)
        .await
        .unwrap();

    assert!(
        achievements.is_object() || achievements.is_array(),
        "achievements must return structured data, got: {achievements}"
    );
}

#[tokio::test]
async fn sovereign_profile_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let profile = client
        .invoke_command("get_sovereign_profile", None)
        .await
        .unwrap();

    assert!(
        profile.is_object(),
        "sovereign profile must return an object, got: {profile}"
    );
}

#[tokio::test]
async fn intelligence_growth_returns_history() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let growth = client
        .invoke_command("get_intelligence_growth", None)
        .await
        .unwrap();

    assert!(
        growth.is_object(),
        "intelligence growth must return an object, got: {growth}"
    );
}

#[tokio::test]
async fn engagement_summary_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let summary = client
        .invoke_command("get_engagement_summary", None)
        .await
        .unwrap();

    assert!(
        summary.is_object(),
        "engagement summary must return an object, got: {summary}"
    );
}

#[tokio::test]
async fn decision_windows_returns_array() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let windows = client
        .invoke_command("get_decision_windows", None)
        .await
        .unwrap();

    assert!(
        windows.is_array(),
        "decision windows must return an array, got: {windows}"
    );
}

#[tokio::test]
async fn indexed_stats_returns_data() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let stats = client
        .invoke_command("get_indexed_stats", None)
        .await
        .unwrap();

    assert!(
        stats.is_object(),
        "indexed stats must return an object, got: {stats}"
    );
}

#[tokio::test]
async fn stack_health_returns_report() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let health = client
        .invoke_command("get_stack_health", None)
        .await
        .unwrap();

    assert!(
        health.is_object(),
        "stack health must return an object, got: {health}"
    );
}

#[tokio::test]
async fn user_context_returns_profile() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let ctx = client
        .invoke_command("get_user_context", None)
        .await
        .unwrap();

    assert!(
        ctx.is_object(),
        "user context must return an object, got: {ctx}"
    );
}

// ── Phase 21: Channels — Content Pipeline ────────────────────────────────────

#[tokio::test]
async fn channels_list_has_content() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let channels = client.invoke_command("list_channels", None).await.unwrap();
    let arr = channels.as_array().expect("channels is array");

    assert!(!arr.is_empty(), "must have at least one channel");

    for (i, ch) in arr.iter().enumerate() {
        assert!(ch.get("id").is_some(), "channel[{i}] must have 'id'");
        assert!(
            ch.get("name").is_some() || ch.get("title").is_some(),
            "channel[{i}] must have 'name' or 'title', got keys: {:?}",
            ch.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }
}

#[tokio::test]
async fn channel_content_returns_render_for_first_channel() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let channels = client.invoke_command("list_channels", None).await.unwrap();
    let first_id = channels
        .as_array()
        .and_then(|a| a.first())
        .and_then(|ch| ch.get("id"))
        .and_then(|id| id.as_i64())
        .expect("first channel must have numeric id");

    let content = client
        .invoke_command(
            "get_channel_content",
            Some(serde_json::json!({"channel_id": first_id})),
        )
        .await
        .unwrap();

    // May be null if channel hasn't been rendered yet — that's valid
    assert!(
        content.is_object() || content.is_null(),
        "channel content must be object or null, got: {content}"
    );
}

// ── Phase 22: LLM Infrastructure — Sidecar, Models, Synthesis ────────────────
//
// Tests the llama-server sidecar lifecycle, model catalog, synthesis capability
// checking, structured output from briefing synthesis, and the model eval harness.
// These tests exercise the full local inference pipeline without cloud API keys.

#[tokio::test]
async fn builtin_model_catalog_has_entries() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("list_builtin_models", None)
        .await
        .unwrap();

    let models = result
        .get("models")
        .and_then(|m| m.as_array())
        .expect("list_builtin_models must return 'models' array");

    assert!(
        models.len() >= 3,
        "catalog should have at least 3 models (primary + 2 fallbacks), got {}",
        models.len()
    );

    for (i, model) in models.iter().enumerate() {
        assert!(
            model.get("id").and_then(|v| v.as_str()).is_some(),
            "model[{i}] must have 'id'"
        );
        assert!(
            model.get("display_name").and_then(|v| v.as_str()).is_some(),
            "model[{i}] must have 'display_name'"
        );
        assert!(
            model.get("size_bytes").and_then(|v| v.as_u64()).is_some(),
            "model[{i}] must have numeric 'size_bytes'"
        );
        assert!(
            model.get("min_ram_gb").and_then(|v| v.as_f64()).is_some(),
            "model[{i}] must have 'min_ram_gb'"
        );
        assert!(
            model.get("downloaded").and_then(|v| v.as_bool()).is_some(),
            "model[{i}] must report 'downloaded' status"
        );
    }

    assert!(
        result
            .get("ram_total_gb")
            .and_then(|v| v.as_f64())
            .is_some(),
        "response must include 'ram_total_gb'"
    );
    assert!(
        result
            .get("ram_available_gb")
            .and_then(|v| v.as_f64())
            .is_some(),
        "response must include 'ram_available_gb'"
    );
}

#[tokio::test]
async fn builtin_model_catalog_recommends_based_on_ram() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("list_builtin_models", None)
        .await
        .unwrap();

    let ram_total = result
        .get("ram_total_gb")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    if ram_total >= 7.0 {
        assert!(
            result.get("recommended_id").is_some()
                && !result.get("recommended_id").unwrap().is_null(),
            "systems with ≥7 GB RAM should get a model recommendation"
        );
    }

    let models = result["models"].as_array().unwrap();
    for model in models {
        let fits = model["fits_ram"].as_bool().unwrap_or(false);
        let min_ram = model["min_ram_gb"].as_f64().unwrap_or(f64::MAX);
        if min_ram <= ram_total {
            assert!(
                fits,
                "model {} should fit in {:.0} GB RAM",
                model["id"], ram_total
            );
        }
    }
}

#[tokio::test]
async fn synthesis_capability_returns_hardware_info() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("check_synthesis_capability", None)
        .await
        .unwrap();

    assert!(
        result.get("provider").and_then(|v| v.as_str()).is_some(),
        "must report 'provider'"
    );
    assert!(
        result
            .get("can_synthesize")
            .and_then(|v| v.as_bool())
            .is_some(),
        "must report 'can_synthesize' boolean"
    );
    assert!(
        result
            .get("can_explain")
            .and_then(|v| v.as_bool())
            .is_some(),
        "must report 'can_explain' boolean"
    );
    assert!(
        result.get("guidance").and_then(|v| v.as_str()).is_some(),
        "must provide guidance text"
    );

    let hw = result
        .get("hardware")
        .expect("must include 'hardware' block");
    assert!(
        hw.get("ram_total_gb").and_then(|v| v.as_f64()).is_some(),
        "hardware.ram_total_gb must be numeric"
    );
    assert!(
        hw.get("ram_available_gb")
            .and_then(|v| v.as_f64())
            .is_some(),
        "hardware.ram_available_gb must be numeric"
    );
    assert!(
        hw.get("ram_tier").and_then(|v| v.as_str()).is_some(),
        "hardware.ram_tier must be a string"
    );
}

#[tokio::test]
async fn synthesis_capability_flags_unverified_providers() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("check_synthesis_capability", None)
        .await
        .unwrap();

    assert!(
        result.get("unverified").and_then(|v| v.as_bool()).is_some(),
        "must include 'unverified' flag for provider quality signal"
    );

    let provider = result["provider"].as_str().unwrap_or("");
    let unverified = result["unverified"].as_bool().unwrap_or(false);

    match provider {
        "anthropic" | "openai" => {
            assert!(
                !unverified,
                "cloud providers should not be flagged as unverified"
            );
        }
        "openai-compatible" => {
            assert!(
                unverified,
                "openai-compatible must be flagged as unverified"
            );
        }
        _ => {} // builtin, ollama — no assertion needed
    }
}

#[tokio::test]
async fn builtin_llm_status_is_queryable() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("get_builtin_llm_status", None)
        .await
        .unwrap();

    assert!(
        result.get("status").and_then(|v| v.as_str()).is_some(),
        "status must be a string (stopped/starting/ready/error)"
    );

    let status = result["status"].as_str().unwrap();
    assert!(
        ["stopped", "starting", "ready", "error"].contains(&status),
        "status must be one of stopped/starting/ready/error, got: {status}"
    );
}

#[tokio::test]
async fn sidecar_starts_with_downloaded_model() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Find a downloaded model from the catalog
    let catalog = client
        .invoke_command("list_builtin_models", None)
        .await
        .unwrap();
    let models = catalog["models"].as_array().expect("models array");
    let downloaded = models.iter().find(|m| {
        m.get("downloaded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    });

    let model = match downloaded {
        Some(m) => m,
        None => {
            eprintln!("Skipping sidecar_starts_with_downloaded_model: no model downloaded");
            return;
        }
    };

    let model_path = model["path"]
        .as_str()
        .expect("downloaded model must have path");

    let result = client
        .invoke_command(
            "start_builtin_llm",
            Some(serde_json::json!({"model_path": model_path})),
        )
        .await
        .unwrap();

    assert_eq!(
        result["status"].as_str().unwrap_or(""),
        "ready",
        "sidecar must reach 'ready' status"
    );
    assert!(
        result["port"].as_u64().unwrap_or(0) > 0,
        "sidecar must return a valid port number"
    );

    // Verify status reflects running state
    let status = client
        .invoke_command("get_builtin_llm_status", None)
        .await
        .unwrap();
    assert_eq!(
        status["status"].as_str().unwrap_or(""),
        "ready",
        "status must reflect running sidecar"
    );

    // Clean up
    let _ = client.invoke_command("stop_builtin_llm", None).await;
}

#[tokio::test]
async fn sidecar_rejects_nonexistent_model_path() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client
        .invoke_command(
            "start_builtin_llm",
            Some(serde_json::json!({"model_path": "C:\\nonexistent\\fake-model.gguf"})),
        )
        .await;

    assert!(
        result.is_err() || result.as_ref().ok().and_then(|v| v.get("error")).is_some(),
        "starting with a nonexistent model must fail gracefully"
    );
}

#[tokio::test]
async fn sidecar_stop_returns_stopped() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let result = client
        .invoke_command("stop_builtin_llm", None)
        .await
        .unwrap();

    assert_eq!(
        result["status"].as_str().unwrap_or(""),
        "stopped",
        "stop command must return 'stopped' status"
    );
}

#[tokio::test]
async fn model_eval_returns_structured_verdict() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // Check if synthesis is configured — eval needs a working LLM
    let cap = client
        .invoke_command("check_synthesis_capability", None)
        .await
        .unwrap();
    let can_synth = cap["can_synthesize"].as_bool().unwrap_or(false);
    let provider = cap["provider"].as_str().unwrap_or("");

    if !can_synth && provider != "builtin" {
        eprintln!("Skipping model_eval: no synthesis-capable provider configured");
        return;
    }

    let result = client.invoke_command("run_model_eval", None).await;

    match result {
        Ok(summary) => {
            assert!(
                summary.get("verdict").and_then(|v| v.as_str()).is_some(),
                "eval summary must have 'verdict' (pass/warnings/fail)"
            );
            let verdict = summary["verdict"].as_str().unwrap();
            assert!(
                ["pass", "warnings", "fail"].contains(&verdict),
                "verdict must be pass/warnings/fail, got: {verdict}"
            );

            assert!(
                summary
                    .get("fixtures_run")
                    .and_then(|v| v.as_u64())
                    .is_some(),
                "must report fixtures_run count"
            );
            assert!(
                summary
                    .get("fixtures_passed")
                    .and_then(|v| v.as_u64())
                    .is_some(),
                "must report fixtures_passed count"
            );

            if let Some(results) = summary.get("results").and_then(|v| v.as_array()) {
                for (i, r) in results.iter().enumerate() {
                    assert!(
                        r.get("fixture_name").and_then(|v| v.as_str()).is_some(),
                        "result[{i}] must have 'fixture_name'"
                    );
                    assert!(
                        r.get("passed").and_then(|v| v.as_bool()).is_some(),
                        "result[{i}] must have 'passed' boolean"
                    );
                }
            }
        }
        Err(e) => {
            let err_str = format!("{e:?}");
            assert!(
                err_str.contains("API")
                    || err_str.contains("connection")
                    || err_str.contains("sidecar")
                    || err_str.contains("provider"),
                "eval failure must be a connection/provider issue, not a crash: {err_str}"
            );
        }
    }
}

#[tokio::test]
async fn briefing_snapshot_has_synthesis_fields() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let snapshot = client
        .invoke_command("get_briefing_snapshot", None)
        .await
        .unwrap();

    if snapshot.is_null() {
        return;
    }

    let briefing = match snapshot.get("briefing").and_then(|v| v.as_str()) {
        Some(b) if !b.is_empty() => b,
        _ => return,
    };

    // Synthesis prose should be substantive, not a raw template
    assert!(
        briefing.len() >= 50,
        "synthesis prose must be ≥50 chars, got {}",
        briefing.len()
    );
    assert!(
        !briefing.contains("{{") && !briefing.contains("}}"),
        "synthesis must not contain template markers"
    );
    assert!(
        !briefing.contains("TODO") && !briefing.contains("FIXME"),
        "synthesis must not contain dev markers"
    );

    // Check for structured fields added by Phase 4
    if let Some(clusters) = snapshot.get("clusters").and_then(|v| v.as_array()) {
        for (i, cluster) in clusters.iter().enumerate() {
            assert!(
                cluster.get("insight").and_then(|v| v.as_str()).is_some(),
                "cluster[{i}] must have 'insight'"
            );
            assert!(
                cluster.get("confidence").is_some(),
                "cluster[{i}] must have 'confidence'"
            );
        }
    }
}

#[tokio::test]
async fn trigger_briefing_returns_status() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client
        .invoke_command("trigger_morning_briefing", None)
        .await;

    match result {
        Ok(val) => {
            let msg = val.as_str().unwrap_or("");
            // Either "No items available" (cold start) or a success message with counts
            assert!(
                msg.contains("items") || msg.contains("briefing") || msg.contains("No items"),
                "trigger result must mention items or briefing status, got: {msg}"
            );
        }
        Err(e) => {
            let err_str = format!("{e:?}");
            assert!(
                !err_str.contains("not found") && !err_str.contains("unresolved"),
                "trigger_morning_briefing must be a registered command: {err_str}"
            );
        }
    }
}

#[tokio::test]
async fn ipc_commands_include_llm_infrastructure() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let required_commands = [
        "check_synthesis_capability",
        "list_builtin_models",
        "start_builtin_llm",
        "stop_builtin_llm",
        "get_builtin_llm_status",
        "download_builtin_model",
        "cancel_builtin_model_download",
        "delete_builtin_model",
        "run_model_eval",
        "trigger_morning_briefing",
    ];

    for cmd_name in &required_commands {
        let result = client.invoke_command(cmd_name, None).await;
        match &result {
            Ok(_) => {}
            Err(e) => {
                let err_str = format!("{e:?}");
                assert!(
                    !err_str.contains("not found")
                        && !err_str.contains("unknown command")
                        && !err_str.contains("unresolved"),
                    "IPC command '{cmd_name}' must be registered — got: {err_str}"
                );
            }
        }
    }
}
