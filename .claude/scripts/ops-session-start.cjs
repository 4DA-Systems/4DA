#!/usr/bin/env node
/**
 * AOS Session Start — UserPromptSubmit hook
 *
 * Runs on every prompt. On first prompt of a new session:
 * 1. Loads ops-state.json (sovereignty score, cadence timestamps, escalation queue)
 * 2. Computes sovereignty score delta since last session
 * 3. Checks cadence triggers (daily/weekly/monthly overdue)
 * 4. Checks escalation queue for pending Tier 2/3 items
 * 5. Checks if immune scan is pending (flagged by session-end after bug fix)
 * 6. Outputs briefing message with instructions
 *
 * Never blocks. If ops-state.json doesn't exist, creates defaults and outputs minimal briefing.
 */

const fs = require('fs');
const path = require('path');

const CLAUDE_DIR = path.dirname(__dirname);
const WISDOM_DIR = path.join(CLAUDE_DIR, 'wisdom');
const OPS_STATE_FILE = path.join(WISDOM_DIR, 'ops-state.json');
const SESSION_MARKER = path.join(WISDOM_DIR, '.current-session');

// Default state structure
const DEFAULT_STATE = {
  sovereignty: {
    score: 0,
    components: {
      buildHealth: 0,
      testHealth: 0,
      sourcePipeline: 0,
      dependencyFreshness: 0,
      invariantCompliance: 0,
      fileSizeCompliance: 0,
      decisionDebt: 0,
      strategicAlignment: 0,
      memoryHealth: 0,
      metabolism: 0,
    },
    lastComputed: null,
  },
  cadence: {
    lastDaily: null,
    lastWeekly: null,
    lastMonthly: null,
  },
  escalationQueue: [],
  immuneScanPending: false,
  immuneContext: null,
  metabolism: {
    fileChangeFrequency: {},
    lastMetabolismScan: null,
  },
  testCounts: {
    history: [],  // Array of { date, rust, frontend, total }
    lastRecorded: null,
  },
  drift: {
    recentSessionCategories: [],
    lastDriftReport: null,
  },
  compound: {
    sessionsCompleted: 0,
    decisionsStored: 0,
    decisionsReferenced: 0,
    antibodiesCreated: 0,
    antibodiesTriggered: 0,
    crystallizationsRun: 0,
    reworkEvents: 0,
  },
};

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('readable', () => {
  let chunk;
  while ((chunk = process.stdin.read()) !== null) input += chunk;
});

process.stdin.on('end', () => {
  try {
    ensureDir(WISDOM_DIR);

    // Use same session detection as wisdom-auto.cjs
    if (!isFirstPrompt()) {
      console.log(JSON.stringify({ status: 'success' }));
      return;
    }

    const state = loadState();
    const briefing = buildBriefing(state);

    if (briefing) {
      console.log(JSON.stringify({ status: 'success', message: briefing }));
    } else {
      console.log(JSON.stringify({ status: 'success' }));
    }
  } catch (e) {
    // Never block the session
    console.log(JSON.stringify({ status: 'success' }));
  }
});

/**
 * Detect first prompt using the same marker file as wisdom-auto.cjs.
 * Checks the marker age — if older than 5 minutes, this is a new session.
 */
function isFirstPrompt() {
  try {
    if (fs.existsSync(SESSION_MARKER)) {
      const marker = JSON.parse(fs.readFileSync(SESSION_MARKER, 'utf8'));
      const age = Date.now() - (marker.timestamp || 0);
      if (age < 5 * 60 * 1000) {
        return false; // Same session
      }
    }
  } catch (e) {}
  // New session — runs before wisdom-auto.cjs in hook order.
  // wisdom-auto.cjs will create/update the marker when it runs next.
  return true;
}

/**
 * Load ops-state.json or create defaults if missing.
 */
function loadState() {
  try {
    if (fs.existsSync(OPS_STATE_FILE)) {
      const raw = JSON.parse(fs.readFileSync(OPS_STATE_FILE, 'utf8'));
      // Merge with defaults to handle new fields added after initial creation
      return deepMerge(DEFAULT_STATE, raw);
    }
  } catch (e) {}

  // Create default state file
  saveJSON(OPS_STATE_FILE, DEFAULT_STATE);
  return { ...DEFAULT_STATE };
}

/**
 * Build the session briefing message.
 */
function buildBriefing(state) {
  const lines = [];
  const instructions = [];

  lines.push('AOS \u2014 Session Briefing');
  lines.push('\u2501'.repeat(22));

  // Sovereignty score
  const score = state.sovereignty.score || 0;
  const lastComputed = state.sovereignty.lastComputed;
  if (lastComputed) {
    lines.push(`Sovereignty: ${score}/100`);
  } else {
    lines.push('Sovereignty: not yet computed \u2014 run /ops to initialize');
    instructions.push('Compute sovereignty score: /ops');
  }

  // Test health
  const testHistory = (state.testCounts && state.testCounts.history) || [];
  if (testHistory.length > 0) {
    const latest = testHistory[testHistory.length - 1];
    let testLine = `TEST HEALTH: ${latest.total} total (${latest.rust} Rust + ${latest.frontend} frontend)`;
    if (testHistory.length >= 2) {
      const previous = testHistory[testHistory.length - 2];
      const delta = latest.total - previous.total;
      if (delta > 0) {
        testLine += ` \u2191${delta} since last session`;
      } else if (delta < 0) {
        testLine += ` \u2193${Math.abs(delta)} since last session`;
      } else {
        testLine += ` \u2192 unchanged since last session`;
      }
      if (delta < 0) {
        instructions.push('Warning: Test count decreased! Investigate regressions.');
      }
    }
    lines.push(testLine);
  } else {
    lines.push('TEST HEALTH: not yet tracked \u2014 run /ops to record baseline');
  }

  // Coverage status
  const coverage = state.coverage;
  if (coverage && coverage.lastRun) {
    const ratio = (coverage.coverageRatio * 100).toFixed(1);
    lines.push(`COVERAGE: ${ratio}% (target 25%) | ${coverage.hotUntestedCount || 0} hot+untested modules`);
    if (coverage.hotUntestedCount > 0) {
      instructions.push('Address hot+untested modules: node scripts/coverage-map.cjs');
    }
  }

  lines.push('');

  // Cadence checks
  lines.push('CADENCE:');
  const now = Date.now();
  const cadence = state.cadence || {};

  const dailyStatus = getCadenceStatus(cadence.lastDaily, 24 * 60 * 60 * 1000, 'daily');
  const weeklyStatus = getCadenceStatus(cadence.lastWeekly, 7 * 24 * 60 * 60 * 1000, 'weekly');
  const monthlyStatus = getCadenceStatus(cadence.lastMonthly, 30 * 24 * 60 * 60 * 1000, 'monthly');

  lines.push(`  Daily: ${dailyStatus.display}`);
  lines.push(`  Weekly: ${weeklyStatus.display}`);
  lines.push(`  Monthly: ${monthlyStatus.display}`);

  if (dailyStatus.overdue) instructions.push('Run overdue cadence: /ops daily');
  if (weeklyStatus.overdue) instructions.push('Run overdue cadence: /ops weekly');
  if (monthlyStatus.overdue) instructions.push('Run overdue cadence: /ops monthly');

  // Escalation queue
  const pending = (state.escalationQueue || []).filter(e => !e.resolved);
  if (pending.length > 0) {
    lines.push('');
    lines.push(`PENDING (${pending.length} item${pending.length > 1 ? 's' : ''}):`);
    for (const item of pending.slice(0, 5)) {
      lines.push(`  [Tier ${item.tier}] ${item.title}${item.summary ? ' \u2014 ' + item.summary : ''}`);
    }
    if (pending.length > 5) {
      lines.push(`  ... and ${pending.length - 5} more`);
    }
    instructions.push('Present decisions: /ops decide');
  }

  // Immune scan pending
  if (state.immuneScanPending) {
    lines.push('');
    lines.push('IMMUNE: Bug fix detected last session \u2014 antibody scan recommended');
    instructions.push('Run immune scan: /ops immune');
  }

  // Instructions
  if (instructions.length > 0) {
    lines.push('');
    lines.push('INSTRUCTIONS:');
    instructions.forEach((inst, i) => {
      lines.push(`${i + 1}. ${inst}`);
    });
    lines.push(`${instructions.length + 1}. Then proceed with user's request`);
  }

  // Compound advantage summary
  const compound = state.compound || {};
  lines.push('');
  lines.push('COMPOUND:');
  lines.push(`  Sessions completed: ${compound.sessionsCompleted || 0}`);
  lines.push(`  Decisions: ${compound.decisionsStored || 0} stored / ${compound.decisionsReferenced || 0} referenced`);
  lines.push(`  Antibodies: ${compound.antibodiesCreated || 0} created / ${compound.antibodiesTriggered || 0} triggered`);

  return lines.join('\n');
}

/**
 * Check cadence status — returns display string and overdue flag.
 */
function getCadenceStatus(lastRun, intervalMs, name) {
  if (!lastRun) {
    return { display: `never run \u2192 run /ops ${name}`, overdue: true };
  }

  const elapsed = Date.now() - new Date(lastRun).getTime();
  const hoursAgo = Math.floor(elapsed / (60 * 60 * 1000));
  const daysAgo = Math.floor(elapsed / (24 * 60 * 60 * 1000));

  const timeStr = daysAgo > 0 ? `${daysAgo}d ago` : `${hoursAgo}h ago`;

  if (elapsed > intervalMs) {
    return { display: `OVERDUE (last ${timeStr}) \u2192 run /ops ${name}`, overdue: true };
  }
  return { display: `current (last ${timeStr})`, overdue: false };
}

/**
 * Deep merge two objects (source values override target).
 */
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

function ensureDir(dir) {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function saveJSON(filepath, data) {
  try { fs.writeFileSync(filepath, JSON.stringify(data, null, 2)); } catch (e) {}
}
