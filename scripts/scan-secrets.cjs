#!/usr/bin/env node
/**
 * scan-secrets.cjs — Comprehensive secret scanner for 4DA repo
 *
 * Scans ALL tracked files for secrets, API keys, tokens, private keys,
 * connection strings, and PII. Matches the patterns in .husky/pre-commit
 * and .husky/pre-push for defense-in-depth.
 *
 * Usage:
 *   node scripts/scan-secrets.cjs           # scan all tracked files
 *   node scripts/scan-secrets.cjs --staged  # scan only staged files
 *   node scripts/scan-secrets.cjs --ci      # CI mode (JSON output)
 *
 * Exit codes:
 *   0 — clean, no secrets found
 *   1 — secrets detected
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// --- Configuration ---

const ARGS = process.argv.slice(2);
const STAGED_ONLY = ARGS.includes('--staged');
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

// --- Secret Patterns ---
// Each pattern has: id, label, regex, and optional exclude regex for false positives

const SECRET_PATTERNS = [
  // API Keys & Tokens
  {
    id: 'openai-proj',
    label: 'OpenAI Project Key',
    regex: /sk-proj-[A-Za-z0-9_-]{20,}/g,
  },
  {
    id: 'anthropic',
    label: 'Anthropic API Key',
    regex: /sk-ant-[A-Za-z0-9_-]{20,}/g,
  },
  {
    id: 'openai-generic',
    label: 'OpenAI Key (generic sk-)',
    regex: /sk-[a-zA-Z0-9]{20,}/g,
    exclude: /sk-ant-|sk-proj-|sk_live|sk_test|sk-[a-z]+-/,
  },
  {
    id: 'github',
    label: 'GitHub Token',
    regex: /gh[pousr]_[A-Za-z0-9]{36,}/g,
  },
  {
    id: 'aws-key',
    label: 'AWS Access Key',
    regex: /(AKIA|ASIA)[0-9A-Z]{16}/g,
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
  },
  {
    id: 'stripe-live',
    label: 'Stripe Live Key',
    regex: /[spr]k_live_[A-Za-z0-9]{20,}/g,
  },
  {
    id: 'keygen-key',
    label: 'Keygen Token',
    regex: /key_[A-Za-z0-9]{20,}/g,
  },
  {
    id: 'keygen-prod',
    label: 'Keygen Production Token',
    regex: /prod_[A-Za-z0-9]{20,}/g,
  },
  {
    id: 'shopify',
    label: 'Shopify Token',
    regex: /shp(at|ca|pa|ss)_[a-fA-F0-9]{32,}/g,
  },
  {
    id: 'npm',
    label: 'npm Token',
    regex: /npm_[A-Za-z0-9]{36,}/g,
  },
  {
    id: 'discord',
    label: 'Discord Token',
    regex: /[MN][A-Za-z0-9]{23,}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,}/g,
  },
  {
    id: 'vercel-key',
    label: 'Vercel Token',
    regex: /vc[ka]_[A-Za-z0-9]{20,}/g,
  },
  {
    id: 'slack',
    label: 'Slack Token',
    regex: /xox[baprs]-[0-9]{10,}-[A-Za-z0-9-]+/g,
  },
  {
    id: 'twilio',
    label: 'Twilio API Key',
    regex: /SK[a-f0-9]{32}/g,
  },
  {
    id: 'sendgrid',
    label: 'SendGrid API Key',
    regex: /SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}/g,
  },
  {
    id: 'mailgun',
    label: 'Mailgun API Key',
    regex: /key-[a-f0-9]{32}/g,
  },

  // Private Keys
  {
    id: 'private-key',
    label: 'Private Key',
    regex: /-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----/g,
  },

  // Connection Strings
  {
    id: 'db-connection',
    label: 'Database Connection String',
    regex: /(mongodb|postgres|mysql|postgresql|redis|amqp):\/\/[^:\s]+:[^@\s]+@/g,
  },

  // Generic credential patterns
  {
    id: 'password',
    label: 'Hardcoded Password',
    regex: /password\s*[:=]\s*['"][^'"]{8,}['"]/gi,
    exclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var/i,
  },
  {
    id: 'secret-value',
    label: 'Hardcoded Secret',
    regex: /secret\s*[:=]\s*['"][^'"]{8,}['"]/gi,
    exclude: /example|placeholder|changeme|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var|client.?secret/i,
  },
  {
    id: 'api-key-value',
    label: 'Hardcoded API Key',
    regex: /api[_-]?key\s*[:=]\s*['"][A-Za-z0-9_-]{16,}['"]/gi,
    exclude: /example|placeholder|your_|test|fake|dummy|TODO|FIXME|xxx|REPLACE|process\.env|std::env|env::|getenv|env_var/i,
  },

  // PII — Australian
  {
    id: 'au-phone',
    label: 'AU Phone Number',
    regex: /\+614\d{8}/g,
  },
  {
    id: 'abn-tfn',
    label: 'ABN/TFN Number',
    regex: /(ABN|TFN|abn|tfn)\s*[:=]?\s*\d{2}\s?\d{3}\s?\d{3}\s?\d{3}/g,
  },

  // Personal emails in source code
  {
    id: 'personal-email',
    label: 'Personal Email Address',
    regex: /[a-zA-Z0-9._%+-]+@(gmail|yahoo|hotmail|outlook|protonmail|icloud)\.(com|net|org)/g,
    exclude: /example|test|fake|dummy|noreply|placeholder|user@gmail|someone@|nobody@|john@|jane@/i,
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
  for (const pattern of SKIP_PATHS) {
    if (pattern.test(filePath)) return true;
  }
  return false;
}

function isAllowlisted(filePath) {
  for (const pattern of ALLOWLISTED_FILES) {
    if (pattern.test(filePath)) return true;
  }
  return false;
}

function scanFile(filePath, repoRoot) {
  const findings = [];
  const fullPath = path.join(repoRoot, filePath);

  // Skip if file doesn't exist (deleted but tracked)
  if (!fs.existsSync(fullPath)) return findings;

  let content;
  try {
    // Check if binary
    const buffer = fs.readFileSync(fullPath);
    // Simple binary check: look for null bytes in first 8KB
    const sample = buffer.slice(0, 8192);
    for (let i = 0; i < sample.length; i++) {
      if (sample[i] === 0) return findings; // binary file, skip
    }
    content = buffer.toString('utf-8');
  } catch (e) {
    if (VERBOSE) console.warn(`  Warning: could not read ${filePath}: ${e.message}`);
    return findings;
  }

  const lines = content.split('\n');

  for (const pattern of SECRET_PATTERNS) {
    // Check file filter (some patterns only apply to source code)
    if (pattern.fileFilter && !pattern.fileFilter.test(filePath)) continue;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      // Reset regex lastIndex for global patterns
      pattern.regex.lastIndex = 0;
      let match;
      while ((match = pattern.regex.exec(line)) !== null) {
        // Check exclude pattern
        if (pattern.exclude && pattern.exclude.test(line)) continue;

        findings.push({
          file: filePath,
          line: i + 1,
          column: match.index + 1,
          pattern: pattern.id,
          label: pattern.label,
          // Redact the actual match — show first 8 chars + ellipsis
          snippet: match[0].length > 12
            ? match[0].substring(0, 8) + '...[REDACTED]'
            : match[0].substring(0, 4) + '...[REDACTED]',
        });
      }
    }
  }

  return findings;
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
    if (isAllowlisted(file)) {
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

main();
