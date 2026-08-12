#!/usr/bin/env node
/**
 * scan-secrets.cjs — THE secret-detection engine for 4DA.
 *
 * SINGLE SOURCE OF TRUTH. The patterns below were previously maintained in
 * THREE places — here, inline in .husky/pre-commit, and again as a flat regex
 * alternation in .husky/pre-push — so a pattern added to one silently missed
 * the other two. Both hooks now shell out to this file. Add a pattern HERE and
 * every layer picks it up.
 *
 * ---------------------------------------------------------------------------
 * THREE MODES, because the three callers genuinely scan different bytes.
 * These are NOT interchangeable; collapsing them would change what gets caught.
 *
 *   (default)      Working-tree content of every TRACKED file.
 *                  Caller: `pnpm run audit:public-ready`-style manual audits.
 *                  Exclusions: ALLOWLISTED_FILES (docs/legal/test fixtures that
 *                  legitimately contain key-shaped text).
 *
 *   --staged       STAGED content (`git show :FILE`) of staged files — NOT the
 *                  working tree, so a secret staged then edited out of the file
 *                  is still caught. Caller: .husky/pre-commit.
 *                  Exclusions: STAGED_EXCLUDE (the hook's historic skip list) +
 *                  per-pattern path exemptions for statutory ABN/phone
 *                  disclosures on legal pages.
 *
 *   --diff-added   Reads a `git log -p` stream on STDIN and scans only ADDED
 *                  (`+`) lines. Caller: .husky/pre-push, which owns the range
 *                  logic (`$RANGE --not origin/main`) because that is
 *                  push-protocol-specific. Uses the high-confidence pattern
 *                  subset only (`pushScan: true`) with NO exclude filters —
 *                  exactly matching the hook's previous flat alternation.
 * ---------------------------------------------------------------------------
 *
 * Usage:
 *   node scripts/scan-secrets.cjs                    # all tracked files
 *   node scripts/scan-secrets.cjs --staged           # staged content (pre-commit)
 *   git log -p ... | node scripts/scan-secrets.cjs --diff-added   # (pre-push)
 *   node scripts/scan-secrets.cjs --ci               # JSON output
 *   node scripts/scan-secrets.cjs --verbose          # show skipped files
 *
 * Exit codes:
 *   0 — clean, no secrets found
 *   1 — secrets detected
 *   2 — could not enumerate files (git failure)
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// --- Configuration ---

const ARGS = process.argv.slice(2);
const STAGED_ONLY = ARGS.includes('--staged');
const DIFF_ADDED = ARGS.includes('--diff-added');
const CI_MODE = ARGS.includes('--ci');
const VERBOSE = ARGS.includes('--verbose') || ARGS.includes('-v');

// Files/directories to always skip
const SKIP_PATHS = [
  /node_modules\//,
  /target\//,
  /dist\//,
  /\.git\//,
  /\.tsbuildinfo$/,
  /\.wasm$/,
  /\.png$/,
  /\.jpg$/,
  /\.jpeg$/,
  /\.gif$/,
  /\.ico$/,
  /\.svg$/,
  /\.woff2?$/,
  /\.ttf$/,
  /\.eot$/,
  /\.mp4$/,
  /\.webm$/,
  /\.mp3$/,
  /\.ogg$/,
  /\.pdf$/,
  /\.zip$/,
  /\.tar$/,
  /\.gz$/,
  /\.exe$/,
  /\.dll$/,
  /\.so$/,
  /\.dylib$/,
  /\.db$/,
  /\.db-shm$/,
  /\.db-wal$/,
  /package-lock\.json$/,
  /pnpm-lock\.yaml$/,
  /yarn\.lock$/,
  /Cargo\.lock$/,
];

// Files that are EXPECTED to mention secret patterns (documentation, examples, configs)
const ALLOWLISTED_FILES = [
  /\.gitignore$/,
  /SECURITY\.md$/,
  /CLAUDE\.md$/,
  /MEMORY\.md$/,
  /\.example$/,
  /settings\.example\.json$/,
  /scan-secrets\.cjs$/,        // this file itself
  /pre-commit$/,               // the hook that defines patterns
  /pre-push$/,                 // the hook that defines patterns
  /INVARIANTS\.md$/,
  /PRODUCT-CATALOG\.md$/,      // public company ABN in legal/merch docs
  /SHOPIFY-LAUNCH-GUIDE\.md$/, // public company ABN
  /SHOPIFY-SETUP-GUIDE\.md$/,  // public company ABN (personal ABN is public record)
  /docker-compose\.yml$/,      // template placeholder secrets, not real values
  /PRE-LAUNCH-PLAN\.md$/,      // public company ABN reference
  /TEAM-RELAY-ARCHITECTURE\.md$/, // architecture doc with placeholder SECRET: env var
  /FAILURE_MODES\.md$/,
  /WISDOM\.md$/,
  /DECISIONS\.md$/,
  // Legal docs — public company ABN is required in these
  /PRIVACY-POLICY\.md$/,
  /TERMS-OF-SERVICE\.md$/,
  /privacy\.njk$/,
  /terms\.njk$/,
  // Test files with intentionally fake keys for redaction testing
  /privacy_tests\.rs$/,
  /privacy_tests_exports\.rs$/,
  // Key format validation — detects key patterns, doesn't contain real keys
  /env_detection\.rs$/,
  /llm\.rs$/,
];

// ---------------------------------------------------------------------------
// STAGED MODE (.husky/pre-commit) exclusions.
// A literal port of the `grep -v` chain the hook used to build $STAGED_FILES.
// Changing this list changes what the commit gate lets through — keep it in
// step with the hook's intent, not with ALLOWLISTED_FILES above (they differ on
// purpose: the audit is stricter about test fixtures, the hook is stricter
// about statutory disclosures).
// ---------------------------------------------------------------------------
const STAGED_EXCLUDE = [
  /node_modules\//,
  /target\//,
  /dist\//,
  /\.gitignore$/,
  /SECURITY\.md$/,
  /\.example/,
  /\.test\./,
  /\.spec\./,
  /privacy_tests/,
  /PRE-LAUNCH-PLAN\.md$/,
  /PRODUCT-CATALOG\.md$/,
  /SHOPIFY-.*GUIDE\.md$/,
  /SHOPIFY-VERIFICATION\.md$/,
  /TEAM-RELAY-ARCHITECTURE\.md$/,
  /docker-compose\.yml$/,
  /terms\.njk$/,
  /privacy\.njk$/,
  /contact\.njk$/,
  /PRIVACY-POLICY\.md$/,
];

// Surfaces where the company ABN/ACN is a REQUIRED statutory disclosure.
// Ported from the `case` globs in .husky/pre-commit (shell `case` globs let `*`
// span `/`, so these are prefix matches, not single-segment matches).
const ABN_EXEMPT_PATHS = [
  /^LICENSE$/, /^LICENSE\.md$/, /^NOTICE$/, /^CLA\.md$/, /^TRADEMARKS\.md$/,
  /^SECURITY\.md$/, /^README\.md$/, /^CONTRIBUTING\.md$/, /^CODE_OF_CONDUCT\.md$/,
  /^NETWORK\.md$/,
  /^docs\/legal\//,
  /^docs\/TRUST-ARCHITECTURE\.md$/,
  /^docs\/NETWORK-TRANSPARENCY\.md$/,
  /^docs\/PRIVACY-PLAIN-LANGUAGE\.md$/,
  /^docs\/BUILD-FROM-SOURCE\.md$/,
  /^docs\/SECURITY-AUDIT-GUIDE\.md$/,
  /^docs\/VERIFY-DOWNLOADS\.md$/,
  /^docs\/RELEASE-NOTES-.*\.md$/,
  /^docs\/launch\//,
  /^docs\/philosophy\//,
  /^site\/src\//,
];

// The public contact page legitimately publishes a business phone number.
const AU_PHONE_EXEMPT_PATHS = [/^site\/src\/contact\.njk$/];

function matchesAny(list, value) {
  return list.some((re) => re.test(value));
}

// --- Secret Patterns ---
// Each pattern has: id, label, regex, and optional exclude regex for false positives.
//
//   pushScan          — included in the --diff-added (pre-push) subset
//   stagedExclude     — exclude regex used INSTEAD of `exclude` in --staged mode,
//                       preserving the pre-commit hook's historic (narrower)
//                       false-positive filter so staged coverage is not weakened
//   stagedExemptPaths — per-pattern path exemptions in --staged mode

const SECRET_PATTERNS = [
  // API Keys & Tokens
  {
    id: 'openai-proj',
    label: 'OpenAI Project Key',
    regex: /sk-proj-[A-Za-z0-9_-]{20,}/g,
    pushScan: true,
  },
  {
    id: 'anthropic',
    label: 'Anthropic API Key',
    regex: /sk-ant-[A-Za-z0-9_-]{20,}/g,
    pushScan: true,
  },
  {
    id: 'openai-generic',
    label: 'OpenAI Key (generic sk-)',
    regex: /sk-[a-zA-Z0-9]{20,}/g,
    exclude: /sk-ant-|sk-proj-|sk_live|sk_test|sk-[a-z]+-/,
    // pre-commit did NOT excuse `sk-<word>-`; keep the commit gate that strict.
    stagedExclude: /sk-ant-|sk-proj-|sk_live|sk_test/,
    pushScan: true,
  },
  {
    id: 'github',
    label: 'GitHub Token',
    regex: /gh[pousr]_[A-Za-z0-9]{36,}/g,
    pushScan: true,
  },
  {
    id: 'aws-key',
    label: 'AWS Access Key',
    regex: /(AKIA|ASIA)[0-9A-Z]{16}/g,
    pushScan: true,
  },
  {
    id: 'aws-secret',
    label: 'AWS Secret Key',
    regex: /aws_secret[_a-zA-Z]*\s*[:=]\s*['"][A-Za-z0-9/+=]{40}/g,
  },
  {
    id: 'google-api',
    label: 'Google API Key',
    regex: /AIza[0-9A-Za-z_-]{35}/g,
    pushScan: true,
  },
  {
    id: 'stripe-live',
    label: 'Stripe Live Key',
    regex: /[spr]k_live_[A-Za-z0-9]{20,}/g,
    pushScan: true,
  },
  {
    id: 'keygen-key',
    label: 'Keygen Token',
    regex: /key_[A-Za-z0-9]{20,}/g,
    pushScan: true,
  },
  {
    id: 'keygen-prod',
    label: 'Keygen Production Token',
    regex: /prod_[A-Za-z0-9]{20,}/g,
    pushScan: true,
  },
  {
    id: 'shopify',
    label: 'Shopify Token',
    regex: /shp(at|ca|pa|ss)_[a-fA-F0-9]{32,}/g,
    pushScan: true,
  },
  {
    id: 'npm',
    label: 'npm Token',
    regex: /npm_[A-Za-z0-9]{36,}/g,
    pushScan: true,
  },
  {
    id: 'discord',
    label: 'Discord Token',
    regex: /[MN][A-Za-z0-9]{23,}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,}/g,
    pushScan: true,
  },
  {
    id: 'vercel-key',
    label: 'Vercel Token',
    regex: /vc[ka]_[A-Za-z0-9]{20,}/g,
    pushScan: true,
  },
  {
    id: 'slack',
    label: 'Slack Token',
    regex: /xox[baprs]-[0-9]{10,}-[A-Za-z0-9-]+/g,
    pushScan: true,
  },
  {
    id: 'twilio',
    label: 'Twilio API Key',
    regex: /SK[a-f0-9]{32}/g,
    pushScan: true,
  },
  {
    id: 'sendgrid',
    label: 'SendGrid API Key',
    regex: /SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}/g,
    pushScan: true,
  },
  {
    id: 'mailgun',
    label: 'Mailgun API Key',
    regex: /key-[a-f0-9]{32}/g,
    pushScan: true,
  },

  // Private Keys
  {
    id: 'private-key',
    label: 'Private Key',
    regex: /-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----/g,
    pushScan: true,
  },

  // Connection Strings
  {
    id: 'db-connection',
    label: 'Database Connection String',
    regex: /(mongodb|postgres|mysql|postgresql|redis|amqp):\/\/[^:\s]+:[^@\s]+@/g,
    // pre-commit's variant allowed whitespace inside the credentials — broader,
    // so the commit gate keeps it.
    stagedRegex: /(mongodb|postgres|mysql|postgresql|redis|amqp):\/\/[^:]+:[^@]+@/g,
    pushScan: true,
  },

  // Generic credential patterns
  {
    id: 'password',
    label: 'Hardcoded Password',
    regex: /password\s*[:=]\s*['"][^'"]{8,}['"]/gi,
    exclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var/i,
    // pre-commit did not require a CLOSING quote (catches truncated literals)
    // and did not excuse env-var reads. Keep the commit gate that strict.
    stagedRegex: /password\s*[:=]\s*['"][^'"]{8,}/gi,
    stagedExclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE/i,
  },
  {
    id: 'secret-value',
    label: 'Hardcoded Secret',
    regex: /secret\s*[:=]\s*['"][^'"]{8,}['"]/gi,
    exclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var|client.?secret/i,
    stagedRegex: /secret\s*[:=]\s*['"][^'"]{8,}/gi,
    stagedExclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE/i,
  },
  {
    id: 'api-key-value',
    label: 'Hardcoded API Key',
    regex: /api[_-]?key\s*[:=]\s*['"][A-Za-z0-9_-]{16,}['"]/gi,
    exclude: /example|placeholder|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var/i,
    stagedRegex: /api[_-]?key\s*[:=]\s*['"][A-Za-z0-9_-]{16,}/gi,
    stagedExclude: /example|placeholder|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE/i,
  },

  // PII — Australian
  {
    id: 'au-phone',
    label: 'AU Phone Number',
    regex: /\+614\d{8}/g,
    stagedExemptPaths: AU_PHONE_EXEMPT_PATHS,
    pushScan: true,
  },
  {
    id: 'abn-tfn',
    label: 'ABN/TFN Number',
    regex: /(ABN|TFN|abn|tfn)\s*[:=]?\s*\d{2}\s?\d{3}\s?\d{3}\s?\d{3}/g,
    // Statutory disclosure is REQUIRED on these surfaces — see ABN_EXEMPT_PATHS.
    stagedExemptPaths: ABN_EXEMPT_PATHS,
  },

  // Personal emails in source code
  {
    id: 'personal-email',
    label: 'Personal Email Address',
    regex: /[a-zA-Z0-9._%+-]+@(gmail|yahoo|hotmail|outlook|protonmail|icloud)\.(com|net|org)/g,
    exclude: /example|test|fake|dummy|noreply|placeholder|user@gmail|someone@|nobody@|john@|jane@/i,
    stagedExclude: /example|test|fake|dummy|noreply|placeholder/i,
    // Only flag in source code files
    fileFilter: /\.(ts|tsx|rs|js|jsx)$/,
  },
];

// --- Main Logic ---

function getFiles() {
  try {
    if (STAGED_ONLY) {
      const output = execSync('git diff --cached --name-only --diff-filter=ACM', {
        encoding: 'utf-8',
        cwd: path.resolve(__dirname, '..'),
      });
      return output.trim().split('\n').filter(Boolean);
    } else {
      const output = execSync('git ls-files', {
        encoding: 'utf-8',
        cwd: path.resolve(__dirname, '..'),
      });
      return output.trim().split('\n').filter(Boolean);
    }
  } catch (e) {
    console.error('Failed to get file list from git:', e.message);
    process.exit(2);
  }
}

function shouldSkipFile(filePath) {
  // In --staged mode the pre-commit hook's own skip list is authoritative.
  if (STAGED_ONLY) return matchesAny(STAGED_EXCLUDE, filePath);
  for (const pattern of SKIP_PATHS) {
    if (pattern.test(filePath)) return true;
  }
  return false;
}

/**
 * Read the STAGED blob for a path (`git show :FILE`), not the working tree.
 * This is what makes the commit gate honest: a secret that was `git add`ed and
 * then deleted from the working copy is still in the commit, and must block.
 */
function readStagedContent(filePath, repoRoot) {
  try {
    // Returns a Buffer (no `encoding`) so the NUL-byte binary check is accurate.
    return execSync(`git show ":${filePath.replace(/"/g, '\\"')}"`, {
      cwd: repoRoot,
      maxBuffer: 64 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null; // unreadable (submodule, deleted, binary blob) — nothing to scan
  }
}

function isAllowlisted(filePath) {
  for (const pattern of ALLOWLISTED_FILES) {
    if (pattern.test(filePath)) return true;
  }
  return false;
}

function redact(text) {
  return text.length > 12
    ? text.substring(0, 8) + '...[REDACTED]'
    : text.substring(0, 4) + '...[REDACTED]';
}

/** Scan already-loaded text. `staged` selects the pre-commit pattern variants. */
function scanContent(content, filePath, { staged = false, patterns = SECRET_PATTERNS } = {}) {
  const findings = [];
  const lines = content.split('\n');

  for (const pattern of patterns) {
    // Check file filter (some patterns only apply to source code)
    if (pattern.fileFilter && !pattern.fileFilter.test(filePath)) continue;
    // Per-pattern path exemptions (statutory ABN / public phone number)
    if (staged && pattern.stagedExemptPaths && matchesAny(pattern.stagedExemptPaths, filePath)) continue;

    const regex = (staged && pattern.stagedRegex) || pattern.regex;
    const exclude = staged
      ? (pattern.stagedExclude !== undefined ? pattern.stagedExclude : pattern.exclude)
      : pattern.exclude;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].replace(/\r$/, '');
      // Reset regex lastIndex for global patterns
      regex.lastIndex = 0;
      let match;
      while ((match = regex.exec(line)) !== null) {
        // Check exclude pattern
        if (exclude && exclude.test(line)) continue;

        findings.push({
          file: filePath,
          line: i + 1,
          column: match.index + 1,
          pattern: pattern.id,
          label: pattern.label,
          // Redact the actual match — show first 8 chars + ellipsis
          snippet: redact(match[0]),
        });
      }
    }
  }

  return findings;
}

function scanFile(filePath, repoRoot) {
  let content;

  if (STAGED_ONLY) {
    const buffer = readStagedContent(filePath, repoRoot);
    if (buffer === null) return [];
    // Binary check on the staged blob: a NUL byte in the first 8KB.
    const sample = buffer.slice(0, 8192);
    for (let i = 0; i < sample.length; i++) {
      if (sample[i] === 0) return [];
    }
    content = buffer.toString('utf-8');
  } else {
    const fullPath = path.join(repoRoot, filePath);
    // Skip if file doesn't exist (deleted but tracked)
    if (!fs.existsSync(fullPath)) return [];
    try {
      // Check if binary
      const buffer = fs.readFileSync(fullPath);
      // Simple binary check: look for null bytes in first 8KB
      const sample = buffer.slice(0, 8192);
      for (let i = 0; i < sample.length; i++) {
        if (sample[i] === 0) return []; // binary file, skip
      }
      content = buffer.toString('utf-8');
    } catch (e) {
      if (VERBOSE) console.warn(`  Warning: could not read ${filePath}: ${e.message}`);
      return [];
    }
  }

  return scanContent(content, filePath, { staged: STAGED_ONLY });
}

/**
 * --diff-added: scan ADDED lines of a `git log -p` stream arriving on stdin.
 * Reproduces the pre-push hook's previous behaviour exactly: the high-confidence
 * pattern subset, applied to `^+` lines, with NO exclude filters.
 */
function runDiffAddedMode() {
  const pushPatterns = SECRET_PATTERNS.filter((p) => p.pushScan);
  let buf = '';
  process.stdin.setEncoding('utf-8');
  process.stdin.on('data', (d) => { buf += d; });
  process.stdin.on('end', () => {
    const added = buf
      .split('\n')
      .filter((l) => l.startsWith('+'))
      .map((l) => l.slice(1)) // drop the diff marker so it can't glue onto a token
      .join('\n');

    const hits = [];
    for (const pattern of pushPatterns) {
      pattern.regex.lastIndex = 0;
      let m;
      while ((m = pattern.regex.exec(added)) !== null) {
        hits.push(`  [${pattern.label}] ${redact(m[0])}`);
        if (m[0].length === 0) pattern.regex.lastIndex++; // guard against zero-width loops
      }
    }

    if (hits.length) {
      console.error([...new Set(hits)].sort().join('\n'));
      process.exit(1);
    }
    process.exit(0);
  });
}

function main() {
  const repoRoot = path.resolve(__dirname, '..');
  const files = getFiles();
  const mode = STAGED_ONLY ? 'staged' : 'tracked';

  if (!CI_MODE) {
    console.log(`\n4DA Secret Scanner`);
    console.log(`==================`);
    console.log(`Scanning ${files.length} ${mode} files...\n`);
  }

  let totalFindings = [];
  let scanned = 0;
  let skipped = 0;
  let allowlisted = 0;

  for (const file of files) {
    if (shouldSkipFile(file)) {
      skipped++;
      continue;
    }
    // ALLOWLISTED_FILES is the AUDIT's allowlist. In --staged mode the
    // pre-commit skip list (applied in shouldSkipFile) is authoritative, and
    // layering the audit allowlist on top would silently widen what the commit
    // gate lets through (e.g. it would stop scanning CLAUDE.md or llm.rs).
    if (!STAGED_ONLY && isAllowlisted(file)) {
      allowlisted++;
      if (VERBOSE) console.log(`  [SKIP] ${file} (allowlisted)`);
      continue;
    }
    scanned++;
    const findings = scanFile(file, repoRoot);
    totalFindings.push(...findings);
  }

  // --- Output ---

  if (CI_MODE) {
    // JSON output for CI integration
    const result = {
      status: totalFindings.length === 0 ? 'clean' : 'secrets_found',
      findings: totalFindings,
      stats: { scanned, skipped, allowlisted, total: files.length },
    };
    console.log(JSON.stringify(result, null, 2));
  } else {
    if (totalFindings.length > 0) {
      console.log('============================================================');
      console.log('  SECRETS DETECTED — Review and remove before committing!');
      console.log('============================================================\n');

      // Group by file
      const byFile = {};
      for (const f of totalFindings) {
        if (!byFile[f.file]) byFile[f.file] = [];
        byFile[f.file].push(f);
      }

      for (const [file, findings] of Object.entries(byFile)) {
        console.log(`  ${file}`);
        for (const f of findings) {
          console.log(`    L${f.line}: [${f.label}] ${f.snippet}`);
        }
        console.log('');
      }

      console.log(`Found ${totalFindings.length} potential secret(s) in ${Object.keys(byFile).length} file(s).`);
      console.log('');
      console.log('Actions:');
      console.log('  1. Remove the secret from the file');
      console.log('  2. Use environment variables or data/settings.json (gitignored)');
      console.log('  3. If false positive, add the file to ALLOWLISTED_FILES in this script');
      console.log('');
    } else {
      console.log('No secrets detected.\n');
    }

    console.log(`Stats: ${scanned} scanned, ${skipped} skipped (binary/deps), ${allowlisted} allowlisted`);
  }

  process.exit(totalFindings.length > 0 ? 1 : 0);
}

// Exported so the pattern set and the path-exemption lists can be unit-tested
// without shelling out (mirrors check-no-window-spawns.cjs / check-remove-by.cjs).
module.exports = {
  SECRET_PATTERNS,
  STAGED_EXCLUDE,
  ABN_EXEMPT_PATHS,
  AU_PHONE_EXEMPT_PATHS,
  ALLOWLISTED_FILES,
  matchesAny,
  scanContent,
};

if (require.main === module) {
  if (DIFF_ADDED) {
    runDiffAddedMode();
  } else {
    main();
  }
}
