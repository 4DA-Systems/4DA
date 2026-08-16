// Regression tests for the money path: refunds, chargebacks and the customer
// resolution that feeds licence issuance.
//
// WHY THIS FILE EXISTS AS A DEPENDENCY-FREE `node --test` SUITE:
// site/ has its own pnpm-lock.yaml and is never `pnpm install`ed by any CI job,
// so any test that imported `stripe` or `@noble/ed25519` could not run in CI at
// all. lib/entitlement.js therefore imports nothing, and this suite runs from
// `pnpm run test:scripts` — which lives in validate.yml's `repo-guards` job,
// the one job with no path filter and no `needs:`. That is the first PR-time
// test gate the Cloudflare payment path has ever had.
//
// What each test would catch if the fix were reverted:
//   - "refunded is a terminal status"    -> the reader/writer mismatch: nothing
//                                           in the repo ever wrote 'refunded',
//                                           so refresh.js's check for it was
//                                           dead code that looked alive.
//   - "duplicate delivery is a no-op"    -> the missing event-id dedup store.
//   - "severity never downgrades"        -> out-of-order Stripe deliveries.
//   - "resolveCustomerId guards first"   -> the TypeError-instead-of-error bug.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  TERMINAL_STATUSES,
  isLifetimeEntitled,
  resolveCustomerId,
  severityOf,
  stripeIdOf,
  terminalStatusPatch,
} from './entitlement.js';

// ---------------------------------------------------------------------------
// The reader/writer contract
// ---------------------------------------------------------------------------

test('refunded and chargeback are terminal, so a lifetime holder loses entitlement', () => {
  assert.ok(TERMINAL_STATUSES.includes('refunded'), "'refunded' must be a terminal status");
  assert.ok(TERMINAL_STATUSES.includes('chargeback'), "'chargeback' must be a terminal status");

  const base = { streets_billing_period: 'lifetime' };
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'active' }), true);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'refunded' }), false);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'chargeback' }), false);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'cancelled' }), false);
});

test('every terminal status is actually producible by terminalStatusPatch', () => {
  // The defect this guards: refresh.js gated on a status string that no code
  // path could ever write. Any status the reader rejects must be writable.
  for (const status of TERMINAL_STATUSES) {
    const patch = terminalStatusPatch({ streets_status: 'active' }, status, '2026-01-01T00:00:00.000Z');
    assert.equal(patch.streets_status, status, `${status} must be writable`);
  }
});

test('a non-lifetime customer is not lifetime-entitled whatever the status', () => {
  assert.equal(isLifetimeEntitled({ streets_billing_period: 'monthly', streets_status: 'active' }), false);
  assert.equal(isLifetimeEntitled({}), false);
  assert.equal(isLifetimeEntitled(null), false);
  assert.equal(isLifetimeEntitled(undefined), false);
});

// ---------------------------------------------------------------------------
// Idempotency by construction — the stand-in for the absent KV dedup store
// ---------------------------------------------------------------------------

test('duplicate delivery of the same refund writes identical bytes', () => {
  const first = terminalStatusPatch(
    { streets_billing_period: 'lifetime', streets_status: 'active' },
    'refunded',
    '2026-03-01T10:00:00.000Z',
  );
  assert.deepEqual(first, { streets_refunded_at: '2026-03-01T10:00:00.000Z', streets_status: 'refunded' });

  // Stripe re-delivers. The customer now already carries the first patch.
  const after = { streets_billing_period: 'lifetime', ...first };
  const second = terminalStatusPatch(after, 'refunded', '2026-03-01T10:07:31.000Z');

  assert.equal(second.streets_refunded_at, '2026-03-01T10:00:00.000Z', 'first-seen timestamp is preserved');
  assert.equal(second.streets_status, undefined, 'no status rewrite on a repeat');
  assert.equal(isLifetimeEntitled({ ...after, ...second }), false);
});

test('a trailing cancellation does not downgrade a chargeback', () => {
  // Stripe emits customer.subscription.deleted AFTER charge.dispute.created for
  // the same customer. Last-write-wins would report a plain cancellation.
  const disputed = terminalStatusPatch({ streets_status: 'active' }, 'chargeback', '2026-03-01T10:00:00.000Z');
  const state = { ...disputed };
  const trailing = terminalStatusPatch(state, 'cancelled', '2026-03-01T10:05:00.000Z');

  assert.equal(trailing.streets_status, undefined, 'chargeback must not be downgraded to cancelled');
  assert.equal(trailing.streets_cancelled_at, '2026-03-01T10:05:00.000Z', 'the cancellation is still recorded');
  assert.equal({ ...state, ...trailing }.streets_status, 'chargeback');
});

test('a refund arriving after a cancellation escalates', () => {
  const cancelled = terminalStatusPatch({ streets_status: 'active' }, 'cancelled', '2026-03-01T10:00:00.000Z');
  const refunded = terminalStatusPatch({ ...cancelled }, 'refunded', '2026-03-02T10:00:00.000Z');
  assert.equal(refunded.streets_status, 'refunded');
  assert.equal(severityOf('refunded') > severityOf('cancelled'), true);
});

test('a refund in a NEW episode gets a fresh timestamp', () => {
  // Refunded, then re-purchased (generateAndStoreLicense writes 'active'),
  // then refunded again. The second refund must not report the first date.
  const state = {
    streets_status: 'active',
    streets_refunded_at: '2026-01-01T00:00:00.000Z',
    streets_billing_period: 'lifetime',
  };
  const patch = terminalStatusPatch(state, 'refunded', '2026-06-01T00:00:00.000Z');
  assert.equal(patch.streets_refunded_at, '2026-06-01T00:00:00.000Z');
  assert.equal(patch.streets_status, 'refunded');
});

test('terminalStatusPatch refuses a status that is not terminal', () => {
  assert.throws(() => terminalStatusPatch({}, 'active', '2026-01-01T00:00:00.000Z'), /not a terminal status/);
  assert.throws(() => terminalStatusPatch({}, 'refunded_maybe', '2026-01-01T00:00:00.000Z'), /not a terminal status/);
});

// ---------------------------------------------------------------------------
// Stripe reference unwrapping
// ---------------------------------------------------------------------------

test('stripeIdOf handles id strings, expanded objects and absent refs', () => {
  assert.equal(stripeIdOf('cus_123'), 'cus_123');
  assert.equal(stripeIdOf({ id: 'ch_456', object: 'charge' }), 'ch_456');
  assert.equal(stripeIdOf(null), null);
  assert.equal(stripeIdOf(undefined), null);
  assert.equal(stripeIdOf({}), null);
});

// ---------------------------------------------------------------------------
// resolveCustomerId ordering
// ---------------------------------------------------------------------------

function fakeStripe({ existing = [], createdId = 'cus_created' } = {}) {
  const calls = { list: [], create: [] };
  return {
    calls,
    customers: {
      async list(args) {
        calls.list.push(args);
        return { data: existing };
      },
      async create(args) {
        calls.create.push(args);
        return { id: createdId };
      },
    },
  };
}

test('resolveCustomerId throws a NAMED error, not a TypeError, with no id and no email', async () => {
  // The bug: `email.toLowerCase()` ran before the caller's `if (!email)` guard,
  // so this path produced "Cannot read properties of null (reading
  // 'toLowerCase')" and the webhook answered a bare 500 Stripe kept retrying.
  const stripe = fakeStripe();
  await assert.rejects(() => resolveCustomerId(stripe, null, null), (err) => {
    assert.ok(!(err instanceof TypeError), `must not be a TypeError, got: ${err}`);
    assert.match(err.message, /no customer id and no email/);
    return true;
  });
  assert.equal(stripe.calls.list.length, 0, 'must not reach Stripe with a null email');
  assert.equal(stripe.calls.create.length, 0);
});

test('resolveCustomerId short-circuits on an existing customer id', async () => {
  const stripe = fakeStripe();
  assert.equal(await resolveCustomerId(stripe, 'cus_existing', null), 'cus_existing');
  assert.equal(stripe.calls.list.length, 0);
});

test('resolveCustomerId normalises the email for both lookup and creation', async () => {
  const found = fakeStripe({ existing: [{ id: 'cus_found' }] });
  assert.equal(await resolveCustomerId(found, null, 'Buyer@Example.COM'), 'cus_found');
  assert.deepEqual(found.calls.list[0], { email: 'buyer@example.com', limit: 1 });

  const fresh = fakeStripe({ existing: [] });
  assert.equal(await resolveCustomerId(fresh, null, 'Buyer@Example.COM'), 'cus_created');
  assert.deepEqual(fresh.calls.create[0], { email: 'buyer@example.com' });
});
