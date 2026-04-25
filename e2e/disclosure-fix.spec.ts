import { test, expect } from "@playwright/test";

var SS = "e2e-results/screenshots/disclosure";

test.describe("Privacy Disclosure Fix", () => {
  test("camelCase IPC params for set_privacy_config", async ({ page }) => {

    // Mock Tauri IPC before page loads to prevent browser-mode redirect
    await page.addInitScript(() => {
      const w = window as any;
      w.__ipc_log = [];
      w.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: any) => {
          w.__ipc_log.push({ cmd, args: JSON.parse(JSON.stringify(args || {})) });
          if (cmd === "get_settings") {
            return {
              version: 1,
              llm: {
                provider: "anthropic",
                model: "claude-sonnet-4-20250514",
                api_key: "",
                has_api_key: true,
                base_url: "",
                rerank_enabled: false,
                rerank_max_items: 10,
                rerank_min_score: 0.3,
                daily_token_limit: 100000,
                daily_cost_limit_cents: 50,
              },
              monitoring: { enabled: false, interval_minutes: 30, notification_threshold: 0.7, schedule: null },
              threshold: 0.5,
              sources: [],
              digest: { enabled: false, time: "08:00", format: "email", smtp_host: "", smtp_port: 587, smtp_user: "", smtp_pass: "", use_tls: true, recipient: "" },
              locale: "en",
              ace: { scan_directories: [], excluded_patterns: [], auto_discover: true },
              privacy: { telemetry_enabled: false, anonymize_queries: false, cloud_llm_disclosure_accepted: false, crash_reporting_opt_in: false, activity_tracking_opt_in: false, llm_content_level: "full" },
              display: { theme: "dark" },
            };
          }
          if (cmd === "get_context_stats") return { total_files: 0, languages: [] };
          if (cmd === "get_sources") return [];
          if (cmd === "detect_environment") return { has_anthropic_env: false, anthropic_env_preview: "", has_openai_env: false, openai_env_preview: "" };
          if (cmd === "get_onboarding_status") return { completed: true, skipped: false };
          if (cmd === "get_monitoring_status") return { running: false, interval_minutes: 30, notification_threshold: 0.7 };
          if (cmd === "get_scan_directories") return [];
          if (cmd === "get_ollama_status") return { running: false, version: "", models: [] };
          if (cmd === "get_tier") return "free";
          if (cmd === "get_system_health") return { db_size_bytes: 0, db_ok: true, disk_free_bytes: 1000000000, uptime_seconds: 0 };
          if (cmd === "get_discovered_context") return [];
          if (cmd === "get_user_context") return { interests: [], skills: [], suggested: [] };
          if (cmd === "get_model_registry") return { providers: {}, fetched_at: 0, version: "0.0.0" };
          if (cmd === "validate_api_key") return { valid: true, format_ok: true, error: null, model_access: [] };
          if (cmd === "get_license_info") return { tier: "free", valid: true };
          if (cmd === "get_usage_stats") return { today_tokens: 0, today_cost_cents: 0, month_tokens: 0, month_cost_cents: 0 };
          if (cmd === "get_summary_badges") return [];
          if (cmd === "get_standing_queries") return [];
          if (cmd === "get_suggested_interests") return [];
          if (cmd === "check_ollama_status") return { running: false, version: "", models: [] };
          if (cmd === "get_privacy_config") return { llm_content_level: "full", proxy_url: null, cloud_llm_disclosure_accepted: false };
          if (cmd === "get_llm_usage") return { used: 0, limit: 100000, limit_reached: false, unlimited: false, cost_used_cents: 0, cost_limit_cents: 50, cost_limit_reached: false };
          if (cmd === "startup_health_check") return { status: "ok" };
          if (cmd === "health_check") return { status: "ok" };
          if (cmd.startsWith("set_")) return null;
          if (cmd.startsWith("test_")) return { ok: true };
          console.warn("[MOCK] unhandled invoke: " + cmd);
          return null;
        },
        transformCallback: (cb: any) => {
          const id = Math.random();
          (window as any)["_" + id] = cb;
          return id;
        },
        convertFileSrc: (path: string) => path,
      };
    });

    // Step 1: Navigate
    console.log("STEP 1: Navigate");
    await page.goto("/", { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => {
      return document.querySelector("[data-settings-trigger]") !== null;
    }, { timeout: 15000 }).catch(() => console.log("  Settings trigger not found"));
    await page.waitForTimeout(1000);
    await page.screenshot({ path: SS + "/01-main.png", fullPage: true });

    // Step 2: Open Settings
    console.log("STEP 2: Open Settings");
    var settingsBtn = page.locator("[data-settings-trigger]");
    if (await settingsBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await settingsBtn.click();
      await page.waitForTimeout(1500);
    } else {
      await page.keyboard.press("Control+,");
      await page.waitForTimeout(1500);
    }
    await page.screenshot({ path: SS + "/02-settings.png", fullPage: true });
    var dlg = await page.locator("[role=\"dialog\"]").first().isVisible().catch(() => false);
    console.log("  Dialog open: " + dlg);
    expect(dlg).toBe(true);

    // Step 3: Intelligence tab
    console.log("STEP 3: Intelligence tab");
    var intTab = page.locator("[role=\"tab\"]:has-text(\"Intelligence\")");
    if (await intTab.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await intTab.first().click();
      await page.waitForTimeout(1000);
      console.log("  tab clicked");
    }
    await page.screenshot({ path: SS + "/03-intelligence.png", fullPage: true });

    // Step 4: Verify provider is anthropic (cloud)
    console.log("STEP 4: Check provider is anthropic");
    var ps = page.locator("#ai-provider-select");
    if (await ps.isVisible({ timeout: 5000 }).catch(() => false)) {
      var val = await ps.inputValue();
      console.log("  Current provider: " + val);
      expect(val).toBe("anthropic");
    }

    // Step 5: Click Save AI Configuration
    console.log("STEP 5: Save AI Configuration");
    var saveBtn = page.locator("button").filter({ hasText: /Save AI Configuration/i });
    expect(await saveBtn.first().isVisible({ timeout: 5000 })).toBe(true);
    await saveBtn.first().click();
    console.log("  Clicked Save");
    await page.waitForTimeout(3000);
    await page.screenshot({ path: SS + "/04-after-save.png", fullPage: true });

    // Step 6: THE KEY VALIDATION - check IPC call params
    console.log("STEP 6: Validate IPC params (THE FIX)");
    var ipcLog = await page.evaluate(() => (window as any).__ipc_log || []);
    var privacyCalls = ipcLog.filter((c: any) => c.cmd === "set_privacy_config");
    console.log("  set_privacy_config calls: " + privacyCalls.length);

    // Cloud provider (anthropic) MUST trigger disclosure acceptance
    expect(privacyCalls.length).toBeGreaterThanOrEqual(1);

    for (var call of privacyCalls) {
      console.log("  Args sent: " + JSON.stringify(call.args));
      var keys = Object.keys(call.args);

      // MUST use camelCase (the fix)
      var snakeKeys = keys.filter((k: string) => k.includes("_"));
      expect(snakeKeys).toHaveLength(0);
      console.log("  PASS: No snake_case parameters found");

      // MUST set cloudLlmDisclosureAccepted to true
      expect(call.args.cloudLlmDisclosureAccepted).toBe(true);
      console.log("  PASS: cloudLlmDisclosureAccepted = true");

      // Old bug: snake_case version MUST be absent
      expect(call.args.cloud_llm_disclosure_accepted).toBeUndefined();
      console.log("  PASS: cloud_llm_disclosure_accepted (snake_case) is absent");
    }

    // Step 7: Audit ALL set_ commands for camelCase compliance
    console.log("STEP 7: Audit all set_ IPC calls");
    var setCalls = ipcLog.filter((c: any) => c.cmd.startsWith("set_"));
    for (var sc of setCalls) {
      var skeys = Object.keys(sc.args);
      var sSnake = skeys.filter((k: string) => k.includes("_"));
      if (sSnake.length > 0) {
        console.log("  FAIL: " + sc.cmd + " has snake_case args: " + sSnake.join(", "));
      } else {
        console.log("  OK: " + sc.cmd + " args: " + skeys.join(", "));
      }
      expect(sSnake).toHaveLength(0);
    }

    // Step 8: Test Connection button
    console.log("STEP 8: Test Connection");
    var testBtn = page.locator("button").filter({ hasText: /Test Connection/i });
    if (await testBtn.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await testBtn.first().click();
      console.log("  Clicked Test Connection");
      await page.waitForTimeout(3000);
    }
    await page.screenshot({ path: SS + "/05-after-test.png", fullPage: true });

    // Step 9: Close and verify main page
    console.log("STEP 9: Close settings");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(1500);
    await page.screenshot({ path: SS + "/06-main-final.png", fullPage: true });

    console.log("=== ALL ASSERTIONS PASSED ===");

  });
});
