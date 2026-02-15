/**
 * File Size Guardian — enforces line-count limits on source files.
 *
 * Usage:
 *   node scripts/check-file-sizes.cjs        # local check
 *   node scripts/check-file-sizes.cjs --ci   # CI mode (GitHub Actions annotations)
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const THRESHOLDS = {
  ts:  { warn: 350, error: 500 },
  tsx: { warn: 350, error: 500 },
  rs:  { warn: 600, error: 1000 },
};

// Files that legitimately exceed thresholds — each must have a justification.
const EXCEPTIONS = {
  // Rust — core engines and large modules
  'src-tauri/src/ace/mod.rs':              'Core ACE engine — tightly coupled state machine',
  'src-tauri/src/db.rs':                   'DB schema + migrations — sequential by nature',
  'src-tauri/src/analysis.rs':             'Analysis pipeline — multi-stage orchestrator',
  'src-tauri/src/ace_commands.rs':         'ACE command surface — Tauri IPC boundary',
  'src-tauri/src/void_engine.rs':          'Void Signal engine — self-contained system',
  'src-tauri/src/domain_profile.rs':       'Domain taxonomy — large classification tables',
  'src-tauri/src/source_fetching.rs':      'Multi-source orchestrator — one fn per source',
  'src-tauri/src/ace/watcher.rs':          'File watcher — OS-specific event handling',
  'src-tauri/src/settings.rs':             'Settings management — serialization + validation',
  'src-tauri/src/digest.rs':               'Digest generation — template + data assembly',
  'src-tauri/src/health.rs':               'System health checks — one check per subsystem',
  'src-tauri/src/anomaly.rs':              'Anomaly detection — statistical algorithms',
  'src-tauri/src/scoring/mod.rs':          'Scoring core — unified scoring pipeline',
  'src-tauri/src/utils.rs':                'Shared utilities — stable helper collection',
  'src-tauri/src/scoring/dependencies.rs': 'Dependency matching — package ecosystem maps',
  'src-tauri/src/settings_commands.rs':    'Settings commands — Tauri IPC boundary',
  'src-tauri/src/commands.rs':             'Core commands — Tauri IPC boundary',
  'src-tauri/src/signals.rs':              'Signal detection — pattern matching rules',
  'src-tauri/src/knowledge_decay.rs':      'Knowledge decay — temporal algorithms',
  'src-tauri/src/ace/scanner.rs':          'Context scanner — filesystem traversal',
  'src-tauri/src/llm.rs':                  'LLM integration — multi-provider client',
  'src-tauri/src/lib.rs':                  'App entrypoint — Tauri plugin registration',

  // TypeScript — complex UI and type definitions
  'src/types.ts':                                   'Type definitions — inherently large',
  'src/components/onboarding/QuickSetupStep.tsx':    'Onboarding wizard — multi-step form',
  'src/components/ResultsView.tsx':                  'Results list + filters — primary view',
  'src/components/void-engine/VoidHeartbeat.tsx':    'Void visualizer — canvas + animation',
  'src/App.tsx':                                     'App shell + routing — single entry point',
  'src/components/SourceConfigPanel.tsx':             'Source config form — per-source fields',
  'src/components/onboarding/ApiKeysStep.tsx':        'API key setup — multi-provider form',
  'src/components/IndexedDocumentsPanel.tsx':          'Document browser — list + detail view',
};

const SCAN_DIRS = ['src', 'src-tauri/src'];
const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', '.git', '_future']);

// ============================================================================
// Implementation
// ============================================================================

const ciMode = process.argv.includes('--ci');

function getExtension(filePath) {
  const ext = path.extname(filePath).slice(1);
  return ext;
}

function countLines(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  // Count newlines; empty file = 0 lines
  if (content.length === 0) return 0;
  let count = 0;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === '\n') count++;
  }
  // If file doesn't end with newline, count the last line
  if (content[content.length - 1] !== '\n') count++;
  return count;
}

function walkDir(dir, results) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }

  for (const entry of entries) {
    if (SKIP_DIRS.has(entry.name)) continue;

    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkDir(fullPath, results);
    } else if (entry.isFile()) {
      const ext = getExtension(entry.name);
      if (THRESHOLDS[ext]) {
        results.push(fullPath);
      }
    }
  }
}

function normalizeSlashes(p) {
  return p.replace(/\\/g, '/');
}

// Collect all source files
const files = [];
for (const dir of SCAN_DIRS) {
  if (fs.existsSync(dir)) {
    walkDir(dir, files);
  }
}

const warnings = [];
const errors = [];

for (const filePath of files) {
  const normalized = normalizeSlashes(filePath);
  if (EXCEPTIONS[normalized]) continue;

  const ext = getExtension(filePath);
  const threshold = THRESHOLDS[ext];
  if (!threshold) continue;

  const lines = countLines(filePath);

  if (lines > threshold.error) {
    errors.push({ file: normalized, lines, limit: threshold.error, level: 'error' });
  } else if (lines > threshold.warn) {
    warnings.push({ file: normalized, lines, limit: threshold.warn, level: 'warn' });
  }
}

// ============================================================================
// Output
// ============================================================================

const allIssues = [...errors, ...warnings].sort((a, b) => b.lines - a.lines);

if (allIssues.length === 0) {
  console.log('File size check passed — all files within limits.');
  process.exit(0);
}

// Table output
const maxFileLen = Math.max(...allIssues.map(i => i.file.length), 4);
const header = 'File'.padEnd(maxFileLen) + '  Lines  Limit  Status';
console.log('\n' + header);
console.log('-'.repeat(header.length));

for (const issue of allIssues) {
  const status = issue.level === 'error' ? 'ERROR' : 'warn';
  const line = `${issue.file.padEnd(maxFileLen)}  ${String(issue.lines).padStart(5)}  ${String(issue.limit).padStart(5)}  ${status}`;
  console.log(line);

  if (ciMode) {
    const annotation = issue.level === 'error' ? '::error' : '::warning';
    console.log(`${annotation} file=${issue.file}::File has ${issue.lines} lines (limit: ${issue.limit}). Split or add to exception list.`);
  }
}

console.log('');

if (errors.length > 0) {
  console.log(`${errors.length} file(s) exceed error threshold. Split large files or add justified exceptions.`);
  process.exit(1);
} else {
  console.log(`${warnings.length} file(s) approaching size limits (warnings only).`);
  process.exit(0);
}
