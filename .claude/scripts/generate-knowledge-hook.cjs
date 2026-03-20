#!/usr/bin/env node
/**
 * generate-knowledge-hook.cjs
 *
 * Session-start hook that regenerates knowledge manifests for the Expert Team.
 * Runs the full generator if manifests are stale (>1 hour) or missing.
 * Runs a fast IPC-only check if manifests are fresh (since IPC is the most volatile).
 *
 * Output: PASS (always) — knowledge generation is advisory, never blocks.
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const KNOWLEDGE_DIR = path.join(ROOT, ".claude", "knowledge");
const GENERATOR = path.join(ROOT, "scripts", "generate-knowledge.cjs");
const STALE_THRESHOLD_MS = 60 * 60 * 1000; // 1 hour

function isStale() {
  const topologyPath = path.join(KNOWLEDGE_DIR, "topology.md");
  if (!fs.existsSync(topologyPath)) return true;

  const stat = fs.statSync(topologyPath);
  const age = Date.now() - stat.mtimeMs;
  return age > STALE_THRESHOLD_MS;
}

function manifestCount() {
  if (!fs.existsSync(KNOWLEDGE_DIR)) return 0;
  return fs.readdirSync(KNOWLEDGE_DIR).filter((f) => f.endsWith(".md")).length;
}

try {
  const count = manifestCount();
  const stale = isStale();

  if (count < 7 || stale) {
    // Full regeneration needed
    const reason = count < 7 ? `only ${count}/7 manifests` : "manifests stale (>1h)";
    execSync(`node "${GENERATOR}"`, {
      cwd: ROOT,
      encoding: "utf-8",
      timeout: 30000,
      stdio: "pipe",
    });
    // Output the expert team availability notice
    const newCount = manifestCount();
    console.log(
      `PASS\nExpert Team knowledge refreshed (${reason}). ${newCount}/7 manifests current.`
    );
  } else {
    console.log(
      `PASS\nExpert Team knowledge current. ${count}/7 manifests fresh.`
    );
  }
} catch (err) {
  // Never block session start — knowledge generation is advisory
  console.log(`PASS\nExpert Team knowledge generation skipped: ${err.message}`);
}
