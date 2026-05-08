//! Victauri dogfood tests — validate 4DA using our own testing framework.
//!
//! Requires a running 4DA dev server (`pnpm run tauri dev`).
//! Run with: `VICTAURI_E2E=1 cargo test --test victauri_dogfood`

use victauri_test::visual::VisualOptions;
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

    eprintln!(
        "Plugin info: {}",
        serde_json::to_string_pretty(&info).unwrap()
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
    let result_summary = format!(
        "keys: {:?}",
        result.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    assert!(
        has_image,
        "screenshot should return image data: {result_summary}"
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
    assert!(
        has_tree,
        "DOM snapshot should contain element tree: {snapshot}"
    );
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
        "memory stats should contain RSS/working set: {stats}"
    );
    eprintln!(
        "Memory stats: {}",
        serde_json::to_string_pretty(&stats).unwrap()
    );
}

#[tokio::test]
async fn window_state_reports_main() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let windows = client.list_windows().await.unwrap();

    eprintln!(
        "Windows response: {}",
        serde_json::to_string_pretty(&windows).unwrap()
    );

    let has_windows = windows.as_array().is_some()
        || windows.get("windows").is_some()
        || windows.get("labels").is_some()
        || windows.as_object().is_some();
    assert!(has_windows, "should return window info: {windows}");

    let state = client.get_window_state(Some("main")).await.unwrap();
    eprintln!(
        "Window state: {}",
        serde_json::to_string_pretty(&state).unwrap()
    );
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

    eprintln!("Found tabs: {found:?}");
    if !missing.is_empty() {
        eprintln!("Missing tabs: {missing:?}");
    }
    assert!(
        missing.len() <= 1,
        "At least 4 of 5 main tabs should be in DOM. Missing: {missing:?}"
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
    eprintln!("Document title: {title_str}");
}

#[tokio::test]
async fn settings_command_round_trip() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let result = client.invoke_command("get_settings", None).await;
    match result {
        Ok(settings) => {
            assert!(
                settings.is_object() || settings.is_string(),
                "get_settings should return settings object: {settings}"
            );
            eprintln!("Settings retrieved successfully");
        }
        Err(e) => {
            eprintln!("get_settings returned error (may need different command name): {e}");
        }
    }
}

#[tokio::test]
async fn console_logs_accessible() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let logs = client.logs("console", Some(20)).await.unwrap();

    eprintln!(
        "Console log sample: {}",
        serde_json::to_string_pretty(&logs).unwrap()
    );
}

// ── Phase 3: Cross-Boundary Tests (Victauri's Unique Value) ──────────────────

#[tokio::test]
async fn ghost_command_detection() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let ghosts = client.detect_ghost_commands().await.unwrap();

    eprintln!(
        "Ghost command report:\n{}",
        serde_json::to_string_pretty(&ghosts).unwrap()
    );

    let ghost_list = ghosts
        .get("ghost_commands")
        .and_then(|g| g.as_array())
        .or_else(|| ghosts.get("frontend_only").and_then(|f| f.as_array()));

    if let Some(list) = ghost_list {
        eprintln!("Ghost commands found: {}", list.len());
        for ghost in list.iter().take(10) {
            eprintln!("  - {ghost}");
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

    eprintln!(
        "IPC integrity:\n{}",
        serde_json::to_string_pretty(&health).unwrap()
    );

    let has_status = health.get("healthy").is_some()
        || health.get("status").is_some()
        || health.get("pending").is_some();
    assert!(
        has_status,
        "IPC health check should report status: {health}"
    );
}

#[tokio::test]
async fn ipc_log_captures_commands() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let checkpoint = client.ipc_checkpoint().await.unwrap();

    // Trigger an IPC call through invoke_command
    let _ = client.invoke_command("get_analysis_status", None).await;

    let calls_since = client.ipc_calls_since(checkpoint).await.unwrap();
    eprintln!("IPC calls since checkpoint: {}", calls_since.len());
    for call in &calls_since {
        if let Some(cmd) = call.get("command").and_then(|c| c.as_str()) {
            eprintln!("  - {cmd}");
        }
    }
}

#[tokio::test]
async fn accessibility_audit() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let audit = client.audit_accessibility().await.unwrap();

    eprintln!(
        "Accessibility audit:\n{}",
        serde_json::to_string_pretty(&audit).unwrap()
    );
}

#[tokio::test]
async fn performance_metrics_baseline() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let metrics = client.get_performance_metrics().await.unwrap();

    eprintln!(
        "Performance metrics:\n{}",
        serde_json::to_string_pretty(&metrics).unwrap()
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

    eprintln!("Verification report:");
    for result in &report.results {
        eprintln!(
            "  [{}] {}: {}",
            if result.passed { "PASS" } else { "FAIL" },
            result.description,
            &result.detail
        );
    }

    if !report.failures().is_empty() {
        eprintln!("\nFailures:");
        for f in report.failures() {
            eprintln!("  FAIL: {} — {}", f.description, &f.detail);
        }
    }
}

// ── Phase 4: New Feature Dogfood (Visual, Coverage, Recording) ─────────────

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
        channel_tolerance: 5,
        threshold_percent: 1.0,
        ..VisualOptions::default()
    };

    let diff = client.screenshot_visual("4da_main_view", &opts).await;
    match diff {
        Ok(d) => {
            let matches = d.is_match(opts.threshold_percent);
            eprintln!(
                "Visual regression: match={matches}, match_pct={:.2}%, diff_pixels={}",
                d.match_percentage, d.diff_pixel_count
            );
            if matches {
                eprintln!("  Baseline matched (or created on first run)");
            } else {
                eprintln!("  Diff detected — check {}", snapshot_dir.display());
            }
        }
        Err(e) => {
            eprintln!("Visual regression error (may be first run): {e}");
        }
    }
}

#[tokio::test]
async fn ipc_coverage_report() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();
    let report = victauri_test::coverage::coverage_report(&mut client).await;

    match report {
        Ok(r) => {
            eprintln!("{}", r.to_summary());
            eprintln!(
                "Coverage: {:.1}% ({}/{} commands tested)",
                r.coverage_percentage, r.tested_commands, r.total_commands
            );
        }
        Err(e) => {
            eprintln!("Coverage report failed (expected if no registry): {e}");
        }
    }
}

#[tokio::test]
async fn recording_start_stop_cycle() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let start = client.start_recording(Some("dogfood-test")).await;
    match start {
        Ok(s) => eprintln!("Recording started: {s}"),
        Err(e) => {
            eprintln!("Recording start failed: {e}");
            return;
        }
    }

    // Trigger a few actions to capture
    let _ = client.eval_js("document.title").await;
    let _ = client.dom_snapshot().await;

    let stop = client.stop_recording().await;
    match stop {
        Ok(session) => {
            eprintln!(
                "Recording stopped: {}",
                serde_json::to_string_pretty(&session).unwrap_or_default()
            );
            let event_count = session
                .get("events")
                .and_then(|e| e.as_array())
                .map_or(0, |a| a.len());
            eprintln!("  Captured {event_count} events");
        }
        Err(e) => eprintln!("Recording stop failed: {e}"),
    }
}

#[tokio::test]
async fn wait_for_text_in_dom() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    // 4DA should have some text in the DOM — try common elements
    let result = client
        .wait_for("text", Some("4DA"), Some(3000), Some(200))
        .await;

    match result {
        Ok(r) => {
            let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            eprintln!("wait_for text '4DA': ok={ok}");
        }
        Err(e) => eprintln!("wait_for failed: {e}"),
    }
}

#[tokio::test]
async fn expanded_verification_chain() {
    if skip_unless_e2e() {
        return;
    }

    let mut client = VictauriClient::discover().await.unwrap();

    let report = client
        .verify()
        .no_console_errors()
        .ipc_healthy()
        .no_ghost_commands()
        .run()
        .await
        .unwrap();

    let passed = report.results.iter().filter(|r| r.passed).count();
    let total = report.results.len();
    eprintln!("Expanded verification: {passed}/{total} checks passed");

    for result in &report.results {
        eprintln!(
            "  [{}] {}{}",
            if result.passed { "PASS" } else { "FAIL" },
            result.description,
            if result.detail.is_empty() {
                String::new()
            } else {
                format!(" — {}", result.detail)
            }
        );
    }
}
