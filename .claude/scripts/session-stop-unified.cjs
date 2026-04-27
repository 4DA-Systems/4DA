#!/usr/bin/env node
/**
 * Unified Session Stop Hook
 *
 * Consolidates session-stop.sh + wisdom-digest.cjs + ops-session-end.cjs
 * into a single process to avoid Cygwin fork exhaustion on Windows.
 *
 * Runs git ONCE, then performs all three tasks:
 * 1. Archive transcript (with dedup + size limit)
 * 2. Write wisdom digest (pending.json for next session)
 * 3. Update ops state (sovereignty, drift, immune system)
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const CLAUDE_DIR = path.join(__dirname, '..');
const PROJECT_ROOT = path.dirname(CLAUDE_DIR);
const SESSIONS_DIR = path.join(CLAUDE_DIR, 'sessions', 'transcripts');
const WISDOM_DIR = path.join(CLAUDE_DIR, 'wisdom');
const STATE_FILE = path.join(WISDOM_DIR, 'state.json');
const PENDING_FILE = path.join(WISDOM_DIR, 'pending.json');
const OPS_STATE_FILE = path.join(WISDOM_DIR, 'ops-state.json');
const SESSIONS_LOG = path.join(CLAUDE_DIR, 'sessions', 'sessions.log');

// Bug fix detection patterns
// Conventional-commits-aware classifier. Matches a commit summary of the form
//   <sha>  fix: ...
//   <sha>  fix(scope): ...
//   <sha>  hotfix: ...
//   <sha>  fix!: ...      (breaking-change marker)
// and nothing else. The older loose `\bfix\b` match false-triggered on
// routine chores like "chore(lint): apply ESLint --fix" because the word
// "fix" appeared anywhere in the subject. The bug-fix signal is the
// conventional-commit TYPE, not the presence of the word.
const FIX_PATTERNS = /^[0-9a-f]{7,}\s+(?:fix|hotfix)(?:\([^)]+\))?!?:\s/i;

// Area categorization for drift tracking
const AREA_MAP = [
  { pattern: /^src-tauri\/src\/(scoring|relevance)/, area: 'scoring' },
  { pattern: /^src-tauri\/src\/sources\//, area: 'sources' },
  { pattern: /^src-tauri\/src\/ace\//, area: 'ace' },
  { pattern: /^src-tauri\/src\/extractors\//, area: 'extractors' },
  { pattern: /^src-tauri\//, area: 'backend' },
  { pattern: /^src\/components\//, area: 'frontend' },
  { pattern: /^src\//, area: 'frontend' },
  { pattern: /^\.ai\//, area: 'architecture' },
  { pattern: /^\.claude\//, area: 'tooling' },
  { pattern: /^mcp-/, area: 'mcp' },
  { pattern: /^specs\//, area: 'specs' },
  { pattern: /^site\//, area: 'design' },
  { pattern: /^scripts\//, area: 'tooling' },
];

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('readable', () => {
  let chunk;
  while ((chunk = process.stdin.read()) !== null) input += chunk;
});

process.stdin.on('end', () => {
  try {
    let hookData = {};
    try { hookData = JSON.parse(input); } catch (_) {}

    const sessionId = hookData.session_id || 'unknown';
    const transcriptPath = hookData.transcript_path || '';

    ensureDir(SESSIONS_DIR);
    ensureDir(WISDOM_DIR);

    // === Run git ONCE for all three tasks ===
    const modifiedFiles = getModifiedFiles();
    const recentCommits = getRecentCommits();

    // === Task 1: Archive transcript ===
    archiveTranscript(sessionId, transcriptPath);

    // === Task 2: Wisdom digest ===
    writeWisdomDigest(modifiedFiles, recentCommits);

    // === Task 3: Ops state update ===
    updateOpsState(modifiedFiles, recentCommits);

    output({ status: 'success' });
  } catch (e) {
    // Never fail the session stop
    output({ status: 'success' });
  }
});

// ===========================================================================
// Task 1: Archive Transcript
// ===========================================================================
function archiveTranscript(sessionId, transcriptPath) {
  try {
    if (!transcriptPath || !fs.existsSync(transcriptPath)) return;

    // Dedup: skip if archived in last 5 minutes
    const existing = fs.readdirSync(SESSIONS_DIR)
      .filter(f => f.includes(sessionId) && f.endsWith('.jsonl'))
      .map(f => ({ name: f, mtime: fs.statSync(path.join(SESSIONS_DIR, f)).mtimeMs }))
      .sort((a, b) => b.mtime - a.mtime);

    if (existing.length > 0 && (Date.now() - existing[0].mtime) < 5 * 60 * 1000) {
      return; // Already archived recently
    }

    // Skip transcripts > 50MB
    const stats = fs.statSync(transcriptPath);
    if (stats.size > 50 * 1024 * 1024) {
      appendLog(`[${new Date().toISOString()}] Skipped archive: ${sessionId} (${(stats.size / 1024 / 1024).toFixed(1)}MB > 50MB limit)`);
      return;
    }

    const timestamp = formatTimestamp();
    const archiveFile = path.join(SESSIONS_DIR, `session_${timestamp}_${sessionId}.jsonl`);
    fs.copyFileSync(transcriptPath, archiveFile);

    const lineCount = fs.readFileSync(archiveFile, 'utf8').split('\n').filter(Boolean).length;
    const sizeMB = (stats.size / 1024 / 1024).toFixed(1);
    appendLog(`[${new Date().toISOString()}] Session archived: ${sessionId} (${lineCount} messages, ${sizeMB}MB)`);

    // Prune: keep only last 20 archives
    const archives = fs.readdirSync(SESSIONS_DIR)
      .filter(f => f.startsWith('session_') && f.endsWith('.jsonl'))
      .map(f => ({ name: f, mtime: fs.statSync(path.join(SESSIONS_DIR, f)).mtimeMs }))
      .sort((a, b) => b.mtime - a.mtime);

    for (const old of archives.slice(20)) {
      fs.unlinkSync(path.join(SESSIONS_DIR, old.name));
    }
  } catch (_) {}
}

// ===========================================================================
// Task 2: Wisdom Digest
// ===========================================================================
function writeWisdomDigest(modifiedFiles, recentCommits) {
  try {
    const areas = detectAreas(modifiedFiles);
    const aiFilesTouched = modifiedFiles.some(f => f.file.startsWith('.ai/'));
    const wisdomFilesTouched = modifiedFiles.some(f => f.file.includes('WISDOM'));

    // Update session counter
    const state = loadJSON(STATE_FILE, { sessionCount: 0 });
    state.sessionCount = (state.sessionCount || 0) + 1;
    state.lastSessionEnd = new Date().toISOString();
    saveJSON(STATE_FILE, state);

    // Only write digest if something meaningful happened
    if (modifiedFiles.length > 0 || recentCommits.length > 0) {
      const digest = {
        timestamp: new Date().toISOString(),
        sessionNumber: state.sessionCount,
        filesModified: modifiedFiles.length,
        fileList: modifiedFiles.slice(0, 15).map(f => f.file),
        areas: [...areas],
        aiFilesTouched,
        wisdomFilesTouched,
        recentCommits: recentCommits.slice(0, 5),
        processed: false,
      };

      saveJSON(PENDING_FILE, digest);
    }
  } catch (_) {}
}

// ===========================================================================
// Task 3: Ops State Update
// ===========================================================================
function updateOpsState(modifiedFiles, recentCommits) {
  try {
    const state = loadOpsState();

    // Bug fix detection — only flag commits that haven't already been scanned
    // by an earlier immune-system run. `scannedBugFixCommits` is a set of
    // commit SHAs (or first-token of commit summary) that have already had
    // an antibody recorded; we skip them to prevent the flag from re-firing
    // every session for the same commit.
    state.scannedBugFixCommits = state.scannedBugFixCommits || [];
    const scannedSet = new Set(state.scannedBugFixCommits);
    const allBugFixCommits = recentCommits.filter(c => FIX_PATTERNS.test(c));
    const newBugFixCommits = allBugFixCommits.filter(c => {
      // Match by commit short SHA (first whitespace-separated token).
      const sha = (c.split(/\s+/)[0] || '').trim();
      return sha && !scannedSet.has(sha);
    });
    if (newBugFixCommits.length > 0) {
      state.immuneScanPending = true;
      state.immuneContext = {
        commits: newBugFixCommits,
        files: modifiedFiles.map(f => f.file),
        timestamp: new Date().toISOString(),
      };
    }

    // Session counter
    state.compound.sessionsCompleted = (state.compound.sessionsCompleted || 0) + 1;

    // File change frequency
    const freq = state.metabolism.fileChangeFrequency || {};
    for (const f of modifiedFiles) {
      freq[f.file] = (freq[f.file] || 0) + 1;
    }
    state.metabolism.fileChangeFrequency = freq;

    // Strategic drift tracking
    const sessionAreas = categorizeSession(modifiedFiles);
    if (sessionAreas.length > 0) {
      if (!state.drift.recentSessionCategories) {
        state.drift.recentSessionCategories = [];
      }
      state.drift.recentSessionCategories.push({
        date: new Date().toISOString(),
        areas: sessionAreas,
        filesChanged: modifiedFiles.length,
        commits: recentCommits.length,
      });
      // Rolling 30-day window
      const thirtyDaysAgo = Date.now() - 30 * 24 * 60 * 60 * 1000;
      state.drift.recentSessionCategories = state.drift.recentSessionCategories.filter(
        e => new Date(e.date).getTime() > thirtyDaysAgo
      );
    }

    saveJSON(OPS_STATE_FILE, state);
  } catch (_) {}
}

// ===========================================================================
// Shared Helpers (git runs ONCE, shared across tasks)
// ===========================================================================

function getModifiedFiles() {
  try {
    const output = execSync('git status --porcelain', {
      cwd: PROJECT_ROOT, encoding: 'utf8', timeout: 10000,
      // Prevent git from waiting for credentials or pager
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0', GIT_PAGER: '' },
    }).trim();
    if (!output) return [];
    return output.split('\n').filter(Boolean).map(line => {
      const match = line.match(/^\s*(\S+)\s+(.*)/);
      if (!match) return null;
      // Normalize Windows backslash paths to forward slashes
      return { status: match[1], file: match[2].replace(/\\/g, '/') };
    }).filter(Boolean);
  } catch (_) { return []; }
}

function getRecentCommits() {
  try {
    const output = execSync('git log --oneline --since="2 hours ago"', {
      cwd: PROJECT_ROOT, encoding: 'utf8', timeout: 10000,
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0', GIT_PAGER: '' },
    }).trim();
    return output ? output.split('\n') : [];
  } catch (_) { return []; }
}

function detectAreas(files) {
  const areas = new Set();
  for (const f of files) {
    if (f.file.startsWith('src-tauri/')) areas.add('backend');
    else if (f.file.startsWith('src/')) areas.add('frontend');
    if (f.file.startsWith('.ai/')) areas.add('architecture');
    if (f.file.startsWith('.claude/')) areas.add('tooling');
    if (f.file.startsWith('specs/')) areas.add('specifications');
    if (f.file.startsWith('mcp-')) areas.add('mcp');
  }
  return areas;
}

function categorizeSession(files) {
  const areas = new Set();
  for (const f of files) {
    for (const mapping of AREA_MAP) {
      if (mapping.pattern.test(f.file)) {
        areas.add(mapping.area);
        break;
      }
    }
  }
  return [...areas];
}

function loadOpsState() {
  const defaults = {
    sovereignty: { score: 0, components: {}, lastComputed: null },
    cadence: { lastDaily: null, lastWeekly: null, lastMonthly: null },
    escalationQueue: [],
    immuneScanPending: false,
    immuneContext: null,
    metabolism: { fileChangeFrequency: {}, lastMetabolismScan: null },
    drift: { recentSessionCategories: [], lastDriftReport: null },
    compound: {
      sessionsCompleted: 0, decisionsStored: 0, decisionsReferenced: 0,
      antibodiesCreated: 0, antibodiesTriggered: 0, crystallizationsRun: 0, reworkEvents: 0,
    },
  };
  try {
    if (fs.existsSync(OPS_STATE_FILE)) {
      return deepMerge(defaults, JSON.parse(fs.readFileSync(OPS_STATE_FILE, 'utf8')));
    }
  } catch (_) {}
  return defaults;
}

function deepMerge(target, source) {
  const result = { ...target };
  for (const key of Object.keys(source)) {
    if (source[key] && typeof source[key] === 'object' && !Array.isArray(source[key])
        && target[key] && typeof target[key] === 'object' && !Array.isArray(target[key])) {
      result[key] = deepMerge(target[key], source[key]);
    } else {
      result[key] = source[key];
    }
  }
  return result;
}

function formatTimestamp() {
  const d = new Date();
  const pad = n => String(n).padStart(2, '0');
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
}

function ensureDir(dir) {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function loadJSON(filepath, fallback) {
  try {
    if (fs.existsSync(filepath)) return JSON.parse(fs.readFileSync(filepath, 'utf8'));
  } catch (_) {}
  return fallback;
}

function saveJSON(filepath, data) {
  try { fs.writeFileSync(filepath, JSON.stringify(data, null, 2)); } catch (_) {}
}

function appendLog(line) {
  try {
    ensureDir(path.dirname(SESSIONS_LOG));
    fs.appendFileSync(SESSIONS_LOG, line + '\n');
  } catch (_) {}
}

function output(obj) {
  console.log(JSON.stringify(obj));
}
