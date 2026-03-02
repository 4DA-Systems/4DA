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
  'src-tauri/src/ace/context.rs':          'ACE context storage — git signals + detected tech',
  'src-tauri/src/ace/behavior.rs':         'ACE behavior tracking — feedback + learned signals',
  'src-tauri/src/db/migrations.rs':        'DB schema migrations — sequential by nature',
  'src-tauri/src/db/sources.rs':           'DB source CRUD — feedback + registry + health',
  'src-tauri/src/analysis.rs':             'Analysis pipeline — multi-stage orchestrator',
  'src-tauri/src/analysis_rerank.rs':      'Post-scoring — reranking, dedup, background analysis',
  'src-tauri/src/ace_commands.rs':         'ACE command surface — Tauri IPC boundary',
  'src-tauri/src/settings/mod.rs':         'Settings management — serialization + validation',
  'src-tauri/src/domain_profile.rs':       'Domain taxonomy — logic + comprehensive test suite',
  'src-tauri/src/source_fetching/fetcher.rs':    'Source fetching — fetch_all_sources + deep fetch orchestration',
  'src-tauri/src/source_fetching/processor.rs':  'Source processing — cache fill + item processing + tests',
  'src-tauri/src/ace/watcher.rs':          'File watcher — OS-specific event handling',
  'src-tauri/src/digest.rs':               'Digest generation — template + data assembly',
  'src-tauri/src/health.rs':               'System health checks — one check per subsystem',
  'src-tauri/src/anomaly.rs':              'Anomaly detection — statistical algorithms',
  'src-tauri/src/scoring/pipeline.rs':     'Scoring pipeline + tests — score_item() + 700 lines of tests',
  'src-tauri/src/utils.rs':                'Shared utilities — stable helper collection',
  'src-tauri/src/scoring/dependencies.rs': 'Dependency matching — package ecosystem maps',
  'src-tauri/src/settings_commands.rs':    'Settings commands — Tauri IPC boundary',
  'src-tauri/src/commands.rs':             'Core commands — Tauri IPC boundary',
  'src-tauri/src/signals.rs':              'Signal detection — pattern matching rules',
  'src-tauri/src/knowledge_decay.rs':      'Knowledge decay — temporal algorithms',
  'src-tauri/src/ace/scanner.rs':          'Context scanner — filesystem traversal',
  'src-tauri/src/llm.rs':                  'LLM integration — multi-provider client',
  'src-tauri/src/lib.rs':                  'App entrypoint — Tauri plugin registration',
  'src-tauri/src/developer_dna.rs':        'Developer DNA — profile generation + SVG export',
  'src-tauri/src/decisions.rs':            'Decision intelligence — CRUD + alignment checks',
  'src-tauri/src/bin/cli.rs':              'CLI binary — command parsing + output formatting',
  'src-tauri/src/delegation.rs':           'Delegation scoring — multi-axis trust assessment',
  'src-tauri/src/stacks/profiles.rs':      'Stack Intelligence — 8 technology profile definitions',
  'src-tauri/src/scoring/benchmark.rs':    'Scoring benchmarks — comprehensive test assertions',
  'src-tauri/src/scoring/pipeline_v2.rs':  'V2 scoring pipeline — experimental alternate scorer',
  'src-tauri/src/scoring/simulation/corpus.rs': 'Scoring simulation corpus — 215 labeled test fixtures for pipeline validation',
  'src-tauri/src/scoring/simulation/differential.rs': 'Scoring simulation System 4 — parameter regression detection tests',
  'src-tauri/src/scoring/simulation/domain_embeddings.rs': 'Scoring simulation — domain embedding fixtures for persona-based testing',
  'src-tauri/src/scoring/simulation/tier2_semantic.rs': 'Scoring simulation — semantic/embedding scoring validation with 12 tests',
  'src-tauri/src/scoring/simulation/golden_snapshot.rs': 'Scoring simulation — golden snapshot test assertions for pipeline validation',
  'src-tauri/src/void_engine/universe.rs': 'Void Universe renderer — experimental, feature-gated',
  'src-tauri/src/sovereign_developer_profile.rs': 'Unified profile aggregation — 10 data sources + markdown/JSON export',
  'src-tauri/src/game_engine.rs':               'GAME engine — 404 lines production + 730 lines integration tests',

  // TypeScript — type registries and complex UI
  'src/lib/commands.ts':                             'IPC command registry — all 107 typed Tauri commands',
  'src/components/ResultsView.tsx':                  'Results list + filters — primary view',
  'src/components/void-engine/VoidHeartbeat.tsx':    'Void visualizer — canvas + animation',
  'src/App.tsx':                                     'App shell + routing — single entry point',
  'src/components/SourceConfigPanel.tsx':             'Source config form — per-source fields',
  'src/components/IndexedDocumentsPanel.tsx':          'Document browser — list + detail view',
  'src/components/BriefingView.tsx':                   'AI briefing — signal cards + synthesis view',
  'src/components/DecisionMemory.tsx':                 'Decision memory — CRUD interface for tech decisions',
  'src/components/TechRadar.tsx':                      'Tech radar — technology adoption visualization',
  'src/components/SettingsModal.tsx':                   'Settings — tabbed configuration panels',
  'src/components/__tests__/smoke.test.tsx':            'Smoke tests — all component render coverage',
  'src/components/FirstRunTransition.test.tsx':          'First-run experience — 18 tests across 6 phases',
  'src/components/result-item/ScoreBreakdownDrawer.tsx': 'Score drawer — factor extraction + comparison + feedback',
  'src/components/ResultsView.test.tsx':                  'ResultsView tests — 17 accessibility and state tests + required mocks',
  'src/components/__tests__/ActionBar.test.tsx':           'ActionBar tests — 26 tests covering all UI states and interactions',
  'src/store/types.ts':                                    'Store type registry — combined slice interfaces and AppStore union type',
};

const SCAN_DIRS = ['src', 'src-tauri/src'];
// Note: .js files are not in THRESHOLDS and thus not checked.
// src/lib/game-components/*.js are GAME compiler output — exempt by design.
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
