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
  rs:  { warn: 800, error: 1500 },
};

// Files that legitimately exceed thresholds — each must have a justification.
const EXCEPTIONS = {
  // Rust — files that structurally exceed 1500-line error threshold
  'src-tauri/src/db/migrations.rs':        'DB schema migrations — sequential by nature',
  'src-tauri/src/settings/mod.rs':         'Settings management — serialization + validation',
  'src-tauri/src/lib.rs':                  'App entrypoint — Tauri plugin registration',
  'src-tauri/src/sources/adapter_resilience_tests.rs': 'Resilience tests — 146 edge-case tests across all 10 source adapters',
  'src-tauri/src/scoring/pipeline_v2.rs':              'V2 scoring pipeline — 8-phase architecture with all signal extraction and combination',
  'src-tauri/src/sso.rs':                              'SSO module — enterprise feature-gated, OAuth/SAML auth + 33 tests',

  // TypeScript — type registries and complex UI
  'src/lib/commands.ts':                             'IPC command registry — all typed Tauri commands (280 with feature-gated variants)',
  'src/components/ResultsView.tsx':                  'Results list + filters — primary view',
  'src/components/void-engine/VoidHeartbeat.tsx':    'Void visualizer — canvas + animation',
  'src/App.tsx':                                     'App shell + routing — single entry point',
  'src/components/SourceConfigPanel.tsx':             'Source config form — per-source fields',
  'src/components/IndexedDocumentsPanel.tsx':          'Document browser — list + detail view',
  'src/components/DecisionMemory.tsx':                 'Decision memory — CRUD interface for tech decisions',
  'src/components/TechRadar.tsx':                      'Tech radar — technology adoption visualization',
  'src/components/__tests__/smoke.test.tsx':            'Smoke tests — all component render coverage',
  'src/components/FirstRunTransition.test.tsx':          'First-run experience — 18 tests across 6 phases',
  'src/components/result-item/ScoreBreakdownDrawer.tsx': 'Score drawer — factor extraction + comparison + feedback',
  'src/components/ResultsView.test.tsx':                  'ResultsView tests — 17 accessibility and state tests + required mocks',
  'src/components/__tests__/ActionBar.test.tsx':           'ActionBar tests — 26 tests covering all UI states and interactions',
  'src/store/types.ts':                                    'Store type registry — combined slice interfaces and AppStore union type',
  'src/components/onboarding/TasteTestStep.test.tsx':      'TasteTest tests — 22 tests across all onboarding phases',
  'src/components/NaturalLanguageSearch.tsx':               'NLS UI — search interface + results + ghost preview',
  'src/components/NaturalLanguageSearch.test.tsx':          'NLS tests — 14 tests for search UI states and interactions',
  'src/components/playbook/PlaybookView.test.tsx':         'Playbook tests — 15 tests for module navigation and progress',
  'src/components/SettingsModal.test.tsx':                  'Settings tests — comprehensive modal and tab interaction tests',
  'src/types/i18n-resources.d.ts':                         'Auto-generated i18n type declarations — regenerated via pnpm i18n:types',
};

const SCAN_DIRS = ['src', 'src-tauri/src'];
// Note: .js files are not in THRESHOLDS and thus not checked.
// src/lib/fourda-components/*.js are GAME compiler output — exempt by design.
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
