#!/usr/bin/env node
/**
 * Wisdom Auto — UserPromptSubmit hook
 *
 * Runs on every prompt. Handles two autonomous functions:
 *
 * 1. SESSION CONTINUITY: On first prompt, checks for a pending wisdom digest
 *    from the previous session and instructs Claude to record consequences.
 *
 * 2. CRYSTALLIZATION TRIGGER: Every N sessions, reminds Claude to run
 *    /crystallize to review accumulated learnings for pattern promotion.
 *
 * This is the "consequence processing" half of the autonomous wisdom layer.
 * Paired with wisdom-digest.cjs (Stop hook) which captures session activity.
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const CLAUDE_DIR = path.dirname(__dirname);
const WISDOM_DIR = path.join(CLAUDE_DIR, 'wisdom');
const STATE_FILE = path.join(WISDOM_DIR, 'state.json');
const PENDING_FILE = path.join(WISDOM_DIR, 'pending.json');
const SESSION_MARKER = path.join(WISDOM_DIR, '.current-session');
const CRYSTALLIZE_INTERVAL = 15;

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('readable', () => {
  let chunk;
  while ((chunk = process.stdin.read()) !== null) input += chunk;
});

process.stdin.on('end', () => {
  try {
    ensureDir(WISDOM_DIR);
    const messages = [];
    const isFirstPrompt = checkFirstPrompt();

    if (isFirstPrompt) {
      // 1. Process pending wisdom digest from previous session
      const digestMsg = processPendingDigest();
      if (digestMsg) messages.push(digestMsg);

      // 2. Check if crystallization is due
      const crystalMsg = checkCrystallization();
      if (crystalMsg) messages.push(crystalMsg);

      // 3. Inject accumulated AWE wisdom (principles, anti-patterns)
      const aweWisdom = getAweWisdom();
      if (aweWisdom) {
        messages.push('AWE WISDOM LAYER — Accumulated Engineering Wisdom\n' + aweWisdom);
      }

      // 4. Prompt for AWE feedback on previous session's decisions
      const aweFeedback = getAweFeedbackPrompt();
      if (aweFeedback) messages.push(aweFeedback);

      // 5. Push developer identity to AWE for personalized wisdom
      pushIdentityToAwe();

      // 6. Nudge triage if auto-detected candidates are waiting
      const triageNudge = checkTriageCandidates();
      if (triageNudge) messages.push(triageNudge);
    }

    if (messages.length > 0) {
      console.log(JSON.stringify({ status: 'success', message: messages.join('\n\n') }));
    } else {
      console.log(JSON.stringify({ status: 'success' }));
    }
  } catch (e) {
    console.log(JSON.stringify({ status: 'success' }));
  }
});

/**
 * Detect if this is the first prompt of a new session.
 * Uses a marker file with a timestamp — if the marker is older than 5 minutes
 * or doesn't exist, this is a new session.
 *
 * This hook runs AFTER ops-session-start.cjs in hook order.
 * It owns the marker file — reads, creates, and updates it.
 */
function checkFirstPrompt() {
  try {
    if (fs.existsSync(SESSION_MARKER)) {
      let marker;
      try {
        marker = JSON.parse(fs.readFileSync(SESSION_MARKER, 'utf8'));
      } catch (parseErr) {
        // Corrupted marker file — treat as new session
        marker = { timestamp: 0 };
      }
      const age = Date.now() - (marker.timestamp || 0);
      // If marker is less than 5 minutes old, same session
      if (age < 5 * 60 * 1000) {
        // Update timestamp to keep session alive
        marker.timestamp = Date.now();
        marker.promptCount = (marker.promptCount || 0) + 1;
        try { fs.writeFileSync(SESSION_MARKER, JSON.stringify(marker)); } catch (_) {}
        return false;
      }
    }
  } catch (e) {}

  // New session — create marker
  try {
    fs.writeFileSync(SESSION_MARKER, JSON.stringify({
      timestamp: Date.now(),
      promptCount: 1,
    }));
  } catch (_) {}
  return true;
}

/**
 * Process pending wisdom digest from previous session.
 * Returns a message string or null.
 */
function processPendingDigest() {
  if (!fs.existsSync(PENDING_FILE)) return null;

  let digest;
  try {
    digest = JSON.parse(fs.readFileSync(PENDING_FILE, 'utf8'));
  } catch (e) { return null; }

  if (digest.processed) return null;

  // Build continuity message
  const areas = digest.areas.join(', ') || 'unknown areas';
  let msg = `WISDOM LAYER — Session Continuity`;
  msg += `\nPrevious session (#${digest.sessionNumber}) modified ${digest.filesModified} files (${areas}).`;

  if (digest.aiFilesTouched) {
    msg += `\nArchitecture docs (.ai/) were modified — verify consistency.`;
  }

  if (digest.recentCommits && digest.recentCommits.length > 0) {
    msg += `\nCommits: ${digest.recentCommits.slice(0, 3).join(' | ')}`;
  }

  msg += `\n`;
  msg += `\nAUTONOMOUS ACTION REQUIRED:`;
  msg += `\n• Check if any decisions were made that should be recorded (remember_decision)`;
  msg += `\n• Check if any failures or gotchas were discovered (remember_learning)`;
  msg += `\n• If nothing noteworthy, proceed — but acknowledge this check was done.`;

  // Mark as processed so it doesn't fire again
  digest.processed = true;
  try { fs.writeFileSync(PENDING_FILE, JSON.stringify(digest, null, 2)); } catch (e) {}

  return msg;
}

/**
 * Check if crystallization review is due.
 * Returns a message string or null.
 */
function checkCrystallization() {
  const state = loadJSON(STATE_FILE, { sessionCount: 0 });
  const lastCrystallize = state.lastCrystallize || 0;
  const sessionsSince = (state.sessionCount || 0) - lastCrystallize;

  if (sessionsSince >= CRYSTALLIZE_INTERVAL) {
    // Update state so we don't re-trigger next prompt
    state.lastCrystallize = state.sessionCount;
    saveJSON(STATE_FILE, state);

    return (
      `WISDOM LAYER — Crystallization Due` +
      `\n${sessionsSince} sessions since last crystallization review.` +
      `\n` +
      `\nAUTONOMOUS ACTION REQUIRED:` +
      `\n• Run /crystallize to review accumulated learnings for pattern promotion.` +
      `\n• Deferrable if current task is urgent (W-6: paralysis is not wisdom).`
    );
  }

  return null;
}

/**
 * Get accumulated wisdom from AWE (principles, anti-patterns).
 * Returns the wisdom text or null if unavailable.
 */
function getAweWisdom() {
  const AWE_BIN = 'D:\\runyourempire\\awe\\target\\release\\awe.exe';
  try {
    if (!fs.existsSync(AWE_BIN)) return null;
    const output = execSync(`"${AWE_BIN}" wisdom --domain software-engineering`, {
      encoding: 'utf8',
      timeout: 5000,
      env: { ...process.env },
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
    // Only inject if there's actual extracted wisdom (principles or anti-patterns)
    // Only inject if there are actual validated principles or confirmed anti-patterns
    if (output.includes('No wisdom') || output.includes('needs at least') || output.includes('More feedback needed') || output.includes('Patterns are forming')) return null;
    // Must contain actual VALIDATED PRINCIPLES or CONFIRMED ANTI-PATTERNS sections
    if (!output.includes('VALIDATED PRINCIPLES') && !output.includes('CONFIRMED ANTI-PATTERNS')) return null;
    // Strip extraction status metadata — only inject the wisdom sections
    const lines = output.split('\n');
    const filtered = [];
    let inWisdomSection = false;
    for (const line of lines) {
      if (line.includes('VALIDATED PRINCIPLES') || line.includes('CONFIRMED ANTI-PATTERNS')) {
        inWisdomSection = true;
      }
      if (line.includes('EXTRACTION STATUS')) {
        inWisdomSection = false;
      }
      if (inWisdomSection) {
        filtered.push(line);
      }
    }
    return filtered.length > 0 ? filtered.join('\n').trim() : null;
  } catch (e) { return null; }
}

/**
 * Check if AWE decisions from the previous session need outcome feedback.
 * Returns a feedback prompt message or null.
 */
function getAweFeedbackPrompt() {
  if (!fs.existsSync(PENDING_FILE)) return null;
  let pending;
  try { pending = JSON.parse(fs.readFileSync(PENDING_FILE, 'utf8')); } catch (e) { return null; }
  if (!pending.aweDecisionIds || pending.aweDecisionIds.length === 0) return null;
  if (pending.aweFeedbackProcessed) return null;

  const ids = pending.aweDecisionIds;
  let msg = 'AWE FEEDBACK — Close the learning loop';
  msg += `\n${ids.length} decision(s) from last session need outcome feedback:`;
  for (const entry of ids) {
    msg += `\n  ${entry.id}: "${entry.query}"`;
  }
  msg += '\n\nFor each decision, call awe_feedback with the decision_id and outcome (confirmed/refuted/partial/unknown).';
  msg += '\nIf outcome is unknown yet, skip — but revisit later.';

  // Mark as processed
  pending.aweFeedbackProcessed = true;
  try { fs.writeFileSync(PENDING_FILE, JSON.stringify(pending, null, 2)); } catch (e) {}

  return msg;
}

/**
 * Push developer identity to AWE for personalized wisdom.
 * Reads from 4DA's database and writes identity.json to AWE's data dir.
 * Non-blocking, best-effort — failures are silent.
 */
function pushIdentityToAwe() {
  try {
    const aweDataDir = path.join(process.env.APPDATA || '', 'awe');
    const identityPath = path.join(aweDataDir, 'identity.json');

    // Check freshness — only update if >1 hour old
    if (fs.existsSync(identityPath)) {
      const age = Date.now() - fs.statSync(identityPath).mtimeMs;
      if (age < 3_600_000) return; // Fresh enough
    }

    // Read project stats from 4DA database
    const dbPath = path.join(__dirname, '..', '..', 'data', '4da.db');
    if (!fs.existsSync(dbPath)) return;

    // Use sqlite3 CLI for a quick read (no native module dependency)
    const projectCount = execSync(
      `sqlite3 "${dbPath}" "SELECT COUNT(*) FROM projects;"`,
      { encoding: 'utf8', timeout: 3000 }
    ).trim();

    const depCount = execSync(
      `sqlite3 "${dbPath}" "SELECT COUNT(DISTINCT name) FROM dependencies;"`,
      { encoding: 'utf8', timeout: 3000 }
    ).trim();

    const itemCount = execSync(
      `sqlite3 "${dbPath}" "SELECT COUNT(*) FROM content_items;"`,
      { encoding: 'utf8', timeout: 3000 }
    ).trim();

    const daysActive = execSync(
      `sqlite3 "${dbPath}" "SELECT CAST((julianday('now') - julianday(MIN(created_at))) AS INTEGER) FROM content_items;"`,
      { encoding: 'utf8', timeout: 3000 }
    ).trim();

    const identity = {
      primary_stack: ['react', 'tauri', 'typescript'],
      adjacent_tech: ['rust', 'sqlite-vec', 'tokio', 'serde'],
      domain_concerns: ['privacy', 'local-first', 'performance', 'zero-config'],
      identity_summary: 'Solo founder building privacy-first desktop intelligence app (4DA)',
      project_count: parseInt(projectCount) || 0,
      dependency_count: parseInt(depCount) || 0,
      days_active: parseInt(daysActive) || 0,
      items_processed: parseInt(itemCount) || 0,
      knowledge_gaps: [],
      blind_spots: [],
    };

    if (!fs.existsSync(aweDataDir)) fs.mkdirSync(aweDataDir, { recursive: true });
    fs.writeFileSync(identityPath, JSON.stringify(identity, null, 2));
  } catch (_) {
    // Identity push is best-effort — never fail the hook
  }
}

/**
 * Check if auto-detected AWE decisions need triage.
 * Returns a nudge message or null.
 */
function checkTriageCandidates() {
  const AWE_BIN = 'D:\\runyourempire\\awe\\target\\release\\awe.exe';
  try {
    if (!fs.existsSync(AWE_BIN)) return null;
    const output = execSync(`"${AWE_BIN}" triage --json --limit 5`, {
      encoding: 'utf8',
      timeout: 5000,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();

    if (!output) return null;
    const data = JSON.parse(output);
    const count = data.total_unvalidated || (Array.isArray(data.candidates) ? data.candidates.length : 0);
    if (count === 0) return null;

    return (
      `AWE TRIAGE — ${count} decision candidate(s) awaiting review` +
      `\nRun awe_triage with action "list" to review, then "confirm" or "dismiss" each.` +
      `\nOnly confirmed decisions feed principle extraction. Quality over quantity.`
    );
  } catch (_) {
    return null;
  }
}

function ensureDir(dir) {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function loadJSON(filepath, fallback) {
  try {
    if (fs.existsSync(filepath)) return JSON.parse(fs.readFileSync(filepath, 'utf8'));
  } catch (e) {}
  return fallback;
}

function saveJSON(filepath, data) {
  try { fs.writeFileSync(filepath, JSON.stringify(data, null, 2)); } catch (e) {}
}
