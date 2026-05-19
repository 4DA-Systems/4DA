// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Ghost Command Detector for 4DA
 *
 * Detects Tauri IPC command handlers that exist in Rust but are never called
 * from the frontend, or that exist as functions but aren't registered in the
 * invoke_handler.
 *
 * Run: node scripts/ghost-commands.cjs
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const ROOT = path.resolve(__dirname, '..');
const RUST_SRC = path.join(ROOT, 'src-tauri', 'src');
const TS_SRC = path.join(ROOT, 'src');
const LIB_RS = path.join(RUST_SRC, 'lib.rs');
const OUTPUT_JSON = path.join(ROOT, '.claude', 'wisdom', 'ghost-commands.json');

// Commands that are intentionally unregistered (feature-gated, in-progress, etc.)
// Add name + reason. These are excluded from the unregistered check and exit code.
const KNOWN_UNREGISTERED = new Set([]);

// Commands that are registered but intentionally have no frontend caller yet.
// Typically: backend-only commands, feature-gated WIP, or commands called via MCP/CLI.
const KNOWN_GHOST = new Set([
  'prepare_embedding_engine', // fastembed-local feature — frontend caller coming
]);

// ANSI color codes
const RED = '\x1b[31m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const CYAN = '\x1b[36m';
const DIM = '\x1b[2m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

// ============================================================================
// File traversal (recursive, no external deps)
// ============================================================================

function walkSync(dir, extensions) {
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
      // Skip target/ and node_modules/
      if (entry.name === 'target' || entry.name === 'node_modules') continue;
      results.push(...walkSync(full, extensions));
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name);
      if (extensions.includes(ext)) {
        results.push(full);
      }
    }
  }
  return results;
}

// ============================================================================
// Step 1: Extract all #[tauri::command] function names from Rust
// ============================================================================

function extractRustCommands() {
  const rustFiles = walkSync(RUST_SRC, ['.rs']);
  const commands = []; // { name, file, line }

  for (const filePath of rustFiles) {
    const content = fs.readFileSync(filePath, 'utf8');
    const lines = content.split('\n');

    for (let i = 0; i < lines.length; i++) {
      const trimmed = lines[i].trim();
      if (trimmed !== '#[tauri::command]') continue;

      // Found #[tauri::command] — scan forward for the fn declaration
      // (there may be other attributes like #[allow(dead_code)] between)
      for (let j = i + 1; j < Math.min(i + 10, lines.length); j++) {
        const fnLine = lines[j].trim();
        // Match: pub async fn name( or pub fn name(
        const match = fnLine.match(/^pub\s+(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)\s*\(/);
        if (match) {
          const relPath = path.relative(ROOT, filePath).replace(/\\/g, '/');
          commands.push({
            name: match[1],
            file: relPath,
            line: j + 1, // 1-indexed
          });
          break;
        }
        // If we hit another attribute or doc comment, keep scanning
        if (fnLine.startsWith('#[') || fnLine.startsWith('//') || fnLine === '') continue;
        // If we hit something else entirely, stop
        break;
      }
    }
  }

  return commands;
}

// ============================================================================
// Step 2: Extract command names referenced in the frontend
// ============================================================================

function extractFrontendCommands() {
  const tsFiles = walkSync(TS_SRC, ['.ts', '.tsx']);
  const commandNames = new Set();

  for (const filePath of tsFiles) {
    const content = fs.readFileSync(filePath, 'utf8');

    // Pattern 1: Direct invoke calls — invoke("cmd") or invoke('cmd') or invoke<Type>("cmd")
    const invokeRegex = /invoke\s*(?:<[^>]*>)?\s*\(\s*['"]([a-z_][a-z0-9_]*)['"]/g;
    let m;
    while ((m = invokeRegex.exec(content)) !== null) {
      commandNames.add(m[1]);
    }

    // Pattern 2: CommandMap keys — the centralized type-safe layer.
    // These appear as `command_name:` at the start of a line (inside CommandMap interface)
    // or as string literals 'command_name' / "command_name" used with the cmd() function.
    const cmdCallRegex = /cmd\s*(?:<[^>]*>)?\s*\(\s*['"]([a-z_][a-z0-9_]*)['"]/g;
    while ((m = cmdCallRegex.exec(content)) !== null) {
      commandNames.add(m[1]);
    }

    // Pattern 3: CommandMap interface keys — `  command_name: {`
    // Only scan commands.ts for this pattern to avoid false positives
    if (filePath.endsWith('commands.ts')) {
      const mapKeyRegex = /^\s+([a-z_][a-z0-9_]*):\s*\{/gm;
      while ((m = mapKeyRegex.exec(content)) !== null) {
        // Filter out known non-command keys
        if (m[1] !== 'params' && m[1] !== 'result') {
          commandNames.add(m[1]);
        }
      }
    }
  }

  return commandNames;
}

// ============================================================================
// Step 3: Extract commands registered in invoke_handler
// ============================================================================

function extractRegisteredCommands() {
  const content = fs.readFileSync(LIB_RS, 'utf8');

  // Find the invoke_handler block
  const handlerStart = content.indexOf('.invoke_handler(tauri::generate_handler![');
  if (handlerStart === -1) {
    console.error(`${RED}ERROR: Could not find invoke_handler in lib.rs${RESET}`);
    return new Set();
  }

  // Find the closing ])
  const handlerEnd = content.indexOf('])', handlerStart);
  if (handlerEnd === -1) {
    console.error(`${RED}ERROR: Could not find end of invoke_handler${RESET}`);
    return new Set();
  }

  const handlerBlock = content.slice(handlerStart, handlerEnd);

  // Extract all module::function_name entries
  // Pattern: module::sub::function_name or just function_name
  const registeredNames = new Set();
  const entryRegex = /([a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)*)\s*[,\]]/g;
  let m;
  while ((m = entryRegex.exec(handlerBlock)) !== null) {
    const fullPath = m[1];
    // Extract just the function name (last segment after ::)
    const parts = fullPath.split('::');
    const fnName = parts[parts.length - 1];
    registeredNames.add(fnName);
  }

  return registeredNames;
}

// ============================================================================
// Step 4: Cross-reference and report
// ============================================================================

function main() {
  console.log(`\n${BOLD}${CYAN}=== 4DA Ghost Command Detector ===${RESET}\n`);

  // Gather data
  const rustCommands = extractRustCommands();
  const frontendCommands = extractFrontendCommands();
  const registeredCommands = extractRegisteredCommands();

  // Deduplicate Rust commands by name (stubs and real implementations coexist —
  // only one is compiled via cfg, but we treat the name as covered if ANY
  // declaration exists)
  const uniqueByName = new Map();
  for (const cmd of rustCommands) {
    if (!uniqueByName.has(cmd.name)) {
      uniqueByName.set(cmd.name, []);
    }
    uniqueByName.get(cmd.name).push(cmd);
  }

  // Classify each unique command
  const live = [];
  const ghosts = [];
  const unregistered = [];

  for (const [name, locations] of uniqueByName) {
    const inFrontend = frontendCommands.has(name);
    const inHandler = registeredCommands.has(name);

    // Pick the primary location (prefer non-stub files)
    const primary = locations.find(l => !l.file.includes('_stub.')) || locations[0];

    if (inFrontend && inHandler) {
      live.push({ name, ...primary });
    } else if (!inHandler && !KNOWN_UNREGISTERED.has(name)) {
      unregistered.push({ name, ...primary, inFrontend });
    } else if (!inFrontend && !KNOWN_GHOST.has(name)) {
      ghosts.push({ name, ...primary });
    }
  }

  // Sort alphabetically
  live.sort((a, b) => a.name.localeCompare(b.name));
  ghosts.sort((a, b) => a.name.localeCompare(b.name));
  unregistered.sort((a, b) => a.name.localeCompare(b.name));

  // ── Report ──

  console.log(`${DIM}Scanned: ${uniqueByName.size} unique Rust commands, ` +
    `${frontendCommands.size} frontend command refs, ` +
    `${registeredCommands.size} invoke_handler registrations${RESET}\n`);

  // Live commands
  console.log(`${GREEN}${BOLD}LIVE${RESET} ${GREEN}(${live.length} commands — in Rust, frontend, and invoke_handler)${RESET}`);
  for (const cmd of live) {
    console.log(`  ${GREEN}✓${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line}${RESET}`);
  }

  // Ghost commands
  if (ghosts.length > 0) {
    console.log(`\n${RED}${BOLD}GHOST${RESET} ${RED}(${ghosts.length} commands — in Rust + invoke_handler, but NOT called from frontend)${RESET}`);
    for (const cmd of ghosts) {
      console.log(`  ${RED}✗${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line}${RESET}`);
    }
  } else {
    console.log(`\n${GREEN}${BOLD}GHOST${RESET} ${GREEN}(0 — no ghost commands detected)${RESET}`);
  }

  // Unregistered commands
  if (unregistered.length > 0) {
    console.log(`\n${YELLOW}${BOLD}UNREGISTERED${RESET} ${YELLOW}(${unregistered.length} commands — #[tauri::command] in Rust, but NOT in invoke_handler)${RESET}`);
    for (const cmd of unregistered) {
      const frontendNote = cmd.inFrontend ? ` ${RED}(frontend expects this!)${RESET}` : '';
      console.log(`  ${YELLOW}⚠${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line}${RESET}${frontendNote}`);
    }
  } else {
    console.log(`\n${GREEN}${BOLD}UNREGISTERED${RESET} ${GREEN}(0 — all commands properly registered)${RESET}`);
  }

  // Summary
  console.log(`\n${BOLD}${CYAN}── Summary ──${RESET}`);
  console.log(`  Total unique commands:  ${uniqueByName.size}`);
  console.log(`  ${GREEN}Live:${RESET}                 ${live.length}`);
  console.log(`  ${RED}Ghost:${RESET}                ${ghosts.length}`);
  console.log(`  ${YELLOW}Unregistered:${RESET}         ${unregistered.length}`);
  console.log(`  Frontend refs:         ${frontendCommands.size}`);
  console.log(`  Handler registrations: ${registeredCommands.size}`);

  const healthPct = uniqueByName.size > 0
    ? ((live.length / uniqueByName.size) * 100).toFixed(1)
    : '100.0';
  console.log(`  IPC health:            ${healthPct}%`);
  console.log();

  // ── Write JSON ──

  const outputDir = path.dirname(OUTPUT_JSON);
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  const report = {
    generated_at: new Date().toISOString(),
    summary: {
      total_unique_commands: uniqueByName.size,
      live: live.length,
      ghost: ghosts.length,
      unregistered: unregistered.length,
      frontend_refs: frontendCommands.size,
      handler_registrations: registeredCommands.size,
      ipc_health_pct: parseFloat(healthPct),
    },
    live: live.map(c => ({ name: c.name, file: c.file, line: c.line })),
    ghost: ghosts.map(c => ({ name: c.name, file: c.file, line: c.line })),
    unregistered: unregistered.map(c => ({
      name: c.name,
      file: c.file,
      line: c.line,
      frontend_expects: c.inFrontend,
    })),
  };

  fs.writeFileSync(OUTPUT_JSON, JSON.stringify(report, null, 2), 'utf8');
  console.log(`${DIM}Report saved to ${path.relative(ROOT, OUTPUT_JSON)}${RESET}\n`);

  // Exit with non-zero if any ghosts or unregistered
  if (ghosts.length > 0 || unregistered.length > 0) {
    process.exit(1);
  }
}

main();
