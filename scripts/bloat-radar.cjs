// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Bloat Radar — Necessity Engine codebase health scanner.
 *
 * Measures LOC, dead-code markers, ghost commands, stub files,
 * single-purpose abstractions, and dependency weight. Produces a
 * 0-100 Bloat Score and saves a JSON snapshot for trend tracking.
 *
 * Usage:  node scripts/bloat-radar.cjs
 * Deps:   Node built-ins only (fs, path, child_process)
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ── Paths ────────────────────────────────────────────────────────────────────

const ROOT = path.resolve(__dirname, '..');
const RUST_SRC = path.join(ROOT, 'src-tauri', 'src');
const TS_SRC = path.join(ROOT, 'src');
const CARGO_TOML = path.join(ROOT, 'src-tauri', 'Cargo.toml');
const PACKAGE_JSON = path.join(ROOT, 'package.json');
const SNAPSHOT_DIR = path.join(ROOT, '.claude', 'wisdom');
const SNAPSHOT_PATH = path.join(SNAPSHOT_DIR, 'bloat-radar-snapshot.json');

// ── ANSI Colors ──────────────────────────────────────────────────────────────

const C = {
  reset:   '\x1b[0m',
  bold:    '\x1b[1m',
  dim:     '\x1b[2m',
  green:   '\x1b[32m',
  yellow:  '\x1b[33m',
  red:     '\x1b[31m',
  cyan:    '\x1b[36m',
  white:   '\x1b[37m',
  bgGreen: '\x1b[42m',
  bgYellow:'\x1b[43m',
  bgRed:   '\x1b[44m',
};

function scoreColor(score) {
  if (score >= 80) return C.green;
  if (score >= 60) return C.yellow;
  return C.red;
}

function scoreLabel(score) {
  if (score >= 80) return 'PASS';
  if (score >= 60) return 'WARN';
  return 'FAIL';
}

// ── File Walking ─────────────────────────────────────────────────────────────

/** Recursively collect files matching a test function. */
function walkDir(dir, testFn, results = []) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      // Skip hidden dirs, target, node_modules
      if (entry.name.startsWith('.') || entry.name === 'target' || entry.name === 'node_modules') continue;
      walkDir(fullPath, testFn, results);
    } else if (testFn(entry.name)) {
      results.push(fullPath);
    }
  }
  return results;
}

/** Count lines in a file. */
function countLines(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  // Handle empty files
  if (content.length === 0) return 0;
  return content.split('\n').length;
}

// ── 1. LOC Counts ────────────────────────────────────────────────────────────

function measureLOC() {
  const rsFiles = walkDir(RUST_SRC, n => n.endsWith('.rs'));
  const tsFiles = walkDir(TS_SRC, n => n.endsWith('.ts') || n.endsWith('.tsx'));

  let rsLOC = 0;
  for (const f of rsFiles) rsLOC += countLines(f);

  let tsLOC = 0;
  let tsCount = 0;
  let tsxCount = 0;
  for (const f of tsFiles) {
    const lines = countLines(f);
    tsLOC += lines;
    if (f.endsWith('.tsx')) tsxCount++;
    else tsCount++;
  }

  return {
    rust: { files: rsFiles.length, loc: rsLOC },
    ts: { files: tsCount, loc: 0 },
    tsx: { files: tsxCount, loc: 0 },
    tsTotal: { files: tsFiles.length, loc: tsLOC },
    // Recount with split for ts vs tsx
    _tsFiles: tsFiles,
  };
}

function measureLOCDetailed() {
  const rsFiles = walkDir(RUST_SRC, n => n.endsWith('.rs'));
  const tsFiles = walkDir(TS_SRC, n => n.endsWith('.ts') && !n.endsWith('.tsx'));
  const tsxFiles = walkDir(TS_SRC, n => n.endsWith('.tsx'));

  let rsLOC = 0;
  for (const f of rsFiles) rsLOC += countLines(f);

  let tsLOC = 0;
  for (const f of tsFiles) tsLOC += countLines(f);

  let tsxLOC = 0;
  for (const f of tsxFiles) tsxLOC += countLines(f);

  return {
    rust: { files: rsFiles.length, loc: rsLOC },
    ts: { files: tsFiles.length, loc: tsLOC },
    tsx: { files: tsxFiles.length, loc: tsxLOC },
    tsTotal: { files: tsFiles.length + tsxFiles.length, loc: tsLOC + tsxLOC },
    _rsFiles: rsFiles,
    _allTsFiles: [...tsFiles, ...tsxFiles],
  };
}

// ── 2. Dead Code Markers ─────────────────────────────────────────────────────

function measureDeadCode(rsFiles) {
  const MARKER = '#[allow(dead_code)]';
  const LEGITIMATE_REASONS = /serde|feature[- ]gat|cfg\(|#\[cfg|stub|deserializ|test|diagnostic|sql\s*schema|debug.*handler/i;
  const SPECULATIVE_REASONS = /not yet wired|not yet called|reserved for|pending|future|phase \d|will be/i;
  const byFile = {};
  let total = 0;
  let speculative = 0;
  let legitimate = 0;
  let unclassified = 0;

  for (const f of rsFiles) {
    const lines = fs.readFileSync(f, 'utf8').split('\n');
    let count = 0;
    const rel = path.relative(RUST_SRC, f).replace(/\\/g, '/');
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].includes(MARKER)) {
        count++;
        const context = (lines[i - 1] || '') + lines[i] + (lines[i + 1] || '');
        if (SPECULATIVE_REASONS.test(context)) {
          speculative++;
        } else if (LEGITIMATE_REASONS.test(context) || rel.endsWith('_stub.rs') || /sources\//.test(rel)) {
          legitimate++;
        } else {
          unclassified++;
        }
      }
    }
    if (count > 0) {
      byFile[rel] = count;
      total += count;
    }
  }

  const sorted = Object.entries(byFile).sort((a, b) => b[1] - a[1]);
  const top10 = sorted.slice(0, 10);

  return { total, speculative, legitimate, unclassified, fileCount: sorted.length, byFile, top10 };
}

// ── 3. Ghost Commands ────────────────────────────────────────────────────────

function measureGhostCommands(rsFiles, tsFiles) {
  // Extract command names from Rust files: lines with #[tauri::command] followed
  // by a line with pub [async] fn <name>
  const commandNames = new Map(); // name -> source file (relative)
  const tauriCommandRe = /#\[tauri::command\]/;
  const fnNameRe = /pub\s+(?:\(crate\)\s+)?(?:async\s+)?fn\s+(\w+)/;

  for (const f of rsFiles) {
    const lines = fs.readFileSync(f, 'utf8').split('\n');
    const rel = path.relative(RUST_SRC, f).replace(/\\/g, '/');
    for (let i = 0; i < lines.length; i++) {
      if (tauriCommandRe.test(lines[i])) {
        // Look at next few lines for the fn declaration
        for (let j = i + 1; j < Math.min(i + 4, lines.length); j++) {
          const m = fnNameRe.exec(lines[j]);
          if (m) {
            // Only add if not already added (avoid duplicates from stub + real)
            if (!commandNames.has(m[1])) {
              commandNames.set(m[1], rel);
            }
            break;
          }
        }
      }
    }
  }

  // Build a set of all command names referenced in the frontend.
  // The frontend uses `cmd('command_name')` which wraps `invoke('command_name')`.
  // The CommandMap interface keys also define known commands.
  // We search for: invoke("name" or invoke('name' or cmd("name" or cmd('name'
  // or as a key in the CommandMap (bare word followed by colon on a line like:
  //   command_name: { params: ... })
  const frontendCommandNames = new Set();

  for (const f of tsFiles) {
    const content = fs.readFileSync(f, 'utf8');

    // Match invoke('name') / invoke("name") / cmd('name') / cmd("name")
    const invokeRe = /(?:invoke|cmd)\s*\(\s*['"](\w+)['"]/g;
    let m;
    while ((m = invokeRe.exec(content)) !== null) {
      frontendCommandNames.add(m[1]);
    }

    // Match CommandMap keys (lines like "  command_name: {")
    // Only in commands.ts, but we scan all files for safety
    const mapKeyRe = /^\s+(\w+)\s*:\s*\{\s*params\s*:/gm;
    while ((m = mapKeyRe.exec(content)) !== null) {
      frontendCommandNames.add(m[1]);
    }

    // Also catch string references like 'command_name' in arrays, sets, etc.
    // e.g. LONG_RUNNING_COMMANDS.has(command) checks, or string arrays
    const stringRefRe = /['"](\w+)['"]/g;
    while ((m = stringRefRe.exec(content)) !== null) {
      // Only add if it looks like a snake_case command (has underscore)
      if (m[1].includes('_')) {
        frontendCommandNames.add(m[1]);
      }
    }
  }

  // Ghost commands: exist in Rust but never referenced in frontend
  const ghosts = [];
  for (const [name, sourceFile] of commandNames) {
    if (!frontendCommandNames.has(name)) {
      ghosts.push({ name, file: sourceFile });
    }
  }

  // Sort for stable output
  ghosts.sort((a, b) => a.name.localeCompare(b.name));

  return {
    totalRustCommands: commandNames.size,
    frontendReferences: frontendCommandNames.size,
    ghostCount: ghosts.length,
    ghosts,
  };
}

// ── 4. Stub Files ────────────────────────────────────────────────────────────

function measureStubs(rsFiles) {
  const stubs = rsFiles.filter(f => path.basename(f).endsWith('_stub.rs'));
  let totalLOC = 0;
  const details = [];

  for (const f of stubs) {
    const loc = countLines(f);
    totalLOC += loc;
    details.push({ file: path.relative(RUST_SRC, f).replace(/\\/g, '/'), loc });
  }

  details.sort((a, b) => b.loc - a.loc);

  return { count: stubs.length, totalLOC, details };
}

// ── 5. Single-Purpose Abstractions ───────────────────────────────────────────

function measureSinglePurpose(rsFiles) {
  // Get top 20 largest Rust files
  const fileSizes = rsFiles.map(f => ({
    path: f,
    size: fs.statSync(f).size,
    rel: path.relative(RUST_SRC, f).replace(/\\/g, '/'),
  }));
  fileSizes.sort((a, b) => b.size - a.size);
  const top20 = fileSizes.slice(0, 20);

  // For each top-20 file, extract pub fn names (excluding #[tauri::command] and test fns)
  const pubFns = []; // { name, file }
  const seenFns = new Set(); // dedup by name::file

  for (const fileInfo of top20) {
    const content = fs.readFileSync(fileInfo.path, 'utf8');
    const lines = content.split('\n');

    // Track if we're inside a #[cfg(test)] module
    let inTestModule = false;
    let braceDepth = 0;
    let testModuleStart = -1;

    // First pass: find #[cfg(test)] module boundaries
    const testRanges = [];
    for (let i = 0; i < lines.length; i++) {
      if (/^\s*#\[cfg\(test\)\]/.test(lines[i])) {
        // Find the opening brace of the module
        for (let j = i + 1; j < Math.min(i + 5, lines.length); j++) {
          if (lines[j].includes('{')) {
            testModuleStart = j;
            break;
          }
        }
        if (testModuleStart >= 0) {
          // Count braces to find end
          braceDepth = 0;
          for (let j = testModuleStart; j < lines.length; j++) {
            for (const ch of lines[j]) {
              if (ch === '{') braceDepth++;
              if (ch === '}') braceDepth--;
            }
            if (braceDepth === 0) {
              testRanges.push([testModuleStart, j]);
              testModuleStart = -1;
              break;
            }
          }
        }
      }
    }

    function isInTestRange(lineNum) {
      return testRanges.some(([start, end]) => lineNum >= start && lineNum <= end);
    }

    // Second pass: extract pub fn names, excluding test and tauri::command fns
    for (let i = 0; i < lines.length; i++) {
      if (isInTestRange(i)) continue;

      const fnMatch = /^\s*pub\s+(?:async\s+)?fn\s+(\w+)/.exec(lines[i]);
      if (!fnMatch) continue;

      // Check if previous non-empty line is #[tauri::command]
      let isTauriCommand = false;
      for (let j = i - 1; j >= Math.max(0, i - 3); j--) {
        const trimmed = lines[j].trim();
        if (trimmed === '') continue;
        if (trimmed === '#[tauri::command]') {
          isTauriCommand = true;
        }
        break;
      }
      if (isTauriCommand) continue;

      // Skip test helper functions
      if (fnMatch[1].startsWith('test_')) continue;

      const key = `${fnMatch[1]}::${fileInfo.rel}`;
      if (!seenFns.has(key)) {
        seenFns.add(key);
        pubFns.push({ name: fnMatch[1], file: fileInfo.rel });
      }
    }
  }

  // For each pub fn, count how many OTHER .rs files reference the name
  // Build an index: for every rs file, read once and check all fn names
  const refCounts = {}; // fnName -> Set of files that reference it (excluding its own)
  for (const fn of pubFns) {
    if (!refCounts[fn.name]) refCounts[fn.name] = new Set();
  }

  const fnNames = new Set(pubFns.map(f => f.name));

  // Read all Rust files once to check references
  for (const f of rsFiles) {
    const content = fs.readFileSync(f, 'utf8');
    const rel = path.relative(RUST_SRC, f).replace(/\\/g, '/');

    for (const fnName of fnNames) {
      // Check if this file references the function (as a word boundary match)
      // Use a simple approach: check if fnName appears as a whole word
      const re = new RegExp(`\\b${fnName}\\b`);
      if (re.test(content)) {
        // Find which pub fn this belongs to
        for (const fn of pubFns) {
          if (fn.name === fnName && fn.file !== rel) {
            refCounts[fnName].add(rel);
          }
        }
      }
    }
  }

  // Single-purpose: only referenced in their own file (0 external references)
  const singlePurpose = pubFns
    .filter(fn => refCounts[fn.name].size === 0)
    .map(fn => ({ name: fn.name, file: fn.file }));

  return {
    sampledFiles: top20.length,
    totalPubFns: pubFns.length,
    singlePurposeCount: singlePurpose.length,
    singlePurpose,
  };
}

// ── 6. Dependency Weight ─────────────────────────────────────────────────────

function measureDependencies() {
  // Parse Cargo.toml [dependencies]
  const cargoContent = fs.readFileSync(CARGO_TOML, 'utf8');
  const cargoLines = cargoContent.split('\n');
  let inDeps = false;
  let cargoDeps = 0;
  const cargoDepNames = [];

  for (const line of cargoLines) {
    const trimmed = line.trim();
    if (trimmed === '[dependencies]') {
      inDeps = true;
      continue;
    }
    if (inDeps && trimmed.startsWith('[')) {
      // Hit next section
      inDeps = false;
      continue;
    }
    if (inDeps && trimmed && !trimmed.startsWith('#') && !trimmed.startsWith('//')) {
      // Check if it's a dependency line (name = ...)
      const depMatch = /^(\S+)\s*=/.exec(trimmed);
      if (depMatch) {
        cargoDeps++;
        cargoDepNames.push(depMatch[1]);
      }
    }
  }

  // Parse package.json
  const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON, 'utf8'));
  const npmDeps = Object.keys(pkg.dependencies || {}).length;
  const npmDevDeps = Object.keys(pkg.devDependencies || {}).length;

  return {
    cargo: { count: cargoDeps, names: cargoDepNames },
    npm: { deps: npmDeps, devDeps: npmDevDeps, total: npmDeps + npmDevDeps },
  };
}

// ── 7. Bloat Score ───────────────────────────────────────────────────────────

function computeScore(deadCode, ghosts, stubs, singlePurpose) {
  let score = 100;

  const deadCodePenalty = Math.floor(deadCode.speculative / 2) * 1 + Math.floor((deadCode.unclassified + deadCode.legitimate) / 10) * 1;
  const ghostPenalty = ghosts.ghostCount * 2;
  const stubPenalty = Math.floor(stubs.totalLOC / 1000) * 1;
  const singlePurposePenalty = singlePurpose.singlePurposeCount * 0.5;

  score -= deadCodePenalty;
  score -= ghostPenalty;
  score -= stubPenalty;
  score -= singlePurposePenalty;

  score = Math.max(0, Math.round(score * 10) / 10);

  return {
    score,
    breakdown: {
      deadCodePenalty,
      ghostPenalty,
      stubPenalty,
      singlePurposePenalty,
    },
  };
}

// ── Output ───────────────────────────────────────────────────────────────────

function printHeader(text) {
  console.log();
  console.log(`${C.bold}${C.cyan}${'='.repeat(60)}${C.reset}`);
  console.log(`${C.bold}${C.cyan}  ${text}${C.reset}`);
  console.log(`${C.bold}${C.cyan}${'='.repeat(60)}${C.reset}`);
}

function printSection(text) {
  console.log();
  console.log(`${C.bold}${C.white}--- ${text} ---${C.reset}`);
}

function printKV(key, value, color = C.white) {
  console.log(`  ${C.dim}${key}:${C.reset} ${color}${value}${C.reset}`);
}

// ── Main ─────────────────────────────────────────────────────────────────────

function main() {
  const startTime = Date.now();

  printHeader('BLOAT RADAR  —  Necessity Engine Scan');

  // 1. LOC
  printSection('1. Lines of Code');
  const loc = measureLOCDetailed();
  printKV('Rust (.rs)', `${loc.rust.loc.toLocaleString()} lines across ${loc.rust.files} files`);
  printKV('TypeScript (.ts)', `${loc.ts.loc.toLocaleString()} lines across ${loc.ts.files} files`);
  printKV('TypeScript (.tsx)', `${loc.tsx.loc.toLocaleString()} lines across ${loc.tsx.files} files`);
  printKV('Frontend total', `${loc.tsTotal.loc.toLocaleString()} lines across ${loc.tsTotal.files} files`);
  printKV('Combined total', `${(loc.rust.loc + loc.tsTotal.loc).toLocaleString()} lines`, C.bold);

  // 2. Dead Code Markers
  printSection('2. Dead Code Markers — #[allow(dead_code)]');
  const deadCode = measureDeadCode(loc._rsFiles);
  printKV('Total annotations', `${deadCode.total}`, deadCode.total > 50 ? C.yellow : C.green);
  printKV('  Speculative (not yet wired)', `${deadCode.speculative}`, deadCode.speculative > 10 ? C.red : C.green);
  printKV('  Legitimate (serde/cfg/test)', `${deadCode.legitimate}`);
  printKV('  Unclassified', `${deadCode.unclassified}`, deadCode.unclassified > 20 ? C.yellow : C.green);
  printKV('Files affected', `${deadCode.fileCount}`);
  if (deadCode.top10.length > 0) {
    console.log(`  ${C.dim}Top files:${C.reset}`);
    for (const [file, count] of deadCode.top10) {
      console.log(`    ${C.dim}${count}${C.reset}  ${file}`);
    }
  }

  // 3. Ghost Commands
  printSection('3. Ghost Commands — Rust commands with no frontend invoke');
  const ghosts = measureGhostCommands(loc._rsFiles, loc._allTsFiles);
  printKV('Total Rust commands', `${ghosts.totalRustCommands}`);
  printKV('Ghost commands', `${ghosts.ghostCount}`, ghosts.ghostCount > 0 ? C.yellow : C.green);
  if (ghosts.ghosts.length > 0) {
    console.log(`  ${C.dim}Unreferenced commands:${C.reset}`);
    for (const g of ghosts.ghosts) {
      console.log(`    ${C.red}${g.name}${C.reset}  ${C.dim}(${g.file})${C.reset}`);
    }
  }

  // 4. Stub Files
  printSection('4. Stub Files — *_stub.rs');
  const stubs = measureStubs(loc._rsFiles);
  printKV('Stub files', `${stubs.count}`);
  printKV('Total stub LOC', `${stubs.totalLOC.toLocaleString()}`);
  if (stubs.details.length > 0) {
    console.log(`  ${C.dim}Stub files:${C.reset}`);
    for (const s of stubs.details) {
      console.log(`    ${C.dim}${s.loc}${C.reset}  ${s.file}`);
    }
  }

  // 5. Single-Purpose Abstractions
  printSection('5. Single-Purpose Abstractions (top 20 largest files)');
  const singlePurpose = measureSinglePurpose(loc._rsFiles);
  printKV('Files sampled', `${singlePurpose.sampledFiles}`);
  printKV('Pub fns found', `${singlePurpose.totalPubFns}`);
  printKV('Single-purpose', `${singlePurpose.singlePurposeCount}`, singlePurpose.singlePurposeCount > 20 ? C.yellow : C.green);
  if (singlePurpose.singlePurpose.length > 0) {
    const display = singlePurpose.singlePurpose.slice(0, 20);
    console.log(`  ${C.dim}Never called externally (showing up to 20):${C.reset}`);
    for (const fn of display) {
      console.log(`    ${fn.name}  ${C.dim}(${fn.file})${C.reset}`);
    }
    if (singlePurpose.singlePurpose.length > 20) {
      console.log(`    ${C.dim}... and ${singlePurpose.singlePurpose.length - 20} more${C.reset}`);
    }
  }

  // 6. Dependency Weight
  printSection('6. Dependency Weight');
  const deps = measureDependencies();
  printKV('Cargo.toml deps', `${deps.cargo.count}`);
  printKV('package.json deps', `${deps.npm.deps}`);
  printKV('package.json devDeps', `${deps.npm.devDeps}`);
  printKV('Total npm', `${deps.npm.total}`);

  // 7. Bloat Score
  const { score, breakdown } = computeScore(deadCode, ghosts, stubs, singlePurpose);
  const color = scoreColor(score);
  const label = scoreLabel(score);

  printSection('7. Bloat Score');
  console.log();
  console.log(`  ${C.bold}${color}  SCORE: ${score} / 100  [${label}]${C.reset}`);
  console.log();
  console.log(`  ${C.dim}Breakdown:${C.reset}`);
  printKV('  dead_code penalty', `-${breakdown.deadCodePenalty} (${deadCode.speculative} speculative / 2 + ${deadCode.legitimate + deadCode.unclassified} legit / 10)`);
  printKV('  ghost cmd penalty', `-${breakdown.ghostPenalty} (${ghosts.ghostCount} ghosts * 2)`);
  printKV('  stub LOC penalty', `-${breakdown.stubPenalty} (${stubs.totalLOC} lines / 1000)`);
  printKV('  single-purpose penalty', `-${breakdown.singlePurposePenalty} (${singlePurpose.singlePurposeCount} fns * 0.5)`);

  const elapsed = Date.now() - startTime;
  console.log();
  console.log(`${C.dim}Scan completed in ${elapsed}ms${C.reset}`);
  console.log();

  // ── Save Snapshot ──────────────────────────────────────────────────────────

  const snapshot = {
    timestamp: new Date().toISOString(),
    elapsed_ms: elapsed,
    loc: {
      rust: { files: loc.rust.files, lines: loc.rust.loc },
      ts: { files: loc.ts.files, lines: loc.ts.loc },
      tsx: { files: loc.tsx.files, lines: loc.tsx.loc },
      total: loc.rust.loc + loc.tsTotal.loc,
    },
    dead_code: {
      total_annotations: deadCode.total,
      speculative: deadCode.speculative,
      legitimate: deadCode.legitimate,
      unclassified: deadCode.unclassified,
      files_affected: deadCode.fileCount,
      top10: deadCode.top10.map(([file, count]) => ({ file, count })),
    },
    ghost_commands: {
      total_rust_commands: ghosts.totalRustCommands,
      ghost_count: ghosts.ghostCount,
      ghosts: ghosts.ghosts.map(g => ({ name: g.name, file: g.file })),
    },
    stub_files: {
      count: stubs.count,
      total_loc: stubs.totalLOC,
      files: stubs.details,
    },
    single_purpose: {
      sampled_files: singlePurpose.sampledFiles,
      total_pub_fns: singlePurpose.totalPubFns,
      single_purpose_count: singlePurpose.singlePurposeCount,
      functions: singlePurpose.singlePurpose,
    },
    dependencies: {
      cargo: deps.cargo.count,
      npm_deps: deps.npm.deps,
      npm_dev_deps: deps.npm.devDeps,
      npm_total: deps.npm.total,
    },
    score: {
      value: score,
      label,
      breakdown,
    },
  };

  try {
    fs.mkdirSync(SNAPSHOT_DIR, { recursive: true });
    fs.writeFileSync(SNAPSHOT_PATH, JSON.stringify(snapshot, null, 2) + '\n');
    console.log(`${C.dim}Snapshot saved to ${path.relative(ROOT, SNAPSHOT_PATH)}${C.reset}`);
  } catch (err) {
    console.error(`${C.red}Failed to save snapshot: ${err.message}${C.reset}`);
  }

  // Record cadence timestamp in ops-state.json
  const OPS_STATE = path.join(ROOT, '.claude', 'wisdom', 'ops-state.json');
  try {
    const ops = JSON.parse(fs.readFileSync(OPS_STATE, 'utf8'));
    if (!ops.cadence) ops.cadence = {};
    ops.cadence.lastBloatRadar = new Date().toISOString();
    fs.writeFileSync(OPS_STATE, JSON.stringify(ops, null, 2) + '\n');
  } catch (_) {
    // ops-state.json may not exist — non-fatal
  }

  // Exit with non-zero if score is below 60 (fail)
  if (score < 60) {
    process.exit(1);
  }
}

main();
