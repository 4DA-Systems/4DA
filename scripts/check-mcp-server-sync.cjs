#!/usr/bin/env node
/**
 * MCP Server Sync Check
 *
 * Detects when changes in the 4DA codebase require an MCP server update.
 * Run manually or hook into session start.
 *
 * Checks:
 *   1. Database schema changes (new columns/tables the MCP server reads)
 *   2. Scoring pipeline changes (affects get_relevant_content, get_actionable_signals)
 *   3. New Tauri commands that could be exposed via MCP
 *   4. Version drift (package.json vs Cargo.toml)
 *   5. MCP server build staleness (src newer than dist)
 *
 * Exit codes:
 *   0 = in sync
 *   1 = update needed (prints reasons)
 */

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const MCP_DIR = path.join(ROOT, "mcp-4da-server");
const MCP_SRC = path.join(MCP_DIR, "src");
const MCP_DIST = path.join(MCP_DIR, "dist");

const issues = [];
const warnings = [];

// ─── 1. Database Schema Changes ─────────────────────────────────────────────
// The MCP server reads from specific tables/columns. If migrations add new
// columns the server could use, or change existing ones, flag it.

const DB_TABLES_MCP_READS = [
  "source_items",
  "user_interests",
  "user_tech_stack",
  "user_topics",
  "content_feedback",
  "project_dependencies",
  "ace_contexts",
  "developer_decisions",
  "agent_memory",
  "decision_windows",
  "learned_preferences",
  "learned_anti_preferences",
];

function checkDatabaseChanges() {
  try {
    // Check git diff for migration or schema files since last MCP server commit
    const lastMcpCommit = execSync(
      'git log -1 --format=%H -- mcp-4da-server/',
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (!lastMcpCommit) return;

    // Check for schema-related changes since that commit
    const schemaFiles = execSync(
      `git diff --name-only ${lastMcpCommit}..HEAD -- "src-tauri/src/db/" "src-tauri/migrations/"`,
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (schemaFiles) {
      const files = schemaFiles.split("\n").filter(Boolean);
      if (files.length > 0) {
        warnings.push(
          `Database layer changed since last MCP update (${files.length} file(s)):\n` +
          files.map(f => `    ${f}`).join("\n") +
          "\n    Check if MCP server queries need updating."
        );
      }
    }
  } catch {
    // git commands may fail in some environments
  }
}

// ─── 2. Scoring Pipeline Changes ────────────────────────────────────────────
// The MCP server reads relevance_score from the DB. If scoring logic changes,
// the scores may have different semantics.

function checkScoringChanges() {
  try {
    const lastMcpCommit = execSync(
      'git log -1 --format=%H -- mcp-4da-server/',
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (!lastMcpCommit) return;

    const scoringFiles = execSync(
      `git diff --name-only ${lastMcpCommit}..HEAD -- "src-tauri/src/scoring/"`,
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (scoringFiles) {
      const files = scoringFiles.split("\n").filter(Boolean);
      if (files.length > 3) {
        warnings.push(
          `Scoring pipeline has ${files.length} changed files since last MCP update.\n` +
          "    MCP tools that read relevance_score may need threshold adjustments."
        );
      }
    }
  } catch {
    // Non-fatal
  }
}

// ─── 3. New Tauri Commands ──────────────────────────────────────────────────
// If new #[tauri::command] handlers are added, they might be candidates for
// new MCP tools.

function checkNewCommands() {
  try {
    const lastMcpCommit = execSync(
      'git log -1 --format=%H -- mcp-4da-server/',
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (!lastMcpCommit) return;

    // Count new tauri::command annotations
    const diff = execSync(
      `git diff ${lastMcpCommit}..HEAD -- "src-tauri/src/" | grep "^+" | grep "#\\[tauri::command\\]" || true`,
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (diff) {
      const count = diff.split("\n").filter(Boolean).length;
      if (count > 0) {
        warnings.push(
          `${count} new #[tauri::command] handler(s) added since last MCP update.\n` +
          "    Review if any should become MCP tools."
        );
      }
    }
  } catch {
    // Non-fatal
  }
}

// ─── 4. Version Drift ───────────────────────────────────────────────────────
// MCP server version should track meaningful releases.

function checkVersionDrift() {
  try {
    const mcpPkg = JSON.parse(
      fs.readFileSync(path.join(MCP_DIR, "package.json"), "utf-8")
    );
    const mcpVersion = mcpPkg.version;

    // Check if index.ts has matching version strings
    const indexTs = fs.readFileSync(path.join(MCP_SRC, "index.ts"), "utf-8");

    const serverVersionMatch = indexTs.match(/version:\s*"([^"]+)"/);
    if (serverVersionMatch && serverVersionMatch[1] !== mcpVersion) {
      issues.push(
        `Version mismatch: package.json says ${mcpVersion} but Server constructor says ${serverVersionMatch[1]}`
      );
    }

    const cliVersionMatch = indexTs.match(/@4da\/mcp-server\s+([\d.]+)/);
    if (cliVersionMatch && cliVersionMatch[1] !== mcpVersion) {
      issues.push(
        `Version mismatch: package.json says ${mcpVersion} but --version CLI says ${cliVersionMatch[1]}`
      );
    }

    // Check server.json
    const serverJson = JSON.parse(
      fs.readFileSync(path.join(MCP_DIR, "server.json"), "utf-8")
    );
    if (serverJson.version !== mcpVersion) {
      issues.push(
        `Version mismatch: package.json says ${mcpVersion} but server.json says ${serverJson.version}`
      );
    }
  } catch (e) {
    warnings.push(`Could not check versions: ${e.message}`);
  }
}

// ─── 5. Build Staleness ─────────────────────────────────────────────────────
// If src/ is newer than dist/, the server needs rebuilding.

function checkBuildStaleness() {
  if (!fs.existsSync(MCP_DIST)) {
    issues.push("MCP server dist/ directory missing — run: cd mcp-4da-server && pnpm run build");
    return;
  }

  try {
    // Get newest src file mtime
    const srcFiles = execSync(
      `find "${MCP_SRC}" -name "*.ts" -not -name "*.test.ts" -newer "${MCP_DIST}/index.js" 2>/dev/null || true`,
      { cwd: ROOT, encoding: "utf-8" }
    ).trim();

    if (srcFiles) {
      const staleFiles = srcFiles.split("\n").filter(Boolean);
      if (staleFiles.length > 0) {
        issues.push(
          `MCP server build is stale — ${staleFiles.length} source file(s) newer than dist.\n` +
          "    Run: cd mcp-4da-server && pnpm run build"
        );
      }
    }
  } catch {
    // find may not be available on all platforms
  }
}

// ─── 6. Tool Count Consistency ──────────────────────────────────────────────
// Verify registry, dispatch, and schemas all agree on tool count.

function checkToolConsistency() {
  try {
    const registryTs = fs.readFileSync(
      path.join(MCP_SRC, "schema-registry.ts"), "utf-8"
    );
    const registryKeys = registryTs.match(/^\s+\w+:\s*\{\s*$/gm);
    const registryCount = registryKeys ? registryKeys.length : 0;

    const dispatchTs = fs.readFileSync(
      path.join(MCP_SRC, "tool-dispatch.ts"), "utf-8"
    );
    const dispatchKeys = dispatchTs.match(/^\s+(\w+):\s*(?:\(|execute)/gm);
    const dispatchCount = dispatchKeys ? dispatchKeys.length : 0;

    const schemaDir = path.join(MCP_SRC, "schemas");
    const schemaCount = fs.existsSync(schemaDir)
      ? fs.readdirSync(schemaDir).filter(f => f.endsWith(".json")).length
      : 0;

    if (registryCount !== dispatchCount) {
      issues.push(
        `Tool count mismatch: registry has ${registryCount} but dispatch has ${dispatchCount}`
      );
    }
    if (registryCount !== schemaCount) {
      issues.push(
        `Tool count mismatch: registry has ${registryCount} but schemas has ${schemaCount}`
      );
    }
  } catch (e) {
    warnings.push(`Could not check tool consistency: ${e.message}`);
  }
}

// ─── Run All Checks ─────────────────────────────────────────────────────────

checkDatabaseChanges();
checkScoringChanges();
checkNewCommands();
checkVersionDrift();
checkBuildStaleness();
checkToolConsistency();

// ─── Report ─────────────────────────────────────────────────────────────────

if (issues.length === 0 && warnings.length === 0) {
  console.log("[MCP Sync] ✓ MCP server is in sync with codebase");
  process.exit(0);
}

if (warnings.length > 0) {
  console.log(`[MCP Sync] ${warnings.length} warning(s):`);
  for (const w of warnings) {
    console.log(`  ⚠ ${w}`);
  }
}

if (issues.length > 0) {
  console.log(`[MCP Sync] ${issues.length} issue(s) requiring attention:`);
  for (const i of issues) {
    console.log(`  ✗ ${i}`);
  }
  process.exit(1);
}

process.exit(0);
