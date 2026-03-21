#!/usr/bin/env node
/**
 * sentinel-hook.cjs — Session-start autonomous bug detection
 *
 * Runs the sentinel scanner and outputs deployment instructions
 * for Claude to follow. Critical signals trigger expert deployment.
 *
 * Hook type: UserPromptSubmit
 * Behavior: Never blocks (always PASS). Outputs actionable alerts.
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const SCANNER = path.join(ROOT, "scripts", "sentinel-scan.cjs");
const STATE_PATH = path.join(ROOT, ".claude", "wisdom", "sentinel-state.json");
const STALE_MS = 30 * 60 * 1000; // 30 minutes — don't re-scan within this window

function isStale() {
  try {
    const state = JSON.parse(fs.readFileSync(STATE_PATH, "utf-8"));
    const age = Date.now() - new Date(state.lastScan).getTime();
    return age > STALE_MS;
  } catch {
    return true; // No state = stale
  }
}

try {
  if (!isStale()) {
    // Recent scan exists — check if it found anything critical
    const state = JSON.parse(fs.readFileSync(STATE_PATH, "utf-8"));
    if (state.counts.critical === 0 && state.counts.warning === 0) {
      console.log("PASS\nSentinel: all clear (recent scan).");
      process.exit(0);
    }
    // Fall through to report existing findings
  } else {
    // Run fresh scan
    execSync(`node "${SCANNER}" --json`, {
      cwd: ROOT,
      timeout: 180000, // 3 min max for full scan
      stdio: ["pipe", "pipe", "pipe"],
    });
  }

  // Read results
  const state = JSON.parse(fs.readFileSync(STATE_PATH, "utf-8"));
  const critical = state.signals.filter((s) => s.severity === "critical");
  const warnings = state.signals.filter((s) => s.severity === "warning");

  if (critical.length === 0 && warnings.length === 0) {
    console.log("PASS\nSentinel: all clear.");
    process.exit(0);
  }

  // Build deployment instructions
  let output = "PASS\n";

  if (critical.length > 0) {
    output += "SENTINEL ALERT — CRITICAL ISSUES DETECTED\n";
    output += "The following issues require immediate expert investigation:\n\n";

    // Deduplicate experts needed
    const expertsNeeded = new Map();
    for (const s of critical) {
      if (s.expert) {
        if (!expertsNeeded.has(s.expert)) {
          expertsNeeded.set(s.expert, []);
        }
        expertsNeeded.get(s.expert).push(s);
      }
    }

    for (const [expert, sigs] of expertsNeeded) {
      const specFile = expert + ".md";
      output += `DEPLOY ${expert}:\n`;
      for (const s of sigs) {
        output += `  Signal: ${s.message}\n`;
        if (s.detail) {
          output += `  Detail: ${s.detail.split("\n")[0]}\n`;
        }
      }
      output += `  Spec: .claude/agents/${specFile}\n`;
      output += `  Action: Read the agent spec, then investigate the signal(s) above.\n\n`;
    }

    // Critical signals without specific expert → War Room
    const unrouted = critical.filter((s) => !s.expert);
    if (unrouted.length > 0) {
      output += "UNROUTED CRITICAL SIGNALS (consider War Room):\n";
      for (const s of unrouted) {
        output += `  [${s.domain}] ${s.message}\n`;
      }
      output += "\n";
    }
  }

  if (warnings.length > 0) {
    output += `SENTINEL WARNINGS (${warnings.length}):\n`;
    for (const s of warnings) {
      output += `  [${s.domain}] ${s.message}\n`;
    }
    output += "\n";
  }

  console.log(output);
} catch (err) {
  // Never block session start
  console.log(`PASS\nSentinel: scan skipped (${err.message})`);
}
