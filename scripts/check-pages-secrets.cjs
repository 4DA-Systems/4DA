#!/usr/bin/env node
//
// Verify the live Cloudflare Pages project has every environment variable the
// payment path needs.
//
// WHY THIS EXISTS
// ---------------
// Licence-key email recovery was dead in production for days and nothing said so.
// The code was correct: with RESEND_API_KEY/RESEND_FROM_EMAIL unset it refused to
// fall back to returning the key (which was the vulnerability it replaced) and
// answered 503. Correct, deliberate, and completely invisible — no test could see
// it, because it is not a code property, and no CI job could see it, because CI
// cannot read a Pages secret.
//
// The only thing that can catch a missing runtime variable is asking the live
// project what it actually has. That is this script.
//
// It reads secret NAMES only. Cloudflare never returns values, which is exactly
// why this is safe to run and safe to put in a release checklist.
//
// USAGE
//   CLOUDFLARE_API_TOKEN=... node scripts/check-pages-secrets.cjs
//   CLOUDFLARE_API_TOKEN=... node scripts/check-pages-secrets.cjs --json
//
// EXIT CODES
//   0  every REQUIRED variable is present
//   1  one or more REQUIRED variables are missing
//   2  could not ask (no token, wrangler missing, API error) — NOT a pass.
//      "Could not check" must never read as "checked and fine"; that conflation
//      is the same defect this script exists to catch.

'use strict';

const { execFileSync } = require('node:child_process');
const path = require('node:path');

const PROJECT = '4da-site';
const SITE_DIR = path.join(__dirname, '..', 'site');

/**
 * Every variable the live payment path reads, and what breaks without it.
 * `required: false` means deliberately absent until a business decision, not
 * "optional" in the sense of "nice to have".
 */
const EXPECTED = [
  { name: 'STRIPE_SECRET_KEY', required: true, gates: 'checkout session creation and every webhook handler' },
  { name: 'STRIPE_WEBHOOK_SECRET', required: true, gates: 'webhook signature verification — without it no licence is ever minted' },
  { name: 'LICENSE_PRIVATE_KEY_HEX', required: true, gates: 'Ed25519 licence signing' },
  { name: 'SIGNAL_PRICE_MONTHLY', required: true, gates: 'monthly checkout' },
  { name: 'SIGNAL_PRICE_ANNUAL', required: true, gates: 'annual checkout' },
  { name: 'SIGNAL_PRICE_LIFETIME', required: true, gates: 'lifetime checkout' },
  { name: 'SITE_URL', required: true, gates: 'checkout success/cancel redirects' },
  {
    name: 'RESEND_API_KEY',
    required: true,
    gates:
      'licence DELIVERY at purchase/renewal and licence RECOVERY. Without it the ' +
      'success page is the only way a buyer ever receives their key, and recovery ' +
      'answers 503.',
  },
  {
    name: 'RESEND_FROM_EMAIL',
    required: true,
    gates: 'same as RESEND_API_KEY — both are needed, either alone does nothing',
  },
  {
    name: 'SIGNAL_LAUNCH_COUPON',
    required: false,
    gates: 'Founding 100 launch special. Dormant BY DESIGN until launch day.',
  },
  { name: 'ENVIRONMENT', required: false, gates: 'environment label only' },
];

/** Parse `  - NAME: Value Encrypted` lines into names. */
function parseSecretNames(out) {
  return out
    .split(/\r?\n/)
    .map((l) => l.match(/^\s*-\s*([A-Z0-9_]+)\s*:/))
    .filter(Boolean)
    .map((m) => m[1]);
}

/**
 * Ask the live project for its variable names.
 *
 * Tries a locally-installed wrangler first, then npx. `shell: true` is required
 * on Windows because the local binary is a `.cmd` shim, which spawnSync cannot
 * execute directly (EINVAL). Every argument here is a module constant — none of
 * it is caller-supplied — so the shell adds no injection surface.
 */
function listSecretNames() {
  const args = ['pages', 'secret', 'list', '--project-name', PROJECT];
  const localBin = path.join(
    SITE_DIR,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'wrangler.cmd' : 'wrangler',
  );
  const attempts = [
    { cmd: `"${localBin}"`, label: 'local wrangler' },
    { cmd: 'npx --yes wrangler', label: 'npx wrangler' },
  ];

  const failures = [];
  for (const attempt of attempts) {
    try {
      const out = execFileSync(`${attempt.cmd} ${args.join(' ')}`, {
        cwd: SITE_DIR,
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        shell: true,
      });
      const names = parseSecretNames(out);
      if (names.length > 0) return names;
      failures.push(`${attempt.label}: ran but parsed 0 names`);
    } catch (err) {
      failures.push(`${attempt.label}: ${String(err.message).split('\n')[0]}`);
    }
  }
  throw new Error(failures.join(' | '));
}

function main(argv) {
  const asJson = argv.includes('--json');

  if (!process.env.CLOUDFLARE_API_TOKEN) {
    process.stderr.write(
      '[pages-secrets] INCONCLUSIVE — CLOUDFLARE_API_TOKEN is not set, so the live\n' +
        '                project was never asked. This is exit 2, not a pass.\n',
    );
    return 2;
  }

  let present;
  try {
    present = listSecretNames();
  } catch (err) {
    process.stderr.write(`[pages-secrets] INCONCLUSIVE — could not query the project: ${err.message}\n`);
    return 2;
  }

  if (present.length === 0) {
    process.stderr.write('[pages-secrets] INCONCLUSIVE — the project reported zero variables; parse or auth problem.\n');
    return 2;
  }

  const missingRequired = EXPECTED.filter((e) => e.required && !present.includes(e.name));
  const missingOptional = EXPECTED.filter((e) => !e.required && !present.includes(e.name));
  const unexpected = present.filter((n) => !EXPECTED.some((e) => e.name === n));

  if (asJson) {
    process.stdout.write(
      `${JSON.stringify({ project: PROJECT, present, missingRequired: missingRequired.map((m) => m.name), missingOptional: missingOptional.map((m) => m.name), unexpected }, null, 2)}\n`,
    );
    return missingRequired.length > 0 ? 1 : 0;
  }

  process.stdout.write(`[pages-secrets] ${PROJECT}: ${present.length} variable(s) configured\n`);

  if (missingRequired.length > 0) {
    process.stderr.write(`\n[pages-secrets] MISSING ${missingRequired.length} REQUIRED variable(s):\n\n`);
    for (const m of missingRequired) {
      process.stderr.write(`  ${m.name}\n      gates: ${m.gates}\n`);
      process.stderr.write(`      fix:   cd site && wrangler pages secret put ${m.name} --project-name ${PROJECT}\n\n`);
    }
    return 1;
  }

  for (const m of missingOptional) {
    process.stdout.write(`  (absent, expected) ${m.name} — ${m.gates}\n`);
  }
  if (unexpected.length > 0) {
    process.stdout.write(`  (present, undocumented) ${unexpected.join(', ')} — add to EXPECTED or remove\n`);
  }
  process.stdout.write('[pages-secrets] OK — every required variable is present.\n');
  return 0;
}

if (require.main === module) {
  process.exitCode = main(process.argv.slice(2));
}

module.exports = { EXPECTED, main };
