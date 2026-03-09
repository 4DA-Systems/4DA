/**
 * validate-commands.cjs
 *
 * Validates IPC command consistency across the 4DA codebase:
 *   1. Rust #[tauri::command] definitions
 *   2. generate_handler![] registrations in lib.rs
 *   3. CommandMap keys in src/lib/commands.ts
 *   4. Raw invoke() calls that bypass the typed CommandMap
 *
 * Usage:  node scripts/validate-commands.cjs
 * Exit:   always 0 (informational only)
 */

const fs = require("fs");
const path = require("path");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Recursively walk a directory, yielding files that match an extension set. */
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
// Paths (relative to repo root)
// ---------------------------------------------------------------------------

const ROOT = path.resolve(__dirname, "..");
const RUST_SRC = path.join(ROOT, "src-tauri", "src");
const LIB_RS = path.join(RUST_SRC, "lib.rs");
const COMMANDS_TS = path.join(ROOT, "src", "lib", "commands.ts");
const FRONTEND_SRC = path.join(ROOT, "src");

// ---------------------------------------------------------------------------
// 1. Parse #[tauri::command] function names from all .rs files
// ---------------------------------------------------------------------------

function parseRustCommands() {
  const rustFiles = walkDir(RUST_SRC, [".rs"]);
  const commands = new Set();

  for (const file of rustFiles) {
    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");

    for (let i = 0; i < lines.length; i++) {
      const trimmed = lines[i].trim();
      // Match #[tauri::command] (possibly with extra attributes on same line)
      if (trimmed.includes("#[tauri::command]")) {
        // Look ahead for the fn declaration
        for (let j = i + 1; j < Math.min(i + 10, lines.length); j++) {
          const fnMatch = lines[j].match(
            /^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/
          );
          if (fnMatch) {
            commands.add(fnMatch[1]);
            break;
          }
        }
      }
    }
  }

  return commands;
}

// ---------------------------------------------------------------------------
// 2. Parse generate_handler![] registrations from lib.rs
// ---------------------------------------------------------------------------

function parseRegisteredCommands() {
  const content = fs.readFileSync(LIB_RS, "utf-8");
  const registered = new Set();

  // Find the generate_handler![ ... ] block (may span many lines)
  const handlerMatch = content.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!handlerMatch) {
    console.warn("  WARNING: Could not find generate_handler![] in lib.rs");
    return registered;
  }

  const block = handlerMatch[1];
  const lines = block.split("\n");

  for (const line of lines) {
    // Strip // comments from the line
    const stripped = line.replace(/\/\/.*$/, "");

    // Only match entries that contain :: (module::function_name)
    // This avoids false positives from comment words
    const entries = stripped.split(",");
    for (const entry of entries) {
      const trimmed = entry.trim();
      if (trimmed && trimmed.includes("::")) {
        // Extract the last segment after the final ::
        const parts = trimmed.split("::");
        const fnName = parts[parts.length - 1].trim();
        if (fnName && /^[a-z_][a-z0-9_]*$/.test(fnName)) {
          registered.add(fnName);
        }
      }
    }
  }

  return registered;
}

// ---------------------------------------------------------------------------
// 3. Parse CommandMap keys from src/lib/commands.ts
// ---------------------------------------------------------------------------

function parseTsCommands() {
  const content = fs.readFileSync(COMMANDS_TS, "utf-8");
  const commands = new Set();
  const lines = content.split("\n");

  for (const line of lines) {
    // Match lines like:   command_name: { params:
    const match = line.match(/^\s+([a-z_][a-z0-9_]*)\s*:\s*\{\s*params\s*:/);
    if (match) {
      commands.add(match[1]);
    }
  }

  return commands;
}

// ---------------------------------------------------------------------------
// 4. Find raw invoke() or invoke< calls in frontend code
// ---------------------------------------------------------------------------

function findRawInvokes() {
  const tsFiles = walkDir(FRONTEND_SRC, [".ts", ".tsx"]);
  const rawInvokes = [];

  // Normalise and compare to exclude commands.ts itself
  const commandsTsNorm = path.normalize(COMMANDS_TS);

  for (const file of tsFiles) {
    if (path.normalize(file) === commandsTsNorm) continue;

    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");

    for (let i = 0; i < lines.length; i++) {
      // Match invoke( or invoke< but not typed wrappers like commands.xxx
      if (/\binvoke\s*[<(]/.test(lines[i])) {
        const rel = path.relative(ROOT, file).replace(/\\/g, "/");
        rawInvokes.push({ file: rel, line: i + 1, text: lines[i].trim() });
      }
    }
  }

  return rawInvokes;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  console.log("=== 4DA IPC Command Validator ===\n");

  // Gather data
  const rustCommands = parseRustCommands();
  const registered = parseRegisteredCommands();
  const tsCommands = parseTsCommands();
  const rawInvokes = findRawInvokes();

  // --- Summary ---
  console.log("--- Summary ---");
  console.log(`  Rust #[tauri::command] definitions : ${rustCommands.size}`);
  console.log(`  generate_handler![] registrations  : ${registered.size}`);
  console.log(`  CommandMap TS keys                 : ${tsCommands.size}`);
  console.log(`  Raw invoke() calls (bypass typed)  : ${rawInvokes.length}`);
  console.log();

  // --- Cross-reference ---
  let issues = 0;

  // Registered but not in TS
  const registeredNotTs = [...registered].filter((c) => !tsCommands.has(c)).sort();
  if (registeredNotTs.length) {
    issues += registeredNotTs.length;
    console.log(`--- Registered in Rust but missing from CommandMap (${registeredNotTs.length}) ---`);
    for (const cmd of registeredNotTs) {
      console.log(`  - ${cmd}`);
    }
    console.log();
  }

  // In TS but not registered
  const tsNotRegistered = [...tsCommands].filter((c) => !registered.has(c)).sort();
  if (tsNotRegistered.length) {
    issues += tsNotRegistered.length;
    console.log(`--- In CommandMap but not registered in generate_handler![] (${tsNotRegistered.length}) ---`);
    for (const cmd of tsNotRegistered) {
      console.log(`  - ${cmd}`);
    }
    console.log();
  }

  // Rust commands defined but never registered
  const unregistered = [...rustCommands].filter((c) => !registered.has(c)).sort();
  if (unregistered.length) {
    issues += unregistered.length;
    console.log(`--- Rust #[tauri::command] defined but not registered (${unregistered.length}) ---`);
    for (const cmd of unregistered) {
      console.log(`  - ${cmd}`);
    }
    console.log();
  }

  // Raw invokes
  if (rawInvokes.length) {
    console.log(`--- Raw invoke() calls bypassing CommandMap (${rawInvokes.length}) ---`);
    for (const inv of rawInvokes) {
      console.log(`  ${inv.file}:${inv.line}  ${inv.text}`);
    }
    console.log();
  }

  // Final verdict
  if (issues === 0 && rawInvokes.length === 0) {
    console.log("All commands are consistent. No issues found.");
  } else {
    console.log(`Found ${issues} cross-reference issue(s) and ${rawInvokes.length} raw invoke(s).`);
  }
}

main();
process.exit(0);
