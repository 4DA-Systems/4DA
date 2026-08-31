// Tests for the Pages-secrets gate.
//
// The script shipped without any, which is a poor look for a gate whose entire
// purpose is refusing to conflate "could not check" with "checked and fine".
//
// The property under test is that three-valued discipline, now extended to the
// sending domain. The dangerous state this exists to catch is NOT a missing
// variable — that was always caught. It is RESEND_API_KEY and RESEND_FROM_EMAIL
// both present while the domain is unverified: it looks configured, reports OK
// under a presence-only check, and 403s on every single licence email.
//
// A resolver that cannot be reached must never be reported as "not verified" —
// otherwise a flaky network fails a release for the wrong reason, which is the
// same conflation wearing different clothes.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const {
  EXPECTED,
  main,
  checkSendingDomain,
  DKIM_HOST,
  SENDING_DOMAIN,
} = require('./check-pages-secrets.cjs');

/** A resolver that rejects with a DNS-shaped error carrying `code`. */
function failing(code) {
  return async () => {
    const err = new Error(`queryTxt ${code} ${DKIM_HOST}`);
    err.code = code;
    throw err;
  };
}

test('DKIM host is derived from the sending domain', () => {
  assert.equal(SENDING_DOMAIN, '4da.ai');
  assert.equal(DKIM_HOST, 'resend._domainkey.4da.ai');
});

test('published TXT records mean verified', async () => {
  const r = await checkSendingDomain(async () => [['p=MIGfMA0GCSq']]);
  assert.equal(r.state, 'verified');
  assert.match(r.detail, /is published/);
});

test('NXDOMAIN / ENOTFOUND / ENODATA all mean genuinely absent', async () => {
  for (const code of ['ENOTFOUND', 'NXDOMAIN', 'ENODATA']) {
    const r = await checkSendingDomain(failing(code));
    assert.equal(r.state, 'absent', `${code} must read as absent`);
    assert.match(r.detail, new RegExp(code));
  }
});

test('resolving to an empty answer is absent, not verified', async () => {
  // A record that exists but carries nothing cannot authenticate anything.
  const r = await checkSendingDomain(async () => []);
  assert.equal(r.state, 'absent');
});

test('an unreachable resolver is UNKNOWN, never absent', async () => {
  // The load-bearing assertion. Reporting these as "absent" would fail a release
  // over a network blip and teach everyone to ignore the gate.
  for (const code of ['ESERVFAIL', 'ETIMEOUT', 'ECONNREFUSED', undefined]) {
    const r = await checkSendingDomain(failing(code));
    assert.equal(r.state, 'unknown', `${code} must read as unknown`);
    assert.notEqual(r.state, 'absent');
  }
});

test('both mailer variables are REQUIRED, and each says the other is needed', () => {
  for (const name of ['RESEND_API_KEY', 'RESEND_FROM_EMAIL']) {
    const entry = EXPECTED.find((e) => e.name === name);
    assert.ok(entry, `${name} must be in EXPECTED`);
    assert.equal(entry.required, true, `${name} must be required`);
  }
  // The launch coupon is deliberately absent, not merely optional — a future
  // edit that flips this to required would break every pre-launch check.
  const coupon = EXPECTED.find((e) => e.name === 'SIGNAL_LAUNCH_COUPON');
  assert.equal(coupon.required, false);
});

test('no token means exit 2 -- inconclusive is not a pass', async () => {
  const saved = process.env.CLOUDFLARE_API_TOKEN;
  delete process.env.CLOUDFLARE_API_TOKEN;
  const origErr = process.stderr.write;
  const lines = [];
  process.stderr.write = (s) => {
    lines.push(String(s));
    return true;
  };
  try {
    const code = await main([]);
    assert.equal(code, 2, 'a missing token must never be 0');
    assert.match(lines.join(''), /INCONCLUSIVE/);
  } finally {
    process.stderr.write = origErr;
    if (saved !== undefined) process.env.CLOUDFLARE_API_TOKEN = saved;
  }
});

test('main is async so the DNS check cannot be silently skipped', () => {
  // Guards a specific regression: if main is ever reverted to sync, the require
  // .main handler would set process.exitCode to a Promise, which coerces to 0 --
  // a permanently green gate.
  const saved = process.env.CLOUDFLARE_API_TOKEN;
  delete process.env.CLOUDFLARE_API_TOKEN;
  const origErr = process.stderr.write;
  process.stderr.write = () => true;
  try {
    const returned = main([]);
    assert.ok(returned instanceof Promise, 'main must return a Promise');
  } finally {
    process.stderr.write = origErr;
    if (saved !== undefined) process.env.CLOUDFLARE_API_TOKEN = saved;
  }
});
