// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Ghost Command Detector for 4DA
 *
 * Detects Tauri IPC command handlers that exist in Rust but are never called
 * from the frontend, or that exist as functions but aren't registered in the
 * invoke_handler.
 *
 * Run: node scripts/ghost-commands.cjs
 *
 * ---------------------------------------------------------------------------
 * WHY THIS SCRIPT LOOKS THE WAY IT DOES (2026-08-14 rewrite)
 * ---------------------------------------------------------------------------
 * The previous version counted every `CommandMap` interface key in
 * src/lib/commands.ts as evidence that a command was CALLED. But a second
 * blocking gate — scripts/validate-commands.cjs — *enforces* that every
 * registered command HAS a CommandMap key. Gate B therefore manufactured the
 * exact artifact gate A accepted as proof of use, so no command could ever be
 * classified a ghost: the detector reported "0 ghosts / IPC health 100%"
 * forever, and persisted that false number to .claude/wisdom/ghost-commands.json.
 *
 * The fix: a CommandMap key is TYPE COVERAGE, not USAGE. It is reported as its
 * own metric (it is genuinely worth knowing the typed contract is complete) and
 * never feeds the live/ghost classification. A command counts as live only when
 * the frontend actually reaches for it by name.
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const ROOT = path.resolve(__dirname, '..');
const BACKLOG_JSON = path.join(__dirname, 'ghost-command-backlog.json');

// Commands that are intentionally unregistered (feature-gated, in-progress, etc.)
// Add name + reason. These are excluded from the unregistered check and exit code.
const KNOWN_UNREGISTERED = new Set([]);

// Known, pre-existing ghosts live in scripts/ghost-command-backlog.json — one
// reviewable, dated, greppable entry each. They are reported loudly but do not
// fail the build, so NEW ghosts block from day one while the backlog is worked
// down. See that file's $comment for the clearing procedure.

// ANSI color codes
const RED = '\x1b[31m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const CYAN = '\x1b[36m';
const DIM = '\x1b[2m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

function repoPaths(root) {
  return {
    rustSrc: path.join(root, 'src-tauri', 'src'),
    tsSrc: path.join(root, 'src'),
    libRs: path.join(root, 'src-tauri', 'src', 'lib.rs'),
    commandsTs: path.join(root, 'src', 'lib', 'commands.ts'),
    outputJson: path.join(root, '.claude', 'wisdom', 'ghost-commands.json'),
  };
}

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

// Matches `#[tauri::command]` and attribute forms like
// `#[tauri::command(rename_all = "snake_case")]`.
const TAURI_COMMAND_ATTR = /^#\[tauri::command\b/;

// Deliberately mirrors scripts/validate-commands.cjs so the two gates agree on
// the denominator. The visibility group is what the old regex got wrong: it
// required `pub ` and therefore missed every `pub(crate) fn` command (15 of
// them), leaving those commands unclassified entirely.
const TAURI_COMMAND_FN = /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/;

function extractRustCommands(rustSrc, root = ROOT) {
  const rustFiles = walkSync(rustSrc, ['.rs']);
  const commands = []; // { name, file, line }

  for (const filePath of rustFiles) {
    const content = fs.readFileSync(filePath, 'utf8');
    const lines = content.split('\n');

    for (let i = 0; i < lines.length; i++) {
      if (!TAURI_COMMAND_ATTR.test(lines[i].trim())) continue;

      // Found the attribute — scan forward for the fn declaration
      // (there may be other attributes like #[allow(dead_code)] between).
      for (let j = i + 1; j < Math.min(i + 10, lines.length); j++) {
        const match = lines[j].match(TAURI_COMMAND_FN);
        if (match) {
          const relPath = path.relative(root, filePath).replace(/\\/g, '/');
          commands.push({
            name: match[1],
            file: relPath,
            line: j + 1, // 1-indexed
          });
          break;
        }
      }
    }
  }

  return commands;
}

// ============================================================================
// Step 2: Extract command names the frontend actually reaches for
// ============================================================================

// Any call whose first argument is a bare string literal: `callee('name')`.
// The callee is filtered by isInvokeLike() below rather than being baked into
// the regex, so wrappers are covered — public/briefing.js reaches the backend
// through `invokeTauri('briefing_open_url', ...)`, which a literal /invoke\(/
// pattern silently misses.
const CALL_WITH_STRING = /\b([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\s*\(\s*['"]([a-z_][a-z0-9_]*)['"]/g;

const isInvokeLike = (callee) => callee === 'cmd' || /invoke/i.test(callee);

// A bare command-name string literal anywhere in the frontend EXCEPT
// src/lib/commands.ts. This catches dynamic dispatch, where the name travels
// through a variable before reaching cmd() — e.g.
//   cmd(action, ...)                      // LearnedPreferencesSection.tsx
//   createToggleDefault(..., 'set_disabled_default_rss_feeds', ...)
// commands.ts is excluded precisely because it declares ALL command names by
// construction (the CommandMap interface plus helper sets like
// LONG_RUNNING_COMMANDS) and so proves nothing about usage.
const NAME_LITERAL = /['"`]([a-z_][a-z0-9_]*)['"`]/g;

// A CommandMap entry: `  some_command: { params: ... }`. TYPE COVERAGE ONLY.
const COMMAND_MAP_KEY = /^\s+([a-z_][a-z0-9_]*)\s*:\s*\{\s*params\s*:/gm;

/**
 * @param {Set<string>} candidates command names worth tracking (keeps the
 *   indirect-reference scan from collecting every string literal in src/).
 */
function extractFrontendUsage(tsSrc, commandsTsPath, candidates, root = ROOT) {
  // Every surface that is part of the SHIPPED app. public/ holds the standalone
  // briefing and notification webviews, which talk to the same IPC layer.
  // e2e/ is deliberately excluded: a command reachable only from a Playwright
  // spec is still dead app surface, and counting tests as usage would recreate
  // the very "artifact proves itself" loop this gate was fixed to avoid.
  const publicDir = path.join(root, 'public');
  const indexHtml = path.join(root, 'index.html');
  const tsFiles = [
    ...walkSync(tsSrc, ['.ts', '.tsx']),
    ...walkSync(publicDir, ['.js', '.html']),
    ...(fs.existsSync(indexHtml) ? [indexHtml] : []),
  ];
  const direct = new Map(); // name -> [ "file:line" ]
  const indirect = new Map(); // name -> [ "file:line" ]
  const typeKeys = new Set(); // CommandMap keys — coverage metric only
  const commandsTsNorm = path.normalize(commandsTsPath);

  const record = (map, name, filePath, source, index) => {
    const rel = path.relative(root, filePath).replace(/\\/g, '/');
    const line = source.slice(0, index).split('\n').length;
    if (!map.has(name)) map.set(name, []);
    map.get(name).push(`${rel}:${line}`);
  };

  for (const filePath of tsFiles) {
    const content = fs.readFileSync(filePath, 'utf8');
    const isCommandsTs = path.normalize(filePath) === commandsTsNorm;
    let m;

    CALL_WITH_STRING.lastIndex = 0;
    while ((m = CALL_WITH_STRING.exec(content)) !== null) {
      if (isInvokeLike(m[1])) record(direct, m[2], filePath, content, m.index);
    }

    if (isCommandsTs) {
      COMMAND_MAP_KEY.lastIndex = 0;
      while ((m = COMMAND_MAP_KEY.exec(content)) !== null) {
        typeKeys.add(m[1]);
      }
      continue; // never harvest name literals from the type/registry file
    }

    NAME_LITERAL.lastIndex = 0;
    while ((m = NAME_LITERAL.exec(content)) !== null) {
      if (candidates.has(m[1])) {
        record(indirect, m[1], filePath, content, m.index);
      }
    }
  }

  return { direct, indirect, typeKeys };
}

// ============================================================================
// Step 3: Extract commands registered in invoke_handler
// ============================================================================

function extractRegisteredCommands(libRs) {
  const content = fs.readFileSync(libRs, 'utf8');

  const handlerMatch = content.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!handlerMatch) {
    console.error(`${RED}ERROR: Could not find generate_handler![] in lib.rs${RESET}`);
    return new Set();
  }

  // Strip // comments and require a `::` path segment. Both matter: the old
  // regex ran over raw text and harvested bare words out of comments (`first`,
  // `reader`, `summaries` came from `// Content (article reader, AI summaries,
  // saved items)`), inflating the registration count by 3.
  const registeredNames = new Set();
  for (const line of handlerMatch[1].split('\n')) {
    const stripped = line.replace(/\/\/.*$/, '');
    for (const entry of stripped.split(',')) {
      const trimmed = entry.trim();
      if (!trimmed || !trimmed.includes('::')) continue;
      const fnName = trimmed.split('::').pop().trim();
      if (/^[a-z_][a-z0-9_]*$/.test(fnName)) registeredNames.add(fnName);
    }
  }

  return registeredNames;
}

// ============================================================================
// Step 4: Backlog allowlist
// ============================================================================

function loadBacklog(backlogPath = BACKLOG_JSON) {
  try {
    const raw = JSON.parse(fs.readFileSync(backlogPath, 'utf8'));
    const map = new Map();
    for (const entry of raw.backlog || []) {
      map.set(entry.command, entry);
    }
    return map;
  } catch {
    return new Map();
  }
}

// ============================================================================
// Step 5: Analysis (pure — no I/O beyond reading the tree)
// ============================================================================

function analyze({ root = ROOT, backlogPath = BACKLOG_JSON } = {}) {
  const p = repoPaths(root);
  const rustCommands = extractRustCommands(p.rustSrc, root);
  const registeredCommands = extractRegisteredCommands(p.libRs);
  const backlog = loadBacklog(backlogPath);

  // Deduplicate Rust commands by name (stubs and real implementations coexist —
  // only one is compiled via cfg, but we treat the name as covered if ANY
  // declaration exists)
  const uniqueByName = new Map();
  for (const cmd of rustCommands) {
    if (!uniqueByName.has(cmd.name)) uniqueByName.set(cmd.name, []);
    uniqueByName.get(cmd.name).push(cmd);
  }

  const { direct, indirect, typeKeys } = extractFrontendUsage(
    p.tsSrc,
    p.commandsTs,
    new Set(uniqueByName.keys()),
    root,
  );

  const liveDirect = [];
  const liveIndirect = [];
  const ghosts = []; // NEW ghosts — these block
  const backlogged = []; // known ghosts — reported, do not block
  const unregistered = [];

  for (const [name, locations] of uniqueByName) {
    const inHandler = registeredCommands.has(name);
    // Prefer a non-stub declaration site
    const primary = locations.find((l) => !l.file.includes('_stub.')) || locations[0];

    if (!inHandler) {
      if (!KNOWN_UNREGISTERED.has(name)) {
        unregistered.push({
          ...primary,
          name,
          inFrontend: direct.has(name) || indirect.has(name),
        });
      }
      continue;
    }

    if (direct.has(name)) {
      liveDirect.push({ ...primary, name, callers: direct.get(name) });
    } else if (indirect.has(name)) {
      liveIndirect.push({ ...primary, name, callers: indirect.get(name) });
    } else if (backlog.has(name)) {
      backlogged.push({ ...primary, name, since: backlog.get(name).since, reason: backlog.get(name).reason });
    } else {
      ghosts.push({ ...primary, name });
    }
  }

  // Backlog entries that no longer describe reality — the allowlist must shrink,
  // never rot.
  const staleBacklog = [];
  for (const [name, entry] of backlog) {
    if (!uniqueByName.has(name)) {
      staleBacklog.push({ name, since: entry.since, why: 'command no longer exists in Rust' });
    } else if (direct.has(name) || indirect.has(name)) {
      staleBacklog.push({ name, since: entry.since, why: 'command now has a frontend caller' });
    }
  }

  const byName = (a, b) => a.name.localeCompare(b.name);
  liveDirect.sort(byName);
  liveIndirect.sort(byName);
  ghosts.sort(byName);
  backlogged.sort(byName);
  unregistered.sort(byName);
  staleBacklog.sort(byName);

  const total = uniqueByName.size;
  const liveCount = liveDirect.length + liveIndirect.length;
  const pct = (n) => (total > 0 ? parseFloat(((n / total) * 100).toFixed(1)) : 0);
  const typedRegistered = [...typeKeys].filter((n) => registeredCommands.has(n)).length;

  return {
    total,
    liveDirect,
    liveIndirect,
    ghosts,
    backlogged,
    unregistered,
    staleBacklog,
    typeKeys,
    frontendRefs: new Set([...direct.keys(), ...indirect.keys()]).size,
    registrations: registeredCommands.size,
    ipcHealthPct: pct(liveCount),
    typeCoveragePct: pct(typedRegistered),
    typedRegistered,
  };
}

// ============================================================================
// Step 6: Report
// ============================================================================

function main() {
  console.log(`\n${BOLD}${CYAN}=== 4DA Ghost Command Detector ===${RESET}\n`);

  const r = analyze();
  const p = repoPaths(ROOT);
  const liveCount = r.liveDirect.length + r.liveIndirect.length;

  console.log(
    `${DIM}Scanned: ${r.total} unique Rust commands, ` +
      `${r.frontendRefs} frontend command refs, ` +
      `${r.registrations} invoke_handler registrations${RESET}\n`,
  );

  console.log(
    `${GREEN}${BOLD}LIVE${RESET} ${GREEN}(${liveCount} commands — registered AND called from the frontend)${RESET}`,
  );
  console.log(
    `  ${DIM}${r.liveDirect.length} via a direct cmd()/invoke() call site, ` +
      `${r.liveIndirect.length} via dynamic dispatch (name literal)${RESET}`,
  );
  for (const cmd of r.liveIndirect) {
    console.log(`  ${GREEN}~${RESET} ${cmd.name} ${DIM}dynamic dispatch <- ${cmd.callers[0]}${RESET}`);
  }

  // NEW ghosts — blocking
  if (r.ghosts.length > 0) {
    console.log(
      `\n${RED}${BOLD}GHOST (NEW)${RESET} ${RED}(${r.ghosts.length} — registered, but NOT called from the frontend)${RESET}`,
    );
    for (const cmd of r.ghosts) {
      console.log(`  ${RED}x${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line}${RESET}`);
    }
  } else {
    console.log(`\n${GREEN}${BOLD}GHOST (NEW)${RESET} ${GREEN}(0 — no new ghost commands)${RESET}`);
  }

  // Known backlog — loud, but non-blocking
  if (r.backlogged.length > 0) {
    console.log(
      `\n${YELLOW}${BOLD}GHOST BACKLOG${RESET} ${YELLOW}(${r.backlogged.length} known dead IPC commands — ` +
        `allowlisted in scripts/ghost-command-backlog.json)${RESET}`,
    );
    for (const cmd of r.backlogged) {
      console.log(`  ${YELLOW}-${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line} (since ${cmd.since})${RESET}`);
    }
  }

  if (r.staleBacklog.length > 0) {
    console.log(
      `\n${YELLOW}${BOLD}STALE ALLOWLIST${RESET} ${YELLOW}(${r.staleBacklog.length} — delete these from ` +
        `scripts/ghost-command-backlog.json)${RESET}`,
    );
    for (const s of r.staleBacklog) {
      console.log(`  ${YELLOW}!${RESET} ${s.name} ${DIM}${s.why}${RESET}`);
    }
  }

  // Unregistered
  if (r.unregistered.length > 0) {
    console.log(
      `\n${YELLOW}${BOLD}UNREGISTERED${RESET} ${YELLOW}(${r.unregistered.length} commands — #[tauri::command] in Rust, but NOT in invoke_handler)${RESET}`,
    );
    for (const cmd of r.unregistered) {
      const note = cmd.inFrontend ? ` ${RED}(frontend expects this!)${RESET}` : '';
      console.log(`  ${YELLOW}!${RESET} ${cmd.name} ${DIM}${cmd.file}:${cmd.line}${RESET}${note}`);
    }
  } else {
    console.log(`\n${GREEN}${BOLD}UNREGISTERED${RESET} ${GREEN}(0 — all commands properly registered)${RESET}`);
  }

  // Summary
  console.log(`\n${BOLD}${CYAN}-- Summary --${RESET}`);
  console.log(`  Total unique commands:  ${r.total}`);
  console.log(`  ${GREEN}Live:${RESET}                 ${liveCount}  ${DIM}(${r.liveDirect.length} direct + ${r.liveIndirect.length} dynamic)${RESET}`);
  console.log(`  ${RED}Ghost (new):${RESET}          ${r.ghosts.length}`);
  console.log(`  ${YELLOW}Ghost (backlog):${RESET}      ${r.backlogged.length}`);
  console.log(`  ${YELLOW}Unregistered:${RESET}         ${r.unregistered.length}`);
  console.log(`  Frontend refs:         ${r.frontendRefs}`);
  console.log(`  Handler registrations: ${r.registrations}`);
  console.log(`  ${BOLD}IPC health:            ${r.ipcHealthPct}%${RESET}  ${DIM}(live / total — commands the app can actually reach)${RESET}`);
  console.log(`  Type coverage:         ${r.typeCoveragePct}%  ${DIM}(CommandMap keys / total — typed contract completeness, NOT usage)${RESET}`);

  if (r.backlogged.length > 0) {
    console.log(
      `\n${YELLOW}${BOLD}!! DEAD IPC BACKLOG: ${r.backlogged.length} of ${r.total} registered commands ` +
        `(${(100 - r.ipcHealthPct).toFixed(1)}%) have no frontend caller.${RESET}`,
    );
    console.log(
      `${YELLOW}   They are allowlisted so this gate blocks NEW regressions only. Work the list down:${RESET}`,
    );
    console.log(`${YELLOW}   scripts/ghost-command-backlog.json${RESET}`);
  }
  console.log();

  // -- Write JSON --

  const outputDir = path.dirname(p.outputJson);
  if (!fs.existsSync(outputDir)) fs.mkdirSync(outputDir, { recursive: true });

  const strip = (c) => ({ name: c.name, file: c.file, line: c.line });

  const report = {
    generated_at: new Date().toISOString(),
    summary: {
      total_unique_commands: r.total,
      live: liveCount,
      live_direct: r.liveDirect.length,
      live_indirect: r.liveIndirect.length,
      ghost: r.ghosts.length,
      ghost_backlog: r.backlogged.length,
      backlog_stale: r.staleBacklog.length,
      unregistered: r.unregistered.length,
      frontend_refs: r.frontendRefs,
      handler_registrations: r.registrations,
      // live / total. NOT type coverage — see the header comment in this file.
      ipc_health_pct: r.ipcHealthPct,
      // CommandMap keys / total. Guaranteed ~100% by validate-commands.cjs, so
      // this says nothing about whether commands are reachable.
      type_coverage_pct: r.typeCoveragePct,
    },
    live: [...r.liveDirect, ...r.liveIndirect].map(strip).sort((a, b) => a.name.localeCompare(b.name)),
    ghost: r.ghosts.map(strip),
    ghost_backlog: r.backlogged.map((c) => ({ ...strip(c), since: c.since, reason: c.reason })),
    backlog_stale: r.staleBacklog,
    unregistered: r.unregistered.map((c) => ({ ...strip(c), frontend_expects: c.inFrontend })),
  };

  fs.writeFileSync(p.outputJson, JSON.stringify(report, null, 2), 'utf8');
  console.log(`${DIM}Report saved to ${path.relative(ROOT, p.outputJson)}${RESET}\n`);

  // Block on NEW ghosts and unregistered commands only. The seeded backlog is
  // reported loudly but does not turn the gate red on day one.
  if (r.ghosts.length > 0 || r.unregistered.length > 0) {
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  analyze,
  extractRustCommands,
  extractRegisteredCommands,
  extractFrontendUsage,
  loadBacklog,
  repoPaths,
};
