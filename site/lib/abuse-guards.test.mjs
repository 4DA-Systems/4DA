// Tests for the KV-backed abuse guards on the licensing endpoints.
//
// The two defects these exist to pin closed:
//   1. A duplicate Stripe delivery of checkout.session.completed / invoice.paid
//      MINTED A SECOND VALID KEY — the dedup gate must skip an event id that
//      already completed, and must NOT skip one whose handler failed (Stripe's
//      retry is the recovery mechanism for failures).
//   2. The unmetered recovery path let ~100 requests for ONE known customer
//      address exhaust the Resend 100/day quota, silencing every licence email
//      of the day — including the delivery mail of real purchases.
//
// And one property that outranks both: every guard FAILS OPEN. A broken KV
// store must degrade to yesterday's behaviour, never block the payment path or
// a legitimate recovery.
//
// Dependency-free `node --test`, like entitlement.test.mjs: site/ has no
// installed dependency tree in CI.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  checkAndCount,
  EVENT_DEDUP_TTL_SECONDS,
  eventKey,
  ipWindowKey,
  isDuplicateEvent,
  mailWindowKey,
  markEventProcessed,
  RECOVERY_MAILS_PER_ADDRESS_PER_DAY,
} from './abuse-guards.js';

/** Minimal in-memory stand-in for a KV namespace binding. */
function fakeKv() {
  const store = new Map();
  const puts = [];
  return {
    store,
    puts,
    async get(key) {
      return store.has(key) ? store.get(key) : null;
    },
    async put(key, value, opts) {
      store.set(key, value);
      puts.push({ key, value, opts });
    },
  };
}

/** A KV whose every operation throws — the fail-open cases. */
const brokenKv = {
  async get() {
    throw new Error('kv unavailable');
  },
  async put() {
    throw new Error('kv unavailable');
  },
};

function silenced(fn) {
  // The fail-open paths log at error level; keep test output clean while
  // still letting the code log.
  return async (...args) => {
    const orig = console.error;
    console.error = () => {};
    try {
      return await fn(...args);
    } finally {
      console.error = orig;
    }
  };
}

// ---------------------------------------------------------------------------
// Event dedup
// ---------------------------------------------------------------------------

test('an unseen event id is not a duplicate; a recorded one is', async () => {
  const kv = fakeKv();
  assert.equal(await isDuplicateEvent(kv, 'evt_123'), false, 'first delivery processes');
  await markEventProcessed(kv, 'evt_123');
  assert.equal(await isDuplicateEvent(kv, 'evt_123'), true, 're-delivery is skipped');
  assert.equal(await isDuplicateEvent(kv, 'evt_456'), false, 'other events unaffected');
});

test('the dedup record carries a TTL longer than Stripe retry window', async () => {
  const kv = fakeKv();
  await markEventProcessed(kv, 'evt_123');
  assert.equal(kv.puts.length, 1);
  assert.equal(kv.puts[0].key, eventKey('evt_123'));
  assert.equal(kv.puts[0].opts.expirationTtl, EVENT_DEDUP_TTL_SECONDS);
  assert.ok(
    EVENT_DEDUP_TTL_SECONDS >= 4 * 24 * 60 * 60,
    'TTL must outlive Stripe live-mode retries (~3 days)',
  );
});

test('dedup FAILS OPEN: a broken KV reads as "not a duplicate"', async () => {
  // Failing closed here would drop paid events on a KV outage.
  const dup = await silenced(isDuplicateEvent)(brokenKv, 'evt_123');
  assert.equal(dup, false);
});

test('a failed dedup write is swallowed, never thrown', async () => {
  // A throw would 500 the webhook AFTER the key was minted, making Stripe
  // retry and mint another — the exact defect this module removes.
  await assert.doesNotReject(silenced(markEventProcessed)(brokenKv, 'evt_123'));
});

// ---------------------------------------------------------------------------
// Rate-limit windows
// ---------------------------------------------------------------------------

test('window keys are stable within a window and normalise the address', () => {
  const at = new Date('2026-08-19T10:15:30.000Z');
  assert.equal(mailWindowKey('  Buyer@Example.COM ', at), 'rl:mail:buyer@example.com:2026-08-19');
  assert.equal(ipWindowKey('203.0.113.7', at), 'rl:ip:203.0.113.7:2026-08-19T10');
  // Case tricks cannot dodge the per-address cap.
  assert.equal(mailWindowKey('A@B.CO', at), mailWindowKey('a@b.co', at));
});

test('window keys roll over with their window', () => {
  const morning = new Date('2026-08-19T10:59:59.000Z');
  const evening = new Date('2026-08-19T11:00:00.000Z');
  const tomorrow = new Date('2026-08-20T10:15:00.000Z');
  assert.notEqual(ipWindowKey('1.2.3.4', morning), ipWindowKey('1.2.3.4', evening));
  assert.notEqual(mailWindowKey('a@b.co', morning), mailWindowKey('a@b.co', tomorrow));
  assert.equal(mailWindowKey('a@b.co', morning), mailWindowKey('a@b.co', evening));
});

test('checkAndCount allows up to the limit, then denies', async () => {
  const kv = fakeKv();
  for (let i = 0; i < RECOVERY_MAILS_PER_ADDRESS_PER_DAY; i++) {
    assert.equal(await checkAndCount(kv, 'rl:test', RECOVERY_MAILS_PER_ADDRESS_PER_DAY, 60), true);
  }
  assert.equal(
    await checkAndCount(kv, 'rl:test', RECOVERY_MAILS_PER_ADDRESS_PER_DAY, 60),
    false,
    'the call past the cap is denied',
  );
  assert.equal(
    await checkAndCount(kv, 'rl:other', RECOVERY_MAILS_PER_ADDRESS_PER_DAY, 60),
    true,
    'other windows are independent',
  );
});

test('a denied call does not keep extending the window record', async () => {
  const kv = fakeKv();
  await checkAndCount(kv, 'rl:test', 1, 60);
  const writesAfterAllow = kv.puts.length;
  await checkAndCount(kv, 'rl:test', 1, 60);
  assert.equal(kv.puts.length, writesAfterAllow, 'deny is read-only');
});

test('a garbage counter value resets rather than denies', async () => {
  const kv = fakeKv();
  kv.store.set('rl:test', 'not-a-number');
  assert.equal(await checkAndCount(kv, 'rl:test', 5, 60), true);
  assert.equal(kv.store.get('rl:test'), '1', 'counter restarts from a sane value');
});

test('rate limiting FAILS OPEN: a broken KV allows the call', async () => {
  // Failing closed would block a real customer recovering a real key because a
  // counter store hiccuped.
  assert.equal(await silenced(checkAndCount)(brokenKv, 'rl:test', 1, 60), true);
});
