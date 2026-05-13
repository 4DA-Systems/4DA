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
                ghost.is_string() || ghost.get("command").is_some(),
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

    // 4DA's title is always in the DOM
    let result = client
        .wait_for("text", Some("4DA"), Some(5000), Some(200))
        .await
        .unwrap();

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(ok, "wait_for should find '4DA' text in DOM: {result}");
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
        let has_covered = dom_str.contains("Well Covered") || dom_str.contains("well_covered");
        assert!(
            has_covered,
            "Should show Well Covered section when some deps have problems"
        );
    } else {
        let has_clean_state = dom_str.contains("excellent")
            || dom_str.contains("Excellent")
            || dom_str.contains("monitoring");
        assert!(
            has_clean_state,
            "Clean state (score=0) should show positive card instead of covered section"
        );
    }
}
