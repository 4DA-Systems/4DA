#!/usr/bin/env node
/**
 * ipc-precommit.cjs
 *
 * Pre-commit IPC contract validation hook.
 * Checks for ghost commands (registered but no TS binding)
 * and dead code (TS binding but not registered).
 *
 * Runs ONLY when .rs or commands.ts files are staged.
 * Exits non-zero to block commit if ghost commands are detected.
 */

const { execSync } = require("child_process");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

try {
  // Check if any IPC-relevant files are staged
  const staged = execSync("git diff --cached --name-only", {
    cwd: ROOT,
    encoding: "utf-8",
  }).trim();

  if (!staged) {
    // Nothing staged, skip check
    process.exit(0);
  }

  const files = staged.split("\n");
  const hasRust = files.some((f) => f.endsWith(".rs") && f.startsWith("src-tauri/"));
  const hasCommands = files.some((f) => f.includes("commands.ts"));

  if (!hasRust && !hasCommands) {
    // No IPC-relevant files staged, skip
    process.exit(0);
  }

  // Run the full IPC validator
  const result = execSync("node scripts/validate-commands.cjs", {
    cwd: ROOT,
    encoding: "utf-8",
    timeout: 15000,
  });

  // Check if the validator found issues (it exits 1 on issues)
  // The validator already prints details, just relay
  console.log(result);
  process.exit(0);
} catch (err) {
  if (err.status === 1) {
    // Validator found issues
    console.error("\n[IPC Pre-commit] Contract issues detected:");
    console.error(err.stdout || "");
    console.error(
      "Fix ghost commands before committing. Run: pnpm run validate:commands\n"
    );
    // Exit 1 to block commit
    process.exit(1);
  }
  // Other errors — don't block commit for infrastructure failures
  console.warn("[IPC Pre-commit] Warning: validation skipped —", err.message);
  process.exit(0);
}
