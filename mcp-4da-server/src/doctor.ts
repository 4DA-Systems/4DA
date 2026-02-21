/**
 * 4DA MCP Doctor — Installation health checker
 *
 * Validates that the MCP server can run correctly:
 * 1. Node.js version is sufficient
 * 2. Native bindings (better-sqlite3) load successfully
 * 3. Database is found and readable
 * 4. LLM providers are configured (optional)
 *
 * Usage:
 *   npx @4da/mcp-server --doctor
 *   4da-mcp --doctor
 */

import { existsSync } from "node:fs";
import { FourDADatabase } from "./db.js";

interface Check {
  name: string;
  status: "pass" | "warn" | "fail";
  detail: string;
}

export function runDoctor(): void {
  console.log("\n  4DA MCP Server — Doctor\n");

  const checks: Check[] = [];

  // 1. Node.js version
  const nodeVersion = process.version;
  const major = parseInt(nodeVersion.slice(1).split(".")[0], 10);
  checks.push({
    name: "Node.js version",
    status: major >= 18 ? "pass" : "fail",
    detail: major >= 18
      ? `${nodeVersion} (>= 18 required)`
      : `${nodeVersion} — Node.js 18+ required`,
  });

  // 2. Native bindings — if we reached here, the dynamic import in db.ts succeeded
  // (it calls process.exit(1) with a diagnostic message on failure)
  checks.push({
    name: "SQLite native bindings",
    status: "pass",
    detail: "better-sqlite3 loaded successfully",
  });

  // 3. Database discovery
  const validation = FourDADatabase.validateDatabase();
  checks.push({
    name: "4DA database",
    status: validation.valid ? "pass" : "warn",
    detail: validation.valid
      ? `Found with ${validation.tables?.length ?? 0} tables`
      : (validation.error ?? "Database not found — run 4DA desktop app first"),
  });

  // 4. Environment variables
  const envDb = process.env.FOURDA_DB_PATH;
  if (envDb) {
    const envExists = existsSync(envDb);
    checks.push({
      name: "FOURDA_DB_PATH",
      status: envExists ? "pass" : "fail",
      detail: envExists ? envDb : `Set to ${envDb} but file does not exist`,
    });
  }

  // 5. LLM providers (optional)
  const llmProvider = process.env.LLM_PROVIDER;
  const hasAnthropic = !!process.env.ANTHROPIC_API_KEY;
  const hasOpenAI = !!process.env.OPENAI_API_KEY;

  if (llmProvider || hasAnthropic || hasOpenAI) {
    checks.push({
      name: "LLM provider",
      status: "pass",
      detail: llmProvider
        ? `${llmProvider} configured`
        : hasAnthropic ? "Anthropic API key set" : "OpenAI API key set",
    });
  } else {
    checks.push({
      name: "LLM provider",
      status: "warn",
      detail: "No API keys set — AI synthesis tools will be unavailable",
    });
  }

  // Print results
  const icons = { pass: "\u2713", warn: "!", fail: "\u2717" };
  const colors = { pass: "\x1b[32m", warn: "\x1b[33m", fail: "\x1b[31m" };
  const reset = "\x1b[0m";

  for (const check of checks) {
    const icon = icons[check.status];
    const color = colors[check.status];
    console.log(`  ${color}${icon}${reset} ${check.name}: ${check.detail}`);
  }

  const failures = checks.filter((c) => c.status === "fail");
  const warnings = checks.filter((c) => c.status === "warn");

  console.log("");
  if (failures.length > 0) {
    console.log(`  ${failures.length} issue(s) need attention.\n`);
    process.exit(1);
  } else if (warnings.length > 0) {
    console.log(`  All clear with ${warnings.length} optional warning(s).\n`);
  } else {
    console.log("  All checks passed.\n");
  }
}
