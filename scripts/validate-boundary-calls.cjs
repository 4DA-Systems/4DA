/**
 * validate-boundary-calls.cjs
 *
 * Layer 2 of the Silent-Failure Defense Architecture (see
 * docs/strategy/SILENT-FAILURE-DEFENSE.md). Greps the codebase for
 * unverified boundary-crossing patterns and flags them.
 *
 * Patterns detected:
 *
 *   1. std::process::Command::new(...).output()  — CLI subprocess without
 *      a nearby status.success() check or stderr scan. The classic silent
 *      failure from the AWE `--stages receive` bug.
 *
 *   2. Hook/script setting `*Pending = true` without a corresponding
 *      `scanned*` dedup set. The idempotency amnesia pattern that made
 *      the immune-scan warning re-fire every session.
 *
 *   3. Raw Command::new("awe" | "ollama" | "git") outside known wrapper
 *      modules — once src-tauri/src/external/ is fully wired, these are
 *      forbidden. Currently WARN only until migration is complete.
 *
 * Usage:
 *   node scripts/validate-boundary-calls.cjs           # informational mode
 *   node scripts/validate-boundary-calls.cjs --strict  # exit 1 on any violation
 *   node scripts/validate-boundary-calls.cjs --json    # output JSON for hooks
 *
 * Exit codes:
 *   0 — no violations, or informational mode
 *   1 — violations found AND --strict was passed
 *
 * This validator is intentionally conservative (high false-negative rate
 * preferred over false-positive rate). It greps for the common patterns,
 * not every possible variant. The goal is to catch 95%+ of the pattern,
 * not 100%. The remaining 5% belongs in code review and Layer 1 type
 * enforcement.
 */

const fs = require("fs");
const path = require("path");

const args = process.argv.slice(2);
const STRICT = args.includes("--strict");
const JSON_MODE = args.includes("--json");

const ROOT = path.resolve(__dirname, "..");

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/**
 * Lookahead window: how many lines after a Command::new(...).output() call
 * we scan for a verification pattern. 10 is enough to catch most real
 * call sites; longer and we start matching unrelated code.
 */
const VERIFICATION_LOOKAHEAD = 10;

/**
 * Verification patterns — any one of these within VERIFICATION_LOOKAHEAD
 * lines of a Command::new(...).output() call marks the call as "verified".
 * Keep this list conservative — false positives here mean real bugs slip
 * through.
 */
const VERIFICATION_PATTERNS = [
  /\.status\.success\(\)/,
  /\.status\(\)\.success\(\)/,
  /!\s*output\.status/,
  /status_code|statuscode/i,
  /Unknown stage/i, // bug-1 regression test assertion
  /stderr.*contains|stderr.*to_string|String::from_utf8.*stderr/,
  /\bmatch\s+.*\.output/, // match expression immediately consuming output
  /run_awe_with_timeout/, // known-safe helper
  /AweClient::/, // known-safe typed wrapper (once landed)
];

/**
 * Paths to skip entirely. Test files, build output, vendored code, and
 * the wrapper module itself (it IS the wrapper, Command::new is expected).
 */
const SKIP_PATTERNS = [
  /node_modules/,
  /target\//,
  /dist\//,
  /src-tauri[\\/]bindings/,
  /src-tauri[\\/]src[\\/]external/, // the wrapper module itself
  /\.test\.|_tests?\.rs$|\btests?\b[\\/]/i,
];

/**
 * Known external binaries that SHOULD eventually go through the
 * src-tauri/src/external/ typed wrappers. Raw Command::new(...) for
 * these outside the wrapper module is a violation.
 *
 * Currently WARN only. Once migration is complete, promote to ERROR.
 */
const WRAPPED_BINARIES = new Set(["awe", "ollama"]);

// ---------------------------------------------------------------------------
// File walker
// ---------------------------------------------------------------------------

function walkDir(dir, extensions) {
  const results = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (SKIP_PATTERNS.some((re) => re.test(full))) continue;
    if (entry.isDirectory()) {
      results.push(...walkDir(full, extensions));
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name);
      if (extensions.includes(ext)) {
        results.push(full);
      }
    }
  }
  return results;
}

// ---------------------------------------------------------------------------
// Check 1: Unverified Command::new().output() calls in Rust
// ---------------------------------------------------------------------------

function checkUnverifiedCommandNew(rustFiles) {
  const violations = [];

  // Match both bare and std-prefixed forms. We want Command::new(...)
  // followed (on the same or subsequent lines, up to the closing .output())
  // by a verification pattern within VERIFICATION_LOOKAHEAD lines.
  const commandNewRegex = /(?:std::process::)?Command::new\s*\(\s*("?[\w&._-]+"?)/;

  for (const file of rustFiles) {
    let content;
    try {
      content = fs.readFileSync(file, "utf-8");
    } catch {
      continue;
    }
    const lines = content.split(/\r?\n/);

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const m = commandNewRegex.exec(line);
      if (!m) continue;

      // Extract the first-argument "binary name" from the match group.
      // Strip quotes, &, path prefixes. Keep only the basename identifier
      // so we can match against WRAPPED_BINARIES.
      let binary = m[1] || "";
      binary = binary.replace(/^&/, "").replace(/^"/, "").replace(/"$/, "");
      // Path variables (e.g. &awe_path, path) become the variable name;
      // we can't statically resolve them, but the name hints at the binary.
      const binaryLower = binary.toLowerCase();
      const looksLikeAwe = /awe/.test(binaryLower);
      const looksLikeOllama = /ollama/.test(binaryLower);

      // Look ahead for a verification pattern within the next N lines.
      const window = lines
        .slice(i, Math.min(i + VERIFICATION_LOOKAHEAD + 1, lines.length))
        .join("\n");
      const isVerified = VERIFICATION_PATTERNS.some((re) => re.test(window));

      // Skip if the line itself is a comment or a function signature
      // mentioning Command::new (e.g., in a doc comment).
      const trimmed = line.trim();
      if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("///")) {
        continue;
      }

      // Flag as violation: unverified Command::new().
      // Severity depends on the binary:
      //   - WRAPPED_BINARIES → warn (should use typed wrapper)
      //   - Other → info (legitimate but unverified; audit manually)
      if (!isVerified) {
        let severity = "info";
        let reason = "Command::new without nearby verification pattern";
        if (looksLikeAwe || looksLikeOllama) {
          severity = "warn";
          reason = `Raw Command::new for ${
            looksLikeAwe ? "awe" : "ollama"
          } — should use src-tauri/src/external/ typed wrapper (Layer 1)`;
        }
        violations.push({
          check: "unverified_command_new",
          severity,
          file: path.relative(ROOT, file),
          line: i + 1,
          binary,
          reason,
          snippet: line.trim().slice(0, 120),
        });
      }
    }
  }

  return violations;
}

// ---------------------------------------------------------------------------
// Check 2: Hook scripts setting *Pending flag without dedup
// ---------------------------------------------------------------------------

function checkHookIdempotency(scriptFiles) {
  const violations = [];

  // Pattern: assignment like `state.xPending = true` or `*Pending: true`.
  const pendingAssignRegex = /([\w.]+[Pp]ending)\s*=\s*true\b/;

  for (const file of scriptFiles) {
    let content;
    try {
      content = fs.readFileSync(file, "utf-8");
    } catch {
      continue;
    }
    const lines = content.split(/\r?\n/);

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const m = pendingAssignRegex.exec(line);
      if (!m) continue;

      const flagName = m[1];
      // Check if the same file has a "scanned" dedup set nearby (within
      // the same function, ~30 lines).
      const windowStart = Math.max(0, i - 30);
      const windowEnd = Math.min(lines.length, i + 30);
      const window = lines.slice(windowStart, windowEnd).join("\n");

      // Dedup marker: any mention of a "scanned" set or an "already handled"
      // check. Conservative pattern.
      const hasDedup =
        /scanned[A-Z]\w*|already\s*scanned|alreadyScanned|\.has\(|new\s*Set\(/.test(window);

      // Also skip if the assignment is inside a default state initializer
      // (file has `false` defaults for these flags — that's fine).
      const isDefault = /=\s*false\b|:\s*false\b/.test(line);

      if (!hasDedup && !isDefault) {
        violations.push({
          check: "hook_idempotency_amnesia",
          severity: "warn",
          file: path.relative(ROOT, file),
          line: i + 1,
          reason: `Sets ${flagName}=true without nearby dedup/scanned-set check — will re-fire every session`,
          snippet: line.trim().slice(0, 120),
        });
      }
    }
  }

  return violations;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  if (!JSON_MODE) {
    console.log("=== 4DA Boundary Call Validator (Layer 2) ===\n");
  }

  const rustFiles = walkDir(path.join(ROOT, "src-tauri", "src"), [".rs"]);
  const scriptFiles = [
    ...walkDir(path.join(ROOT, ".claude", "scripts"), [".cjs", ".js", ".mjs", ".sh"]),
    ...walkDir(path.join(ROOT, ".claude", "hooks"), [".cjs", ".js", ".mjs", ".sh"]),
    ...walkDir(path.join(ROOT, "scripts"), [".cjs", ".js", ".mjs", ".sh"]),
  ];

  const unverifiedCommands = checkUnverifiedCommandNew(rustFiles);
  const hookAmnesia = checkHookIdempotency(scriptFiles);

  const allViolations = [...unverifiedCommands, ...hookAmnesia];

  // Summary counters
  const summary = {
    rust_files_scanned: rustFiles.length,
    script_files_scanned: scriptFiles.length,
    unverified_command_new: unverifiedCommands.length,
    unverified_command_new_wrapped_binary: unverifiedCommands.filter(
      (v) => v.severity === "warn"
    ).length,
    hook_idempotency_amnesia: hookAmnesia.length,
    total_violations: allViolations.length,
  };

  if (JSON_MODE) {
    process.stdout.write(
      JSON.stringify({ summary, violations: allViolations }, null, 2) + "\n"
    );
  } else {
    console.log(`--- Summary ---`);
    console.log(`  Rust files scanned:                ${summary.rust_files_scanned}`);
    console.log(`  Script files scanned:              ${summary.script_files_scanned}`);
    console.log(`  Unverified Command::new:           ${summary.unverified_command_new}`);
    console.log(
      `  ... of which for wrapped binaries: ${summary.unverified_command_new_wrapped_binary} (should use src-tauri/src/external/)`
    );
    console.log(`  Hook idempotency amnesia:          ${summary.hook_idempotency_amnesia}`);
    console.log(`  Total violations:                  ${summary.total_violations}\n`);

    if (allViolations.length > 0) {
      // Group by check type
      const byCheck = {};
      for (const v of allViolations) {
        byCheck[v.check] = byCheck[v.check] || [];
        byCheck[v.check].push(v);
      }

      for (const [check, list] of Object.entries(byCheck)) {
        console.log(`--- ${check} (${list.length}) ---`);
        // Show first 20 of each check to avoid wall-of-text on large backlogs
        for (const v of list.slice(0, 20)) {
          console.log(
            `  [${v.severity}] ${v.file}:${v.line}`
          );
          console.log(`    ${v.reason}`);
          console.log(`    > ${v.snippet}`);
        }
        if (list.length > 20) {
          console.log(`  ... and ${list.length - 20} more`);
        }
        console.log();
      }
    } else {
      console.log("All boundary calls verified. No silent-failure patterns detected.\n");
    }

    // Strict mode exits nonzero on any violation
    if (STRICT && allViolations.length > 0) {
      console.log("--strict mode: violations found, exiting 1");
      process.exit(1);
    }
  }

  // Default: informational mode, always exit 0
  process.exit(0);
}

main();
