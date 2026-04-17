#!/usr/bin/env node
/**
 * Public-readiness audit — on-demand, non-blocking.
 *
 * Run before flipping the repo to public, periodically during development,
 * or any time you suspect drift. Unlike the pre-commit gate, this audits
 * the ENTIRE tracked file set and reports all findings, not just staged
 * changes. Exits non-zero if any blocking finding is detected.
 *
 * Invocation:
 *   node scripts/public-readiness-audit.cjs
 *   pnpm run audit:public-ready
 *
 * What it checks:
 *   1. Root-level docs match the public allowlist
 *   2. Mixed-directory allowlists (.ai/, docs/strategy/) are respected
 *   3. No PII patterns in any tracked file
 *   4. No suspicious filename patterns (secret, credential, private, etc)
 *   5. No API key / JWT / private-key block patterns in content
 *   6. Aggressive-phrasing review (warn only, not blocking)
 *   7. LICENSE + README exist
 *
 * Output: grouped findings by severity, exit code reflects blocking count.
 */

const { execSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const GREEN = '\x1b[32m';
const CYAN = '\x1b[36m';
const BOLD = '\x1b[1m';
const DIM = '\x1b[2m';
const RESET = '\x1b[0m';

const REPO_ROOT = execSync('git rev-parse --show-toplevel', { encoding: 'utf8' }).trim();
const ALLOWLIST = JSON.parse(
  fs.readFileSync(path.join(REPO_ROOT, 'scripts/doc-allowlist.json'), 'utf8')
);

// Sync with scripts/check-doc-location.cjs
const ROOT_BLOCK_PATTERNS = [
  /PLAN\.md$/i, /-PLAN\.md$/i, /^PLAN-/i,
  /STRATEGY\.md$/i, /-STRATEGY-/i,
  /AUDIT\.md$/i, /-AUDIT-/i,
  /CHECKLIST\.md$/i, /ROADMAP\.md$/i,
  /TRAJECTORY\.md$/i, /^TRAJECTORY-/i,
  /^VIRAL-/i, /^LAUNCH-/i, /^PRE-LAUNCH-/i, /^FIRST-CONTACT-/i,
  /^FORTIFICATION-/i, /^EXECUTION-/i, /^ASCENT-/i, /^BATTLE-/i,
  /^MASTER-/i, /^BARRIER-/i, /^IMPROVEMENTS-/i, /^CRITICAL-/i,
  /^DEVELOPER-OS-/i, /^NOTIFICATION-INTELLIGENCE-/i,
  /^INTELLIGENCE-CONSOLE-/i,
  /^whats-next\.md$/i, /^WHATS-NEXT\.md$/i, /^NEXT-STEPS\.md$/i,
  /^MISSION_/i, /^SHIP_/i,
];

const MIXED_DIRS = ['.ai/', 'docs/strategy/'];

const PII_PATTERNS = [
  { label: 'Personal Gmail (runyourempirehq)', regex: /runyourempirehq@gmail\.com/gi },
  { label: 'Personal Gmail (4dasystems)',     regex: /4dasystems@gmail\.com/gi },
];

// Files where the PII pattern is meta-reference (documenting what NOT to track),
// not an actual leak. Keep in sync with scripts/check-doc-location.cjs.
const PII_EXCLUDE_PATHS = new Set([
  'scripts/check-doc-location.cjs',
  'scripts/public-readiness-audit.cjs',
  'scripts/doc-allowlist.json',
  '.claude/rules/document-hygiene.md',
  'CLAUDE.md',
]);

const SECRET_PATTERNS = [
  { label: 'OpenAI secret key',       regex: /sk-[a-zA-Z0-9]{32,}/ },
  { label: 'Stripe live secret key',  regex: /sk_live_[a-zA-Z0-9]{20,}/ },
  { label: 'Slack bot token',         regex: /xoxb-[a-zA-Z0-9-]{40,}/ },
  { label: 'GitHub personal token',   regex: /ghp_[a-zA-Z0-9]{30,}/ },
  { label: 'AWS access key',          regex: /\bAKIA[A-Z0-9]{16}\b/ },
  { label: 'Google API key',          regex: /\bAIza[a-zA-Z0-9_-]{30,}\b/ },
  { label: 'JWT',                     regex: /\beyJ[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\b/ },
  { label: 'Private key block',       regex: /BEGIN (RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY/ },
];

const SUSPICIOUS_FILENAMES = [
  /\bsecret\b/i, /\bcredential(s)?\b/i, /\bpassword(s)?\b/i,
  /\.env$/, /\.pem$/, /\.p12$/, /\.pfx$/, /\.key$/,
  /privkey/i, /api[-_.]key/i,
];

const SUSPICIOUS_EXCLUDE = [
  /\.example$/, /\.sample$/,
  /^docs\//, /^\.github\//, /\.md$/, /scripts\//,
  /site\/src\//, /merch-print-ready\//, /package\.json$/, /pnpm-lock\.yaml$/,
  /settings\/keystore\.rs$/, // the secure-storage implementation
  /privacy_tests\//,
  /password-page\.html$/,
  /^src\//, /^src-tauri\/src\/settings\//,
];

// Patterns only match STRATEGIC tone (adjacent to competitor/market/category)
// not natural English usage ("dominate a niche", "signals dominate composite").
const AGGRESSIVE_PATTERNS = [
  { label: 'market/competitor domination thesis',
    regex: /(market|competitor|category|total|absolute|global)\s+dominat(e|ion|ing)/i },
  { label: 'domination thesis',
    regex: /thesis.*dominat(e|ion)|dominat(e|ion)\s+thesis/i },
  { label: 'destroy competitor', regex: /destroy\s+competitor/i },
  { label: 'kill competitor',    regex: /kill\s+competitor/i },
  { label: 'weaponise/weaponize against',
    regex: /weaponis?e\s+against|weaponis?ed?\s+(to|for)/i },
  { label: 'nuclear option / nuclear defense (strategic posturing)',
    regex: /nuclear\s+(defense|defence|option)/i },
];

function getTrackedFiles() {
  const raw = execSync('git ls-files', { encoding: 'utf8' });
  return raw.split('\n').filter(Boolean);
}

function readSafe(filepath) {
  try {
    const abs = path.join(REPO_ROOT, filepath);
    const stat = fs.statSync(abs);
    if (stat.size > 2 * 1024 * 1024) return null;
    const content = fs.readFileSync(abs, 'utf8');
    if (content.includes('\u0000')) return null; // binary
    return content;
  } catch { return null; }
}

function check1_rootAllowlist(files, findings) {
  const rootAllow = new Set(ALLOWLIST.root.files);
  for (const f of files) {
    if (f.includes('/')) continue;
    if (!f.endsWith('.md')) continue;
    if (rootAllow.has(f)) continue;
    if (ROOT_BLOCK_PATTERNS.some(re => re.test(f))) {
      findings.push({ sev: 'block', rule: 'root-allowlist', file: f,
        msg: 'repo-root .md not on public allowlist and matches block pattern' });
    }
  }
}

function check2_mixedDirs(files, findings) {
  for (const f of files) {
    for (const dir of MIXED_DIRS) {
      if (!f.startsWith(dir)) continue;
      const rel = f.slice(dir.length);
      if (rel.includes('/')) {
        findings.push({ sev: 'block', rule: 'mixed-nested', file: f,
          msg: `nested path under ${dir} — should be flat or moved` });
        continue;
      }
      const allowed = new Set((ALLOWLIST[dir] && ALLOWLIST[dir].files) || []);
      if (!allowed.has(rel)) {
        findings.push({ sev: 'block', rule: 'mixed-allowlist', file: f,
          msg: `not on ${dir} allowlist (see scripts/doc-allowlist.json)` });
      }
    }
  }
}

function check3_pii(files, findings) {
  for (const f of files) {
    if (PII_EXCLUDE_PATHS.has(f)) continue;
    const content = readSafe(f);
    if (!content) continue;
    for (const { label, regex } of PII_PATTERNS) {
      if (regex.test(content)) {
        findings.push({ sev: 'block', rule: 'pii', file: f, msg: label });
      }
    }
  }
}

function check4_suspiciousFilenames(files, findings) {
  for (const f of files) {
    if (SUSPICIOUS_EXCLUDE.some(re => re.test(f))) continue;
    for (const re of SUSPICIOUS_FILENAMES) {
      if (re.test(f)) {
        findings.push({ sev: 'warn', rule: 'suspicious-filename', file: f,
          msg: `filename matches ${re}` });
        break;
      }
    }
  }
}

function check5_secrets(files, findings) {
  for (const f of files) {
    const content = readSafe(f);
    if (!content) continue;
    for (const { label, regex } of SECRET_PATTERNS) {
      const match = content.match(regex);
      if (match) {
        findings.push({ sev: 'block', rule: 'secret', file: f,
          msg: `${label}: ${match[0].slice(0, 20)}...` });
      }
    }
  }
}

function check6_aggressivePhrasing(files, findings) {
  for (const f of files) {
    if (!f.endsWith('.md') && !f.endsWith('.mdx')) continue;
    const content = readSafe(f);
    if (!content) continue;
    for (const { label, regex } of AGGRESSIVE_PATTERNS) {
      if (regex.test(content)) {
        findings.push({ sev: 'warn', rule: 'aggressive-phrasing', file: f,
          msg: `matches "${label}"` });
      }
    }
  }
}

function check7_presence(files, findings) {
  const required = ['README.md', 'LICENSE'];
  for (const r of required) {
    if (!files.includes(r)) {
      findings.push({ sev: 'block', rule: 'missing-required', file: r,
        msg: `required file missing at repo root` });
    }
  }
}

function main() {
  const files = getTrackedFiles();
  const findings = [];

  check1_rootAllowlist(files, findings);
  check2_mixedDirs(files, findings);
  check3_pii(files, findings);
  check4_suspiciousFilenames(files, findings);
  check5_secrets(files, findings);
  check6_aggressivePhrasing(files, findings);
  check7_presence(files, findings);

  const blocks = findings.filter(f => f.sev === 'block');
  const warns  = findings.filter(f => f.sev === 'warn');

  console.log('');
  console.log(`${BOLD}${CYAN}Public Readiness Audit${RESET}`);
  console.log(`${DIM}Tracked files: ${files.length}${RESET}`);
  console.log('');

  const byRule = {};
  for (const f of findings) {
    byRule[f.rule] = byRule[f.rule] || [];
    byRule[f.rule].push(f);
  }

  for (const rule of Object.keys(byRule)) {
    const group = byRule[rule];
    const sev = group[0].sev;
    const color = sev === 'block' ? RED : YELLOW;
    const icon = sev === 'block' ? '✗' : '⚠';
    console.log(`${color}${BOLD}${icon} ${rule}${RESET}  (${group.length})`);
    for (const f of group) {
      console.log(`  ${color}${f.file}${RESET}${DIM} — ${f.msg}${RESET}`);
    }
    console.log('');
  }

  if (blocks.length === 0 && warns.length === 0) {
    console.log(`${GREEN}${BOLD}✓ No findings. Repo passes public-readiness audit.${RESET}`);
    console.log('');
    return 0;
  }

  console.log(`${BOLD}Summary:${RESET} ${RED}${blocks.length} blocking${RESET}, ${YELLOW}${warns.length} warning${RESET}`);
  console.log('');

  if (blocks.length > 0) {
    console.log(`${RED}${BOLD}Not ready for public release.${RESET} Resolve blocking findings above.`);
    console.log('');
    return 1;
  }

  console.log(`${GREEN}${BOLD}Ready for public release${RESET} (warnings are advisory).`);
  console.log('');
  return 0;
}

process.exit(main());
