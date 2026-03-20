'use strict';

/**
 * impact-analysis.cjs
 *
 * Blast-radius analyzer for the 4DA codebase.
 * Given a file path, shows direct dependents, affected Tauri commands,
 * frontend callers, test coverage, git churn, and risk assessment.
 *
 * Usage:
 *   node scripts/impact-analysis.cjs src-tauri/src/scoring/mod.rs
 *   node scripts/impact-analysis.cjs src/components/ResultsView.tsx
 *   node scripts/impact-analysis.cjs src-tauri/src/types.rs --json
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const ROOT = path.resolve(__dirname, '..');
const RUST_SRC = path.join(ROOT, 'src-tauri', 'src');
const TS_SRC = path.join(ROOT, 'src');
const LIB_RS = path.join(RUST_SRC, 'lib.rs');
const COMMANDS_TS = path.join(ROOT, 'src', 'lib', 'commands.ts');

// ---------------------------------------------------------------------------
// ANSI colors
// ---------------------------------------------------------------------------

const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  cyan: '\x1b[36m',
  yellow: '\x1b[33m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  magenta: '\x1b[35m',
  white: '\x1b[37m',
};

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
    if (['node_modules', 'target', 'dist', '.git'].includes(entry.name)) continue;
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

/** Normalize a path to forward slashes for consistent display. */
function norm(p) {
  return p.replace(/\\/g, '/');
}

/** Get path relative to ROOT with forward slashes. */
function rel(p) {
  return norm(path.relative(ROOT, p));
}

// ---------------------------------------------------------------------------
// Step 1: Build Rust import/dependency graph
// ---------------------------------------------------------------------------

/**
 * Convert a file path to a Rust module path.
 * e.g. src-tauri/src/scoring/mod.rs -> scoring
 *      src-tauri/src/scoring/pipeline.rs -> scoring::pipeline
 *      src-tauri/src/types.rs -> types
 */
function fileToModulePath(filePath) {
  const relative = norm(path.relative(RUST_SRC, filePath));
  let modPath = relative
    .replace(/\.rs$/, '')
    .replace(/\/mod$/, '')
    .replace(/\//g, '::');
  return modPath;
}

/**
 * Build a reverse dependency map for Rust modules.
 * Returns { moduleName -> Set<filePaths that import it> }
 */
function buildRustDependencyGraph() {
  const rustFiles = walkDir(RUST_SRC, ['.rs']);
  // Forward map: module_name -> set of files that import it
  const reverseMap = new Map();

  for (const file of rustFiles) {
    let content;
    try {
      content = fs.readFileSync(file, 'utf-8');
    } catch {
      continue;
    }

    const lines = content.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();

      // use crate::module_name;
      // use crate::module_name::*;
      // use crate::module_name::{Item1, Item2};
      const useCrate = trimmed.match(/^use\s+crate::([a-z_][a-z0-9_]*)/);
      if (useCrate) {
        const mod = useCrate[1];
        if (!reverseMap.has(mod)) reverseMap.set(mod, new Set());
        reverseMap.get(mod).add(file);
      }

      // use crate::{module_name, other_module};
      const useGroup = trimmed.match(/^use\s+crate::\{([^}]+)\}/);
      if (useGroup) {
        const items = useGroup[1].split(',');
        for (const item of items) {
          const mod = item.trim().split('::')[0].trim();
          if (/^[a-z_][a-z0-9_]*$/.test(mod)) {
            if (!reverseMap.has(mod)) reverseMap.set(mod, new Set());
            reverseMap.get(mod).add(file);
          }
        }
      }

      // pub(crate) use module_name::*;
      const pubUse = trimmed.match(/^pub(?:\(crate\))?\s+use\s+crate::([a-z_][a-z0-9_]*)/);
      if (pubUse) {
        const mod = pubUse[1];
        if (!reverseMap.has(mod)) reverseMap.set(mod, new Set());
        reverseMap.get(mod).add(file);
      }

      // use super::module_name
      const useSuper = trimmed.match(/^use\s+super::([a-z_][a-z0-9_]*)/);
      if (useSuper) {
        const mod = useSuper[1];
        if (!reverseMap.has(mod)) reverseMap.set(mod, new Set());
        reverseMap.get(mod).add(file);
      }

      // mod module_name; declarations (in parent files)
      const modDecl = trimmed.match(/^(?:pub(?:\(crate\))?\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;/);
      if (modDecl) {
        const mod = modDecl[1];
        if (!reverseMap.has(mod)) reverseMap.set(mod, new Set());
        reverseMap.get(mod).add(file);
      }
    }
  }

  return reverseMap;
}

/** Get direct Rust dependents of a given module. */
function getRustDependents(filePath, reverseMap) {
  const modPath = fileToModulePath(filePath);
  // Try the full path and also just the leaf module name
  const modName = modPath.split('::')[0]; // top-level module name
  const leafName = modPath.split('::').pop();

  const dependents = new Set();

  // Match against the top-level module name (most common import pattern)
  if (reverseMap.has(modName)) {
    for (const dep of reverseMap.get(modName)) {
      if (norm(dep) !== norm(filePath)) {
        dependents.add(dep);
      }
    }
  }

  // If it's a submodule (scoring::pipeline), also check the leaf name
  if (leafName !== modName && reverseMap.has(leafName)) {
    for (const dep of reverseMap.get(leafName)) {
      if (norm(dep) !== norm(filePath)) {
        dependents.add(dep);
      }
    }
  }

  return [...dependents];
}

// ---------------------------------------------------------------------------
// Step 2: Map commands to files
// ---------------------------------------------------------------------------

/** Parse #[tauri::command] functions from all .rs files -> Map<command_name, file_path>. */
function parseRustCommands() {
  const rustFiles = walkDir(RUST_SRC, ['.rs']);
  const commands = new Map();

  for (const file of rustFiles) {
    let content;
    try {
      content = fs.readFileSync(file, 'utf-8');
    } catch {
      continue;
    }
    const lines = content.split('\n');

    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trim().includes('#[tauri::command]')) {
        for (let j = i + 1; j < Math.min(i + 10, lines.length); j++) {
          const fnMatch = lines[j].match(/^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/);
          if (fnMatch) {
            commands.set(fnMatch[1], file);
            break;
          }
        }
      }
    }
  }

  return commands;
}

/** Parse generate_handler![] in lib.rs -> Set of registered command names. */
function parseRegisteredCommands() {
  let content;
  try {
    content = fs.readFileSync(LIB_RS, 'utf-8');
  } catch {
    return new Set();
  }

  const registered = new Set();
  const handlerMatch = content.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!handlerMatch) return registered;

  const block = handlerMatch[1];
  const lines = block.split('\n');

  for (const line of lines) {
    const stripped = line.replace(/\/\/.*$/, '');
    const entries = stripped.split(',');
    for (const entry of entries) {
      const trimmed = entry.trim();
      if (trimmed && trimmed.includes('::')) {
        const parts = trimmed.split('::');
        const fnName = parts[parts.length - 1].trim();
        if (fnName && /^[a-z_][a-z0-9_]*$/.test(fnName)) {
          registered.add(fnName);
        }
      }
    }
  }

  return registered;
}

/**
 * Find commands defined in or transitively depending on the target file.
 * Returns array of { command, file, direct }
 */
function getAffectedCommands(targetFile, dependentFiles, commandMap) {
  const affected = [];
  const normalTarget = norm(targetFile);

  for (const [cmdName, cmdFile] of commandMap) {
    const normalCmd = norm(cmdFile);
    if (normalCmd === normalTarget) {
      affected.push({ command: cmdName, file: cmdFile, direct: true });
    } else if (dependentFiles.some(d => norm(d) === normalCmd)) {
      affected.push({ command: cmdName, file: cmdFile, direct: false });
    }
  }

  return affected;
}

// ---------------------------------------------------------------------------
// Step 3: Map frontend callers
// ---------------------------------------------------------------------------

/**
 * Search TS/TSX files for cmd('command_name') invocations.
 * Returns Map<command_name, Set<file_paths>>
 */
function buildFrontendCallerMap() {
  const tsFiles = walkDir(TS_SRC, ['.ts', '.tsx']);
  const callerMap = new Map();
  const commandsTsNorm = norm(COMMANDS_TS);

  for (const file of tsFiles) {
    if (norm(file) === commandsTsNorm) continue;

    let content;
    try {
      content = fs.readFileSync(file, 'utf-8');
    } catch {
      continue;
    }

    // Match cmd('command_name') or cmd("command_name")
    const regex = /cmd\(\s*['"]([a-z_][a-z0-9_]*)['"]/g;
    let match;
    while ((match = regex.exec(content)) !== null) {
      const cmdName = match[1];
      if (!callerMap.has(cmdName)) callerMap.set(cmdName, new Set());
      callerMap.get(cmdName).add(file);
    }
  }

  return callerMap;
}

/**
 * Get frontend callers for the affected commands.
 * Returns array of { file, commands }
 */
function getFrontendCallers(affectedCommands, callerMap) {
  const fileCommands = new Map();

  for (const { command } of affectedCommands) {
    const callers = callerMap.get(command);
    if (callers) {
      for (const caller of callers) {
        if (!fileCommands.has(caller)) fileCommands.set(caller, []);
        fileCommands.get(caller).push(command);
      }
    }
  }

  return [...fileCommands.entries()].map(([file, commands]) => ({
    file,
    commands,
  }));
}

// ---------------------------------------------------------------------------
// Step 3b: TypeScript import graph (for TS input files)
// ---------------------------------------------------------------------------

/**
 * Find TypeScript files that import from the target file.
 * Returns array of file paths.
 */
function getTsDependents(targetFile) {
  const tsFiles = walkDir(TS_SRC, ['.ts', '.tsx']);
  const dependents = [];

  // Build possible import paths from the target
  const relFromSrc = norm(path.relative(TS_SRC, targetFile));
  const withoutExt = relFromSrc.replace(/\.(tsx?|js|jsx)$/, '');
  const basename = path.basename(targetFile).replace(/\.(tsx?|js|jsx)$/, '');

  // Patterns: relative import, @/ alias import
  const aliasPath = withoutExt; // e.g. components/ResultsView
  const normalTarget = norm(targetFile);

  for (const file of tsFiles) {
    if (norm(file) === normalTarget) continue;

    let content;
    try {
      content = fs.readFileSync(file, 'utf-8');
    } catch {
      continue;
    }

    // Check @/ alias imports (e.g. from '@/components/ResultsView')
    if (content.includes(aliasPath) || content.includes(basename)) {
      // More precise check
      const importRegex = /(?:import|from)\s+['"]([^'"]+)['"]/g;
      let m;
      while ((m = importRegex.exec(content)) !== null) {
        const importPath = m[1];
        // @/ alias
        if (importPath.startsWith('@/') && importPath.includes(aliasPath.replace(/\/index$/, ''))) {
          dependents.push(file);
          break;
        }
        // Relative import
        if (importPath.startsWith('.')) {
          const resolved = norm(path.resolve(path.dirname(file), importPath));
          if (resolved.includes(withoutExt) || resolved.includes(basename)) {
            dependents.push(file);
            break;
          }
        }
      }
    }
  }

  return dependents;
}

/**
 * Find hooks and stores used by a TypeScript file.
 * Returns { hooks: string[], stores: string[] }
 */
function getTsUsages(filePath) {
  let content;
  try {
    content = fs.readFileSync(filePath, 'utf-8');
  } catch {
    return { hooks: [], stores: [] };
  }

  const hooks = new Set();
  const stores = new Set();

  // Find React hooks: useXxx(
  const hookRegex = /\b(use[A-Z][a-zA-Z0-9]*)\s*\(/g;
  let m;
  while ((m = hookRegex.exec(content)) !== null) {
    hooks.add(m[1]);
  }

  // Find store imports (zustand/jotai patterns)
  const storeRegex = /\b(use[A-Z][a-zA-Z]*Store)\b/g;
  while ((m = storeRegex.exec(content)) !== null) {
    stores.add(m[1]);
  }

  return { hooks: [...hooks], stores: [...stores] };
}

// ---------------------------------------------------------------------------
// Step 4: Find test files
// ---------------------------------------------------------------------------

/**
 * Find test files for a given source file.
 * Returns array of { file, testCount }
 */
function findTestFiles(filePath) {
  const tests = [];
  const ext = path.extname(filePath);
  const basename = path.basename(filePath, ext);
  const dir = path.dirname(filePath);

  if (ext === '.rs') {
    // Check for <basename>_tests.rs in same directory
    const testFile = path.join(dir, `${basename}_tests.rs`);
    if (fs.existsSync(testFile)) {
      const count = countTests(testFile, 'rust');
      tests.push({ file: testFile, testCount: count });
    }

    // Check for inline #[cfg(test)] in the file itself
    try {
      const content = fs.readFileSync(filePath, 'utf-8');
      if (content.includes('#[cfg(test)]')) {
        const count = countTests(filePath, 'rust');
        if (count > 0) {
          tests.push({ file: filePath, testCount: count, inline: true });
        }
      }
    } catch { /* ignore */ }

    // Also check for test modules declared in lib.rs
    // e.g. mod scoring_tests;
    try {
      const libContent = fs.readFileSync(LIB_RS, 'utf-8');
      const testModName = `${basename}_tests`;
      if (libContent.includes(`mod ${testModName}`)) {
        const testModFile = path.join(RUST_SRC, `${testModName}.rs`);
        if (fs.existsSync(testModFile) && !tests.some(t => norm(t.file) === norm(testModFile))) {
          const count = countTests(testModFile, 'rust');
          tests.push({ file: testModFile, testCount: count });
        }
      }
    } catch { /* ignore */ }
  } else if (['.ts', '.tsx'].includes(ext)) {
    // Check for <basename>.test.tsx / <basename>.test.ts
    for (const testExt of ['.test.tsx', '.test.ts', '.spec.tsx', '.spec.ts']) {
      const testFile = path.join(dir, `${basename}${testExt}`);
      if (fs.existsSync(testFile)) {
        const count = countTests(testFile, 'ts');
        tests.push({ file: testFile, testCount: count });
      }
    }

    // Check __tests__ directory
    const testsDir = path.join(dir, '__tests__');
    if (fs.existsSync(testsDir)) {
      for (const testExt of ['.test.tsx', '.test.ts', '.spec.tsx', '.spec.ts']) {
        const testFile = path.join(testsDir, `${basename}${testExt}`);
        if (fs.existsSync(testFile)) {
          const count = countTests(testFile, 'ts');
          tests.push({ file: testFile, testCount: count });
        }
      }
    }
  }

  return tests;
}

/** Count test functions in a file. */
function countTests(filePath, lang) {
  let content;
  try {
    content = fs.readFileSync(filePath, 'utf-8');
  } catch {
    return 0;
  }

  if (lang === 'rust') {
    return (content.match(/#\[test\]/g) || []).length;
  } else {
    // Count it( and test( patterns
    return (content.match(/\b(?:it|test)\s*\(/g) || []).length;
  }
}

// ---------------------------------------------------------------------------
// Step 5: Git frequency
// ---------------------------------------------------------------------------

function getGitFrequency(filePath) {
  try {
    const output = execSync(
      `git log --oneline --since="30 days ago" -- "${filePath}"`,
      { cwd: ROOT, encoding: 'utf-8', timeout: 10000 }
    ).trim();
    return output ? output.split('\n').length : 0;
  } catch {
    return 0;
  }
}

function getRecentCommits(filePath, limit) {
  try {
    const output = execSync(
      `git log --oneline --since="30 days ago" -${limit || 10} -- "${filePath}"`,
      { cwd: ROOT, encoding: 'utf-8', timeout: 10000 }
    ).trim();
    return output ? output.split('\n') : [];
  } catch {
    return [];
  }
}

function classifyFrequency(commits) {
  if (commits >= 10) return 'HOT';
  if (commits >= 3) return 'WARM';
  if (commits >= 1) return 'COLD';
  return 'DEAD';
}

// ---------------------------------------------------------------------------
// Step 6: Risk assessment
// ---------------------------------------------------------------------------

function assessRisk(frequency, testFiles) {
  const freq = classifyFrequency(frequency);
  const totalTests = testFiles.reduce((sum, t) => sum + t.testCount, 0);
  const hasCoverage = totalTests > 0;
  const wellTested = totalTests >= 5;

  if (freq === 'HOT' && !wellTested) {
    return { level: 'HIGH', reason: 'high churn with few/no tests' };
  }
  if (freq === 'HOT' && wellTested) {
    return { level: 'MEDIUM', reason: 'high churn but well-tested' };
  }
  if (freq === 'WARM' && !hasCoverage) {
    return { level: 'HIGH', reason: 'moderate churn with no tests' };
  }
  if (freq === 'WARM') {
    return { level: 'MEDIUM', reason: 'moderate churn, some coverage' };
  }
  if (freq === 'COLD' && !hasCoverage) {
    return { level: 'MEDIUM', reason: 'low churn but untested' };
  }
  if (freq === 'COLD') {
    return { level: 'LOW', reason: 'low churn, tested' };
  }
  // DEAD
  if (!hasCoverage) {
    return { level: 'MEDIUM', reason: 'no recent changes, untested' };
  }
  return { level: 'LOW', reason: 'stable, tested' };
}

// ---------------------------------------------------------------------------
// Output: Terminal
// ---------------------------------------------------------------------------

function printResult(result) {
  const { targetFile, isRust, directDependents, affectedCommands, frontendCallers,
    testFiles, gitCommits, frequency, frequencyLabel, risk, tsDependents, tsUsages } = result;

  console.log();
  console.log(`${c.bold}${c.cyan}IMPACT ANALYSIS: ${rel(targetFile)}${c.reset}`);
  console.log();

  // Direct dependents
  if (isRust) {
    console.log(`${c.bold}${c.yellow}Direct dependents${c.reset} (import this module):`);
    if (directDependents.length === 0) {
      console.log(`  ${c.dim}(none found)${c.reset}`);
    } else {
      for (const dep of directDependents) {
        // Find which commands are in this dependent
        const cmdsInDep = affectedCommands
          .filter(ac => norm(ac.file) === norm(dep))
          .map(ac => ac.command);
        const suffix = cmdsInDep.length > 0 ? ` ${c.dim}(via ${cmdsInDep.join(', ')})${c.reset}` : '';
        console.log(`  ${rel(dep)}${suffix}`);
      }
    }
    console.log();

    // Commands affected
    console.log(`${c.bold}${c.yellow}Commands affected:${c.reset}`);
    if (affectedCommands.length === 0) {
      console.log(`  ${c.dim}(none)${c.reset}`);
    } else {
      const direct = affectedCommands.filter(a => a.direct).map(a => a.command);
      const indirect = affectedCommands.filter(a => !a.direct).map(a => a.command);
      if (direct.length > 0) {
        console.log(`  ${c.green}direct:${c.reset} ${direct.join(', ')}`);
      }
      if (indirect.length > 0) {
        console.log(`  ${c.dim}transitive:${c.reset} ${indirect.join(', ')}`);
      }
    }
    console.log();

    // Frontend callers
    console.log(`${c.bold}${c.yellow}Frontend callers:${c.reset}`);
    if (frontendCallers.length === 0) {
      console.log(`  ${c.dim}(none)${c.reset}`);
    } else {
      for (const caller of frontendCallers) {
        console.log(`  ${rel(caller.file)} ${c.dim}(${caller.commands.join(', ')})${c.reset}`);
      }
    }
    console.log();
  } else {
    // TypeScript file
    console.log(`${c.bold}${c.yellow}Imported by:${c.reset}`);
    if (tsDependents.length === 0) {
      console.log(`  ${c.dim}(none found)${c.reset}`);
    } else {
      for (const dep of tsDependents) {
        console.log(`  ${rel(dep)}`);
      }
    }
    console.log();

    if (tsUsages.hooks.length > 0 || tsUsages.stores.length > 0) {
      console.log(`${c.bold}${c.yellow}Uses:${c.reset}`);
      if (tsUsages.hooks.length > 0) {
        console.log(`  ${c.green}hooks:${c.reset} ${tsUsages.hooks.join(', ')}`);
      }
      if (tsUsages.stores.length > 0) {
        console.log(`  ${c.green}stores:${c.reset} ${tsUsages.stores.join(', ')}`);
      }
      console.log();
    }

    // If TS file calls commands, show those too
    const callerMap = buildFrontendCallerMap();
    const commandsCalled = [];
    for (const [cmdName, callers] of callerMap) {
      if ([...callers].some(f => norm(f) === norm(targetFile))) {
        commandsCalled.push(cmdName);
      }
    }
    if (commandsCalled.length > 0) {
      console.log(`${c.bold}${c.yellow}Calls commands:${c.reset}`);
      console.log(`  ${commandsCalled.join(', ')}`);
      console.log();
    }
  }

  // Test files
  console.log(`${c.bold}${c.yellow}Test files:${c.reset}`);
  if (testFiles.length === 0) {
    console.log(`  ${c.red}(none found)${c.reset}`);
  } else {
    for (const tf of testFiles) {
      const label = tf.inline ? ' (inline)' : '';
      console.log(`  ${rel(tf.file)} ${c.dim}(${tf.testCount} tests${label})${c.reset}`);
    }
  }
  console.log();

  // Recent changes
  console.log(`${c.bold}${c.yellow}Recent changes${c.reset} (30 days): ${frequency} commits`);
  if (gitCommits.length > 0) {
    for (const commit of gitCommits.slice(0, 5)) {
      console.log(`  ${c.dim}${commit}${c.reset}`);
    }
    if (gitCommits.length > 5) {
      console.log(`  ${c.dim}... and ${gitCommits.length - 5} more${c.reset}`);
    }
  }

  // Risk
  const riskColor = risk.level === 'HIGH' ? c.red : risk.level === 'MEDIUM' ? c.yellow : c.green;
  console.log(
    `${c.bold}Frequency:${c.reset} ${frequencyLabel} | ` +
    `${c.bold}Risk:${c.reset} ${riskColor}${risk.level}${c.reset} ` +
    `${c.dim}(${risk.reason})${c.reset}`
  );
  console.log();
}

// ---------------------------------------------------------------------------
// Output: JSON
// ---------------------------------------------------------------------------

function toJson(result) {
  return {
    target: rel(result.targetFile),
    type: result.isRust ? 'rust' : 'typescript',
    direct_dependents: result.isRust
      ? result.directDependents.map(d => rel(d))
      : result.tsDependents.map(d => rel(d)),
    affected_commands: result.affectedCommands.map(a => ({
      command: a.command,
      file: rel(a.file),
      direct: a.direct,
    })),
    frontend_callers: result.frontendCallers.map(fc => ({
      file: rel(fc.file),
      commands: fc.commands,
    })),
    test_files: result.testFiles.map(tf => ({
      file: rel(tf.file),
      test_count: tf.testCount,
      inline: tf.inline || false,
    })),
    git: {
      commits_30d: result.frequency,
      frequency: result.frequencyLabel,
      recent: result.gitCommits.slice(0, 10),
    },
    risk: result.risk,
    ...(result.tsUsages ? {
      hooks: result.tsUsages.hooks,
      stores: result.tsUsages.stores,
    } : {}),
  };
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const filePaths = args.filter(a => a !== '--json');

  if (filePaths.length === 0) {
    console.error('Usage: node scripts/impact-analysis.cjs <file-path> [--json]');
    console.error('');
    console.error('Examples:');
    console.error('  node scripts/impact-analysis.cjs src-tauri/src/scoring/mod.rs');
    console.error('  node scripts/impact-analysis.cjs src/components/ResultsView.tsx');
    console.error('  node scripts/impact-analysis.cjs src-tauri/src/types.rs --json');
    process.exit(1);
  }

  // Resolve the target file
  let targetFile = filePaths[0];
  if (!path.isAbsolute(targetFile)) {
    targetFile = path.resolve(ROOT, targetFile);
  }

  if (!fs.existsSync(targetFile)) {
    console.error(`Error: file not found: ${targetFile}`);
    process.exit(1);
  }

  const ext = path.extname(targetFile);
  const isRust = ext === '.rs';
  const isTs = ['.ts', '.tsx'].includes(ext);

  if (!isRust && !isTs) {
    console.error(`Error: unsupported file type: ${ext} (expected .rs, .ts, or .tsx)`);
    process.exit(1);
  }

  // Build analysis
  let directDependents = [];
  let affectedCommands = [];
  let frontendCallers = [];
  let tsDependents = [];
  let tsUsages = { hooks: [], stores: [] };

  if (isRust) {
    // Step 1: Rust dependency graph
    const reverseMap = buildRustDependencyGraph();
    directDependents = getRustDependents(targetFile, reverseMap);

    // Step 2: Commands
    const commandMap = parseRustCommands();
    const registered = parseRegisteredCommands();

    // Filter to only registered commands
    const registeredCommands = new Map();
    for (const [name, file] of commandMap) {
      if (registered.has(name)) {
        registeredCommands.set(name, file);
      }
    }

    affectedCommands = getAffectedCommands(targetFile, directDependents, registeredCommands);

    // Step 3: Frontend callers
    const callerMap = buildFrontendCallerMap();
    frontendCallers = getFrontendCallers(affectedCommands, callerMap);
  } else {
    // TypeScript file
    tsDependents = getTsDependents(targetFile);
    tsUsages = getTsUsages(targetFile);

    // Also check if this file calls any commands
    const callerMap = buildFrontendCallerMap();
    // Build affected commands list from commands this file calls
    const commandMap = parseRustCommands();
    for (const [cmdName, callers] of callerMap) {
      if ([...callers].some(f => norm(f) === norm(targetFile))) {
        const cmdFile = commandMap.get(cmdName);
        if (cmdFile) {
          affectedCommands.push({ command: cmdName, file: cmdFile, direct: true });
        }
      }
    }

    // Frontend callers = files that import this TS file
    // (already captured in tsDependents, but for JSON consistency we keep frontendCallers empty for TS)
    frontendCallers = [];
  }

  // Step 4: Test files
  const testFiles = findTestFiles(targetFile);

  // Step 5: Git frequency
  const frequency = getGitFrequency(targetFile);
  const gitCommits = getRecentCommits(targetFile, 10);
  const frequencyLabel = classifyFrequency(frequency);

  // Step 6: Risk
  const risk = assessRisk(frequency, testFiles);

  const result = {
    targetFile,
    isRust,
    directDependents,
    affectedCommands,
    frontendCallers,
    testFiles,
    gitCommits,
    frequency,
    frequencyLabel,
    risk,
    tsDependents,
    tsUsages,
  };

  if (jsonMode) {
    console.log(JSON.stringify(toJson(result), null, 2));
  } else {
    printResult(result);
  }
}

main();
