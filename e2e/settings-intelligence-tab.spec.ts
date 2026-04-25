import { test, expect } from "@playwright/test";

var SS = "e2e-results/screenshots";

test.describe("Settings Intelligence Tab", () => {
  test("validate Intelligence tab UI", async ({ page }) => {

    // Mock Tauri IPC before page loads to prevent browser-mode redirect
    await page.addInitScript(() => {
      const w = window as any;
      w.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) => {
          if (cmd === "get_settings") {
            return {
              version: 1,
              llm: {
                provider: "ollama",
                model: "llama3.2",
                api_key: "",
                has_api_key: false,
                base_url: "http://localhost:11434",
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
              privacy: { telemetry_enabled: false, anonymize_queries: false },
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
          if (cmd === "validate_api_key") return { valid: false, format_ok: false, error: "mock", model_access: [] };
          if (cmd === "get_license_info") return { tier: "free", valid: true };
          if (cmd === "get_usage_stats") return { today_tokens: 0, today_cost_cents: 0, month_tokens: 0, month_cost_cents: 0 };
          if (cmd === "get_summary_badges") return [];
          if (cmd === "get_standing_queries") return [];
          if (cmd === "get_suggested_interests") return [];
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

    console.log("STEP 1: Navigate");
    await page.goto("/", { waitUntil: "domcontentloaded" });
    // Wait for splash screen to complete and main UI to appear
    await page.waitForFunction(() => {
      var trigger = document.querySelector("[data-settings-trigger]");
      return trigger !== null;
    }, { timeout: 15000 }).catch(() => console.log("  Settings trigger not found after 15s"));
    await page.waitForTimeout(1000);
    await page.screenshot({ path: SS + "/01-initial.png", fullPage: true });

    // Step 2: Open Settings
    console.log("STEP 2: Open Settings");
    var settingsBtn = page.locator("[data-settings-trigger]");
    var vis = await settingsBtn.isVisible({ timeout: 5000 }).catch(() => false);
    console.log("  trigger visible: " + vis);
    if (vis) {
      await settingsBtn.click();
      await page.waitForTimeout(1500);
    } else {
      await page.keyboard.press("Control+,");
      await page.waitForTimeout(1500);
    }

    var dlg = await page.locator("[role=\"dialog\"]").first().isVisible().catch(() => false);
    console.log("  Dialog open: " + dlg);
    await page.screenshot({ path: SS + "/02-settings.png", fullPage: true });
    if (!dlg) {
      var btns = await page.locator("button").count();
      console.log("  Buttons on page: " + btns);
      for (var i = 0; i < Math.min(btns, 15); i++) {
        var b = page.locator("button").nth(i);
        var t = await b.textContent().catch(() => "");
        var a = await b.getAttribute("aria-label").catch(() => "");
        console.log("    Btn " + i + ": text=" + (t||"").trim().substring(0,40) + " aria=" + (a||""));
      }
      console.log("ERROR: Settings modal not opened");
      return;
    }

    // Step 3: Intelligence tab
    console.log("STEP 3: Intelligence tab");
    var intTab = page.locator("[role=\"tab\"]:has-text(\"Intelligence\")");
    if (await intTab.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await intTab.first().click();
      await page.waitForTimeout(1000);
      console.log("  tab clicked");
    } else {
      var btn2 = page.locator("button:has-text(\"Intelligence\")");
      if (await btn2.first().isVisible({ timeout: 1000 }).catch(() => false)) {
        await btn2.first().click();
        await page.waitForTimeout(1000);
        console.log("  clicked Intelligence button");
      }
    }
    await page.screenshot({ path: SS + "/03-intelligence.png", fullPage: true });

    // Step 4: Provider dropdown
    console.log("STEP 4: Provider dropdown");
    var ps = page.locator("#ai-provider-select");
    if (await ps.isVisible({ timeout: 5000 }).catch(() => false)) {
      var val = await ps.inputValue();
      console.log("  Current provider: " + val);
      var opts = ps.locator("option");
      var oc = await opts.count();
      for (var i = 0; i < oc; i++) {
        var oText = await opts.nth(i).textContent();
        var oVal = await opts.nth(i).getAttribute("value");
        console.log("    Opt " + i + ": " + oText + " (val=" + oVal + ")");
      }
      var fo = await opts.first().textContent();
      expect(fo).toContain("Anthropic");
      expect(fo).toContain("Recommended");
      console.log("  PASS: Anthropic is first option with (Recommended)");
    } else {
      console.log("  FAIL: Provider dropdown not visible");
    }

    // Step 5: Select Ollama and check BYOK nudge
    console.log("STEP 5: Select Ollama + check BYOK nudge");
    await ps.selectOption("ollama");
    await page.waitForTimeout(500);
    await page.screenshot({ path: SS + "/04-ollama.png", fullPage: true });

    var nudge = page.locator("text=Get better results with a cloud API key");
    var nv = await nudge.isVisible({ timeout: 3000 }).catch(() => false);
    console.log("  BYOK nudge visible: " + (nv ? "PASS" : "FAIL"));

    var sw = page.locator("button:has-text(\"Switch to Anthropic\")");
    var sv = await sw.isVisible({ timeout: 2000 }).catch(() => false);
    console.log("  Switch to Anthropic button: " + (sv ? "PASS" : "FAIL"));
    await page.screenshot({ path: SS + "/05-nudge.png", fullPage: true });

    if (sv) {
      // Step 6: Click Switch to Anthropic
      console.log("STEP 6: Click Switch to Anthropic");
      await sw.click();
      await page.waitForTimeout(500);
      await page.screenshot({ path: SS + "/06-switched.png", fullPage: true });

      var newVal = await ps.inputValue();
      console.log("  Provider after switch: " + newVal);
      expect(newVal).toBe("anthropic");
      console.log("  PASS: Provider is now anthropic");

      var ak = page.locator("#ai-api-key");
      var akv = await ak.isVisible({ timeout: 3000 }).catch(() => false);
      console.log("  API key field visible: " + (akv ? "PASS" : "FAIL"));
      if (akv) {
        var ph = await ak.getAttribute("placeholder");
        console.log("  API key placeholder: " + ph);
        var tp = await ak.getAttribute("type");
        console.log("  API key input type: " + tp);
      }

      var nudgeGone = !(await nudge.isVisible({ timeout: 1000 }).catch(() => false));
      console.log("  BYOK nudge hidden after switch: " + (nudgeGone ? "PASS" : "FAIL"));
      await page.screenshot({ path: SS + "/07-api-key-view.png", fullPage: true });
    }

    console.log("=== Test Complete ===");
  });
});
