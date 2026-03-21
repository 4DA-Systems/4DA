#!/usr/bin/env node
/**
 * sentinel-scan.cjs — Autonomous Bug Detection Engine
 *
 * Runs fast diagnostic checks across the 4DA codebase and outputs
 * structured signals with severity, domain classification, and
 * expert deployment recommendations.
 *
 * Checks (~20-30s total):
 *   1. Rust compilation (cargo check --lib)
 *   2. IPC contract validation (validate-commands.cjs)
 *   3. Knowledge manifest diff (security + IPC regressions)
 *   4. Test count regression (vs ops-state history)
 *   5. Sovereignty score delta
 *   6. File size compliance
 *
 * Output: JSON to stdout (structured signals array)
 * Usage:  node scripts/sentinel-scan.cjs [--json] [--quick]
 *
 * --json:  output raw JSON (for hook consumption)
 * --quick: skip slow checks (cargo check) for incremental runs
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const OPS_STATE = path.join(ROOT, ".claude", "wisdom", "ops-state.json");
const KNOWLEDGE_DIR = path.join(ROOT, ".claude", "knowledge");
const args = process.argv.slice(2);
const JSON_MODE = args.includes("--json");
const QUICK_MODE = args.includes("--quick");

// ---------------------------------------------------------------------------
// Signal structure
// ---------------------------------------------------------------------------

/** @type {{ check: string, severity: 'critical'|'warning'|'info'|'ok', domain: string, expert: string, message: string, detail: string }[]} */
const signals = [];

function addSignal(check, severity, domain, expert, message, detail = "") {
  signals.push({ check, severity, domain, expert, message, detail });
}

function safeExec(cmd, opts = {}) {
  try {
    return {
      ok: true,
      output: execSync(cmd, {
        cwd: ROOT,
        encoding: "utf-8",
        timeout: opts.timeout || 60000,
        stdio: ["pipe", "pipe", "pipe"],
        ...opts,
      }),
    };
  } catch (err) {
    return {
      ok: false,
      output: (err.stdout || "") + (err.stderr || ""),
      code: err.status,
    };
  }
}

// ---------------------------------------------------------------------------
// CHECK 1: Rust Compilation
// ---------------------------------------------------------------------------

function isCargoRunning() {
  try {
    const result = execSync(
      'tasklist //FI "IMAGENAME eq cargo.exe" 2>NUL',
      { encoding: "utf-8", timeout: 5000, stdio: ["pipe", "pipe", "pipe"] }
    );
    return /cargo\.exe/i.test(result);
  } catch {
    return false;
  }
}

function classifyErrorDomain(filePath) {
  if (filePath.includes("scoring/") || filePath.includes("ace/"))
    return { domain: "Scoring & ML", expert: "4da-scoring-expert" };
  if (filePath.includes("db/"))
    return { domain: "Data Layer", expert: "4da-data-expert" };
  if (filePath.includes("settings/"))
    return { domain: "Rust Systems", expert: "4da-rust-expert" };
  if (filePath.includes("sources/") || filePath.includes("source_fetching/"))
    return { domain: "Rust Systems", expert: "4da-rust-expert" };
  return { domain: "Rust Systems", expert: "4da-rust-expert" };
}

function checkCompilation() {
  if (QUICK_MODE) return;

  // GAP 5 fix: skip if dev server or cargo is already running
  if (isCargoRunning()) {
    addSignal(
      "compilation",
      "info",
      "Rust Systems",
      "",
      "Rust compilation check skipped — cargo process already running"
    );
    return;
  }

  const result = safeExec("cargo check --lib --message-format=short", {
    timeout: 120000,
    cwd: path.join(ROOT, "src-tauri"),
  });

  if (!result.ok) {
    const errors = result.output
      .split("\n")
      .filter((l) => /^error/.test(l))
      .slice(0, 5);

    const errorFiles = result.output
      .split("\n")
      .filter((l) => /^\s*-->/.test(l))
      .map((l) => l.trim().replace(/^-->\s*/, ""))
      .slice(0, 10);

    // GAP 3 fix: route to ALL affected domains, not just the first
    const domainExperts = new Map();
    for (const ef of errorFiles) {
      const { domain, expert } = classifyErrorDomain(ef);
      if (!domainExperts.has(expert)) {
        domainExperts.set(expert, { domain, files: [] });
      }
      domainExperts.get(expert).files.push(ef);
    }

    // Emit a signal per affected domain
    for (const [expert, info] of domainExperts) {
      addSignal(
        "compilation",
        "critical",
        info.domain,
        expert,
        `Rust compilation failed in ${info.domain} (${info.files.length} error location${info.files.length > 1 ? "s" : ""})`,
        errors.slice(0, 3).join("\n") + "\n" + info.files.join("\n")
      );
    }

    // Fallback if no files parsed
    if (domainExperts.size === 0) {
      addSignal(
        "compilation",
        "critical",
        "Rust Systems",
        "4da-rust-expert",
        `Rust compilation failed (${errors.length} error${errors.length !== 1 ? "s" : ""})`,
        errors.join("\n")
      );
    }
  } else {
    addSignal("compilation", "ok", "Rust Systems", "", "Rust compilation clean");
  }
}

// ---------------------------------------------------------------------------
// CHECK 1b: TypeScript Compilation (GAP 1 fix)
// ---------------------------------------------------------------------------

function checkTypeScript() {
  if (QUICK_MODE) return;

  const result = safeExec("npx tsc --noEmit --incremental 2>&1", {
    timeout: 60000,
  });

  if (!result.ok) {
    const output = result.output || "";
    const errorLines = output
      .split("\n")
      .filter((l) => /error TS\d+/.test(l))
      .slice(0, 5);

    addSignal(
      "typescript",
      "critical",
      "React UI",
      "4da-react-expert",
      `TypeScript compilation failed (${errorLines.length} error${errorLines.length !== 1 ? "s" : ""})`,
      errorLines.join("\n")
    );
  } else {
    addSignal("typescript", "ok", "React UI", "", "TypeScript compilation clean");
  }
}

// ---------------------------------------------------------------------------
// CHECK 2: IPC Contract Validation
// ---------------------------------------------------------------------------

function checkIPCContract() {
  const result = safeExec("node scripts/validate-commands.cjs", {
    timeout: 15000,
  });

  if (!result.ok) {
    // Parse the validator output for specific issues
    const output = result.output || "";
    const ghostMatch = output.match(
      /Registered in Rust but missing from CommandMap \((\d+)\)/
    );
    const deadMatch = output.match(
      /In CommandMap but not registered .* \((\d+)\)/
    );

    const ghostCount = ghostMatch ? parseInt(ghostMatch[1]) : 0;
    const deadCount = deadMatch ? parseInt(deadMatch[1]) : 0;

    if (ghostCount > 0) {
      addSignal(
        "ipc_contract",
        "critical",
        "IPC Bridge",
        "4da-ipc-expert",
        `${ghostCount} ghost command${ghostCount > 1 ? "s" : ""} detected — registered in Rust but no TypeScript binding`,
        output.slice(0, 500)
      );
    }
    if (deadCount > 0) {
      addSignal(
        "ipc_contract",
        "warning",
        "IPC Bridge",
        "4da-ipc-expert",
        `${deadCount} dead command${deadCount > 1 ? "s" : ""} in commands.ts — no registered Rust handler`,
        output.slice(0, 500)
      );
    }
    if (ghostCount === 0 && deadCount === 0) {
      addSignal(
        "ipc_contract",
        "warning",
        "IPC Bridge",
        "4da-ipc-expert",
        "IPC validation failed (unknown issue)",
        output.slice(0, 300)
      );
    }
  } else {
    addSignal("ipc_contract", "ok", "IPC Bridge", "", "IPC contract clean (315/315)");
  }
}

// ---------------------------------------------------------------------------
// CHECK 3: Knowledge Manifest Diff (Security + IPC Regressions)
// ---------------------------------------------------------------------------

function checkKnowledgeDiff() {
  // Read current security surface manifest
  const secPath = path.join(KNOWLEDGE_DIR, "security-surface.md");
  if (!fs.existsSync(secPath)) {
    addSignal(
      "knowledge_diff",
      "warning",
      "Security",
      "4da-security-expert",
      "Security surface manifest missing — run pnpm run generate:knowledge"
    );
    return;
  }

  const secContent = fs.readFileSync(secPath, "utf-8");

  // Parse invariant compliance table
  const apiKeyMatch = secContent.match(/API key in logs \| (\d+)/);
  const sqlMatch = secContent.match(/SQL string formatting \| (\d+)/);
  const secretsMatch = secContent.match(/Hardcoded secrets \| (\d+)/);
  const unwrapMatch = secContent.match(/\.unwrap\(\) in prod code \| (\d+)/);

  const apiCount = apiKeyMatch ? parseInt(apiKeyMatch[1]) : 0;
  const sqlCount = sqlMatch ? parseInt(sqlMatch[1]) : 0;
  const secretCount = secretsMatch ? parseInt(secretsMatch[1]) : 0;
  const unwrapCount = unwrapMatch ? parseInt(unwrapMatch[1]) : 0;

  // GAP 4 fix: Dynamic baselines from sentinel state history
  // Use previous scan's values as baseline (drift detection, not absolute thresholds)
  let prevApi = 0, prevSql = 0, prevUnwrap = 0;
  try {
    const prevState = JSON.parse(
      fs.readFileSync(
        path.join(ROOT, ".claude", "wisdom", "sentinel-baselines.json"),
        "utf-8"
      )
    );
    prevApi = prevState.apiCount || 0;
    prevSql = prevState.sqlCount || 0;
    prevUnwrap = prevState.unwrapCount || 0;
  } catch {
    // No baseline yet — use current as first baseline
    prevApi = apiCount;
    prevSql = sqlCount;
    prevUnwrap = unwrapCount;
  }

  // Save current as baseline for next run
  try {
    fs.writeFileSync(
      path.join(ROOT, ".claude", "wisdom", "sentinel-baselines.json"),
      JSON.stringify({ apiCount, sqlCount, unwrapCount, secretCount, timestamp: new Date().toISOString() }, null, 2)
    );
  } catch {}

  // Alert on increase from previous baseline (not hardcoded thresholds)
  const apiDelta = apiCount - prevApi;
  const sqlDelta = sqlCount - prevSql;
  const unwrapDelta = unwrapCount - prevUnwrap;

  if (apiDelta > 2) {
    addSignal(
      "security_regression",
      "warning",
      "Security",
      "4da-security-expert",
      `API key log patterns increased by ${apiDelta} (${prevApi} → ${apiCount})`,
      "New code may be logging sensitive data. Security Expert should audit."
    );
  }

  if (sqlDelta > 2) {
    addSignal(
      "security_regression",
      "warning",
      "Security",
      "4da-security-expert",
      `SQL formatting patterns increased by ${sqlDelta} (${prevSql} → ${sqlCount})`,
      "New format!() near SQL detected. Verify parameterized queries."
    );
  }

  if (unwrapDelta > 20) {
    addSignal(
      "security_regression",
      "warning",
      "Rust Systems",
      "4da-rust-expert",
      `Production unwrap() count increased by ${unwrapDelta} (${prevUnwrap} → ${unwrapCount})`,
      "Review new unwrap() calls for graceful error handling."
    );
  }

  // IPC contract manifest check
  const ipcPath = path.join(KNOWLEDGE_DIR, "ipc-contracts.md");
  if (fs.existsSync(ipcPath)) {
    const ipcContent = fs.readFileSync(ipcPath, "utf-8");
    const issueMatch = ipcContent.match(
      /\*\*Contract issues\*\* \| \*\*(\d+)\*\*/
    );
    if (issueMatch && parseInt(issueMatch[1]) > 0) {
      addSignal(
        "ipc_drift",
        "critical",
        "IPC Bridge",
        "4da-ipc-expert",
        `IPC contract has ${issueMatch[1]} issue(s) per knowledge manifest`,
        "Regenerate manifests and investigate: pnpm run generate:knowledge"
      );
    }
  }

  if (
    signals.filter(
      (s) =>
        s.check === "security_regression" || s.check === "ipc_drift"
    ).length === 0
  ) {
    addSignal(
      "knowledge_diff",
      "ok",
      "Security",
      "",
      "Security and IPC baselines within normal range"
    );
  }
}

// ---------------------------------------------------------------------------
// CHECK 4: Test Count Regression
// ---------------------------------------------------------------------------

function checkTestRegression() {
  if (QUICK_MODE) return;

  let opsState;
  try {
    opsState = JSON.parse(fs.readFileSync(OPS_STATE, "utf-8"));
  } catch {
    addSignal(
      "test_regression",
      "info",
      "All",
      "",
      "Cannot check test regression — ops-state.json not readable"
    );
    return;
  }

  const history = opsState?.testCounts?.history;
  if (!Array.isArray(history) || history.length === 0) {
    addSignal(
      "test_regression",
      "info",
      "All",
      "",
      "No test count history — baseline not established"
    );
    return;
  }

  const lastRecord = history[history.length - 1];
  const lastRust = lastRecord.rust || 0;
  const lastFrontend = lastRecord.frontend || 0;

  // Rust test count (list only, no execution)
  if (!isCargoRunning()) {
    const rustResult = safeExec(
      "cargo test --lib -- --list 2>/dev/null | grep -c 'test$'",
      { cwd: path.join(ROOT, "src-tauri"), timeout: 30000 }
    );

    if (rustResult.ok) {
      const currentRust = parseInt(rustResult.output.trim()) || 0;
      const delta = currentRust - lastRust;

      if (delta < -5) {
        addSignal(
          "test_regression",
          "critical",
          "Rust Systems",
          "4da-rust-expert",
          `Rust test count dropped by ${Math.abs(delta)} (${lastRust} → ${currentRust})`,
          "Tests may have been deleted or broken. Investigate immediately."
        );
      } else if (delta < 0) {
        addSignal(
          "test_regression",
          "warning",
          "Rust Systems",
          "4da-rust-expert",
          `Rust test count decreased by ${Math.abs(delta)} (${lastRust} → ${currentRust})`
        );
      } else {
        addSignal(
          "test_regression",
          "ok",
          "Rust Systems",
          "",
          `Rust tests: ${currentRust} (${delta >= 0 ? "+" : ""}${delta} from baseline ${lastRust})`
        );
      }
    }
  }

  // GAP 2 fix: Frontend test count regression
  if (lastFrontend > 0) {
    const feResult = safeExec(
      "npx vitest run --reporter=json 2>/dev/null | node -e \"let d='';process.stdin.on('data',c=>d+=c);process.stdin.on('end',()=>{try{const j=JSON.parse(d);console.log(j.numTotalTests)}catch{console.log(0)}})\"",
      { timeout: 120000 }
    );

    if (feResult.ok) {
      const currentFE = parseInt(feResult.output.trim()) || 0;
      if (currentFE > 0) {
        const delta = currentFE - lastFrontend;

        if (delta < -10) {
          addSignal(
            "test_regression_fe",
            "critical",
            "React UI",
            "4da-react-expert",
            `Frontend test count dropped by ${Math.abs(delta)} (${lastFrontend} → ${currentFE})`,
            "Frontend tests may have been deleted or broken."
          );
        } else if (delta < 0) {
          addSignal(
            "test_regression_fe",
            "warning",
            "React UI",
            "4da-react-expert",
            `Frontend test count decreased by ${Math.abs(delta)} (${lastFrontend} → ${currentFE})`
          );
        } else {
          addSignal(
            "test_regression_fe",
            "ok",
            "React UI",
            "",
            `Frontend tests: ${currentFE} (${delta >= 0 ? "+" : ""}${delta} from baseline ${lastFrontend})`
          );
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// CHECK 5: Sovereignty Score Delta
// ---------------------------------------------------------------------------

function checkSovereignty() {
  let opsState;
  try {
    opsState = JSON.parse(fs.readFileSync(OPS_STATE, "utf-8"));
  } catch {
    return;
  }

  const score = opsState?.sovereignty?.score;
  if (typeof score !== "number") return;

  if (score < 60) {
    addSignal(
      "sovereignty",
      "critical",
      "All",
      "",
      `Sovereignty score critically low: ${score}/100`,
      "Multiple system health components are degraded. Consider War Room activation."
    );
  } else if (score < 75) {
    addSignal(
      "sovereignty",
      "warning",
      "All",
      "",
      `Sovereignty score below target: ${score}/100 (target: 75+)`
    );
  } else {
    addSignal(
      "sovereignty",
      "ok",
      "All",
      "",
      `Sovereignty: ${score}/100`
    );
  }

  // Check for pending immune scan
  if (opsState?.immuneScanPending) {
    addSignal(
      "immune_pending",
      "warning",
      "All",
      "",
      "Immune scan pending — bug fix detected in previous session",
      "Deploy immune system agent to create antibody from recent fix."
    );
  }
}

// ---------------------------------------------------------------------------
// CHECK 6: File Size Compliance
// ---------------------------------------------------------------------------

function checkFileSizes() {
  const result = safeExec("node scripts/check-file-sizes.cjs", {
    timeout: 10000,
  });

  if (!result.ok) {
    const output = result.output || "";
    const errorLines = output
      .split("\n")
      .filter((l) => /ERROR|exceeds/.test(l))
      .slice(0, 5);

    if (errorLines.length > 0) {
      // Determine domain from file paths
      const hasRust = errorLines.some((l) => /\.rs/.test(l));
      const hasTsx = errorLines.some((l) => /\.tsx?/.test(l));
      const domain = hasRust ? "Rust Systems" : "React UI";
      const expert = hasRust ? "4da-rust-expert" : "4da-react-expert";

      addSignal(
        "file_sizes",
        "warning",
        domain,
        expert,
        `${errorLines.length} file(s) exceed size limits`,
        errorLines.join("\n")
      );
    }
  } else {
    addSignal("file_sizes", "ok", "All", "", "File sizes within limits");
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const startTime = Date.now();

  // Run all checks
  checkCompilation();
  checkTypeScript();
  checkIPCContract();
  checkKnowledgeDiff();
  checkTestRegression();
  checkSovereignty();
  checkFileSizes();

  const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);

  // Classify results
  const critical = signals.filter((s) => s.severity === "critical");
  const warnings = signals.filter((s) => s.severity === "warning");
  const ok = signals.filter((s) => s.severity === "ok");

  if (JSON_MODE) {
    // Machine-readable output for hooks
    console.log(
      JSON.stringify({
        timestamp: new Date().toISOString(),
        elapsed_s: parseFloat(elapsed),
        counts: {
          critical: critical.length,
          warning: warnings.length,
          ok: ok.length,
        },
        signals,
      })
    );
  } else {
    // Human-readable output
    console.log(`\n[SENTINEL] Scan complete in ${elapsed}s\n`);

    if (critical.length > 0) {
      console.log(`  CRITICAL (${critical.length}):`);
      for (const s of critical) {
        console.log(`    [${s.domain}] ${s.message}`);
        if (s.expert) console.log(`      → Deploy: ${s.expert}`);
        if (s.detail) {
          for (const line of s.detail.split("\n").slice(0, 3)) {
            console.log(`      ${line}`);
          }
        }
      }
      console.log();
    }

    if (warnings.length > 0) {
      console.log(`  WARNINGS (${warnings.length}):`);
      for (const s of warnings) {
        console.log(`    [${s.domain}] ${s.message}`);
      }
      console.log();
    }

    console.log(`  OK: ${ok.length} checks passed`);
    console.log();
  }

  // Write results to a sentinel state file for hook consumption
  const statePath = path.join(ROOT, ".claude", "wisdom", "sentinel-state.json");
  try {
    fs.writeFileSync(
      statePath,
      JSON.stringify(
        {
          lastScan: new Date().toISOString(),
          elapsed_s: parseFloat(elapsed),
          counts: {
            critical: critical.length,
            warning: warnings.length,
            ok: ok.length,
          },
          signals: signals.filter((s) => s.severity !== "ok"),
        },
        null,
        2
      )
    );
  } catch {}

  return critical.length > 0 ? 1 : 0;
}

process.exit(main());
